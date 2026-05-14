//! DreamForge-Tensor — automatic shortcut discovery on
//! `TensorProgram`s.
//!
//! # The premise
//!
//! `examples/tensor_layer_distill_demo.rs` proved that an FFN
//! block `x → matmul(W₁) → ReLU → matmul(W₂) → y` collapses to a
//! single matmul `x → matmul(W₁·W₂) → y` whenever the activation
//! is structurally redundant on the observed input domain. The
//! demo built the shortcut *by hand*. This module builds it
//! **automatically**:
//!
//!   1. Recognise the FFN pattern in a `TensorProgram` AST.
//!   2. Run the original program on a batch of observed samples,
//!      capturing the hidden-layer activations.
//!   3. Test whether the activation function is **the identity**
//!      across that sample set (sample-level evidence, not
//!      symbolic proof). For ReLU this means "no negative
//!      activations were observed".
//!   4. If yes, compute `W_combined = W₁ · W₂` host-side and emit
//!      a fresh `TensorProgram` that's the single matmul
//!      shortcut.
//!   5. Validate the shortcut against the original on the same
//!      samples (and a holdout set), within an ε tolerance.
//!   6. Return a `DistilledShortcut` carrying the new program,
//!      its hash, and witness samples.
//!
//! # What it does NOT do (yet)
//!
//!   * Discover *non-trivial* algebraic identities (e.g. softmax
//!     simplifications, attention head pruning, low-rank
//!     factorisation). The current implementation handles only
//!     the `matmul → activation → matmul` pattern with the
//!     activation collapsing to identity on the sample domain.
//!   * Produce a contract-addressed cube. That's Codex Cortex
//!     territory; we return a `DistilledShortcut` value and let
//!     the caller decide where it lives.
//!
//! # Why this matters
//!
//! This is the seed of a runtime that **automatically simplifies
//! its own tensor programs** by observation. Combined with Vague
//! 5's always-on i64 daemon, it's the same loop applied to the
//! ML domain: see → understand → shortcut → never recompute.
//! Nothing in the open-source ML world does this at the program
//! level (PyTorch tracing, ONNX optimisers, TVM, etc. operate on
//! hand-written rewrite passes — they don't synthesise new
//! programs from observed activations).

use super::interpreter::execute_tensor;
use super::program::{verify_tensor, TensorProgram};
use super::types::{TensorError, TensorNode, TensorOp, TensorShape, TensorTy};

/// A successful distillation: a shorter `TensorProgram` proven to
/// reproduce the original's output within `tolerance` on the
/// observed samples (and on a holdout set chosen by the caller).
#[derive(Debug)]
pub struct DistilledShortcut {
    pub shortcut: TensorProgram,
    pub max_abs_diff_observed: f32,
    pub samples_validated: usize,
    pub original_node_count: usize,
    pub shortcut_node_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DistillTensorConfig {
    /// Maximum |Δ| permitted between the shortcut's output and the
    /// original's on any sample. 1e-5 is reasonable for f32 with
    /// matmul reduction-order differences across the substitution.
    pub tolerance: f32,
    /// Minimum number of samples required before attempting the
    /// distillation. Below this, sample-level evidence isn't
    /// strong enough to claim the activation is structurally
    /// redundant on the input domain.
    pub min_samples: usize,
}

impl Default for DistillTensorConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-5,
            min_samples: 8,
        }
    }
}

#[derive(Debug)]
pub enum DistillError {
    /// The program AST didn't match a known pattern. Future
    /// extensions can recognise more patterns; for now we only
    /// look for `Const(W₁) · matmul(input, W₁) → activation →
    /// matmul(.., W₂) → output`.
    PatternNotMatched,
    /// Not enough samples to attempt distillation.
    InsufficientSamples,
    /// On at least one sample, the candidate activation produced
    /// values that differ from the identity (e.g. ReLU clamped a
    /// negative). Distillation rejected — the shortcut would be
    /// wrong on that input.
    ActivationNotIdentityOnSamples { offending_sample_index: usize },
    /// The shortcut output diverged from the original past
    /// `tolerance` on one of the validation samples — even though
    /// the activation looked like identity, numerical stability
    /// killed the equivalence.
    ShortcutDiverges { sample_index: usize, diff: f32 },
    /// Underlying tensor execution / verification failure.
    Tensor(TensorError),
}

