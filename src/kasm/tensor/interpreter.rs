//! Reference interpreter for KASM-Tensor. Scalar, slow, deliberately
//! straightforward. Its purpose is to be the **bit-exact ground truth**
//! against which any future JIT, MLIR/Mojo lowering, GPU backend, or
//! oracle approximation must agree.
//!
//! Architecture (Ω-3.3.1) : un interpréteur **polymorphe**
//! `execute_tensor_polymorphic` consomme des `TensorValue` (enum F32 /
//! Rational), dispatch par dtype et par opcode. Les fonctions historiques
//! `execute_tensor` (f32) et `execute_tensor_rational` (Rational) sont
//! des wrappers fins qui convertissent les types fortement typés vers
//! `TensorValue` puis dépaquetent le résultat.

use crate::numeric::{Numeric, Posit16, Posit32, Rational};

use super::program::TensorProgram;
use super::types::{TensorError, TensorOp, TensorShape, TensorTy};

/// Valeur runtime d'un nœud tenseur : f32, Rational, Posit16, ou Posit32.
/// L'interpréteur polymorphe maintient ces valeurs dans un `Vec<Option<TensorValue>>`
/// indexé par numéro de nœud.
#[derive(Clone, Debug)]
pub enum TensorValue {
    F32(Vec<f32>),
    Rational(Vec<Rational>),
    Posit16(Vec<Posit16>),
    Posit32(Vec<Posit32>),
}

