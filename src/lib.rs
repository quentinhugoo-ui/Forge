//! SCAN/KASM: one hot node, one tiny bytecode, one content store.

mod codec;
mod key;
pub mod kasm;
mod memory;
pub mod agent;
pub mod apply;
pub mod atlas;
pub mod brain;
pub mod compute_core;
pub mod cpu_bits;
pub mod cpu_simd;
pub mod fast_hash;
pub mod godel;
pub mod introspection;
pub mod landauer;
pub mod meta;
mod monster;
mod store;

// `atlas::Atlas` is reached via the explicit `scan::atlas::Atlas` path.
// We do NOT re-export it at the crate root to avoid collision with the
// pre-existing `monster::atlas::Atlas` (retrieval atlas) which is already
// re-exported via the `monster` glob below.

/// Φ.maintenance.tmp-cleanup (2026-05-02) — Drop-guard wrapper around
/// a `.codex-tmp/...` scratch directory used by tests + benchmarks.
///
/// **Why** : the historical pattern `let _ = std::fs::remove_dir_all(&path);`
/// at the END of a test leaks the dir on **any** earlier panic
/// (assertion failure, unwrap, etc.). Cumulated over months of dev, this
/// produced 5 155 orphan dirs (~496 MB) in `.codex-tmp/` (cleanup
/// 2026-05-02).
///
/// **How** : `TmpDir` runs `remove_dir_all` in its `Drop` impl, which
/// kicks **even on panic unwind** — so a failed assertion still cleans
/// up its scratch.
///
/// **Doctrine** : zero new deps (replaces what `tempfile` crate would
/// do). ~30 lines, no module proliferation.
///
/// Usage in tests :
/// ```ignore
/// let path = TmpDir::new(fresh_path("xxx"));
/// let store = Store::open(path.as_ref()).unwrap();
/// // ... assertions ... — if any panic, Drop kicks and cleans up.
/// ```
pub struct TmpDir(std::path::PathBuf);

impl TmpDir {
    /// Wrap a path in the Drop-guard. The path is **not** created here ;
    /// the caller (typically `Store::open`) creates it and this wrapper
    /// only ensures cleanup.
    pub fn new(path: std::path::PathBuf) -> Self {
        Self(path)
    }

    /// Borrow the wrapped path. Cheap (zero alloc).
    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<std::path::Path> for TmpDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        // Best-effort cleanup. If the dir doesn't exist (e.g. test
        // never created it), the error is silently swallowed — that's
        // by design for a Drop-guard.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Φ.maintenance.fresh-path (2026-05-02) — factorise les 21 sites
/// dupliqués `fn fresh_path(tag) -> PathBuf` à travers `src/` (test
/// helpers godel/, monster/, store, etc.).
///
/// Format : `.codex-tmp/{prefix}-{tag}-{nanos}/`
///
/// Combiné avec `TmpDir`, le pattern test idiomatique devient :
/// ```ignore
/// let path = TmpDir::new(fresh_tmp_path("scan-monster", "test1"));
/// let store = Store::open(path.as_ref()).unwrap();
/// // ... assertions ... — Drop kick et nettoie même sur panic.
/// ```
pub fn fresh_tmp_path(prefix: &str, tag: &str) -> std::path::PathBuf {
    let mut p = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(".codex-tmp");
    p.push(format!("{prefix}-{tag}-{nanos}"));
    p
}

#[cfg(test)]
mod tmp_dir_tests {
    use super::TmpDir;
    use std::path::PathBuf;
    

    fn fresh_path(tag: &str) -> PathBuf {
        super::fresh_tmp_path("tmpdir-test", tag)
    }

    #[test]
    fn tmpdir_cleans_up_on_normal_drop() {
        let path = fresh_path("normal");
        std::fs::create_dir_all(&path).unwrap();
        assert!(path.exists());
        {
            let _guard = TmpDir::new(path.clone());
        } // guard dropped here
        assert!(!path.exists(), "TmpDir Drop must remove the dir");
    }

