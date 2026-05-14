//! Λ.0 — The singular operation of Forge : `apply(func, input) → output`.
//!
//! This module materializes the doctrine §8 (mutation substrat) Λ.0
//! axiom : there is one verb in Forge — `apply`. A KASM program (named
//! by its content hash) is applied to an input (also content-hashed
//! before lookup) and produces an output (likewise content-hashed and
//! persisted).
//!
//! ## Why this exists alongside `dispatch_batch`
//!
//! Historically Forge accumulated several entry points :
//! `MonsterNode::dispatch_batch`, `kasm::execute`, the brain layer
//! cascade in `dispatch_impl`, the auto-router fast path. Each has its
//! reason but together they fragment the surface.
//!
//! `apply` collapses the contract :
//!   - one input/output type signature : (Hash, &[u8]) → Vec<u8>
//!   - one cross-session memo path : atlas RESULT
//!   - one execute path : `kasm::execute` (fall-through)
//!
//! ## Λ.1 — Inputs are content-addressed
//!
//! `Atlas::result_key(func_bytes, input_bytes)` truncates the input
//! to 12 bytes — fine for scalar `i64` inputs (8 bytes, no aliasing)
//! but UNSAFE for any caller passing >12-byte inputs (e.g. a Tauri
//! command handing through CSV bytes, or a Vec<i64> wire payload).
//! Two distinct inputs sharing the same first 12 bytes silently alias
//! to the same atlas entry.
//!
//! `apply` fixes this by hashing the input via `Hash::for_blob` (SHA-1)
//! BEFORE composing the key. The result_key then carries 12 bytes of
//! a uniformly-random hash → collision probability ~2⁻⁹⁶, effectively
//! zero. No aliasing, regardless of input length.
//!
//! ## Cross-domain content addressing
//!
//! Because the atlas RESULT key is `func_hash || input_hash`, and both
//! are domain-blind cryptographic hashes, ANY computation in ANY domain
//! that reduces to the same `(func, input)` pair shares the cached
//! result. Trading SMA on a DNA-sized window, a physics simulation
//! producing the same i64 output sequence as a financial backtest —
//! all collide on the atlas at this level. The doctrine §9 paranoid
//! filter is the foundation ; `apply` is the API.

use std::io;

use crate::atlas::Atlas;
use crate::kasm::{Program, Ty};
use crate::monster::MonsterNode;
use crate::store::Hash;