impl TensorValue {
    pub fn dtype(&self) -> TensorTy {
        match self {
            TensorValue::F32(_) => TensorTy::F32,
            TensorValue::Rational(_) => TensorTy::Rational,
            TensorValue::Posit16(_) => TensorTy::Posit16,
            TensorValue::Posit32(_) => TensorTy::Posit32,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            TensorValue::F32(v) => v.len(),
            TensorValue::Rational(v) => v.len(),
            TensorValue::Posit16(v) => v.len(),
            TensorValue::Posit32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn as_f32(&self) -> Option<&[f32]> {
        if let TensorValue::F32(v) = self { Some(v) } else { None }
    }

    fn as_rational(&self) -> Option<&[Rational]> {
        if let TensorValue::Rational(v) = self { Some(v) } else { None }
    }

    fn as_posit16(&self) -> Option<&[Posit16]> {
        if let TensorValue::Posit16(v) = self { Some(v) } else { None }
    }

    fn as_posit32(&self) -> Option<&[Posit32]> {
        if let TensorValue::Posit32(v) = self { Some(v) } else { None }
    }
}

/// Interpréteur polymorphe — point d'entrée canonique Ω-3.3.1.
///
/// Accepte des inputs hétérogènes (F32 ou Rational) et dispatch par opcode.
/// Une opération exige que ses opérandes soient du dtype attendu (e.g.
/// `AddF32` exige deux `TensorValue::F32`) ; sinon `DtypeMismatch`.
///
/// Le résultat est de la même forme que le node `Output` du programme.
pub fn execute_tensor_polymorphic(
    program: &TensorProgram,
    inputs: &[TensorValue],
) -> Result<TensorValue, TensorError> {
    if inputs.len() != program.inputs() as usize {
        return Err(TensorError::TooManyInputs);
    }
    let nodes = program.nodes();
    let pool = program.const_pool();
    let mut values: Vec<Option<TensorValue>> = vec![None; nodes.len()];

    for (i, node) in nodes.iter().enumerate() {
        let result: TensorValue = match node.op {
            TensorOp::Input => {
                let slot = node.imm as usize;
                let raw = inputs.get(slot).ok_or(TensorError::BadSlot {
                    node: i as u32,
                    slot: node.imm,
                })?;
                if raw.dtype() != node.dtype {
                    return Err(TensorError::DtypeMismatch { node: i as u32 });
                }
                let expected = node.shape.elements();
                if raw.len() != expected {
                    return Err(TensorError::ShapeMismatch {
                        node: i as u32,
                        reason: "input slot length mismatches declared shape",
                    });
                }
                raw.clone()
            }
            TensorOp::Const => {
                let off = node.b as usize;
                let len = node.imm as usize;
                let bytes = &pool[off..off + len];
                match node.dtype {
                    TensorTy::F32 => {
                        let mut out = Vec::with_capacity(node.shape.elements());
                        for chunk in bytes.chunks_exact(4) {
                            out.push(f32::from_le_bytes(chunk.try_into().unwrap()));
                        }
                        TensorValue::F32(out)
                    }
                    TensorTy::Rational => {
                        let mut out = Vec::with_capacity(node.shape.elements());
                        for chunk in bytes.chunks_exact(32) {
                            let r = <Rational as crate::numeric::BitStable>::from_canonical_bytes(chunk)
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                            out.push(r);
                        }
                        TensorValue::Rational(out)
                    }
                    TensorTy::Posit16 => {
                        let mut out = Vec::with_capacity(node.shape.elements());
                        for chunk in bytes.chunks_exact(2) {
                            let bits = u16::from_le_bytes(chunk.try_into().unwrap());
                            out.push(Posit16::from_bits(bits));
                        }
                        TensorValue::Posit16(out)
                    }
                    TensorTy::Posit32 => {
                        let mut out = Vec::with_capacity(node.shape.elements());
                        for chunk in bytes.chunks_exact(4) {
                            let bits = u32::from_le_bytes(chunk.try_into().unwrap());
                            out.push(Posit32::from_bits(bits));
                        }
                        TensorValue::Posit32(out)
                    }
                }
            }
            TensorOp::AddF32 => {
                let (lhs, rhs) = read_two_f32(&values, node.a, node.b, i)?;
                TensorValue::F32(lhs.iter().zip(rhs.iter()).map(|(a, b)| a + b).collect())
            }
            TensorOp::MulF32 => {
                let (lhs, rhs) = read_two_f32(&values, node.a, node.b, i)?;
                TensorValue::F32(lhs.iter().zip(rhs.iter()).map(|(a, b)| a * b).collect())
            }
            TensorOp::AddRational => {
                let (lhs, rhs) = read_two_rational(&values, node.a, node.b, i)?;
                let out: Vec<Rational> = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| a.checked_add(*b).ok_or(TensorError::DtypeMismatch { node: i as u32 }))
                    .collect::<Result<_, _>>()?;
                TensorValue::Rational(out)
            }
            TensorOp::MulRational => {
                let (lhs, rhs) = read_two_rational(&values, node.a, node.b, i)?;
                let out: Vec<Rational> = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| a.checked_mul(*b).ok_or(TensorError::DtypeMismatch { node: i as u32 }))
                    .collect::<Result<_, _>>()?;
                TensorValue::Rational(out)
            }
            TensorOp::MatmulTile => {
                let (lhs, rhs) = read_two_f32(&values, node.a, node.b, i)?;
                let lhs_shape = nodes[node.a as usize].shape;
                let rhs_shape = nodes[node.b as usize].shape;
                let m = lhs_shape.d[0] as usize;
                let k = lhs_shape.d[1] as usize;
                let n = rhs_shape.d[1] as usize;
                let mut out = vec![0.0f32; m * n];
                for row in 0..m {
                    for col in 0..n {
                        let mut acc = 0.0f32;
                        for kk in 0..k {
                            acc += lhs[row * k + kk] * rhs[kk * n + col];
                        }
                        out[row * n + col] = acc;
                    }
                }
                TensorValue::F32(out)
            }
            TensorOp::MatmulTileRational => {
                let (lhs, rhs) = read_two_rational(&values, node.a, node.b, i)?;
                let lhs_shape = nodes[node.a as usize].shape;
                let rhs_shape = nodes[node.b as usize].shape;
                let m = lhs_shape.d[0] as usize;
                let k = lhs_shape.d[1] as usize;
                let n = rhs_shape.d[1] as usize;
                let mut out = vec![Rational::zero(); m * n];
                for row in 0..m {
                    for col in 0..n {
                        let mut acc = Rational::zero();
                        for kk in 0..k {
                            let p = lhs[row * k + kk]
                                .checked_mul(rhs[kk * n + col])
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                            acc = acc
                                .checked_add(p)
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                        }
                        out[row * n + col] = acc;
                    }
                }
                TensorValue::Rational(out)
            }
            TensorOp::AddPosit16 => {
                let (lhs, rhs) = read_two_posit16(&values, node.a, node.b, i)?;
                let out: Vec<Posit16> = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| {
                        a.checked_add(*b)
                            .ok_or(TensorError::DtypeMismatch { node: i as u32 })
                    })
                    .collect::<Result<_, _>>()?;
                TensorValue::Posit16(out)
            }
            TensorOp::MulPosit16 => {
                let (lhs, rhs) = read_two_posit16(&values, node.a, node.b, i)?;
                let out: Vec<Posit16> = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| {
                        a.checked_mul(*b)
                            .ok_or(TensorError::DtypeMismatch { node: i as u32 })
                    })
                    .collect::<Result<_, _>>()?;
                TensorValue::Posit16(out)
            }
            TensorOp::MatmulTilePosit16 => {
                let (lhs, rhs) = read_two_posit16(&values, node.a, node.b, i)?;
                let lhs_shape = nodes[node.a as usize].shape;
                let rhs_shape = nodes[node.b as usize].shape;
                let m = lhs_shape.d[0] as usize;
                let k = lhs_shape.d[1] as usize;
                let n = rhs_shape.d[1] as usize;
                let mut out = vec![Posit16::ZERO; m * n];
                for row in 0..m {
                    for col in 0..n {
                        let mut acc = Posit16::ZERO;
                        for kk in 0..k {
                            let p = lhs[row * k + kk]
                                .checked_mul(rhs[kk * n + col])
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                            acc = acc
                                .checked_add(p)
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                        }
                        out[row * n + col] = acc;
                    }
                }
                TensorValue::Posit16(out)
            }
            TensorOp::AddPosit32 => {
                let (lhs, rhs) = read_two_posit32(&values, node.a, node.b, i)?;
                let out: Vec<Posit32> = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| {
                        a.checked_add(*b)
                            .ok_or(TensorError::DtypeMismatch { node: i as u32 })
                    })
                    .collect::<Result<_, _>>()?;
                TensorValue::Posit32(out)
            }
            TensorOp::MulPosit32 => {
                let (lhs, rhs) = read_two_posit32(&values, node.a, node.b, i)?;
                let out: Vec<Posit32> = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| {
                        a.checked_mul(*b)
                            .ok_or(TensorError::DtypeMismatch { node: i as u32 })
                    })
                    .collect::<Result<_, _>>()?;
                TensorValue::Posit32(out)
            }
            TensorOp::MatmulTilePosit32 => {
                let (lhs, rhs) = read_two_posit32(&values, node.a, node.b, i)?;
                let lhs_shape = nodes[node.a as usize].shape;
                let rhs_shape = nodes[node.b as usize].shape;
                let m = lhs_shape.d[0] as usize;
                let k = lhs_shape.d[1] as usize;
                let n = rhs_shape.d[1] as usize;
                let mut out = vec![Posit32::ZERO; m * n];
                for row in 0..m {
                    for col in 0..n {
                        let mut acc = Posit32::ZERO;
                        for kk in 0..k {
                            let p = lhs[row * k + kk]
                                .checked_mul(rhs[kk * n + col])
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                            acc = acc
                                .checked_add(p)
                                .ok_or(TensorError::DtypeMismatch { node: i as u32 })?;
                        }
                        out[row * n + col] = acc;
                    }
                }
                TensorValue::Posit32(out)
            }
            TensorOp::ReduceSumAxis => {
                let src = read_f32(&values, node.a, i)?;
                let src_shape = nodes[node.a as usize].shape;
                let axis = node.imm as u8;
                TensorValue::F32(reduce_sum(src, &src_shape, axis))
            }
            TensorOp::Softmax => {
                let src = read_f32(&values, node.a, i)?;
                let src_shape = nodes[node.a as usize].shape;
                let axis = node.imm as u8;
                TensorValue::F32(softmax(src, &src_shape, axis))
            }
            TensorOp::Output => {
                let src = read_value(&values, node.a, i)?;
                if src.dtype() != node.dtype {
                    return Err(TensorError::DtypeMismatch { node: i as u32 });
                }
                src.clone()
            }
            TensorOp::ReluF32 => {
                let src = read_f32(&values, node.a, i)?;
                TensorValue::F32(src.iter().map(|x| x.max(0.0)).collect())
            }
            TensorOp::TanhF32 => {
                let src = read_f32(&values, node.a, i)?;
                TensorValue::F32(src.iter().map(|x| x.tanh()).collect())
            }
            TensorOp::SigmoidF32 => {
                let src = read_f32(&values, node.a, i)?;
                TensorValue::F32(src.iter().map(|x| 1.0 / (1.0 + (-x).exp())).collect())
            }
            TensorOp::GeluTanhF32 => {
                let src = read_f32(&values, node.a, i)?;
                TensorValue::F32(
                    src.iter()
                        .map(|&x| {
                            let inner = 0.7978845608028654_f32 * (x + 0.044715_f32 * x * x * x);
                            0.5 * x * (1.0 + inner.tanh())
                        })
                        .collect(),
                )
            }
        };
        values[i] = Some(result);
    }

    let last = values.last().and_then(|v| v.clone()).ok_or(TensorError::NoOutput)?;
    Ok(last)
}

