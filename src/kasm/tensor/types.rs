//! Type system for KASM-Tensor: dtype, shape, op enum, node layout.
//!
//! Every node is a fixed 32-byte struct so a `TensorProgram` is a
//! flat byte array (header + nodes + footer) just like KASM-Int —
//! the substrate philosophy is preserved (content-addressed,
//! restart-survivable, `git push`-able).

use std::fmt;

// ---------- format constants ----------

pub const TENSOR_MAGIC: &[u8; 8] = b"SCANT01\0";
pub const TENSOR_VERSION: u8 = 1;

/// Header layout (32 bytes total):
///   * 8  : magic
///   * 1  : version
///   * 1  : reserved (dialect target — CPU only for now)
///   * 1  : inputs count
///   * 1  : outputs count
///   * 4  : fuel (≥ nodes_len, terminates verification)
///   * 4  : nodes_len
///   * 12 : reserved (zeroed) for alignment + future use
pub const TENSOR_HEADER_LEN: usize = 32;
pub const TENSOR_FOOTER_LEN: usize = 20; // SHA-1-style footer for self-checksum

/// Fixed 32 bytes per node.
pub const TENSOR_NODE_LEN: usize = 32;

/// Bound: a tensor program holds at most this many ops. Same
/// philosophy as KASM-Int's `MAX_NODES` — terminating bound visible
/// at verify time.
pub const TENSOR_MAX_NODES: usize = 4096;

/// Maximum tensor rank. Two dimensions cover the bulk of the ML
/// substrate we care about right now (matrices, vectors). Higher rank
/// (batched matmul, attention multi-head) is a follow-up.
pub const TENSOR_MAX_DIMS: usize = 2;

/// Maximum number of program inputs/outputs.
pub const TENSOR_MAX_SLOTS: u8 = 16;

/// Maximum extent of any single dimension. 4096×4096 covers a
/// transformer head; bigger would push us past the "embedded ML"
/// regime and need a different verifier.
pub const TENSOR_MAX_DIM_EXTENT: usize = 4096;

// ---------- dtype ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TensorTy {
    F32 = 1,
    /// Ω-3.3 — `Rational` (i128 num / i128 denom). Exact, associatif
    /// bit-exact, content-addressable. 32 bytes par élément.
    Rational = 2,
    /// Ω-3.3.2 — `Posit16` (ES=1). 2 bytes/élément. Plus associatif que f32
    /// mais pas exact. Pour les workloads où l'on veut perf+stabilité
    /// supérieure à f32 sans payer le coût Rational.
    Posit16 = 3,
    /// Ω-3.3.2 — `Posit32` (ES=2). 4 bytes/élément. Précision supérieure
    /// à f32 sur la plage centrale, plus associatif que f32.
    Posit32 = 4,
}

impl TensorTy {
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            1 => Some(TensorTy::F32),
            2 => Some(TensorTy::Rational),
            3 => Some(TensorTy::Posit16),
            4 => Some(TensorTy::Posit32),
            _ => None,
        }
    }

    pub fn byte_size(&self) -> usize {
        match self {
            TensorTy::F32 => 4,
            TensorTy::Rational => 32, // i128 num + i128 denom
            TensorTy::Posit16 => 2,
            TensorTy::Posit32 => 4,
        }
    }
}

// ---------- shape ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TensorShape {
    /// Dimensions encoded as `[d0, d1]`. A 1-D tensor has `dims = 1`
    /// and only `d[0]` is meaningful. A scalar has `dims = 0`.
    pub d: [u32; TENSOR_MAX_DIMS],
    pub dims: u8,
}

impl TensorShape {
    pub fn scalar() -> Self {
        Self { d: [0; TENSOR_MAX_DIMS], dims: 0 }
    }

    pub fn vec(n: usize) -> Result<Self, TensorError> {
        if n == 0 || n > TENSOR_MAX_DIM_EXTENT {
            return Err(TensorError::DimOutOfRange(n));
        }
        let mut d = [0u32; TENSOR_MAX_DIMS];
        d[0] = n as u32;
        Ok(Self { d, dims: 1 })
    }