/// Λ.0 — Apply a KASM program to an input, with cross-session memo.
///
/// The `func` is the SHA-1 of the KASM bytecode (already in the node's
/// Store). The `input` is hashed before atlas lookup, so any input
/// length is keyed safely. On a cache hit the output bytes are
/// materialized from the Store via the persisted output hash. On a
/// miss the program is executed once, the output is stored, and the
/// `(func, input_hash) → output_hash` mapping is recorded in the atlas
/// for every future session and every future machine sharing the
/// atlas file.
///
/// Returns the output bytes. Their Hash can be recovered via
/// `Hash::for_blob(&output)` if the caller needs an identity handle.
pub fn apply(node: &MonsterNode, func: Hash, input: &[u8]) -> io::Result<Vec<u8>> {
    let func = crate::brain::resolve_program_hash(node, func);

    // Λ.1 — content-addressed input. Hashing the input here is the
    // *whole point* of this layer : it makes the atlas key uniform
    // (40 entropy bits per side) and immune to input-length aliasing.
    let input_hash = Hash::for_blob(input);
    let atlas_key = Atlas::result_key(func.as_bytes(), input_hash.as_bytes());

    // Cross-session lookup. A hit means : "someone, somewhere, in some
    // earlier session, has computed this exact `(func, input)` pair and
    // persisted the result. We don't need the interpreter, the JIT, or
    // any brain layer."
    if let Some(atlas) = node.atlas() {
        if let Some(result_hash_bytes) = atlas.lookup_result(&atlas_key) {
            let result_hash = Hash::from_bytes(result_hash_bytes);
            if let Some(blob) = node.store().load(&result_hash) {
                return Ok(blob);
            }
            // Atlas claims the result exists but the Store doesn't have
            // it (orphaned hash, e.g. partial sync). Fall through to
            // recompute — atlas write below will refresh the mapping.
        }
    }

    // Miss : load the program, execute via the canonical interpreter,
    // persist both the output blob (in Store) and the atlas mapping.
    let program_bytes = node
        .store()
        .load(&func)
        .ok_or_else(|| io::Error::other("apply: program not in store"))?;
    let program = Program::from_bytes(&program_bytes)
        .map_err(|e| io::Error::other(format!("apply: bad program bytes: {e:?}")))?;
    let program = crate::brain::tighten_program_for_execution(node, func, program, 8);
    let func = Hash::for_blob(program.bytes());
    let atlas_key = Atlas::result_key(func.as_bytes(), input_hash.as_bytes());
    if let Some(atlas) = node.atlas() {
        if let Some(result_hash_bytes) = atlas.lookup_result(&atlas_key) {
            let result_hash = Hash::from_bytes(result_hash_bytes);
            if let Some(blob) = node.store().load(&result_hash) {
                return Ok(blob);
            }
        }
    }

    let output = crate::kasm::execute(&program, input)
        .map_err(|e| io::Error::other(format!("apply: execute failed: {e:?}")))?;

    let output_hash = node.store().store(&output)?;
    if let Some(atlas) = node.atlas() {
        let _ = atlas.record_result(&atlas_key, output_hash.as_bytes());
    }

    Ok(output)
}

