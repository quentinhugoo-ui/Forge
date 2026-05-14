//! Φ.1 — `BulkEvaluator` trait + batch dispatch API.
//!
//! Foundation phase for the Forge **CPU brain + GPU muscle** architecture.
//! The brain (`MonsterNode`) walks each call through its cheap layers
//! (RAM cache, structural rule, learned oracle, disk memo) — only the
//! genuinely-novel ones reach the bulk evaluator.
//!
//! ```text
//! dispatch_batch(node, calls, evaluator):
//!     for each call in calls:
//!         brain hit (try_lookup_call)? → DispatchResult::Hit
//!         else                          → BulkEvaluator::eval_batch
//!             ingest into RAM cache (single batched write lock)
//!                                       → DispatchResult::Computed
//! ```
//!
//! The trait's blanket `impl BulkEvaluator for MonsterNode` is the
//! reference backend (uses the local interpreter via the standard
//! `dispatch_impl` slow lane). Future phases plug in:
//!
//! - **Φ.2**  `CpuSimdEvaluator`  (AVX-512 / NEON / scalaire)
//! - **Φ.6**  `WgpuEvaluator`     (Vulkan compute, cross-vendor)
//! - **Φ.7**  `CudaEvaluator` / `MetalEvaluator`
//! - **Φ.8**  `RemoteEvaluator`   (federated atlas pull/exec)
//!
//! ## Φ.1.1 — Via Negativa amputation (post-bench, 8 middlemen cut)
//!
//! 1. **Cut #1** — `miss_calls.push(call.clone())` éliminé : on passe
//!    des slices `BatchInput<'a>` à l'evaluator, zéro Vec<u8> cloné.
//! 2. **Cut #2** — `Vec<Option<DispatchResult>>` puis `.collect()`
//!    remplacé par un seul `Vec<DispatchResult>` rempli par index.
//! 3. **Cut #3** — `Vec<Vec<u8>>` côté output remplacé par `PackedOutput`
//!    (un seul Vec packé + offsets). Zéro alloc par résultat côté GPU.
//! 4. **Cut #4** — `Arc::from(bytes.into_boxed_slice())` remplacé par
//!    `Arc::from(slice)` direct (un memcpy au lieu de deux conversions).
//! 5. **Cut #5** — `remember_call` par miss → `remember_calls` batched
//!    (1 write lock pour N entries, vs N).
//! 6. **Cut #6** — input scattered `Vec<u8>` par call → `BatchInput<'a>`
//!    avec slice borrow. Trait redesign : zéro alloc d'input côté
//!    evaluator (les futurs backends GPU peuvent mémoriser le slice
//!    dans un buffer plat).
//! 7. **Cut #7** — `HotProgram` cache **par batch** (HashMap) :
//!    `hot_program(func)` n'est appelé qu'une fois par func distinct.
//! 8. **Cut #8** — `RamKey` calculé une seule fois par call (pendant le
//!    triage), puis carrié vers l'ingestion miss au lieu d'être
//!    recalculé.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use crate::Hash;

use super::cache::RamKey;
use super::hotplan::{is_cpu_auto_routable, HotPlan, HotProgram};
use super::stats::{MonsterCall, MonsterSource, PhysicalEnvelope};
use super::MonsterNode;
use crate::kasm;

/// Caller-owned dispatch unit. Cloneable so a `dispatch_batch` consumer
/// can keep an owned copy alongside dispatch results.
#[derive(Clone, Debug)]
pub struct BatchCall {
    pub func: Hash,
    pub args: Vec<u8>,
}

impl BatchCall {
    pub fn new(func: Hash, args: Vec<u8>) -> Self {
        Self { func, args }
    }
}

/// Borrowed dispatch unit handed to the `BulkEvaluator`. Avoids cloning
/// the args vec on miss (cut #1+#6). Backends that want to bulk-pack
/// inputs into a contiguous buffer can iterate `args` once and append.
#[derive(Clone, Copy, Debug)]
pub struct BatchInput<'a> {
    pub func: Hash,
    pub args: &'a [u8],
}