    pub fn matrix(rows: usize, cols: usize) -> Result<Self, TensorError> {
        if rows == 0 || rows > TENSOR_MAX_DIM_EXTENT {
            return Err(TensorError::DimOutOfRange(rows));
        }
        if cols == 0 || cols > TENSOR_MAX_DIM_EXTENT {
            return Err(TensorError::DimOutOfRange(cols));
        }
        let mut d = [0u32; TENSOR_MAX_DIMS];
        d[0] = rows as u32;
        d[1] = cols as u32;
        Ok(Self { d, dims: 2 })
    }

    pub fn elements(&self) -> usize {
        if self.dims == 0 {
            return 1;
        }
        let mut n: usize = 1;
        for i in 0..self.dims as usize {
            n = n.saturating_mul(self.d[i] as usize);
        }
        n
    }

    pub fn rank(&self) -> u8 {
        self.dims
    }

    /// Encode the shape into 9 bytes: `[dims, d0_le, d1_le]`.
    pub fn encode(&self, out: &mut [u8; 9]) {
        out[0] = self.dims;
        out[1..5].copy_from_slice(&self.d[0].to_le_bytes());
        out[5..9].copy_from_slice(&self.d[1].to_le_bytes());
    }

    pub fn decode(bytes: &[u8; 9]) -> Result<Self, TensorError> {
        let dims = bytes[0];
        if dims > TENSOR_MAX_DIMS as u8 {
            return Err(TensorError::ShapeRankInvalid(dims));
        }
        let d0 = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
        let d1 = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
        let d = [d0, d1];
        for i in 0..dims as usize {
            if d[i] == 0 || d[i] as usize > TENSOR_MAX_DIM_EXTENT {
                return Err(TensorError::DimOutOfRange(d[i] as usize));
            }
        }
        // Trailing dims must be zero.
        for i in dims as usize..TENSOR_MAX_DIMS {
            if d[i] != 0 {
                return Err(TensorError::ShapeNonZeroPastRank(i));
            }
        }
        Ok(Self { d, dims })
    }
}

impl fmt::Display for TensorShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dims {
            0 => write!(f, "scalar"),
            1 => write!(f, "[{}]", self.d[0]),
            2 => write!(f, "[{}, {}]", self.d[0], self.d[1]),
            _ => write!(f, "<rank {}>", self.dims),
        }
    }
}

// ---------- op enum ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TensorOp {
    /// `imm = literal_index` into the program's constants pool. Shape
    /// + dtype come from the node's own fields.
    Const = 1,
    /// `imm = input_slot`. Shape + dtype encoded on the node.
    Input = 2,
    /// `a = source_node`. Shape + dtype must match `a`.
    Output = 3,

    // ---- elementwise ----
    /// `a, b` must have identical shape + dtype; result has the same.
    AddF32 = 10,
    /// `a, b` same shape + dtype.
    MulF32 = 11,
    /// Ω-3.3 — addition Rational (élémentwise). `a, b` même shape, dtype Rational.
    AddRational = 12,
    /// Ω-3.3 — multiplication Rational (élémentwise). `a, b` même shape, dtype Rational.
    MulRational = 13,
    /// Ω-3.3.2 — addition Posit16 (élémentwise).
    AddPosit16 = 14,
    /// Ω-3.3.2 — multiplication Posit16 (élémentwise).
    MulPosit16 = 15,
    /// Ω-3.3.2 — addition Posit32 (élémentwise).
    AddPosit32 = 16,
    /// Ω-3.3.2 — multiplication Posit32 (élémentwise).
    MulPosit32 = 17,

    // ---- structural ----
    /// `a = lhs (M×K)`, `b = rhs (K×N)`. Result `(M×N)`. Both F32.
    MatmulTile = 20,
    /// Ω-3.3 — Matmul Rational. Mêmes shapes que MatmulTile, dtype Rational.
    /// **Associativé bit-exacte** : ferme le mur Tension 12 sur tenseur.
    MatmulTileRational = 23,
    /// Ω-3.3.2 — Matmul Posit16. Plus stable que f32, pas exact.
    MatmulTilePosit16 = 24,
    /// Ω-3.3.2 — Matmul Posit32. Précision améliorée vs f32.
    MatmulTilePosit32 = 25,
    /// `a = source`. `imm = axis` (0 or 1). Reduces that axis,
    /// producing a tensor of one lower rank.
    ReduceSumAxis = 21,
    /// `a = source`. `imm = axis` along which softmax normalises.
    /// Output has same shape + dtype as input.
    Softmax = 22,

    // ---- activations (elementwise, shape-preserving) ----
    /// `a = source`. f(x) = max(x, 0). Bit-deterministic on f32
    /// (no exp/tanh) — the only activation that survives the
    /// `float_assoc_wall` argument with no contract caveat.
    ReluF32 = 30,
    /// `a = source`. f(x) = tanh(x). Uses libstd `f32::tanh`,
    /// numerically deterministic on the same machine but
    /// implementation-dependent across libc versions —
    /// MUST be paired with a `NumericContract` if memos are
    /// shared cross-host (Codex Cortex layer's responsibility).
    TanhF32 = 31,
    /// `a = source`. f(x) = 1 / (1 + exp(-x)). Same caveat as
    /// `TanhF32` — exp is libstd-dependent.
    SigmoidF32 = 32,
    /// `a = source`. GeLU using the **tanh approximation**:
    /// `0.5 * x * (1 + tanh(√(2/π) * (x + 0.044715 * x³)))`.
    /// This is the canonical "tanh-GeLU" used by most production
    /// transformers; the alternative (erf-based) would need a
    /// distinct op and a distinct semantic fingerprint.
    GeluTanhF32 = 33,
}