    #[test]
    fn tmpdir_cleans_up_on_panic_unwind() {
        // Φ.maintenance.tmp-cleanup régression — if a test panics,
        // TmpDir's Drop kick must STILL run (panic unwind invokes
        // destructors). This is the whole point of the Drop-guard
        // pattern vs the historical `let _ = remove_dir_all(...)`
        // at end-of-fn which leaks on panic.
        let path = fresh_path("panic");
        std::fs::create_dir_all(&path).unwrap();
        assert!(path.exists());

        let path_for_thread = path.clone();
        let result = std::thread::spawn(move || {
            let _guard = TmpDir::new(path_for_thread);
            panic!("simulated test failure mid-test");
        })
        .join();
        assert!(result.is_err(), "thread should have panicked");
        assert!(
            !path.exists(),
            "TmpDir Drop must clean up even after panic unwind \
             (this is the whole point of the guard)"
        );
    }
}

// Φ.μ.7.2 — re-exports compat : `extract` est désormais physiquement
// dans `agent/extract/`, et `numeric` dans `kasm/numeric/`. Les paths
// publics historiques `scan::extract` et `scan::numeric` sont préservés
// via re-export sans churn de l'API surface.
pub use agent::extract;
pub use kasm::numeric;

pub use codec::{
    nanocube_pack_recipe_i64, nanocube_unpack_recipe_i64, pack_lossless, unpack_lossless,
    CodecError,
};
pub use brain::{
    brain_semantic_ref, brain_substitution_ref, publish_program_substitution,
    publish_semantic_attractor, resolve_program_hash, tighten_program_for_execution, BrainAction,
    BrainMemory, ForgeBrain, BRAIN_HEAD_REF, BRAIN_LATEST_ACTIVE_REF, BRAIN_SEMANTIC_REF_PREFIX,
    BRAIN_STATE_REF, BRAIN_SUBSTITUTION_REF_PREFIX,
};
pub use kasm::{PartialEvalReport, RewriteReport};
pub use key::CallKey;
pub use memory::MemoryGovernor;
pub use monster::{
    append_lab_log_slices, audit_loss, build_diverse_inputs, candidates_per_sec,
    canonical_outputs, collapse_audit_sample, contract_probe_inputs, default_lab_threads,
    execute_i64, extract_atoms_v2, find_parasites, fnv64_outputs, format_atom_catalogue_lines,
    format_jsonl, format_kcps, format_target_summary_lines, frontier_target_sample,
    generate_random_kasm_program, glyph_market_score, meta_glyph_phase,
    migrate_legacy_hot_atlas, open_shared_lab_store, output_fingerprint, parse_jsonl_line,
    percentile, random_evolve_config, random_target, read_lab_catalogue_summaries,
    read_lab_entries, recent_frontier_scores, spawn_lab_worker, target_fingerprint,
    tier1_contract_targets, Atlas, AtlasCounters, AtlasIngest, BatchCall, BatchInput,
    BatchPackedOutput, BulkEvaluator, ChainDepthHistogram, DispatchResult, DistillConfig,
    DistillDaemon, ExperimentOutcome, ExperimentResult, FrontierWeights, GlyphIntel,
    InlineCache, KasmStructure, LabCounters, SizeClass,
    LabExperimentReport, LabProbeResult, LabProbeStatus, LiveAtlas, LiveAtlasCounters,
    LogEntry, LogOutcome, MarketCounters, MetaGlyphCounters, MonsterCall,
    MonsterEvolutionConfig, MonsterEvolutionOutcome, MonsterNode, MonsterSource,
    MonsterStats, MonsterTrainingConfig, MonsterTrainingOutcome, MonsterValue, SynthProgress, SynthProgressFn,
    ParasiteReport, PhysicalEnvelope, PredictedSlot,
    ProgramAnalysis, ProgramEntry, SelfImproveBudget, SelfImproveDiscovery, SelfImproveMode,
    SelfImproveReport, StridePredictor, TargetCounters, TargetTemplate, XorShift64,
    CudaStatus, GpuNode, GpuNodeBootstrap, GpuNodeRuntime, GPUNODE_BOOTSTRAP_PATH, DEFAULT_PROBES,
    INLINE_MASK, INLINE_SLOTS, LEGACY_HOT_ATLAS_PATH, LIVE_ATLAS_PATH,
    bootstrap_gpunodes, bootstrap_gpunodes_best_effort, dx12_available, find_cuda_bin_dir,
    gpu_capability_report, register_cuda_dll_path, take_last_cuda_status, vulkan_available,
    walkforward_windows, WalkForwardConfig, WalkForwardError, WalkForwardResult, WalkForwardWindow,
};
#[cfg(feature = "wgpu")]
pub use monster::run_wgpu_universal_for_test;

#[cfg(feature = "cuda")]
pub mod cuda_min;
#[cfg(feature = "cuda")]
pub use cuda_min::{
    cuda_min_available as forge_cuda_min_available, kasm_runtime as forge_kasm_runtime,
    KASM_INTERPRET_FATBIN as FORGE_KASM_INTERPRET_FATBIN,
};

// Φ.ν.2.a — constantes lab (renommées pour éviter shadow avec d'autres
// const candidates côté SDK). `LAB_LOG_PATH` est le chemin canonique du
// log JSONL append-only ; `LAB_DEFAULT_ITERATIONS` la valeur sentinelle
// quand `lab_runner` est invoqué sans argument.
pub use monster::lab::{
    DEFAULT_ITERATIONS as LAB_DEFAULT_ITERATIONS, LOG_PATH as LAB_LOG_PATH,
};
pub use monster::{SwarmKnowledgeFrame, SwarmMemo, SwarmPresence};
pub use store::{Hash, Store};