// ---------------------------------------------------------------------------
// Wrappers : préservent la signature historique tout en passant par le
// chemin polymorphe canonique.
// ---------------------------------------------------------------------------

/// Run a verified `TensorProgram` (F32 only) against the given inputs.
/// Wrapper sur `execute_tensor_polymorphic`.
pub fn execute_tensor(
    program: &TensorProgram,
    inputs: &[Vec<f32>],
) -> Result<Vec<f32>, TensorError> {
    let inputs_poly: Vec<TensorValue> =
        inputs.iter().map(|v| TensorValue::F32(v.clone())).collect();
    let result = execute_tensor_polymorphic(program, &inputs_poly)?;
    match result {
        TensorValue::F32(v) => Ok(v),
        _ => Err(TensorError::DtypeMismatch {
            node: (program.nodes().len() as u32).saturating_sub(1),
        }),
    }
}

/// Exécute un `TensorProgram` Rational. Wrapper sur `execute_tensor_polymorphic`.
pub fn execute_tensor_rational(
    program: &TensorProgram,
    inputs: &[Vec<Rational>],
) -> Result<Vec<Rational>, TensorError> {
    let inputs_poly: Vec<TensorValue> =
        inputs.iter().map(|v| TensorValue::Rational(v.clone())).collect();
    let result = execute_tensor_polymorphic(program, &inputs_poly)?;
    match result {
        TensorValue::Rational(v) => Ok(v),
        _ => Err(TensorError::DtypeMismatch {
            node: (program.nodes().len() as u32).saturating_sub(1),
        }),
    }
}

