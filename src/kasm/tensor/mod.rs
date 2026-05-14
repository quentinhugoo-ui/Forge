//! KASM-Tensor — additive **parallel dialect** for tensor ML
//! programs, sitting alongside KASM-Int (the original i64→i64 DAG)
//! without modifying it.
//!
//! Why a separate dialect, not an extension of KASM-Int:
//!   * KASM-Int's 8-byte node format (`a, b, imm`) doesn't carry a
//!     shape or a dtype. Stuffing both in would either break the
//!     existing JIT (`src/kasm/jit.rs`) or balloon every i64 program.
//!   * KASM-Int's `MAX_NODES = 32 768` and 1-input/1-output limit are
//!     wrong shapes for tensor pipelines (a single attention head is
//!     ~10 ops but ~50 KB of constants).
//!   * The verifier, semantic_fingerprint, and DreamForge are tuned
//!     for scalar-output rules.
//!
//! The two dialects share **the substrate**:
//!   * Same `Hash` content-addressing (a `TensorProgram` is a blob,
//!     hashable via `Hash::for_blob` exactly like a `Program`).
//!   * Same `Store` for persistence.
//!   * Same memo-ref convention (`refs/memo/<call_key>` once we wire
//!     it up — out of scope for this session, owned by Codex's
//!     Memory Cortex track).
//!
//! Scope of this introductory layer (deliberately tiny):
//!   * `TensorTy::F32` only.
//!   * Shapes up to 2 dimensions.
//!   * Op set: `Const`, `Input`, `Output`, `AddF32`, `MulF32`,
//!     `MatmulTile`, `ReduceSumAxis`, `Softmax`.
//!
//! Anything richer (BF16, more dtypes, attention/conv ops, JIT,
//! gradients) belongs to a follow-up. The present module is a
//! **substrate** — enough to let Mojo+scan, `#[scan]` proc-macro
//! Rust, DreamForge-Tensor, and layer-level memo all target a real,
//! verifiable, content-addressed tensor IR.

mod distill;
mod interpreter;
mod program;
mod types;

#[cfg(test)]
mod tests;

pub use distill::{
    try_distill_ffn_block, DistillError, DistillTensorConfig, DistilledShortcut,
};
pub use interpreter::{
    execute_tensor, execute_tensor_polymorphic, execute_tensor_posit16, execute_tensor_posit32,
    execute_tensor_rational, TensorValue,
};
pub use program::{verify_tensor, TensorProgram};
pub use types::{
    KernelFamily, NumericContract, QuantGrid, ReductionTree, RoundMode, TensorError,
    TensorErrorBudget, TensorNode, TensorOp, TensorShape, TensorTy, TENSOR_HEADER_LEN,
    TENSOR_MAGIC, TENSOR_MAX_DIMS, TENSOR_MAX_NODES, TENSOR_MAX_SLOTS, TENSOR_NODE_LEN,
    TENSOR_VERSION,
};