impl TensorOp {
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            1 => Some(TensorOp::Const),
            2 => Some(TensorOp::Input),
            3 => Some(TensorOp::Output),
            10 => Some(TensorOp::AddF32),
            11 => Some(TensorOp::MulF32),
            12 => Some(TensorOp::AddRational),
            13 => Some(TensorOp::MulRational),
            14 => Some(TensorOp::AddPosit16),
            15 => Some(TensorOp::MulPosit16),
            16 => Some(TensorOp::AddPosit32),
            17 => Some(TensorOp::MulPosit32),
            20 => Some(TensorOp::MatmulTile),
            23 => Some(TensorOp::MatmulTileRational),
            24 => Some(TensorOp::MatmulTilePosit16),
            25 => Some(TensorOp::MatmulTilePosit32),
            21 => Some(TensorOp::ReduceSumAxis),
            22 => Some(TensorOp::Softmax),
            30 => Some(TensorOp::ReluF32),
            31 => Some(TensorOp::TanhF32),
            32 => Some(TensorOp::SigmoidF32),
            33 => Some(TensorOp::GeluTanhF32),
            _ => None,
        }
    }
}

// ---------- node ----------

/// 32-byte fixed node layout:
///   *  0..1  : op (u8)
///   *  1..2  : dtype (u8)
///   *  2..6  : a (u32 — predecessor node index OR slot/axis depending on op)
///   *  6..10 : b (u32 — second predecessor for binary ops, or const-pool offset for `Const`)
///   * 10..14 : imm (i32 — slot for Input, axis for reduce/softmax, const-pool length for Const)
///   * 14..23 : shape (9 bytes)
///   * 23..32 : reserved (zeroed)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorNode {
    pub op: TensorOp,
    pub dtype: TensorTy,
    pub a: u32,
    pub b: u32,
    pub imm: i32,
    pub shape: TensorShape,
}

impl TensorNode {
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.push(self.op as u8);
        out.push(self.dtype as u8);
        out.extend_from_slice(&self.a.to_le_bytes());
        out.extend_from_slice(&self.b.to_le_bytes());
        out.extend_from_slice(&self.imm.to_le_bytes());
        let mut shape_buf = [0u8; 9];
        self.shape.encode(&mut shape_buf);
        out.extend_from_slice(&shape_buf);
        out.extend_from_slice(&[0u8; 9]); // reserved, zeroed
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TensorError> {
        if bytes.len() < TENSOR_NODE_LEN {
            return Err(TensorError::TruncatedNode);
        }
        let op = TensorOp::from_u8(bytes[0]).ok_or(TensorError::UnknownOp(bytes[0]))?;
        let dtype = TensorTy::from_u8(bytes[1]).ok_or(TensorError::UnknownDtype(bytes[1]))?;
        let a = u32::from_le_bytes(bytes[2..6].try_into().unwrap());
        let b = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        let imm = i32::from_le_bytes(bytes[10..14].try_into().unwrap());
        let shape = TensorShape::decode(bytes[14..23].try_into().unwrap())?;
        // Last 9 bytes must be zero — leaves room for future fields
        // without breaking the wire format.
        for &x in &bytes[23..32] {
            if x != 0 {
                return Err(TensorError::ReservedNonZero);
            }
        }
        Ok(TensorNode { op, dtype, a, b, imm, shape })
    }

