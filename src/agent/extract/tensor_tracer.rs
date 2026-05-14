//! Ω-2.0.7 — Extraction tensorielle (first mile).
//!
//! Pendant que `tracer.rs` extrait des fonctions Rust scalaires vers
//! `kasm::Program`, ce module extrait des compositions tensorielles
//! vers `kasm::tensor::TensorProgram`.
//!
//! ## Scope honnête de cette première étape
//!
//! - dtype : **F32 uniquement**.
//! - shapes : **vec(N) 1-D uniquement** (compatible avec `AddF32` /
//!   `MulF32` élémentwise).
//! - ops : `Input`, `Const`, `AddF32`, `MulF32`, `Output`.
//!
//! Ce qui est délibérément **reporté** (Ω-2.0.7.x) :
//!
//! - shapes 2-D (matrices, matmul) — `MatmulTile` exige une
//!   manipulation différente du shape result.
//! - dtypes Rational / Posit16 / Posit32.
//! - reduce / softmax / activations (relu, tanh, sigmoid, gelu).
//! - extraction depuis une closure Rust pure (analogue à `extract()`
//!   pour les scalaires) — exigerait un `TensorTracer` opérateur-overloadé,
//!   plus la machinerie thread-local de `tracer.rs`.
//!
//! L'API actuelle est un **builder fluent** : on construit un
//! `TensorProgram` étape par étape, et on appelle `.build()` à la fin.

use crate::kasm::tensor::{
    TensorError, TensorNode, TensorProgram, TensorShape, TensorTy, TENSOR_MAX_NODES,
};

/// Erreur de l'extraction tensorielle.
#[derive(Debug)]
pub enum TensorExtractError {
    /// Trop de nœuds dans le builder.
    TooManyNodes,
    /// Référence vers un handle invalide (out-of-bounds).
    BadHandle(u32),
    /// Erreur pendant la finalisation `TensorProgram::new`.
    Tensor(TensorError),
    /// Pas d'output défini avant `build()`.
    NoOutput,
    /// Shapes incompatibles entre opérandes.
    ShapeMismatch,
    /// dtypes incompatibles entre opérandes.
    DtypeMismatch,
    /// Shape invalide à la construction (ex: `vec(0)`).
    BadShape(TensorError),
}

impl std::fmt::Display for TensorExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TensorExtractError::TooManyNodes => write!(f, "too many tensor nodes"),
            TensorExtractError::BadHandle(h) => write!(f, "bad tensor handle {h}"),
            TensorExtractError::Tensor(e) => write!(f, "tensor: {e}"),
            TensorExtractError::NoOutput => write!(f, "no output set before build()"),
            TensorExtractError::ShapeMismatch => write!(f, "operands have mismatching shape"),
            TensorExtractError::DtypeMismatch => write!(f, "operands have mismatching dtype"),
            TensorExtractError::BadShape(e) => write!(f, "bad shape: {e}"),
        }
    }
}

impl std::error::Error for TensorExtractError {}

impl From<TensorError> for TensorExtractError {
    fn from(e: TensorError) -> Self {
        TensorExtractError::Tensor(e)
    }
}

/// Handle opaque vers un nœud `TensorProgram` en cours de construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorHandle {
    idx: u32,
    shape: TensorShape,
    dtype: TensorTy,
}

impl TensorHandle {
    pub fn shape(&self) -> TensorShape {
        self.shape
    }
    pub fn dtype(&self) -> TensorTy {
        self.dtype
    }
}

/// Builder fluent pour `TensorProgram`. Première implémentation
/// volontairement réduite : F32 + shape vec(N) + Add/Mul/Output.
///
/// L'invariant : tout `TensorHandle` retourné par les méthodes ci-dessous
/// référence un nœud déjà inséré dans `nodes` et ne deviendra jamais
/// invalide pendant la vie du builder.
pub struct TensorTracer {
    nodes: Vec<TensorNode>,
    const_pool: Vec<u8>,
    inputs: u8,
    output_idx: Option<u32>,
}