/// Packed result buffer returned by `BulkEvaluator::eval_batch`. The
/// `bytes` field is a single contiguous allocation; result `i` lives
/// at `bytes[offsets[i]..offsets[i+1]]`. `offsets.len() == calls.len() + 1`
/// always (last entry = total length).
///
/// Cut #3 — replaces the previous `Vec<io::Result<Vec<u8>>>` with one
/// allocation. GPU backends produce results in a flat device buffer
/// natively; this layout matches that, so a future Wgpu/CUDA evaluator
/// can `cudaMemcpyDeviceToHost` directly into `bytes`.
#[derive(Debug, Clone, Default)]
pub struct PackedOutput {
    pub bytes: Vec<u8>,
    pub offsets: Vec<u32>,
}

impl PackedOutput {
    pub fn with_capacity(calls: usize, total_bytes_hint: usize) -> Self {
        let mut offsets = Vec::with_capacity(calls + 1);
        offsets.push(0);
        Self {
            bytes: Vec::with_capacity(total_bytes_hint),
            offsets,
        }
    }

    /// Append one result to the packed buffer, updating offsets.
    pub fn push_result(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
        self.offsets.push(self.bytes.len() as u32);
    }

    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn slice(&self, idx: usize) -> &[u8] {
        let start = self.offsets[idx] as usize;
        let end = self.offsets[idx + 1] as usize;
        &self.bytes[start..end]
    }
}

/// Outcome of a single batched call after `dispatch_batch` has run.
///
/// The `MonsterCall` payload is identical between `Hit` and `Computed`:
/// the variant only records *whether the bulk evaluator was invoked*.
/// Callers that only want the result can ignore the variant; callers
/// that want to track brain-vs-backend pressure (Φ.12 metrics) read it.
#[derive(Debug, Clone)]
pub enum DispatchResult {
    /// Brain produced the result without touching the bulk evaluator.
    /// Source ∈ {RamMemo, StructuralRule, LearnedOracle, GitMemo}.
    Hit(MonsterCall),
    /// Brain missed; the supplied `BulkEvaluator` produced bytes, which
    /// were ingested into the brain's RAM cache. Source is always
    /// `ExecutedHot` for the reference impl; concrete evaluators may
    /// re-tag through their own dedicated `MonsterSource` once added.
    Computed(MonsterCall),
}

impl DispatchResult {
    /// Underlying `MonsterCall` regardless of brain/backend origin.
    pub fn call(&self) -> &MonsterCall {
        match self {
            Self::Hit(c) | Self::Computed(c) => c,
        }
    }

    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit(_))
    }

    pub fn is_computed(&self) -> bool {
        matches!(self, Self::Computed(_))
    }
}

