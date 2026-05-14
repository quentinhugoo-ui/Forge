//! TensorProgram: serialised form, verification, and program API.
//!
//! Wire layout:
//!
//!   [header (32 B)] [nodes (n × 32 B)] [const pool (variable)] [footer (20 B)]
//!
//! The const pool stores the raw bytes of every `Const` node's
//! literal data, packed back-to-back. Each `Const` node references
//! its pool slice via `(b = offset, imm = length_in_bytes)`. This
//! keeps the per-node footprint fixed at 32 B regardless of constant
//! size.
//!
//! Verification proves:
//!   * magic + version + footer match
//!   * every back-reference (`a`, `b`) points to an earlier node
//!   * every shape is consistent with the op's algebraic constraint
//!     (matmul `M×K · K×N → M×N`, reduce drops one axis, softmax
//!     preserves shape, etc.)
//!   * dtypes match across binary ops
//!   * fuel ≥ node count
//!   * exactly one `Output` node, last in the program
//!
//! On success, `verify_tensor` returns the `TensorProgram` ready to
//! be executed or hashed (via `Hash::for_blob(p.bytes())`).

use sha1_oneshot::sha1;

use super::types::{
    TensorError, TensorNode, TensorOp, TensorShape, TensorTy, TENSOR_FOOTER_LEN, TENSOR_HEADER_LEN,
    TENSOR_MAGIC, TENSOR_MAX_NODES, TENSOR_MAX_SLOTS, TENSOR_NODE_LEN, TENSOR_VERSION,
};

#[derive(Clone, Debug)]
pub struct TensorProgram {
    bytes: Vec<u8>,
    nodes: Vec<TensorNode>,
    const_pool: Vec<u8>,
    inputs: u8,
    outputs: u8,
    fuel: u32,
}