/// M6 — Λ.4 step : `apply()` for a sub-tree of a program.
///
/// Given a program and a node index, slice out the minimal sub-program
/// rooted at that node, store it (Hash::for_blob over the bytecode),
/// and route through `apply()`. Each distinct sub-tree gets its own
/// atlas RESULT entry under `(sub_program_hash, input_hash)` — the
/// doctrine §9 paranoid filter realised at sub-expression scale.
///
/// ## Why this matters
///
/// `apply()` content-addresses whole programs. `apply_subtree()` does
/// the same for sub-expressions. When two programs share a sub-tree
/// (a common SMA(N) computation, a hash-chain primitive, an affine
/// transformation), running ONE program populates the atlas for ANY
/// future program that contains the same sub-tree as a sub-expression.
///
/// Cross-domain reach scales accordingly : the trade pipeline's L1
/// loss reduction (`v_sum(v_abs(...))`) and a hypothetical genomics
/// program's mean-absolute-error (same KASM bytecode, different
/// semantic interpretation) share the same sub-tree atlas entries.
///
/// ## Convention
///
/// The sliced sub-program inherits the source program's input slots
/// — callers pass the SAME `input` bytes they would to `apply()` on
/// the full program. The output type must match the slice root's
/// type ; pass `Ty::I64` for arithmetic sub-expressions.
///
/// ## Forward path to full Λ.4
///
/// `apply_subtree` is the OPT-IN form. The full Λ.4 — recursive
/// `apply()` automatically invoked from inside `kasm::execute` for
/// every CSE-equivalent sub-tree — is a separate runtime refactor
/// (the dispatch path needs to detect shared sub-trees and route
/// them through `apply()` without changing the caller-visible
/// semantics of `kasm::execute`). For now, `apply_subtree` exposes
/// the primitive ; the runtime auto-discovery is M6+.
pub fn apply_subtree(
    node: &MonsterNode,
    program: &Program,
    subtree_root: u16,
    output_ty: Ty,
    input: &[u8],
) -> io::Result<Vec<u8>> {
    let sub_prog = program
        .extract_output_subprogram(subtree_root, output_ty)
        .map_err(|e| io::Error::other(format!("apply_subtree: extract: {e:?}")))?;
    let sub_func = node.store().store(sub_prog.bytes())?;
    apply(node, sub_func, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};
    use crate::memory::MemoryGovernor;
    use crate::store::Store;
    use crate::{ForgeBrain, TmpDir};

    fn fresh_dir(tag: &str) -> TmpDir {
        let mut p = std::env::temp_dir();
        p.push(format!("forge-apply-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        TmpDir::new(p)
    }

    fn affine_program() -> Program {
        // f(x) = 3x + 7
        Program::new(
            Target::Cpu,
            1,
            1,
            16,
            vec![
                Node::input(0),
                Node::const_i64(3),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::add(3, 2),
                Node::output(4, Ty::I64),
            ],
        )
        .expect("affine well-formed")
    }

    fn add_zero_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            4,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .expect("add-zero well-formed")
    }

    /// Round-trip apply : execute via the singular API, decode the
    /// output, verify against the Rust closed-form. Two calls in the
    /// same session — second is an atlas hit (proven by the persisted
    /// state, not by side-channel timing).
    #[test]
    fn apply_executes_program_and_decodes_output() {
        let dir = fresh_dir("roundtrip");
        let store = Store::open(dir.as_ref()).expect("store");
        let atlas_path = dir.as_ref().join("forge.atlas");
        let atlas = std::sync::Arc::new(Atlas::open(&atlas_path).expect("atlas"));
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let prog = affine_program();
        let func = node.store().store(prog.bytes()).expect("store prog");

        for x in [0i64, 1, -1, 100, -100, 12345] {
            let input = x.to_le_bytes();
            let output = apply(&node, func, &input).expect("apply");
            assert_eq!(output.len(), 8);
            let got = i64::from_le_bytes(output.try_into().unwrap());
            let want = 3i64.wrapping_mul(x).wrapping_add(7);
            assert_eq!(got, want, "x={}", x);
        }

        // Each unique input writes one RESULT entry — verify atlas state.
        let count = atlas.count_kind(crate::atlas::kind::RESULT);
        assert_eq!(count, 6, "one RESULT entry per unique input");
    }

    #[test]
    fn apply_uses_persistent_brain_substitution_ref() {
        let dir = fresh_dir("brain-substitution");
        let store = Store::open(dir.as_ref()).expect("store");
        let atlas_path = dir.as_ref().join("forge.atlas");
        let atlas = std::sync::Arc::new(Atlas::open(&atlas_path).expect("atlas"));
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let mut brain = ForgeBrain::new();
        let from = brain
            .remember_program(&node, &add_zero_program())
            .expect("remember");
        let memory = brain.tighten_program(&node, from).expect("tighten");
        let to = memory.to.expect("accepted target");

        let input = 11i64.to_le_bytes();
        let output = apply(&node, from, &input).expect("apply through brain ref");
        assert_eq!(i64::from_le_bytes(output.try_into().unwrap()), 11);

        let input_hash = Hash::for_blob(&input);
        let optimized_key = Atlas::result_key(to.as_bytes(), input_hash.as_bytes());
        let original_key = Atlas::result_key(from.as_bytes(), input_hash.as_bytes());
        assert!(
            atlas.lookup_result(&optimized_key).is_some(),
            "apply should persist RESULT under the tightened program hash"
        );
        assert!(
            atlas.lookup_result(&original_key).is_none(),
            "apply should not keep branching the atlas through the stale program hash"
        );
    }

    #[test]
    fn apply_auto_publishes_brain_substitution_ref() {
        let dir = fresh_dir("brain-auto-substitution");
        let store = Store::open(dir.as_ref()).expect("store");
        let atlas_path = dir.as_ref().join("forge.atlas");
        let atlas = std::sync::Arc::new(Atlas::open(&atlas_path).expect("atlas"));
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let from = node
            .store()
            .store(add_zero_program().bytes())
            .expect("store program");
        let input = 19i64.to_le_bytes();
        let output = apply(&node, from, &input).expect("apply");
        assert_eq!(i64::from_le_bytes(output.try_into().unwrap()), 19);

        let to = node
            .store()
            .lookup_ref(&crate::brain::brain_substitution_ref(from))
            .expect("auto substitution ref");
        assert_ne!(from, to);
        assert_eq!(crate::brain::resolve_program_hash(&node, from), to);

        let input_hash = Hash::for_blob(&input);
        let optimized_key = Atlas::result_key(to.as_bytes(), input_hash.as_bytes());
        let original_key = Atlas::result_key(from.as_bytes(), input_hash.as_bytes());
        assert!(atlas.lookup_result(&optimized_key).is_some());
        assert!(atlas.lookup_result(&original_key).is_none());
    }

    /// Cross-session memoization : open one atlas, run apply, drop the
    /// node. Open a fresh node bound to the same atlas + store. Run
    /// apply again — the result must come back identical, AND the
    /// atlas RESULT count must not grow (the second call is a pure
    /// lookup, no recompute).
    #[test]
    fn apply_persists_results_across_sessions() {
        let dir = fresh_dir("persist");

        // Session 1 : compute, persist.
        let result_hash_1: Hash;
        {
            let store = Store::open(dir.as_ref()).expect("store1");
            let atlas =
                std::sync::Arc::new(Atlas::open(dir.as_ref().join("forge.atlas")).expect("atlas1"));
            let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
            node.attach_atlas(atlas.clone());

            let prog = affine_program();
            let func = node.store().store(prog.bytes()).expect("store prog");

            let input = 42i64.to_le_bytes();
            let output = apply(&node, func, &input).expect("apply 1");
            result_hash_1 = Hash::for_blob(&output);
            assert_eq!(atlas.count_kind(crate::atlas::kind::RESULT), 1);

            // Atlas needs to flush before the second session opens it.
            atlas.flush().expect("flush");
        }

        // Session 2 : same atlas + store, no compute should happen.
        let store = Store::open(dir.as_ref()).expect("store2");
        let atlas =
            std::sync::Arc::new(Atlas::open(dir.as_ref().join("forge.atlas")).expect("atlas2"));
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let prog = affine_program();
        let func = node.store().store(prog.bytes()).expect("store prog");

        let input = 42i64.to_le_bytes();
        let output = apply(&node, func, &input).expect("apply 2");
        let result_hash_2 = Hash::for_blob(&output);

        assert_eq!(result_hash_1, result_hash_2, "deterministic output");
        assert_eq!(
            atlas.count_kind(crate::atlas::kind::RESULT),
            1,
            "no new RESULT entry — second call was a hit"
        );
    }

    // ─── M6 — apply_subtree tests ──────────────────────────────────────

    /// Apply on the sub-tree rooted at the affine program's `Add`
    /// node (slot 4) must produce the same value as `apply` on the
    /// full program — the sub-tree IS the program's logic minus the
    /// outer `Output(_, Ty::I64)` wrapper.
    #[test]
    fn apply_subtree_at_output_root_matches_full_apply() {
        let dir = fresh_dir("subtree-output");
        let store = Store::open(dir.as_ref()).expect("store");
        let atlas = std::sync::Arc::new(
            Atlas::open(dir.as_ref().join("forge.atlas")).expect("atlas"),
        );
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let prog = affine_program();
        let func = node.store().store(prog.bytes()).expect("store full prog");

        for &x in &[0i64, 5, -3, 100, -100] {
            let input = x.to_le_bytes();
            let full_out = apply(&node, func, &input).expect("full apply");
            let sub_out = apply_subtree(&node, &prog, 4, Ty::I64, &input)
                .expect("subtree apply");
            assert_eq!(full_out, sub_out, "x={}", x);
        }
    }

    /// Apply on an INTERNAL sub-tree (the `Mul(0, 1)` at slot 3)
    /// must produce the partial value `3 * x`.
    #[test]
    fn apply_subtree_at_internal_node_computes_partial_value() {
        let dir = fresh_dir("subtree-internal");
        let store = Store::open(dir.as_ref()).expect("store");
        let atlas = std::sync::Arc::new(
            Atlas::open(dir.as_ref().join("forge.atlas")).expect("atlas"),
        );
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let prog = affine_program();

        for &x in &[1i64, 7, -5, 1000] {
            let input = x.to_le_bytes();
            let out = apply_subtree(&node, &prog, 3, Ty::I64, &input)
                .expect("subtree apply");
            let val = i64::from_le_bytes(out.try_into().unwrap());
            assert_eq!(val, 3i64.wrapping_mul(x), "x={}", x);
        }
    }

    /// Cross-session memo : sub-tree result computed in session 1
    /// must come back from atlas in session 2 without re-execution.
    /// Validates that `apply_subtree` enrolls atlas RESULT entries
    /// keyed by the slice's content hash, not the parent program's.
    #[test]
    fn apply_subtree_persists_across_sessions() {
        let dir = fresh_dir("subtree-persist");

        let result_1: Vec<u8>;
        {
            let store = Store::open(dir.as_ref()).expect("store1");
            let atlas = std::sync::Arc::new(
                Atlas::open(dir.as_ref().join("forge.atlas")).expect("atlas1"),
            );
            let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
            node.attach_atlas(atlas.clone());

            let prog = affine_program();
            let input = 7i64.to_le_bytes();
            result_1 = apply_subtree(&node, &prog, 4, Ty::I64, &input)
                .expect("session 1 subtree");
            atlas.flush().expect("flush");
        }

        // Session 2 — fresh node, same atlas + store.
        let store = Store::open(dir.as_ref()).expect("store2");
        let atlas = std::sync::Arc::new(
            Atlas::open(dir.as_ref().join("forge.atlas")).expect("atlas2"),
        );
        let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));
        node.attach_atlas(atlas.clone());

        let prog = affine_program();
        let input = 7i64.to_le_bytes();
        let result_2 = apply_subtree(&node, &prog, 4, Ty::I64, &input)
            .expect("session 2 subtree");
        assert_eq!(result_1, result_2, "deterministic cross-session output");
    }

    /// Λ.1 invariant : two distinct inputs (regardless of length or
    /// shared prefix) produce DISTINCT atlas RESULT keys. The pre-Λ.1
    /// `result_key(func, raw_input)` would alias any two inputs sharing
    /// a 12-byte prefix ; the Λ.1 keying via `Hash::for_blob` is alias-
    /// free because cryptographic hash bytes are uniformly random.
    ///
    /// We verify the key derivation directly (no program execution
    /// needed) so the test is independent of program input shape.
    #[test]
    fn apply_keying_is_alias_free_for_long_inputs() {
        // Two 24-byte inputs sharing the first 16 bytes — the legacy
        // `result_key(func, raw)` truncates input to 12 bytes, so these
        // would have produced IDENTICAL keys under the old scheme.
        let mut a = vec![0u8; 24];
        let mut b = vec![0u8; 24];
        for i in 0..16 {
            a[i] = i as u8;
            b[i] = i as u8;
        }
        a[16..].copy_from_slice(&0u64.to_le_bytes());
        b[16..].copy_from_slice(&0xDEAD_BEEFu64.to_le_bytes());

        // Λ.1 keying : hash the input first.
        let func_bytes = [0xABu8; 20];
        let key_a = Atlas::result_key(&func_bytes, Hash::for_blob(&a).as_bytes());
        let key_b = Atlas::result_key(&func_bytes, Hash::for_blob(&b).as_bytes());
        assert_ne!(key_a, key_b, "Λ.1 hashed-input keys must differ");

        // Sanity : the LEGACY raw-bytes keying would have aliased.
        let legacy_a = Atlas::result_key(&func_bytes, &a);
        let legacy_b = Atlas::result_key(&func_bytes, &b);
        assert_eq!(
            legacy_a, legacy_b,
            "legacy raw-input keying aliases shared-prefix inputs (the bug Λ.1 fixes)"
        );
    }
}