/// Wrapper Posit16. Convertit inputs Posit16 en `TensorValue` puis dépaquette.
pub fn execute_tensor_posit16(
    program: &TensorProgram,
    inputs: &[Vec<Posit16>],
) -> Result<Vec<Posit16>, TensorError> {
    let inputs_poly: Vec<TensorValue> =
        inputs.iter().map(|v| TensorValue::Posit16(v.clone())).collect();
    let result = execute_tensor_polymorphic(program, &inputs_poly)?;
    match result {
        TensorValue::Posit16(v) => Ok(v),
        _ => Err(TensorError::DtypeMismatch {
            node: (program.nodes().len() as u32).saturating_sub(1),
        }),
    }
}

/// Wrapper Posit32. Convertit inputs Posit32 en `TensorValue` puis dépaquette.
pub fn execute_tensor_posit32(
    program: &TensorProgram,
    inputs: &[Vec<Posit32>],
) -> Result<Vec<Posit32>, TensorError> {
    let inputs_poly: Vec<TensorValue> =
        inputs.iter().map(|v| TensorValue::Posit32(v.clone())).collect();
    let result = execute_tensor_polymorphic(program, &inputs_poly)?;
    match result {
        TensorValue::Posit32(v) => Ok(v),
        _ => Err(TensorError::DtypeMismatch {
            node: (program.nodes().len() as u32).saturating_sub(1),
        }),
    }
}

// ---------------------------------------------------------------------------
// Helpers polymorphes
// ---------------------------------------------------------------------------

fn read_value<'a>(
    values: &'a [Option<TensorValue>],
    idx: u32,
    consumer: usize,
) -> Result<&'a TensorValue, TensorError> {
    let i = idx as usize;
    if i >= consumer {
        return Err(TensorError::BackrefOutOfBounds {
            node: consumer as u32,
            index: idx,
        });
    }
    values[i].as_ref().ok_or(TensorError::BackrefOutOfBounds {
        node: consumer as u32,
        index: idx,
    })
}

fn read_f32<'a>(
    values: &'a [Option<TensorValue>],
    idx: u32,
    consumer: usize,
) -> Result<&'a [f32], TensorError> {
    read_value(values, idx, consumer)?
        .as_f32()
        .ok_or(TensorError::DtypeMismatch { node: consumer as u32 })
}

fn read_rational<'a>(
    values: &'a [Option<TensorValue>],
    idx: u32,
    consumer: usize,
) -> Result<&'a [Rational], TensorError> {
    read_value(values, idx, consumer)?
        .as_rational()
        .ok_or(TensorError::DtypeMismatch { node: consumer as u32 })
}