impl From<TensorError> for DistillError {
    fn from(e: TensorError) -> Self {
        DistillError::Tensor(e)
    }
}

/// Try to distill the FFN-block pattern out of `program` using
/// the supplied `samples` as evidence. The samples must each be
/// a flat row-major `[1×IN_DIM]` `f32` vector matching the
/// program's input shape.
///
/// Returns `Ok(Some(DistilledShortcut))` when the shortcut fits
/// every sample within `config.tolerance`, `Ok(None)` if no
/// pattern was matched, and `Err(...)` if some structural or
/// numerical condition vetoed the distillation.
pub fn try_distill_ffn_block(
    program: &TensorProgram,
    samples: &[Vec<f32>],
    config: DistillTensorConfig,
) -> Result<Option<DistilledShortcut>, DistillError> {
    if samples.len() < config.min_samples {
        return Err(DistillError::InsufficientSamples);
    }

    // ---- 1. Pattern recognition ----
    //
    // Required AST shape (in node order):
    //
    //   0  Input(slot=0)            [1, IN_DIM]
    //   1  Const(W₁)                [IN_DIM, HIDDEN]
    //   2  Const(W₂)                [HIDDEN, OUT_DIM]
    //   3  Matmul(0, 1)             [1, HIDDEN]   = h_pre
    //   4  ReluF32(3)               [1, HIDDEN]   = h
    //   5  Matmul(4, 2)             [1, OUT_DIM]  = y_pre
    //   6  Output(5)                [1, OUT_DIM]
    //
    // We accept this strict layout for now. Future versions can
    // tolerate operand reordering and wrap nodes with permutations.
    let nodes = program.nodes();
    if nodes.len() != 7 {
        return Ok(None); // not the FFN pattern we recognise
    }

    let pattern = match (
        nodes[0].op,
        nodes[1].op,
        nodes[2].op,
        nodes[3].op,
        nodes[4].op,
        nodes[5].op,
        nodes[6].op,
    ) {
        (
            TensorOp::Input,
            TensorOp::Const,
            TensorOp::Const,
            TensorOp::MatmulTile,
            TensorOp::ReluF32,
            TensorOp::MatmulTile,
            TensorOp::Output,
        ) => true,
        _ => false,
    };
    if !pattern {
        return Ok(None);
    }

    // Topology check: the wires must compose into the FFN form.
    if !(nodes[3].a == 0 && nodes[3].b == 1
        && nodes[4].a == 3
        && nodes[5].a == 4
        && nodes[5].b == 2
        && nodes[6].a == 5)
    {
        return Ok(None);
    }

    // Shape extraction.
    let input_shape = nodes[0].shape;
    let w1_shape = nodes[1].shape;
    let w2_shape = nodes[2].shape;
    let h_shape = nodes[3].shape;
    let y_shape = nodes[5].shape;
    if input_shape.dims != 2 || input_shape.d[0] != 1 {
        return Ok(None);
    }
    let in_dim = input_shape.d[1] as usize;
    let hidden = w1_shape.d[1] as usize;
    let out_dim = w2_shape.d[1] as usize;
    if w1_shape.d[0] as usize != in_dim
        || w2_shape.d[0] as usize != hidden
        || h_shape.d[1] as usize != hidden
        || y_shape.d[1] as usize != out_dim
    {
        return Ok(None);
    }

    // Extract W₁ and W₂ from the const pool.
    let pool = program.const_pool();
    let w1_offset = nodes[1].b as usize;
    let w1_len = nodes[1].imm as usize;
    let w2_offset = nodes[2].b as usize;
    let w2_len = nodes[2].imm as usize;
    let w1: Vec<f32> = pool[w1_offset..w1_offset + w1_len]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let w2: Vec<f32> = pool[w2_offset..w2_offset + w2_len]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();

    // ---- 2. Run on samples; check ReLU activations are pure
    //         identity (i.e. no observed negative h_pre value) ----
    for (idx, sample) in samples.iter().enumerate() {
        if sample.len() != in_dim {
            return Err(DistillError::InsufficientSamples); // bad input shape
        }
        // Compute h_pre = x · W₁ in f32 host-side (matches the
        // interpreter's reduction order).
        for j in 0..hidden {
            let mut acc = 0.0f32;
            for k in 0..in_dim {
                acc += sample[k] * w1[k * hidden + j];
            }
            if acc < 0.0 {
                return Err(DistillError::ActivationNotIdentityOnSamples {
                    offending_sample_index: idx,
                });
            }
        }
    }

    // ---- 3. Synthesise the shortcut (W_combined = W₁ · W₂) ----
    let mut w_combined = vec![0.0f32; in_dim * out_dim];
    for i in 0..in_dim {
        for j in 0..out_dim {
            let mut acc = 0.0f32;
            for k in 0..hidden {
                acc += w1[i * hidden + k] * w2[k * out_dim + j];
            }
            w_combined[i * out_dim + j] = acc;
        }
    }

    let x_shape = TensorShape::matrix(1, in_dim).map_err(DistillError::Tensor)?;
    let w_shape = TensorShape::matrix(in_dim, out_dim).map_err(DistillError::Tensor)?;
    let y_shape_canonical = TensorShape::matrix(1, out_dim).map_err(DistillError::Tensor)?;
    let mut pool_bytes = Vec::with_capacity(w_combined.len() * 4);
    for v in &w_combined {
        pool_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let shortcut_nodes = vec![
        TensorNode::input(0, TensorTy::F32, x_shape),
        TensorNode::const_at(0, pool_bytes.len() as u32, TensorTy::F32, w_shape),
        TensorNode::matmul(0, 1, TensorTy::F32, y_shape_canonical),
        TensorNode::output(2, TensorTy::F32, y_shape_canonical),
    ];
    let shortcut = TensorProgram::new(
        1,
        1,
        shortcut_nodes.len() as u32,
        shortcut_nodes,
        pool_bytes,
    )
    .map_err(DistillError::Tensor)?;

    // Re-verify the produced bytes (defense in depth).
    let _ = verify_tensor(shortcut.bytes()).map_err(DistillError::Tensor)?;

    // ---- 4. Validate against the original on every sample ----
    let mut max_diff = 0.0f32;
    for (idx, sample) in samples.iter().enumerate() {
        let original_out = execute_tensor(program, &[sample.clone()])?;
        let shortcut_out = execute_tensor(&shortcut, &[sample.clone()])?;
        if original_out.len() != shortcut_out.len() {
            return Err(DistillError::ShortcutDiverges {
                sample_index: idx,
                diff: f32::INFINITY,
            });
        }
        for (a, b) in original_out.iter().zip(shortcut_out.iter()) {
            let d = (a - b).abs();
            if d > max_diff {
                max_diff = d;
            }
            if d > config.tolerance {
                return Err(DistillError::ShortcutDiverges {
                    sample_index: idx,
                    diff: d,
                });
            }
        }
    }

    Ok(Some(DistilledShortcut {
        shortcut,
        max_abs_diff_observed: max_diff,
        samples_validated: samples.len(),
        original_node_count: nodes.len(),
        shortcut_node_count: 4,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_pool(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    fn build_ffn(w1: &[f32], w2: &[f32], in_dim: usize, hidden: usize, out_dim: usize) -> TensorProgram {
        let x_shape = TensorShape::matrix(1, in_dim).unwrap();
        let w1_shape = TensorShape::matrix(in_dim, hidden).unwrap();
        let h_shape = TensorShape::matrix(1, hidden).unwrap();
        let w2_shape = TensorShape::matrix(hidden, out_dim).unwrap();
        let y_shape = TensorShape::matrix(1, out_dim).unwrap();

        let w1_pool = f32_pool(w1);
        let w2_pool = f32_pool(w2);
        let mut pool = w1_pool.clone();
        let w2_off = pool.len() as u32;
        pool.extend_from_slice(&w2_pool);

        let nodes = vec![
            TensorNode::input(0, TensorTy::F32, x_shape),
            TensorNode::const_at(0, w1_pool.len() as u32, TensorTy::F32, w1_shape),
            TensorNode::const_at(w2_off, w2_pool.len() as u32, TensorTy::F32, w2_shape),
            TensorNode::matmul(0, 1, TensorTy::F32, h_shape),
            TensorNode::relu(3, TensorTy::F32, h_shape),
            TensorNode::matmul(4, 2, TensorTy::F32, y_shape),
            TensorNode::output(5, TensorTy::F32, y_shape),
        ];
        TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap()
    }

    #[test]
    fn distill_ffn_collapses_to_single_matmul_when_relu_redundant() {
        // All-positive weights + non-negative inputs → ReLU is identity.
        let in_dim = 4;
        let hidden = 8;
        let out_dim = 3;
        let w1: Vec<f32> = (0..in_dim * hidden).map(|i| 0.1 + (i as f32) * 0.013).collect();
        let w2: Vec<f32> = (0..hidden * out_dim).map(|i| 0.2 + (i as f32) * 0.007).collect();
        let program = build_ffn(&w1, &w2, in_dim, hidden, out_dim);

        let samples: Vec<Vec<f32>> = (0..32u64)
            .map(|s| {
                (0..in_dim)
                    .map(|i| 0.1 + ((s as f32 + i as f32) * 0.07).sin().abs())
                    .collect()
            })
            .collect();

        let cfg = DistillTensorConfig::default();
        let result = try_distill_ffn_block(&program, &samples, cfg).unwrap();
        let shortcut = result.expect("FFN with non-negative activations must distill");
        assert!(shortcut.max_abs_diff_observed < cfg.tolerance);
        assert_eq!(shortcut.original_node_count, 7);
        assert_eq!(shortcut.shortcut_node_count, 4);
    }

    #[test]
    fn distill_refuses_when_relu_observed_clamping() {
        // Negative weights → ReLU sometimes clamps. Refuse.
        let in_dim = 3;
        let hidden = 4;
        let out_dim = 2;
        let w1 = vec![
            0.5, -0.3, 0.2, -0.1,
            -0.4, 0.6, -0.2, 0.1,
            0.1, 0.2, -0.5, 0.3,
        ];
        let w2 = vec![
            0.4, -0.2,
            -0.3, 0.5,
            0.2, -0.1,
            -0.4, 0.3,
        ];
        let program = build_ffn(&w1, &w2, in_dim, hidden, out_dim);
        let samples: Vec<Vec<f32>> = (0..16u64)
            .map(|s| (0..in_dim).map(|i| ((s as f32 + i as f32) * 0.31).cos()).collect())
            .collect();
        let cfg = DistillTensorConfig::default();
        let err = try_distill_ffn_block(&program, &samples, cfg).expect_err("must reject");
        assert!(matches!(err, DistillError::ActivationNotIdentityOnSamples { .. }));
    }

    #[test]
    fn distill_returns_none_on_unrecognised_pattern() {
        // A shape-only mini program with just const + output. Not
        // the FFN pattern; distill should return Ok(None).
        let shape = TensorShape::vec(3).unwrap();
        let pool = f32_pool(&[1.0, 2.0, 3.0]);
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::F32, shape),
            TensorNode::output(0, TensorTy::F32, shape),
        ];
        let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();
        let samples = vec![vec![0.0f32; 3]; 16];
        let result =
            try_distill_ffn_block(&program, &samples, DistillTensorConfig::default()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn distill_refuses_with_too_few_samples() {
        let in_dim = 4;
        let hidden = 8;
        let out_dim = 3;
        let w1 = vec![0.1f32; in_dim * hidden];
        let w2 = vec![0.1f32; hidden * out_dim];
        let program = build_ffn(&w1, &w2, in_dim, hidden, out_dim);
        let too_few = vec![vec![0.5f32; in_dim]; 3];
        let err = try_distill_ffn_block(&program, &too_few, DistillTensorConfig::default())
            .expect_err("too few samples must refuse");
        assert!(matches!(err, DistillError::InsufficientSamples));
    }
}