    // ---- constructors (compile-time-friendly) ----

    pub fn input(slot: u8, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::Input, dtype, a: 0, b: 0, imm: slot as i32, shape }
    }

    pub fn const_at(pool_offset: u32, pool_len: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self {
            op: TensorOp::Const,
            dtype,
            a: 0,
            b: pool_offset,
            imm: pool_len as i32,
            shape,
        }
    }

    pub fn add(a: u32, b: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::AddF32, dtype, a, b, imm: 0, shape }
    }

    pub fn mul(a: u32, b: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::MulF32, dtype, a, b, imm: 0, shape }
    }

    pub fn matmul(a: u32, b: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::MatmulTile, dtype, a, b, imm: 0, shape }
    }

    pub fn reduce_sum(a: u32, axis: u8, dtype: TensorTy, shape: TensorShape) -> Self {
        Self {
            op: TensorOp::ReduceSumAxis,
            dtype,
            a,
            b: 0,
            imm: axis as i32,
            shape,
        }
    }

    pub fn softmax(a: u32, axis: u8, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::Softmax, dtype, a, b: 0, imm: axis as i32, shape }
    }

    pub fn output(a: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::Output, dtype, a, b: 0, imm: 0, shape }
    }

    pub fn relu(a: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::ReluF32, dtype, a, b: 0, imm: 0, shape }
    }

    pub fn tanh(a: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::TanhF32, dtype, a, b: 0, imm: 0, shape }
    }

    pub fn sigmoid(a: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::SigmoidF32, dtype, a, b: 0, imm: 0, shape }
    }

    pub fn gelu_tanh(a: u32, dtype: TensorTy, shape: TensorShape) -> Self {
        Self { op: TensorOp::GeluTanhF32, dtype, a, b: 0, imm: 0, shape }
    }

    // ---- Ω-3.3 Rational constructors ----

    pub fn add_rational(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::AddRational, dtype: TensorTy::Rational, a, b, imm: 0, shape }
    }

    pub fn mul_rational(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::MulRational, dtype: TensorTy::Rational, a, b, imm: 0, shape }
    }

    pub fn matmul_rational(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::MatmulTileRational, dtype: TensorTy::Rational, a, b, imm: 0, shape }
    }

    // ---- Ω-3.3.2 Posit16 / Posit32 constructors ----

    pub fn add_posit16(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::AddPosit16, dtype: TensorTy::Posit16, a, b, imm: 0, shape }
    }

    pub fn mul_posit16(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::MulPosit16, dtype: TensorTy::Posit16, a, b, imm: 0, shape }
    }

    pub fn matmul_posit16(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::MatmulTilePosit16, dtype: TensorTy::Posit16, a, b, imm: 0, shape }
    }

    pub fn add_posit32(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::AddPosit32, dtype: TensorTy::Posit32, a, b, imm: 0, shape }
    }

    pub fn mul_posit32(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::MulPosit32, dtype: TensorTy::Posit32, a, b, imm: 0, shape }
    }

    pub fn matmul_posit32(a: u32, b: u32, shape: TensorShape) -> Self {
        Self { op: TensorOp::MatmulTilePosit32, dtype: TensorTy::Posit32, a, b, imm: 0, shape }
    }
}

// ---------- error ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorError {
    Truncated,
    TruncatedNode,
    BadMagic,
    BadVersion(u8),
    BadFooter,
    BadNodeCount(usize),
    FuelTooSmall,
    UnknownOp(u8),
    UnknownDtype(u8),
    DimOutOfRange(usize),
    ShapeRankInvalid(u8),
    ShapeNonZeroPastRank(usize),
    BackrefOutOfBounds { node: u32, index: u32 },
    ShapeMismatch { node: u32, reason: &'static str },
    DtypeMismatch { node: u32 },
    BadAxis { node: u32, axis: i32 },
    BadSlot { node: u32, slot: i32 },
    NoOutput,
    TooManyInputs,
    TooManyOutputs,
    ConstPoolOverflow,
    ReservedNonZero,
}

impl fmt::Display for TensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tensor error: {self:?}")
    }
}