/// Forge bulk evaluator trait — the seam between the CPU brain and any
/// backend that can execute a batch of calls and return a packed buffer
/// of result bytes.
///
/// Contract:
/// - Input `calls: &[BatchInput<'_>]` borrows args from the caller — no
///   per-call allocation.
/// - Output `PackedOutput` carries all results in one contiguous Vec
///   plus an offset table. `output.offsets.len() == calls.len() + 1`.
/// - One whole-batch `io::Error` on failure (no per-call result).
///   Implementations that need finer error granularity can encode
///   per-call failure in the bytes (e.g. magic prefix); for now the
///   simpler all-or-nothing semantics matches GPU launch behaviour.
///
/// `Send + Sync` so evaluators can be shared across worker threads
/// without further synchronization on the implementor's part.
pub trait BulkEvaluator: Send + Sync {
    fn eval_batch(&self, calls: &[BatchInput<'_>]) -> io::Result<PackedOutput>;

    /// Optional pre-warm hook : the brain calls this once per batch
    /// with the **deduplicated** list of distinct func hashes that the
    /// upcoming `eval_batch` will reference. Lets remote / GPU backends
    /// upload the program code once instead of N times. Default impl
    /// is a no-op (the local interpreter shares the program cache via
    /// `MonsterNode`).
    fn prepare(&self, _funcs: &[Hash]) -> io::Result<()> {
        Ok(())
    }
}

/// Reference backend: the local `MonsterNode` interpreter slow lane.
/// Each call goes through the standard hot-plan execution and the
/// resulting bytes are appended to a shared `PackedOutput`. For batches
/// where the brain has already filtered out the cached calls (typical
/// `dispatch_batch` usage), this just degrades to the interpreter —
/// which is exactly the semantics Phase 2 will replace with
/// SIMD-accelerated bulk eval.
impl BulkEvaluator for MonsterNode {
    fn eval_batch(&self, calls: &[BatchInput<'_>]) -> io::Result<PackedOutput> {
        self.gpunode_runtime.eval_batch(self, calls)
    }
}

impl MonsterNode {
    /// Φ.1 — Filter a batch through the brain and dispatch the misses
    /// to the supplied evaluator. One `DispatchResult` per input call,
    /// in the same order.
    ///
    /// Brain layers walked (cheap): RAM cache → wire-key → structural
    /// rule → learned oracle → disk memo. Only the genuinely-novel calls
    /// reach `evaluator.eval_batch`. Their bytes are then ingested into
    /// the RAM cache (under a single write lock — cut #5) so subsequent
    /// identical calls hit the brain.
    ///
    /// This method does **not** persist to git or push to the atlas —
    /// that decision belongs to the caller (lab_runner ingests through
    /// `LiveAtlas::submit_batch`; an interactive REPL might not). Keeps
    /// the dispatch loop allocation-light.
    pub fn dispatch_batch(
        &self,
        calls: &[BatchCall],
        evaluator: &dyn BulkEvaluator,
    ) -> io::Result<Vec<DispatchResult>> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }

        // ────────────────────────────────────────────────────────────
        // CPU-fast batch bypass — Cut #3 first-principles audit.
        //
        // Si TOUTES les calls ciblent un programme `is_cpu_auto_routable`
        // (HotPlan rule OR Interpret ≤ 64 nodes), la cascade dispatch_impl
        // (~3 µs/call cumulés : RamKey + L0-L5 lookups + locks + observe)
        // est pure overhead — l'auto-router v0/v1/v2 produit le résultat
        // en 75-300 ns avec zero lock.
        //
        // On skip TOUT et on inline directement. Mesure attendue (DNA
        // bench `complement` à 5071 ns/call avec dispatch_batch) : 5071 →
        // ~200-500 ns/call. 10-25× speedup sur le path dispatch_batch
        // pour les programmes Léger.
        if let Some(results) = try_dispatch_batch_cpu_fast(self, calls)? {
            return Ok(results);
        }

        let n = calls.len();

        // Cut #2 — single output Vec, sentinel-filled, written by index.
        // The sentinel is overwritten exactly once per slot; final Vec
        // is returned without re-collection.
        let sentinel_call = MonsterCall {
            result: Hash::from_bytes([0u8; 20]),
            source: MonsterSource::RamMemo,
            envelope: Default::default(),
        };
        let mut out: Vec<DispatchResult> = vec![DispatchResult::Hit(sentinel_call); n];

        // Cut #7 — share `Arc<HotProgram>` across the batch by func hash.
        // The 1-slot `last` cache short-circuits the HashMap when a
        // run of consecutive calls share the same func (the common
        // case — saves 1 SipHash per call).
        let mut hot_cache: HashMap<Hash, Arc<HotProgram>> = HashMap::new();
        let mut last: Option<(Hash, Arc<HotProgram>)> = None;

        // Miss tracking — references only (cut #1+#6), pre-computed
        // RamKeys carried (cut #8).
        let mut miss_idxs: Vec<usize> = Vec::new();
        let mut miss_inputs: Vec<BatchInput<'_>> = Vec::new();
        let mut miss_keys: Vec<RamKey> = Vec::new();
        // Within-batch miss dedup: if two calls share the same (func, args),
        // compute once and broadcast the result. atlas_key → position in miss_idxs.
        let mut miss_by_atlas_key: HashMap<[u8; 32], usize> = HashMap::new();
        let mut dup_targets: Vec<(usize, usize)> = Vec::new();

        let atlas_arc = self.atlas();

        for (idx, call) in calls.iter().enumerate() {
            let func = crate::brain::resolve_program_hash(self, call.func);
            // Compute atlas key once — used for cross-session lookup AND
            // intra-batch dedup. Lifting before `if let Some(atlas)` keeps
            // dedup active even when atlas is absent.
            let input_hash = Hash::for_blob(&call.args);
            let atlas_key = crate::atlas::Atlas::result_key(
                func.as_bytes(),
                input_hash.as_bytes(),
            );

            if let Some(atlas) = atlas_arc.as_ref() {
                if let Some(result_bytes) = atlas.lookup_result(&atlas_key) {
                    out[idx] = DispatchResult::Hit(MonsterCall {
                        result: Hash::from_bytes(result_bytes),
                        source: MonsterSource::GitMemo,
                        envelope: Default::default(),
                    });
                    continue;
                }
            }

            let hot = match &last {
                Some((h, hot)) if *h == func => Arc::clone(hot),
                _ => {
                    let h = if let Some(h) = hot_cache.get(&func) {
                        Arc::clone(h)
                    } else {
                        let h = self.hot_program(&func)?;
                        hot_cache.insert(func, Arc::clone(&h));
                        h
                    };
                    last = Some((func, Arc::clone(&h)));
                    h
                }
            };
            let key = RamKey::for_args(hot.semantic_fingerprint, &call.args);
            match self.try_lookup_call(&hot, &call.args, key.clone())? {
                Some(monster_call) => {
                    out[idx] = DispatchResult::Hit(monster_call);
                }
                None => {
                    // Dedup: if an identical call is already queued as a miss,
                    // defer this slot — it broadcasts from the canonical result.
                    if let Some(&src_pos) = miss_by_atlas_key.get(&atlas_key) {
                        dup_targets.push((idx, src_pos));
                    } else {
                        miss_by_atlas_key.insert(atlas_key, miss_idxs.len());
                        miss_idxs.push(idx);
                        miss_inputs.push(BatchInput {
                            func,
                            args: &call.args,
                        });
                        miss_keys.push(key);
                    }
                }
            }
        }

        if !miss_inputs.is_empty() {
            // Cut #7 (continuation) — tell the evaluator which programs
            // it will need, deduped. Default impl no-ops; remote/GPU
            // backends override.
            let distinct_funcs: Vec<Hash> = hot_cache.keys().copied().collect();
            evaluator.prepare(&distinct_funcs)?;

            let packed = evaluator.eval_batch(&miss_inputs)?;
            if packed.offsets.len() != miss_inputs.len() + 1 {
                return Err(io::Error::other(format!(
                    "BulkEvaluator returned {} offsets for {} calls (expected {})",
                    packed.offsets.len(),
                    miss_inputs.len(),
                    miss_inputs.len() + 1,
                )));
            }

            // Cut #5 — collect all ingestion entries, then one batched
            // `remember_calls` (single write lock + single LRU lock).
            let mut to_remember: Vec<(RamKey, Hash, Arc<[u8]>)> =
                Vec::with_capacity(miss_inputs.len());
            for (i, &idx) in miss_idxs.iter().enumerate() {
                let bytes_slice = packed.slice(i);
                // Cut #4 — `Arc::from(slice)` is one memcpy + one alloc;
                // skips the previous `Vec → Box → Arc` chain.
                let arc_bytes: Arc<[u8]> = Arc::from(bytes_slice);
                let result = Hash::for_blob(&arc_bytes);
                let key = miss_keys[i].clone();

                // Atlas RESULT cross-session write — Λ.0b hashed keying.
                if let Some(atlas) = atlas_arc.as_ref() {
                    let input_hash = Hash::for_blob(miss_inputs[i].args);
                    let atlas_key = crate::atlas::Atlas::result_key(
                        miss_inputs[i].func.as_bytes(),
                        input_hash.as_bytes(),
                    );
                    let _ = atlas.record_result(&atlas_key, result.as_bytes());
                }

                to_remember.push((key, result, Arc::clone(&arc_bytes)));
                out[idx] = DispatchResult::Computed(MonsterCall {
                    result,
                    source: MonsterSource::ExecutedHot,
                    envelope: Default::default(),
                });
            }
            self.remember_calls(to_remember);

            // Broadcast: fill duplicate slots from their canonical computed result.
            // The canonical slot is already set to Computed; tag the dup as Hit
            // (bulk evaluator was not invoked for it).
            for (dup_out_idx, src_pos) in dup_targets {
                if let DispatchResult::Computed(ref mc) = out[miss_idxs[src_pos]] {
                    out[dup_out_idx] = DispatchResult::Hit(MonsterCall {
                        result: mc.result,
                        source: MonsterSource::RamMemo,
                        envelope: Default::default(),
                    });
                }
            }
        }

        Ok(out)
    }
}

/// dispatch_batch entry bypass — skip cascade entirely si TOUS les
/// programmes du batch sont CPU-auto-routables.
///
/// Returns:
///   - Ok(Some(results)) : bypass appliqué, results prêts
///   - Ok(None)          : un programme ne fitte pas, fallback cascade
///   - Err(_)            : erreur fatale (hot_program ou execute)
fn try_dispatch_batch_cpu_fast(
    node: &MonsterNode,
    calls: &[BatchCall],
) -> io::Result<Option<Vec<DispatchResult>>> {
    // Pré-check : tous les programmes du batch doivent être CPU-routable.
    // Single-program batch : cas dominant (lab_runner, gpu_dispatch bench).
    use std::collections::HashMap;
    let mut hot_cache: HashMap<Hash, Arc<HotProgram>> = HashMap::new();
    for call in calls {
        let func = crate::brain::resolve_program_hash(node, call.func);
        if hot_cache.contains_key(&func) {
            continue;
        }
        let hot = node.hot_program(&func)?;
        if !is_cpu_auto_routable(&hot) {
            return Ok(None);
        }
        hot_cache.insert(func, hot);
    }

    // Tous routables. Inline auto-router pour chaque call.
    let mut results: Vec<DispatchResult> = Vec::with_capacity(calls.len());
    let mut last: Option<(Hash, Arc<HotProgram>)> = None;
    for call in calls {
        let func = crate::brain::resolve_program_hash(node, call.func);
        // Récupère hot via le cache local (single HashMap probe).
        let hot = match &last {
            Some((h, hot)) if *h == func => Arc::clone(hot),
            _ => {
                let h = Arc::clone(hot_cache.get(&func).unwrap());
                last = Some((func, Arc::clone(&h)));
                h
            }
        };

        // Args sanity : 8-byte i64 input attendu pour les fast paths.
        if call.args.len() != 8 {
            // Programme déclaré routable mais args inattendus → fallback.
            return Ok(None);
        }
        let arg = i64::from_le_bytes(call.args[..8].try_into().unwrap());

        // Inline auto-router v0/v1/v2.
        let result_i64: i64 = match &hot.plan {
            HotPlan::AffineI64 { input_slot: 0, mul, add } => {
                arg.wrapping_mul(*mul).wrapping_add(*add)
            }
            HotPlan::HashChain { input_slot: 0, rounds } => {
                let mut value = arg;
                for _ in 0..*rounds {
                    value = kasm::hash_i64(value);
                }
                value
            }
            HotPlan::StaticOutput(bytes) => {
                if bytes.len() < 8 {
                    return Ok(None);
                }
                i64::from_le_bytes(bytes[..8].try_into().unwrap())
            }
            HotPlan::Interpret => {
                match kasm::try_execute_i64_inline(&hot.program, arg) {
                    Some(v) => v,
                    None => return Ok(None),  // Fallback safe
                }
            }
            // Autres variants (input_slot != 0, etc) : fallback cascade.
            _ => return Ok(None),
        };

        let bytes_arc: Arc<[u8]> = Arc::from(&result_i64.to_le_bytes()[..]);
        let result_hash = Hash::for_blob(&bytes_arc);

        // Atlas RESULT cross-session write — even fast-bypass results go
        // into the atlas so future sessions retrieve without recomputing.
        if let Some(atlas) = node.atlas() {
            let atlas_key = crate::atlas::Atlas::result_key(func.as_bytes(), &call.args);
            let _ = atlas.record_result(&atlas_key, result_hash.as_bytes());
        }

        // Hit (pas Computed) : le brain a entièrement traité ce call, sans
        // toucher l'evaluator. C'est la sémantique attendue par les tests
        // qui distinguent brain-hit vs evaluator-computed.
        results.push(DispatchResult::Hit(MonsterCall {
            result: result_hash,
            source: MonsterSource::StructuralRule,
            envelope: PhysicalEnvelope::default(),
        }));
    }

    // Atomic bump du compteur rule_hits — visibility pour les benches.
    node.stats_atomic.rule_hits.fetch_add(
        calls.len() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );
    Ok(Some(results))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    

    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};
    use crate::{MemoryGovernor, Store};

    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("scan-dispatch", tag)
    }

    fn affine_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            6,
            vec![
                Node::input(0),
                Node::const_i64(9),
                Node::mul(0, 1),
                Node::const_i64(1),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn opaque_program() -> Program {
        let rounds = 120;
        let mut nodes = vec![Node::input(0)];
        let mut current = 0u16;
        for _ in 0..rounds {
            let eq = nodes.len() as u16;
            nodes.push(Node::eq(current, current));
            let zero = nodes.len() as u16;
            nodes.push(Node::const_i64(0));
            let selected = nodes.len() as u16;
            nodes.push(Node::select_i64(eq, current, zero));
            current = selected;
        }
        let two = nodes.len() as u16;
        nodes.push(Node::const_i64(2));
        let doubled = nodes.len() as u16;
        nodes.push(Node::mul(current, two));
        let five = nodes.len() as u16;
        nodes.push(Node::const_i64(5));
        let shifted = nodes.len() as u16;
        nodes.push(Node::add(doubled, five));
        nodes.push(Node::output(shifted, Ty::I64));
        Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
    }

    fn make_node(tag: &str) -> (PathBuf, MonsterNode) {
        let path = fresh_path(tag);
        let node = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (path, node)
    }

    #[test]
    fn dispatch_batch_empty_returns_empty() {
        let (path, node) = make_node("empty");
        let results = node.dispatch_batch(&[], &node).unwrap();
        assert!(results.is_empty());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn dispatch_batch_affine_short_circuits_in_brain() {
        let (path, node) = make_node("affine");
        let func = node.store().store(affine_program().bytes()).unwrap();
        let arg = 3i64.to_le_bytes().to_vec();
        let calls = vec![BatchCall::new(func, arg)];

        let r = node.dispatch_batch(&calls, &node).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].is_hit(), "affine rule should produce a brain Hit");
        assert!(matches!(
            r[0].call().source,
            MonsterSource::StructuralRule | MonsterSource::RamMemo
        ));

        let r2 = node.dispatch_batch(&calls, &node).unwrap();
        assert!(r2[0].is_hit());
        // Post auto-router CPU-fast bypass : pour les programmes Léger
        // (AffineI64 ici), le 2e call passe par le bypass inline qui
        // re-exécute la rule (5 ns) au lieu de toucher le cache (~150 ns).
        // Donc StructuralRule est valide post-bypass ; RamMemo l'est aussi
        // si le bypass ne fire pas (ex : dispatch_impl path direct).
        assert!(matches!(
            r2[0].call().source,
            MonsterSource::StructuralRule | MonsterSource::RamMemo
        ));

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn dispatch_batch_opaque_first_call_routes_to_evaluator() {
        let (path, node) = make_node("opaque");
        let func = node.store().store(opaque_program().bytes()).unwrap();
        let arg = 7i64.to_le_bytes().to_vec();
        let calls = vec![BatchCall::new(func, arg)];

        let r = node.dispatch_batch(&calls, &node).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].is_computed(), "opaque first call should be Computed");

        let r2 = node.dispatch_batch(&calls, &node).unwrap();
        assert!(r2[0].is_hit(), "second identical call must be brain Hit");
        assert!(matches!(r2[0].call().source, MonsterSource::RamMemo));

        let _ = std::fs::remove_dir_all(&path);
    }

    /// Recording evaluator: every call it sees gets logged. Lets us
    /// assert that brain hits never reach the evaluator.
    struct RecordingEvaluator {
        inner: Arc<MonsterNode>,
        seen: std::sync::Mutex<Vec<(Hash, Vec<u8>)>>,
    }

    impl BulkEvaluator for RecordingEvaluator {
        fn eval_batch(&self, calls: &[BatchInput<'_>]) -> io::Result<PackedOutput> {
            let mut seen = self.seen.lock().unwrap();
            for c in calls {
                seen.push((c.func, c.args.to_vec()));
            }
            drop(seen);
            self.inner.eval_batch(calls)
        }
    }

    #[test]
    fn dispatch_batch_persists_results_across_sessions_via_atlas() {
        // Simulates two sessions sharing the same `forge.atlas` file.
        // Session 1 computes the result on a cold node. Session 2 opens
        // a fresh node bound to the same atlas file — the dispatch must
        // return the cached result without invoking the bulk evaluator.
        use crate::atlas::Atlas;
        use std::sync::Arc as StdArc;

        let store_path = fresh_path("atlas-session-store");
        let atlas_path = fresh_path("atlas-session-atlas");

        let opaque = opaque_program();
        let arg_bytes = 7i64.to_le_bytes();
        let calls: Vec<BatchCall> = vec![BatchCall::new(
            // We need a stable func across sessions: store, get hash, reuse.
            {
                let store = Store::open(&store_path).unwrap();
                store.store(opaque.bytes()).unwrap()
            },
            arg_bytes.to_vec(),
        )];
        let func = calls[0].func;

        // Session 1 — compute and persist.
        let session1_evaluator_count;
        {
            let node = MonsterNode::new(
                Store::open(&store_path).unwrap(),
                MemoryGovernor::new(1024 * 1024),
            );
            let atlas = StdArc::new(Atlas::open(&atlas_path).unwrap());
            node.attach_atlas(StdArc::clone(&atlas));

            let recorder = RecordingEvaluator {
                inner: StdArc::new(MonsterNode::new(
                    Store::open(&store_path).unwrap(),
                    MemoryGovernor::new(1024 * 1024),
                )),
                seen: std::sync::Mutex::new(Vec::new()),
            };
            let r = node.dispatch_batch(&calls, &recorder).unwrap();
            assert!(r[0].is_computed(), "session 1 should compute (cold atlas)");
            session1_evaluator_count = recorder.seen.lock().unwrap().len();
            assert!(
                session1_evaluator_count > 0,
                "evaluator should have been invoked in session 1"
            );

            // Atlas should now contain a RESULT entry for this call,
            // keyed by the Λ.0b hashed-input scheme.
            let input_hash = Hash::for_blob(&arg_bytes);
            let atlas_key = Atlas::result_key(func.as_bytes(), input_hash.as_bytes());
            assert!(
                atlas.lookup_result(&atlas_key).is_some(),
                "atlas must hold the freshly-computed result"
            );
        }

        // Session 2 — fresh node, same atlas file. Result must come back
        // without invoking the evaluator.
        {
            let node = MonsterNode::new(
                Store::open(&store_path).unwrap(),
                MemoryGovernor::new(1024 * 1024),
            );
            let atlas = StdArc::new(Atlas::open(&atlas_path).unwrap());
            node.attach_atlas(StdArc::clone(&atlas));

            let recorder = RecordingEvaluator {
                inner: StdArc::new(MonsterNode::new(
                    Store::open(&store_path).unwrap(),
                    MemoryGovernor::new(1024 * 1024),
                )),
                seen: std::sync::Mutex::new(Vec::new()),
            };
            let r = node.dispatch_batch(&calls, &recorder).unwrap();
            assert!(
                r[0].is_hit(),
                "session 2 must return Hit from atlas without recomputing"
            );
            assert_eq!(
                recorder.seen.lock().unwrap().len(),
                0,
                "evaluator MUST NOT be invoked when atlas already holds the result"
            );
        }

        let _ = std::fs::remove_dir_all(&store_path);
        let _ = std::fs::remove_file(&atlas_path);
    }

    #[test]
    fn dispatch_batch_brain_hits_never_touch_evaluator() {
        let path = fresh_path("noeval");
        let node = Arc::new(MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        ));
        let func = node.store().store(affine_program().bytes()).unwrap();

        let recorder = RecordingEvaluator {
            inner: Arc::clone(&node),
            seen: std::sync::Mutex::new(Vec::new()),
        };

        let calls: Vec<BatchCall> = (0..16)
            .map(|i| BatchCall::new(func, (i as i64).to_le_bytes().to_vec()))
            .collect();
        let r = node.dispatch_batch(&calls, &recorder).unwrap();
        assert_eq!(r.len(), 16);
        assert!(r.iter().all(|x| x.is_hit()));
        assert_eq!(
            recorder.seen.lock().unwrap().len(),
            0,
            "evaluator must not see any calls when the brain hits"
        );

        let _ = std::fs::remove_dir_all(&path);
    }
}