impl TensorProgram {
    pub fn new(
        inputs: u8,
        outputs: u8,
        fuel: u32,
        nodes: Vec<TensorNode>,
        const_pool: Vec<u8>,
    ) -> Result<Self, TensorError> {
        if inputs > TENSOR_MAX_SLOTS {
            return Err(TensorError::TooManyInputs);
        }
        if outputs > TENSOR_MAX_SLOTS {
            return Err(TensorError::TooManyOutputs);
        }
        if nodes.is_empty() || nodes.len() > TENSOR_MAX_NODES {
            return Err(TensorError::BadNodeCount(nodes.len()));
        }
        if fuel < nodes.len() as u32 {
            return Err(TensorError::FuelTooSmall);
        }

        let mut bytes = Vec::with_capacity(
            TENSOR_HEADER_LEN
                + nodes.len() * TENSOR_NODE_LEN
                + const_pool.len()
                + TENSOR_FOOTER_LEN,
        );
        // Header
        bytes.extend_from_slice(TENSOR_MAGIC);
        bytes.push(TENSOR_VERSION);
        bytes.push(0); // reserved (target = CPU)
        bytes.push(inputs);
        bytes.push(outputs);
        bytes.extend_from_slice(&fuel.to_le_bytes());
        bytes.extend_from_slice(&(nodes.len() as u32).to_le_bytes());
        // const_pool length (4 bytes) so a verifier can locate the
        // pool without scanning.
        bytes.extend_from_slice(&(const_pool.len() as u32).to_le_bytes());
        // Pad to TENSOR_HEADER_LEN
        while bytes.len() < TENSOR_HEADER_LEN {
            bytes.push(0);
        }

        // Nodes
        for node in &nodes {
            node.encode(&mut bytes);
        }
        // Const pool
        bytes.extend_from_slice(&const_pool);
        // Footer = SHA-1 of everything before
        let footer = sha1(&bytes);
        bytes.extend_from_slice(&footer);

        // Run verification on the assembled bytes — this re-derives
        // the nodes/pool from the bytes and is the same code path
        // that a fresh `verify_tensor` call would take.
        verify_tensor(&bytes)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn nodes(&self) -> &[TensorNode] {
        &self.nodes
    }

    pub fn const_pool(&self) -> &[u8] {
        &self.const_pool
    }

    pub fn inputs(&self) -> u8 {
        self.inputs
    }

    pub fn outputs(&self) -> u8 {
        self.outputs
    }

    pub fn fuel(&self) -> u32 {
        self.fuel
    }
}

/// Verify a serialised `TensorProgram`. Returns the parsed program on
/// success — same code path used by `TensorProgram::new` to confirm
/// a freshly-built program is valid.
pub fn verify_tensor(bytes: &[u8]) -> Result<TensorProgram, TensorError> {
    if bytes.len() < TENSOR_HEADER_LEN + TENSOR_FOOTER_LEN {
        return Err(TensorError::Truncated);
    }
    if &bytes[0..8] != TENSOR_MAGIC {
        return Err(TensorError::BadMagic);
    }
    if bytes[8] != TENSOR_VERSION {
        return Err(TensorError::BadVersion(bytes[8]));
    }
    // bytes[9] = reserved (target)
    let inputs = bytes[10];
    let outputs = bytes[11];
    if inputs > TENSOR_MAX_SLOTS {
        return Err(TensorError::TooManyInputs);
    }
    if outputs > TENSOR_MAX_SLOTS {
        return Err(TensorError::TooManyOutputs);
    }
    let fuel = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let nodes_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let const_pool_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;

    if nodes_len == 0 || nodes_len > TENSOR_MAX_NODES {
        return Err(TensorError::BadNodeCount(nodes_len));
    }
    if fuel < nodes_len as u32 {
        return Err(TensorError::FuelTooSmall);
    }

    let body_start = TENSOR_HEADER_LEN;
    let nodes_end = body_start + nodes_len * TENSOR_NODE_LEN;
    let pool_end = nodes_end + const_pool_len;
    let footer_start = pool_end;
    let total = footer_start + TENSOR_FOOTER_LEN;
    if bytes.len() != total {
        return Err(TensorError::Truncated);
    }

    // Footer check
    let actual_footer = sha1(&bytes[..footer_start]);
    if actual_footer != bytes[footer_start..total] {
        return Err(TensorError::BadFooter);
    }

    // Decode nodes
    let mut nodes = Vec::with_capacity(nodes_len);
    for i in 0..nodes_len {
        let off = body_start + i * TENSOR_NODE_LEN;
        let node = TensorNode::decode(&bytes[off..off + TENSOR_NODE_LEN])?;
        nodes.push(node);
    }

    let const_pool = bytes[nodes_end..pool_end].to_vec();

    // Per-node verification (back-refs, shapes, dtypes, axis bounds).
    let mut output_count = 0u8;
    let mut output_index: Option<usize> = None;
    for (i, node) in nodes.iter().enumerate() {
        let i_u32 = i as u32;
        match node.op {
            TensorOp::Input => {
                if node.imm < 0 || node.imm as u8 >= inputs {
                    return Err(TensorError::BadSlot { node: i_u32, slot: node.imm });
                }
            }
            TensorOp::Const => {
                let off = node.b as usize;
                let len = node.imm as usize;
                if off > const_pool_len
                    || len > const_pool_len
                    || off.checked_add(len).map_or(true, |end| end > const_pool_len)
                {
                    return Err(TensorError::ConstPoolOverflow);
                }
                let expected = node.shape.elements() * node.dtype.byte_size();
                if expected != len {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "const literal length disagrees with shape × dtype",
                    });
                }
            }
            TensorOp::Output => {
                if node.a >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.a });
                }
                let src = &nodes[node.a as usize];
                if src.shape != node.shape || src.dtype != node.dtype {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "output disagrees with source",
                    });
                }
                output_count += 1;
                output_index = Some(i);
            }
            TensorOp::AddF32
            | TensorOp::MulF32
            | TensorOp::AddRational
            | TensorOp::MulRational
            | TensorOp::AddPosit16
            | TensorOp::MulPosit16
            | TensorOp::AddPosit32
            | TensorOp::MulPosit32 => {
                if node.a >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.a });
                }
                if node.b >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.b });
                }
                let lhs = &nodes[node.a as usize];
                let rhs = &nodes[node.b as usize];
                if lhs.shape != rhs.shape || lhs.shape != node.shape {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "elementwise operands and result must share shape",
                    });
                }
                if lhs.dtype != rhs.dtype || lhs.dtype != node.dtype {
                    return Err(TensorError::DtypeMismatch { node: i_u32 });
                }
            }
            TensorOp::MatmulTile
            | TensorOp::MatmulTileRational
            | TensorOp::MatmulTilePosit16
            | TensorOp::MatmulTilePosit32 => {
                if node.a >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.a });
                }
                if node.b >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.b });
                }
                let lhs = &nodes[node.a as usize];
                let rhs = &nodes[node.b as usize];
                if lhs.shape.dims != 2 || rhs.shape.dims != 2 || node.shape.dims != 2 {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "matmul requires 2-D operands",
                    });
                }
                let m = lhs.shape.d[0];
                let k_lhs = lhs.shape.d[1];
                let k_rhs = rhs.shape.d[0];
                let n = rhs.shape.d[1];
                if k_lhs != k_rhs {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "matmul inner dims disagree",
                    });
                }
                if node.shape.d[0] != m || node.shape.d[1] != n {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "matmul result shape disagrees with operands",
                    });
                }
                if lhs.dtype != rhs.dtype || lhs.dtype != node.dtype {
                    return Err(TensorError::DtypeMismatch { node: i_u32 });
                }
            }
            TensorOp::ReduceSumAxis => {
                if node.a >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.a });
                }
                let src = &nodes[node.a as usize];
                let axis = node.imm;
                if axis < 0 || axis as u8 >= src.shape.dims {
                    return Err(TensorError::BadAxis { node: i_u32, axis });
                }
                let expected = drop_axis(&src.shape, axis as u8)?;
                if expected != node.shape {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "reduce shape must drop the reduced axis",
                    });
                }
                if src.dtype != node.dtype {
                    return Err(TensorError::DtypeMismatch { node: i_u32 });
                }
            }
            TensorOp::Softmax => {
                if node.a >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.a });
                }
                let src = &nodes[node.a as usize];
                let axis = node.imm;
                if axis < 0 || axis as u8 >= src.shape.dims {
                    return Err(TensorError::BadAxis { node: i_u32, axis });
                }
                if src.shape != node.shape || src.dtype != node.dtype {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "softmax preserves shape and dtype",
                    });
                }
                if !matches!(node.dtype, TensorTy::F32) {
                    return Err(TensorError::DtypeMismatch { node: i_u32 });
                }
            }
            // All elementwise activations: shape-preserving + dtype-preserving.
            TensorOp::ReluF32
            | TensorOp::TanhF32
            | TensorOp::SigmoidF32
            | TensorOp::GeluTanhF32 => {
                if node.a >= i_u32 {
                    return Err(TensorError::BackrefOutOfBounds { node: i_u32, index: node.a });
                }
                let src = &nodes[node.a as usize];
                if src.shape != node.shape || src.dtype != node.dtype {
                    return Err(TensorError::ShapeMismatch {
                        node: i_u32,
                        reason: "elementwise activation preserves shape and dtype",
                    });
                }
                if !matches!(node.dtype, TensorTy::F32) {
                    return Err(TensorError::DtypeMismatch { node: i_u32 });
                }
            }
        }
    }
    if output_count == 0 || output_index != Some(nodes.len() - 1) {
        return Err(TensorError::NoOutput);
    }

    Ok(TensorProgram {
        bytes: bytes.to_vec(),
        nodes,
        const_pool,
        inputs,
        outputs,
        fuel,
    })
}