// ---------- Numeric contract (Φ.μ.7) ----------
// Content-addressé : deux exécutions tensorielles produisent le même
// hash si et seulement si elles partagent dtype + arbre de réduction
// + famille de kernel + budget d'erreur. C'est la clé pour distinguer
// `(a+b)+c` et `a+(b+c)` dans les memos cross-machine sans casser
// l'identité cryptographique.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReductionTree {
    Ltr = 1,
    Pairwise = 2,
    Avx2Tile = 3,
    CudaBlock = 4,
    GpuWarp = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum KernelFamily {
    Scalar = 1,
    Avx2 = 2,
    Avx512 = 3,
    CudaCublas = 4,
    Metal = 5,
    Rocm = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoundMode {
    NearestEven = 1,
    TowardZero = 2,
    Down = 3,
    Up = 4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TensorErrorBudget {
    pub max_ulp: u32,
    pub max_abs: f32,
    pub max_rel: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuantGrid {
    pub round_mode: RoundMode,
    pub bits: u8,
}

const NUMERIC_CONTRACT_MAGIC: &[u8; 4] = b"NCT1";

#[derive(Clone, Debug, PartialEq)]
pub struct NumericContract {
    pub dtype: TensorTy,
    pub reduction_tree: ReductionTree,
    pub kernel_family: KernelFamily,
    pub tile_shape: Option<(u16, u16, u16)>,
    pub quant_grid: Option<QuantGrid>,
    pub error_budget: TensorErrorBudget,
}

impl NumericContract {
    /// Contrat strict : f32 scalaire LTR, zéro tolérance d'ULP.
    /// L'option canonique pour le bit-exact reproductible.
    pub fn strict_f32_scalar_ltr() -> Self {
        Self {
            dtype: TensorTy::F32,
            reduction_tree: ReductionTree::Ltr,
            kernel_family: KernelFamily::Scalar,
            tile_shape: None,
            quant_grid: None,
            error_budget: TensorErrorBudget {
                max_ulp: 0,
                max_abs: 0.0,
                max_rel: 0.0,
            },
        }
    }

    /// Sérialisation déterministe (28 octets) destinée à être hashée
    /// pour produire l'identité content-addressée du contrat.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, TensorError> {
        if !self.error_budget.max_abs.is_finite() || !self.error_budget.max_rel.is_finite() {
            return Err(TensorError::ReservedNonZero);
        }
        let mut out = Vec::with_capacity(28);
        out.extend_from_slice(NUMERIC_CONTRACT_MAGIC);
        out.push(1); // version
        out.push(self.dtype as u8);
        out.push(self.reduction_tree as u8);
        out.push(self.kernel_family as u8);
        match self.tile_shape {
            Some((m, n, k)) => {
                out.push(1);
                out.extend_from_slice(&m.to_le_bytes());
                out.extend_from_slice(&n.to_le_bytes());
                out.extend_from_slice(&k.to_le_bytes());
            }
            None => out.extend_from_slice(&[0; 7]),
        }
        match self.quant_grid {
            Some(grid) => {
                out.push(1);
                out.push(grid.round_mode as u8);
                out.push(grid.bits);
            }
            None => out.extend_from_slice(&[0; 3]),
        }
        out.extend_from_slice(&self.error_budget.max_ulp.to_le_bytes());
        out.extend_from_slice(&self.error_budget.max_abs.to_bits().to_le_bytes());
        out.extend_from_slice(&self.error_budget.max_rel.to_bits().to_le_bytes());
        Ok(out)
    }
}

#[cfg(test)]
mod numeric_contract_tests {
    use super::*;

    #[test]
    fn strict_contract_canonicalizes() {
        let c = NumericContract::strict_f32_scalar_ltr();
        let bytes = c.canonical_bytes().unwrap();
        assert_eq!(&bytes[..4], NUMERIC_CONTRACT_MAGIC);
    }

    #[test]
    fn reduction_tree_change_changes_canonical_bytes() {
        let strict = NumericContract::strict_f32_scalar_ltr();
        let mut pairwise = strict.clone();
        pairwise.reduction_tree = ReductionTree::Pairwise;
        assert_ne!(
            strict.canonical_bytes().unwrap(),
            pairwise.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn non_finite_error_budget_rejected() {
        let mut c = NumericContract::strict_f32_scalar_ltr();
        c.error_budget.max_abs = f32::NAN;
        assert!(c.canonical_bytes().is_err());
    }
}

impl std::error::Error for TensorError {}