fn read_two_f32<'a>(
    values: &'a [Option<TensorValue>],
    a: u32,
    b: u32,
    consumer: usize,
) -> Result<(&'a [f32], &'a [f32]), TensorError> {
    let lhs = read_f32(values, a, consumer)?;
    let rhs = read_f32(values, b, consumer)?;
    Ok((lhs, rhs))
}

fn read_two_rational<'a>(
    values: &'a [Option<TensorValue>],
    a: u32,
    b: u32,
    consumer: usize,
) -> Result<(&'a [Rational], &'a [Rational]), TensorError> {
    let lhs = read_rational(values, a, consumer)?;
    let rhs = read_rational(values, b, consumer)?;
    Ok((lhs, rhs))
}

fn read_posit16<'a>(
    values: &'a [Option<TensorValue>],
    idx: u32,
    consumer: usize,
) -> Result<&'a [Posit16], TensorError> {
    read_value(values, idx, consumer)?
        .as_posit16()
        .ok_or(TensorError::DtypeMismatch { node: consumer as u32 })
}

fn read_posit32<'a>(
    values: &'a [Option<TensorValue>],
    idx: u32,
    consumer: usize,
) -> Result<&'a [Posit32], TensorError> {
    read_value(values, idx, consumer)?
        .as_posit32()
        .ok_or(TensorError::DtypeMismatch { node: consumer as u32 })
}

fn read_two_posit16<'a>(
    values: &'a [Option<TensorValue>],
    a: u32,
    b: u32,
    consumer: usize,
) -> Result<(&'a [Posit16], &'a [Posit16]), TensorError> {
    let lhs = read_posit16(values, a, consumer)?;
    let rhs = read_posit16(values, b, consumer)?;
    Ok((lhs, rhs))
}

fn read_two_posit32<'a>(
    values: &'a [Option<TensorValue>],
    a: u32,
    b: u32,
    consumer: usize,
) -> Result<(&'a [Posit32], &'a [Posit32]), TensorError> {
    let lhs = read_posit32(values, a, consumer)?;
    let rhs = read_posit32(values, b, consumer)?;
    Ok((lhs, rhs))
}

fn reduce_sum(src: &[f32], shape: &TensorShape, axis: u8) -> Vec<f32> {
    match shape.dims {
        1 => {
            // Reducing the only axis → scalar (1-element vector).
            let s: f32 = src.iter().sum();
            vec![s]
        }
        2 => {
            let rows = shape.d[0] as usize;
            let cols = shape.d[1] as usize;
            if axis == 0 {
                // Sum across rows → output length = cols
                let mut out = vec![0.0f32; cols];
                for r in 0..rows {
                    for c in 0..cols {
                        out[c] += src[r * cols + c];
                    }
                }
                out
            } else {
                // axis == 1, sum across columns → output length = rows
                let mut out = vec![0.0f32; rows];
                for r in 0..rows {
                    let mut acc = 0.0f32;
                    for c in 0..cols {
                        acc += src[r * cols + c];
                    }
                    out[r] = acc;
                }
                out
            }
        }
        _ => Vec::new(),
    }
}

fn softmax(src: &[f32], shape: &TensorShape, axis: u8) -> Vec<f32> {
    // Numerically stable softmax: subtract max along the axis, then
    // exp/sum.
    match shape.dims {
        1 => {
            let max = src.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = src.iter().map(|x| (x - max).exp()).collect();
            let sum: f32 = exps.iter().sum();
            exps.into_iter().map(|e| e / sum).collect()
        }
        2 => {
            let rows = shape.d[0] as usize;
            let cols = shape.d[1] as usize;
            let mut out = vec![0.0f32; rows * cols];
            if axis == 1 {
                // Softmax along each row independently.
                for r in 0..rows {
                    let row = &src[r * cols..(r + 1) * cols];
                    let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    let exps: Vec<f32> = row.iter().map(|x| (x - max).exp()).collect();
                    let sum: f32 = exps.iter().sum();
                    for c in 0..cols {
                        out[r * cols + c] = exps[c] / sum;
                    }
                }
            } else {
                // axis == 0: softmax along each column.
                for c in 0..cols {
                    let mut col_vals: Vec<f32> = (0..rows).map(|r| src[r * cols + c]).collect();
                    let max = col_vals.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    for v in col_vals.iter_mut() {
                        *v = (*v - max).exp();
                    }
                    let sum: f32 = col_vals.iter().sum();
                    for r in 0..rows {
                        out[r * cols + c] = col_vals[r] / sum;
                    }
                }
            }
            out
        }
        _ => src.to_vec(),
    }
}