impl TensorTracer {
    /// Crée un nouveau builder vide.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            const_pool: Vec::new(),
            inputs: 0,
            output_idx: None,
        }
    }

    fn push(&mut self, node: TensorNode) -> Result<u32, TensorExtractError> {
        if self.nodes.len() >= TENSOR_MAX_NODES {
            return Err(TensorExtractError::TooManyNodes);
        }
        let idx = self.nodes.len() as u32;
        self.nodes.push(node);
        Ok(idx)
    }

    /// Ajoute un slot d'input F32 de shape donnée. Le slot est
    /// numéroté automatiquement (0, 1, 2, ... dans l'ordre d'appel).
    pub fn input_f32(&mut self, shape: TensorShape) -> Result<TensorHandle, TensorExtractError> {
        let slot = self.inputs;
        let node = TensorNode::input(slot, TensorTy::F32, shape);
        let idx = self.push(node)?;
        self.inputs = self.inputs.checked_add(1).ok_or(TensorExtractError::TooManyNodes)?;
        Ok(TensorHandle { idx, shape, dtype: TensorTy::F32 })
    }

    /// Ajoute une constante F32 de shape vec(N). Le slice `data` doit
    /// contenir exactement `shape.elements()` valeurs.
    pub fn const_f32(
        &mut self,
        shape: TensorShape,
        data: &[f32],
    ) -> Result<TensorHandle, TensorExtractError> {
        if data.len() != shape.elements() {
            return Err(TensorExtractError::ShapeMismatch);
        }
        let pool_offset = self.const_pool.len() as u32;
        for v in data {
            self.const_pool.extend_from_slice(&v.to_le_bytes());
        }
        let pool_len = (data.len() * 4) as u32;
        let node = TensorNode::const_at(pool_offset, pool_len, TensorTy::F32, shape);
        let idx = self.push(node)?;
        Ok(TensorHandle { idx, shape, dtype: TensorTy::F32 })
    }

    /// Addition élémentwise. `a` et `b` doivent avoir la même shape +
    /// dtype.
    pub fn add(
        &mut self,
        a: TensorHandle,
        b: TensorHandle,
    ) -> Result<TensorHandle, TensorExtractError> {
        if a.shape != b.shape {
            return Err(TensorExtractError::ShapeMismatch);
        }
        if a.dtype != b.dtype {
            return Err(TensorExtractError::DtypeMismatch);
        }
        let node = TensorNode::add(a.idx, b.idx, a.dtype, a.shape);
        let idx = self.push(node)?;
        Ok(TensorHandle { idx, shape: a.shape, dtype: a.dtype })
    }

    /// Multiplication élémentwise. Mêmes contraintes que `add`.
    pub fn mul(
        &mut self,
        a: TensorHandle,
        b: TensorHandle,
    ) -> Result<TensorHandle, TensorExtractError> {
        if a.shape != b.shape {
            return Err(TensorExtractError::ShapeMismatch);
        }
        if a.dtype != b.dtype {
            return Err(TensorExtractError::DtypeMismatch);
        }
        let node = TensorNode::mul(a.idx, b.idx, a.dtype, a.shape);
        let idx = self.push(node)?;
        Ok(TensorHandle { idx, shape: a.shape, dtype: a.dtype })
    }

    /// Marque `h` comme output (le programme n'en supporte qu'un seul
    /// pour cette première étape).
    pub fn output(&mut self, h: TensorHandle) -> Result<(), TensorExtractError> {
        let node = TensorNode::output(h.idx, h.dtype, h.shape);
        let idx = self.push(node)?;
        self.output_idx = Some(idx);
        Ok(())
    }

    /// Finalise le builder en `TensorProgram`. Échoue si aucun output
    /// n'a été défini.
    pub fn build(self) -> Result<TensorProgram, TensorExtractError> {
        if self.output_idx.is_none() {
            return Err(TensorExtractError::NoOutput);
        }
        let fuel = self.nodes.len() as u32;
        let outputs = 1u8;
        let prog = TensorProgram::new(self.inputs, outputs, fuel, self.nodes, self.const_pool)?;
        Ok(prog)
    }
}

impl Default for TensorTracer {
    fn default() -> Self {
        Self::new()
    }
}

