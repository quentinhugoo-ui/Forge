//! KASM v0.1: tiny verified structural microcode.
//!
//! A program is a bounded DAG of 8-byte nodes. There is no heap, no loop,
//! and no hidden state: verification proves every reference points backward,
//! so execution always terminates.
//!
//! The implementation is split into four sibling modules:
//!  * `types`       — Op/Ty/Target/Node + KasmError + reports.
//!  * `program`     — `Program` struct, `verify`, helpers (hash, hex, ...).
//!  * `interpreter` — `execute`, `compose` and value handling.
//!  * `optimizer`   — `canonicalize`, `simplify`, `cse`,
//!                    `semantic_fingerprint`, `static_output`.
//!
//! Public paths (`crate::kasm::Program`, `crate::kasm::execute`, ...)
//! are preserved through the re-exports below.

pub mod columnar;
pub mod errno;
pub mod execution;
pub mod fixed;
pub mod interpreter;
pub mod jit;
pub mod mlir;
pub mod nanbox;
pub mod numeric;
pub mod ohlcv;
mod optimizer;
pub mod order_book;
mod program;
pub mod proof;
pub mod rank;
pub mod reservoir;
pub mod resampler;
pub mod rewrite;
pub mod self_host;
pub mod self_host_lite;
pub mod ssa;
pub mod strategy;
pub mod tensor;
pub mod threaded;
pub mod timestamp;
mod types;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_e2e_audit;

pub use interpreter::{compose, execute, execute_with_fractal, try_execute_i64_inline, FractalDispatcher};
pub use mlir::{
    canonical_mlir_text, emit_mlir, hash_mlir_canonical, hash_mlir_canonical_hex, parse_mlir,
    MlirError,
};
pub use optimizer::{canonicalize, cse, semantic_fingerprint, simplify, static_output};
pub use program::{MultiMethod, Program};
// Legacy `verify` n'est plus exposé hors du crate (Ω-1.0 critère #4).
// Les consommateurs externes utilisent `Program::from_bytes` (binaire) ou
// `Program::from_mlir` (texte). Les usages internes au module `kasm`
// restent via `super::program::verify`.
pub use types::{
    F64SubOp, KasmError, Node, Op, PartialEvalReport, ProgramSig, RewriteReport, Target, Ty,
    FOOTER_LEN, HEADER_LEN, MAX_NODES, MAX_SLOTS, NODE_LEN,
};

pub(crate) use program::hash_i64;