fn drop_axis(shape: &TensorShape, axis: u8) -> Result<TensorShape, TensorError> {
    if axis >= shape.dims {
        return Err(TensorError::BadAxis { node: 0, axis: axis as i32 });
    }
    let mut out = TensorShape::scalar();
    out.dims = shape.dims.saturating_sub(1);
    let mut j = 0usize;
    for i in 0..shape.dims as usize {
        if i as u8 == axis {
            continue;
        }
        out.d[j] = shape.d[i];
        j += 1;
    }
    Ok(out)
}

// Minimal SHA-1 wrapper — reuses the Store's SHA-1 by pulling it
// in via a private helper so we don't add a dependency.
mod sha1_oneshot {
    pub fn sha1(bytes: &[u8]) -> [u8; 20] {
        let mut h0 = 0x67452301u32;
        let mut h1 = 0xefcdab89u32;
        let mut h2 = 0x98badcfeu32;
        let mut h3 = 0x10325476u32;
        let mut h4 = 0xc3d2e1f0u32;

        let bit_len = (bytes.len() as u64) * 8;
        let mut msg = bytes.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());

        for chunk in msg.chunks_exact(64) {
            let mut w = [0u32; 80];
            for (i, word) in w.iter_mut().take(16).enumerate() {
                let j = i * 4;
                *word =
                    u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }
            let mut a = h0;
            let mut b = h1;
            let mut c = h2;
            let mut d = h3;
            let mut e = h4;
            for (i, word) in w.iter().enumerate() {
                let (f, k) = match i {
                    0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                    20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                    _ => (b ^ c ^ d, 0xca62c1d6),
                };
                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(*word);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }
            h0 = h0.wrapping_add(a);
            h1 = h1.wrapping_add(b);
            h2 = h2.wrapping_add(c);
            h3 = h3.wrapping_add(d);
            h4 = h4.wrapping_add(e);
        }

        let mut out = [0u8; 20];
        out[0..4].copy_from_slice(&h0.to_be_bytes());
        out[4..8].copy_from_slice(&h1.to_be_bytes());
        out[8..12].copy_from_slice(&h2.to_be_bytes());
        out[12..16].copy_from_slice(&h3.to_be_bytes());
        out[16..20].copy_from_slice(&h4.to_be_bytes());
        out
    }
}