/// Sucre : extrait via une closure qui prend un `&mut TensorTracer`.
///
/// Convention : la closure DOIT appeler `tracer.output(...)` exactement
/// une fois ; sinon `build()` échoue avec `NoOutput`.
pub fn extract_tensor<F>(f: F) -> Result<TensorProgram, TensorExtractError>
where
    F: FnOnce(&mut TensorTracer) -> Result<(), TensorExtractError>,
{
    let mut tracer = TensorTracer::new();
    f(&mut tracer)?;
    tracer.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::tensor::execute_tensor;

    #[test]
    fn tensor_tracer_identity_via_input_output() {
        // f([x; 4]) = x : un seul Input → Output.
        let shape = TensorShape::vec(4).unwrap();
        let prog = extract_tensor(|t| {
            let x = t.input_f32(shape)?;
            t.output(x)?;
            Ok(())
        })
        .expect("build");
        assert_eq!(prog.inputs(), 1);
        assert_eq!(prog.outputs(), 1);

        // Exécution : input = [1.0, 2.0, 3.0, 4.0] → output identique.
        let out = execute_tensor(&prog, &[vec![1.0, 2.0, 3.0, 4.0]]).expect("exec");
        assert_eq!(out.as_slice(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn tensor_tracer_add_two_inputs() {
        // f(a, b) = a + b sur vec(3).
        let shape = TensorShape::vec(3).unwrap();
        let prog = extract_tensor(|t| {
            let a = t.input_f32(shape)?;
            let b = t.input_f32(shape)?;
            let s = t.add(a, b)?;
            t.output(s)?;
            Ok(())
        })
        .expect("build");
        assert_eq!(prog.inputs(), 2);

        let out = execute_tensor(
            &prog,
            &[vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 30.0]],
        )
        .expect("exec");
        assert_eq!(out.as_slice(), &[11.0, 22.0, 33.0]);
    }

    #[test]
    fn tensor_tracer_const_plus_input() {
        // f(x) = x + [1, 2, 3, 4]
        let shape = TensorShape::vec(4).unwrap();
        let prog = extract_tensor(|t| {
            let x = t.input_f32(shape)?;
            let c = t.const_f32(shape, &[1.0, 2.0, 3.0, 4.0])?;
            let s = t.add(x, c)?;
            t.output(s)?;
            Ok(())
        })
        .expect("build");

        let out = execute_tensor(&prog, &[vec![10.0, 20.0, 30.0, 40.0]]).expect("exec");
        assert_eq!(out.as_slice(), &[11.0, 22.0, 33.0, 44.0]);
    }

    #[test]
    fn tensor_tracer_mul_then_add() {
        // f(a, b) = a * b + b sur vec(2).
        let shape = TensorShape::vec(2).unwrap();
        let prog = extract_tensor(|t| {
            let a = t.input_f32(shape)?;
            let b = t.input_f32(shape)?;
            let p = t.mul(a, b)?;
            let s = t.add(p, b)?;
            t.output(s)?;
            Ok(())
        })
        .expect("build");

        let out = execute_tensor(&prog, &[vec![2.0, 3.0], vec![5.0, 7.0]]).expect("exec");
        // a*b + b = [2*5+5, 3*7+7] = [15, 28]
        assert_eq!(out.as_slice(), &[15.0, 28.0]);
    }

    #[test]
    fn tensor_tracer_shape_mismatch_rejected() {
        let s4 = TensorShape::vec(4).unwrap();
        let s3 = TensorShape::vec(3).unwrap();
        let result = extract_tensor(|t| {
            let a = t.input_f32(s4)?;
            let b = t.input_f32(s3)?;
            let _ = t.add(a, b)?; // doit Err(ShapeMismatch)
            Ok(())
        });
        assert!(matches!(result, Err(TensorExtractError::ShapeMismatch)));
    }

    #[test]
    fn tensor_tracer_no_output_fails() {
        let result = extract_tensor(|t| {
            let _ = t.input_f32(TensorShape::vec(2).unwrap())?;
            // pas d'appel à t.output(...)
            Ok(())
        });
        assert!(matches!(result, Err(TensorExtractError::NoOutput)));
    }
}
