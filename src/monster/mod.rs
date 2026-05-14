//! MonsterNode: the hot-path KASM execution node.
//!
//! Sprint 1 refactor: the four `Mutex<VecDeque<...>>` caches were
//! collapsed into:
//!  * `cache`    — unified memo+result map, indexed by `CallKey`.
//!  * `programs` — verified-and-prepared `HotProgram` map.
//!  * `lru`      — small bookkeeping queue of recent `CallKey`s.
//!  * `oracles`  — affine learner state, one entry per program.
//!  * `stats_atomic` — atomic counters; `stats()` snapshots on demand.
//!
//! Sub-modules: `cache`, `exec`, `hotplan`, `oracle`, `swarm_io`,
//! `stats`. The struct lives here so every sibling `impl` block sees
//! the `pub(in crate::monster)` fields.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use crate::{CallKey, Hash, MemoryGovernor, Store};

mod atlas;
pub mod arena_lt;
pub mod bump;
mod cache;
pub mod cow_snapshot;
mod dispatch;
pub mod disruptor;
mod evolve;
mod exec;
mod gpunode;
pub(crate) mod gpu_synth;
mod hotplan;
pub mod huge_pages;
pub mod intrusive_index;
pub mod lab;
pub mod lua_table;
pub mod mmap_store;
pub mod mono_audit;
pub mod nnue;
mod oracle;
pub mod prefault;
pub mod seminaive;
pub mod seqlock;
pub mod slab;
pub mod speed_ablation;
pub mod static_pool;
pub mod stats;
mod swarm;
pub mod swiss_table;
mod train;
pub mod via_negativa;
pub mod walkforward;

pub use atlas::{
    canonical_outputs, fnv64_outputs, migrate_legacy_hot_atlas, output_fingerprint, Atlas,
    AtlasIngest, LiveAtlas, LiveAtlasCounters, LEGACY_HOT_ATLAS_PATH, LIVE_ATLAS_PATH,
};

pub use dispatch::{
    BatchCall, BatchInput, BulkEvaluator, DispatchResult,
    PackedOutput as BatchPackedOutput,
};

pub use hotplan::SizeClass;

/// Phase 12.0 — résumé public de l'analyse structurale d'un programme
/// KASM tel que vu par `dispatch_batch`. Calculé une fois au load via
/// `MonsterNode::analyze_program`. Aujourd'hui purement informatif —
/// la consommation par le triage multi-échelle (Phase 12.1+) viendra
/// dans des sessions ultérieures.
///
/// (Le nom évite la collision avec `monster::lab::ProgramAnalysis`
/// qui décrit autre chose côté synthétiseur.)
#[derive(Debug, Clone)]
pub struct KasmStructure {
    pub size_label: &'static str,
    pub node_count: u32,
    /// Ops apparaissant ≥ 2 fois dans le DAG. `(nom_op, count)`.
    /// Vide si le programme n'a aucune répétition d'op (e.g. SplitMix64
    /// = `Input → Hash64 → Output`, chaque op unique → vide).
    pub recurring_ops: Vec<(String, u32)>,
    /// Vrai si le programme contient au moins un op récurrent.
    pub is_decomposable: bool,
}

pub use swarm::{SwarmKnowledgeFrame, SwarmMemo, SwarmPresence};

#[cfg(test)]
mod tests;

pub use oracle::{DistillConfig, DistillDaemon, DEFAULT_PROBES};
pub use stats::{
    read_cycles, MonsterCall, MonsterSource, MonsterStats, MonsterValue, PhysicalEnvelope,
};
pub use evolve::{
    MonsterEvolutionConfig, MonsterEvolutionOutcome,
};
pub use gpunode::{
    bootstrap_gpunodes, bootstrap_gpunodes_best_effort, dx12_available, find_cuda_bin_dir,
    gpu_capability_report, register_cuda_dll_path, take_last_cuda_status, vulkan_available,
    CudaStatus, GpuNode, GpuNodeBootstrap, GpuNodeRuntime, GPUNODE_BOOTSTRAP_PATH,
};
#[cfg(feature = "wgpu")]
pub use gpunode::run_wgpu_universal_for_test;
pub use lab::{
    append_lab_log_slices, audit_loss, build_diverse_inputs, candidates_per_sec,
    collapse_audit_sample, contract_probe_inputs, default_lab_threads, execute_i64,
    extract_atoms_v2, find_parasites, format_atom_catalogue_lines, format_jsonl, format_kcps,
    format_target_summary_lines, generate_random_kasm_program, frontier_target_sample,
    meta_glyph_phase, open_shared_lab_store, parse_jsonl_line, percentile,
    random_evolve_config, random_target, read_lab_catalogue_summaries, read_lab_entries,
    recent_frontier_scores, spawn_lab_worker, target_fingerprint,
    tier1_contract_targets, AtlasCounters, ChainDepthHistogram, ExperimentOutcome,
    ExperimentResult, FrontierWeights, GlyphIntel, LabCounters, LabExperimentReport,
    LabProbeResult, LabProbeStatus, LogEntry, LogOutcome, MarketCounters, MetaGlyphCounters,
    ParasiteReport, ProgramAnalysis, ProgramEntry, SelfImproveBudget, SelfImproveDiscovery,
    SelfImproveMode, SelfImproveReport, TargetCounters, TargetTemplate, XorShift64,
    glyph_market_score,
};
// `DEFAULT_ITERATIONS` et `LOG_PATH` sont accédés depuis `lib.rs` via le
// chemin direct `monster::lab::*` puis renommés en `LAB_*` pour éviter
// la collision lexicale avec d'autres const lab/SDK. Les re-exporter ici
// déclencherait un warning `unused_imports` car personne ne les consomme
// via `monster::*`.
pub use train::{MonsterTrainingConfig, MonsterTrainingOutcome, SynthProgress, SynthProgressFn};
pub use walkforward::{
    windows as walkforward_windows, WalkForwardConfig, WalkForwardError, WalkForwardResult,
    WalkForwardWindow,
};

use cache::{AtomicStats, CacheSlot, IdentityBuildHasher, RamKey, RAMKEY_INITIAL_CAPACITY};
use hotplan::HotProgram;
use oracle::OracleState;

pub use cache::{InlineCache, PredictedSlot, StridePredictor, INLINE_MASK, INLINE_SLOTS};

/// Maximum number of `(CallKey, CallKey)` pairs returned by
/// [`MonsterNode::convergent_pairs`]. Prevents pathological O(N^2)
/// blow-ups when many programs converge on a single result.
pub const CONVERGENT_PAIRS_CAP: usize = 1000;

pub struct MonsterNode {
    /// `Arc`-wrapped so a single `Store` can be shared across many
    /// co-resident `MonsterNode`s without re-opening the repository.
    pub(in crate::monster) store: Arc<Store>,
    pub(in crate::monster) governor: MemoryGovernor,
    pub(in crate::monster) cache: RwLock<HashMap<RamKey, CacheSlot, IdentityBuildHasher>>,
    pub(in crate::monster) programs: RwLock<HashMap<Hash, std::sync::Arc<HotProgram>>>,
    pub(in crate::monster) lru: Mutex<VecDeque<RamKey>>,
    pub(in crate::monster) oracles: RwLock<HashMap<[u8; 32], OracleState>>,
    pub(in crate::monster) stats_atomic: AtomicStats,
    /// Reverse memo index: result OID -> set of `RamKey`s that produced
    /// that result. Maintained alongside the forward `cache` insert/evict
    /// path. Deliberately NOT charged against the memory governor
    /// because it is metadata describing user data already accounted for
    /// by the forward cache; double-charging would skew eviction
    /// pressure for queries that don't care about reverse lookups.
    pub(in crate::monster) reverse: RwLock<HashMap<Hash, HashSet<RamKey>>>,
    /// Per-program result index: program semantic fingerprint -> set of
    /// distinct result hashes produced by that program. Same metadata
    /// rationale as `reverse`: not governor-charged.
    pub(in crate::monster) results_by_program:
        RwLock<HashMap<[u8; 32], HashSet<Hash>>>,
    /// When `false`, all reverse-index bookkeeping is skipped. The
    /// V7 default is `false` (use `new`/`shared`); set to `true` via
    /// `new_with_reverse_index`/`shared_with_reverse_index` if you
    /// need the analytic API.
    pub(in crate::monster) reverse_enabled: bool,
    /// Sticky bit set the first time a swarm-imported memo is inserted
    /// (`RamKey::Wire(...)` via `import_swarm_memos`). On single-node
    /// setups it stays `false` for the life of the node, letting the
    /// hot path skip both `key.to_call_key()` (SHA-1 + heap alloc)
    /// AND `lookup_wire` entirely on every cache miss. Saves ~200-400 ns
    /// per miss in single-node benchmarks; zero impact on swarm setups.
    pub(in crate::monster) wire_seen_ever: AtomicBool,
    /// Phase 12.1 — mémoization op-level pour les ops pures unaires
    /// `i64 → i64` (Hash64 d'abord). Activé uniquement sur le slow
    /// lane interpreter quand le programme est `is_decomposable()`.
    /// Pour les programmes monomorphes type SplitMix64 (atomique), ce
    /// cache n'est jamais touché — silencieux par construction.
    pub(in crate::monster) op_memo:
        RwLock<HashMap<(crate::kasm::Op, i64), i64>>,
    /// Logical GPUnode runtime (one logical node per detected adapter).
    pub(in crate::monster) gpunode_runtime: GpuNodeRuntime,
    /// Π.25 + Π.27 wire — MmapStore optionnel pour fast read zero-copy
    /// du `forge.cas` via IntrusiveBlobIndex (16 bytes/entry, binary
    /// search O(log N)). Activé via `enable_mmap_view()` ; au boot,
    /// `None` (fallback transparent à `Store::load`).
    ///
    /// Quand activé, `dispatch_impl` Layer 5 essaie d'abord le mmap
    /// (lookup ~50 ns) avant de tomber sur `Store::load` (read syscall
    /// ~5 µs). Pour les workloads scientifiques avec atlas pré-calculé
    /// (k-mer hashes, Black-Scholes greeks, conformer energies), gain
    /// 100× sur load path.
    pub(in crate::monster) mmap_view: RwLock<Option<crate::monster::mmap_store::MmapStore>>,
    /// Σ.3 wire — BumpAllocator scratch pool 64 KB pour callers qui
    /// veulent zero-alloc workflow. Reset entre opérations via
    /// `bump_reset()`. Use cases :
    ///   - Lab synthesis : alloc des intermediates par round
    ///   - Tauri RT mode : éviter heap alloc dans hot path UI
    ///   - Bench : scratch pour packed inputs/outputs
    pub(in crate::monster) scratch_bump:
        std::sync::Mutex<crate::monster::bump::BumpAllocator>,
    /// Π.29 wire — SlabAllocator i64 pool pour primitives qui veulent
    /// allocation+free LIFO O(1). Use cases :
    ///   - Cache de Value::I64 intermediates
    ///   - Slot pool pour OracleState i64 fields
    ///   - Lab candidate program scoring buffers
    pub(in crate::monster) value_slab:
        std::sync::Mutex<crate::monster::slab::SlabAllocator<i64>>,
    /// Π.7 wire — StaticPool<i64> 1024 slots pré-alloués au boot, JAMAIS
    /// re-alloué runtime. Garantit 0 alloc dans le hot path → latence
    /// déterministe. Use cases :
    ///   - **Trading HFT** : pool d'order IDs, position slots, tick refs
    ///   - **Spatial/aerospace** : missions critiques avec contraintes RAM
    ///     strictes (TigerBeetle pattern)
    ///   - **Médical real-time** : monitoring cardiaque embarqué
    ///   - **Finance settlement** : compute déterministe sans GC pause
    pub(in crate::monster) call_pool:
        std::sync::Mutex<crate::monster::static_pool::StaticPool<i64>>,
    /// Π.4 wire — SpscRing<i64> 256-slot lock-free pour producteur-
    /// consommateur cross-thread. Pattern LMAX HFT. Use cases :
    ///   - **Trading** : market data feed → strategy evaluator (canonique)
    ///   - **Médical RT** : sensor stream → analytics pipeline
    ///   - **Spatial telemetry** : sensor → ground processing async
    ///   - **Science** : simulation events → analyzer
    ///   - **Finance** : tx events → settlement async
    pub(in crate::monster) event_ring:
        std::sync::Arc<crate::monster::disruptor::SpscRing<i64>>,
    /// Σ.23 wire — Seqlock<i64> single-writer/many-readers pour latest
    /// domain state sans lock le hot path. Pattern Linux gettimeofday.
    /// Use cases :
    ///   - **Trading** : latest market price (1 feed writer, N strategy readers)
    ///   - **Médical** : latest sensor value (1 hardware writer, N analyzers)
    ///   - **Spatial** : latest attitude/position (1 sensor, N control loops)
    ///   - **Finance** : latest portfolio state (1 reconciler, N viewers)
    ///   - **Science** : latest simulation tick (1 sim, N visualizers)
    pub(in crate::monster) domain_state:
        crate::monster::seqlock::Seqlock<i64>,
    /// Π.1 wire — NNUE Stockfish-style int8 neural network 4→16→1.
    /// Évaluateur incrémental où changer 1 feature recalcule juste la
    /// row affectée du hidden layer, pas tout. Use cases :
    ///   - **Trading** : prédire slippage/impact d'un ordre (NN sur
    ///     orderbook state, recompute = updater 1 feature, pas tout)
    ///   - **Chimie** : énergie de molécule incrémentale (changer 1
    ///     atome → NN update ~100× plus rapide que full recompute)
    ///   - **Médical** : pathology score sur features patient
    ///   - **Spatial** : anomaly detection sur telemetry
    ///   - **Science** : classifier expériences vs noise
    ///   - **Lab oracle alternatif** : remplacer les 5 algos d'inférence
    ///     par un NN qui apprend les patterns observés
    pub(in crate::monster) oracle_nnue:
        std::sync::Mutex<crate::monster::nnue::NnueNetwork>,
    /// Π.8 wire — SeminaiveEngine Datalog evaluation. Permet d'exprimer
    /// des règles "head :- body" et de calculer la transitive closure
    /// efficacement. Use cases :
    ///   - **Bioinfo** : "gene G on chromosome C, C in region R, R
    ///     conserved across species S" → conservation closure
    ///   - **Chimie** : "molécule M contient F, F implies reactivity R"
    ///     → propriétés dérivées en chaîne
    ///   - **Médical** : "patient P has symptom S, S indicates disease D"
    ///     → diagnostic chains
    ///   - **Trading** : "asset A correlates with B, B correlates with C"
    ///     → indirect correlations
    ///   - **Spatial** : "subsystem A depends on B, B depends on C"
    ///     → fault propagation analysis
    ///   - **Science** : règles symboliques sur l'atlas mining
    ///
    /// Lazily-initialized — None au boot, set via `seminaive_load_rules()`
    /// quand le caller a un programme métier de règles à exécuter.
    pub(in crate::monster) seminaive_engine: std::sync::RwLock<
        Option<crate::monster::seminaive::SeminaiveEngine>,
    >,
    /// Π.31 wire — CowSnapshotter pour Atlas checkpoint O(1) via Arc::clone.
    /// Pattern Redis BGSAVE / PostgreSQL CHECKPOINT. Use cases :
    ///   - **Atlas checkpointing** : snapshot avant un commit lab risqué,
    ///     restore en O(1) si régression mesurée
    ///   - **Trading** : snapshot du portfolio state à chaque tick →
    ///     replay scenarios pour stress test
    ///   - **Médical** : snapshot patient state pour rollback erreur
    ///   - **Spatial** : snapshot mission state → fault recovery
    ///   - **Science** : snapshot simulation state → restart points
    ///   - **Chimie** : snapshot conformer search state → branch & restart
    pub(in crate::monster) state_snapshot:
        std::sync::Mutex<crate::monster::cow_snapshot::CowSnapshotter>,
    /// Π.13 wire — LuaTable<u64> auto-array/hash hybrid pour atlas avec
    /// clés mixtes dense/sparse. Pattern Lua 5.0+ tables. Use cases :
    ///   - **Bioinfo** : k-mer index où la plupart sont denses (2-bit
    ///     encoding 0..2^21) + special markers sparses
    ///   - **Trading** : tick index par timestamp (dense) + special
    ///     events (sparse)
    ///   - **Spatial** : telemetry par seq_no (dense) + alerts (sparse)
    ///   - **Médical** : patient_id index (dense) + edge cases (sparse)
    pub(in crate::monster) domain_index:
        std::sync::RwLock<crate::monster::lua_table::LuaTable<u64>>,
    /// Unified redundancy atlas — set by `attach_atlas()`, read by both
    /// MonsterNode runtime paths and external owners (e.g. Tauri backend).
    /// Default `None` keeps tests + library callers fully backward-compat.
    pub(in crate::monster) atlas:
        std::sync::RwLock<Option<std::sync::Arc<crate::atlas::Atlas>>>,
}

impl MonsterNode {
    /// Build a fast-path node. **Reverse index is OFF by default**
    /// (V7 default flip — saves 2 RwLock writes + 2 HashSet allocs
    /// per cache miss). Use [`new_with_reverse_index`] if you need the
    /// analytic API (`calls_for_result`, `results_for_program`,
    /// `convergent_pairs`).
    pub fn new(store: Store, governor: MemoryGovernor) -> Self {
        Self::with_options(Arc::new(store), governor, false)
    }

    /// Build a node WITH the reverse memo index enabled. Pays 2
    /// RwLock writes + 2 HashSet entry inserts per cache miss in
    /// exchange for the analytic API. Useful for observers,
    /// gödel-loop verifiers, and colony introspection — anything
    /// that calls `calls_for_result`, `results_for_program`, or
    /// `convergent_pairs`.
    pub fn new_with_reverse_index(store: Store, governor: MemoryGovernor) -> Self {
        Self::with_options(Arc::new(store), governor, true)
    }

    /// Construct a node that **shares** an existing `Arc<Store>` with
    /// other co-resident nodes. The colony scenario: instead of every
    /// node opening its own libgit2 repository (and paying ~100 KB +
    /// FDs per instance), thousands of nodes hold a `Clone` of the same
    /// `Arc<Store>` and contend on its internal `Mutex<Repository>`.
    ///
    /// Each node still has its own caches, oracles, governor, and
    /// stats — only the persistent substrate is shared.
    ///
    /// **Reverse index is OFF by default** in V7. Use
    /// [`shared_with_reverse_index`] if you need it.
    pub fn shared(store: Arc<Store>, governor: MemoryGovernor) -> Self {
        Self::with_options(store, governor, false)
    }

    /// Same as [`shared`], with the reverse memo index enabled.
    pub fn shared_with_reverse_index(store: Arc<Store>, governor: MemoryGovernor) -> Self {
        Self::with_options(store, governor, true)
    }

    /// Lite constructor — every node-local hash map starts
    /// at **capacity 0** (LRU deque too) and the reverse index is off
    /// by default. The amortised first-`insert` cost is paid only by
    /// nodes that actually receive a call; idle nodes carry just the
    /// pointer-sized fields of `MonsterNode` (488 B stack + a handful
    /// of pointers), making 100 000 co-resident instances tractable.
    ///
    /// Trade-off: a node's first cache miss does a small heap alloc
    /// instead of using the pre-warmed bucket array. For colonies
    /// where a tiny fraction of nodes are ever active that's a win;
    /// for high-traffic single nodes use [`new`] / [`shared`] which
    /// pre-allocate `RAMKEY_INITIAL_CAPACITY = 1024` buckets.
    pub fn shared_lite(store: Arc<Store>, governor: MemoryGovernor) -> Self {
        Self {
            store,
            governor,
            cache: RwLock::new(HashMap::with_capacity_and_hasher(
                0,
                IdentityBuildHasher::default(),
            )),
            programs: RwLock::new(HashMap::with_capacity(0)),
            lru: Mutex::new(VecDeque::with_capacity(0)),
            oracles: RwLock::new(HashMap::with_capacity(0)),
            stats_atomic: AtomicStats::default(),
            reverse: RwLock::new(HashMap::with_capacity(0)),
            results_by_program: RwLock::new(HashMap::with_capacity(0)),
            reverse_enabled: false,
            wire_seen_ever: AtomicBool::new(false),
            op_memo: RwLock::new(HashMap::with_capacity(0)),
            gpunode_runtime: GpuNodeRuntime::bootstrap_best_effort(),
            mmap_view: RwLock::new(None),
            scratch_bump: std::sync::Mutex::new(
                crate::monster::bump::BumpAllocator::with_capacity(64 * 1024),
            ),
            value_slab: std::sync::Mutex::new(
                crate::monster::slab::SlabAllocator::new(),
            ),
            call_pool: std::sync::Mutex::new(
                crate::monster::static_pool::StaticPool::with_capacity(1024),
            ),
            event_ring: std::sync::Arc::new(
                crate::monster::disruptor::SpscRing::with_capacity(256),
            ),
            domain_state: crate::monster::seqlock::Seqlock::new(0),
            oracle_nnue: std::sync::Mutex::new(
                crate::monster::nnue::NnueNetwork::from_seed(0x4F52_4147_4500_0001),
            ),
            seminaive_engine: std::sync::RwLock::new(None),
            state_snapshot: std::sync::Mutex::new(
                crate::monster::cow_snapshot::CowSnapshotter::empty(),
            ),
            domain_index: std::sync::RwLock::new(
                crate::monster::lua_table::LuaTable::new(),
            ),
            atlas: std::sync::RwLock::new(None),
        }
    }

    fn with_options(store: Arc<Store>, governor: MemoryGovernor, reverse_enabled: bool) -> Self {
        Self {
            store,
            governor,
            cache: RwLock::new(HashMap::with_capacity_and_hasher(
                RAMKEY_INITIAL_CAPACITY,
                IdentityBuildHasher::default(),
            )),
            programs: RwLock::new(HashMap::new()),
            lru: Mutex::new(VecDeque::with_capacity(RAMKEY_INITIAL_CAPACITY)),
            oracles: RwLock::new(HashMap::new()),
            stats_atomic: AtomicStats::default(),
            reverse: RwLock::new(HashMap::new()),
            results_by_program: RwLock::new(HashMap::new()),
            reverse_enabled,
            wire_seen_ever: AtomicBool::new(false),
            op_memo: RwLock::new(HashMap::new()),
            gpunode_runtime: GpuNodeRuntime::bootstrap_best_effort(),
            mmap_view: RwLock::new(None),
            scratch_bump: std::sync::Mutex::new(
                crate::monster::bump::BumpAllocator::with_capacity(64 * 1024),
            ),
            value_slab: std::sync::Mutex::new(
                crate::monster::slab::SlabAllocator::new(),
            ),
            call_pool: std::sync::Mutex::new(
                crate::monster::static_pool::StaticPool::with_capacity(1024),
            ),
            event_ring: std::sync::Arc::new(
                crate::monster::disruptor::SpscRing::with_capacity(256),
            ),
            domain_state: crate::monster::seqlock::Seqlock::new(0),
            oracle_nnue: std::sync::Mutex::new(
                crate::monster::nnue::NnueNetwork::from_seed(0x4F52_4147_4500_0001),
            ),
            seminaive_engine: std::sync::RwLock::new(None),
            state_snapshot: std::sync::Mutex::new(
                crate::monster::cow_snapshot::CowSnapshotter::empty(),
            ),
            domain_index: std::sync::RwLock::new(
                crate::monster::lua_table::LuaTable::new(),
            ),
            atlas: std::sync::RwLock::new(None),
        }
    }

    /// Attach a shared `Atlas` (redundancy hash store) so this node and
    /// any external owner (e.g. Tauri backend) reach the same persisted
    /// CSE / trace / sub-tree fingerprints. Call once at startup; idempotent.
    pub fn attach_atlas(&self, atlas: std::sync::Arc<crate::atlas::Atlas>) {
        *self.atlas.write().expect("atlas slot poisoned") = Some(atlas);
    }

    /// Returns a clone of the shared atlas if one is attached.
    pub fn atlas(&self) -> Option<std::sync::Arc<crate::atlas::Atlas>> {
        self.atlas
            .read()
            .expect("atlas slot poisoned")
            .as_ref()
            .map(std::sync::Arc::clone)
    }

    /// Active le MmapStore zero-copy read view sur le `forge.cas`. Une
    /// seule lecture upfront du fichier complet → builds un IntrusiveBlobIndex
    /// 16 bytes/entry. Tous les loads suivants prennent le path mmap
    /// (50 ns) au lieu de `Store::load` (5 µs syscall + alloc).
    ///
    /// À appeler après que les programmes initiaux soient stockés.
    /// Si nouveau `Store::store()` arrive après, le mmap devient stale
    /// pour ces blobs — `disable_mmap_view()` puis re-enable pour rebuilder.
    ///
    /// Use cases :
    /// - Bioinfo : 100M k-mer hashes pré-calculés, retrieve at near-RAM
    /// - Finance : 1B Black-Scholes greeks pré-évalués
    /// - Chimie : conformer energies pré-évaluées
    pub fn enable_mmap_view(&self) -> std::io::Result<()> {
        let cas_path = self.store.path().join("forge.cas");
        let mmap = crate::monster::mmap_store::MmapStore::open(&cas_path)
            .map_err(|e| std::io::Error::other(format!("MmapStore open: {e:?}")))?;
        *self.mmap_view.write().unwrap() = Some(mmap);
        Ok(())
    }

    /// Désactive le mmap view (libère la RAM backing). À appeler avant
    /// des écritures massives qui rendraient le mmap obsolète.
    pub fn disable_mmap_view(&self) {
        *self.mmap_view.write().unwrap() = None;
    }

    /// Σ.3 wire — alloue via le BumpAllocator scratch. Le caller doit
    /// `bump_reset()` quand il a fini d'utiliser le buffer (release
    /// l'espace pour le prochain caller).
    /// Use case : lab synth qui alloue des candidats par round +
    /// Tauri RT mode zero-alloc.
    pub fn bump_alloc(&self, layout: std::alloc::Layout) -> Option<*mut u8> {
        self.scratch_bump.lock().unwrap().try_alloc(layout)
    }

    /// Σ.3 wire — reset le BumpAllocator scratch. À appeler quand le
    /// caller a fini avec ses allocations courantes.
    pub fn bump_reset(&self) {
        self.scratch_bump.lock().unwrap().reset();
    }

    /// Σ.3 wire — bytes utilisés par le scratch bump (observabilité).
    pub fn bump_used_bytes(&self) -> usize {
        self.scratch_bump.lock().unwrap().bytes_used()
    }

    /// Π.29 wire — alloue un slot i64 dans le SlabAllocator value_slab.
    /// Retourne un handle opaque ; le caller récupère/free via les
    /// methodes correspondantes.
    pub fn value_slab_alloc(&self, value: i64) -> crate::monster::slab::SlabHandle {
        self.value_slab.lock().unwrap().alloc(value)
    }

    /// Π.29 wire — free un slot i64 du SlabAllocator (LIFO reuse).
    pub fn value_slab_free(&self, handle: crate::monster::slab::SlabHandle) {
        self.value_slab.lock().unwrap().free(handle);
    }

    /// Π.29 wire — copie le i64 du slot pointé par handle (lecture).
    pub fn value_slab_get(&self, handle: crate::monster::slab::SlabHandle) -> i64 {
        *self.value_slab.lock().unwrap().get(handle)
    }

    // ─── Tier 2 APIs : arena_lt + static_pool + disruptor + seqlock ─────

    /// Π.26 wire (arena_lt) — exécute `f` dans un ArenaScope basé sur le
    /// scratch_bump. Tout ce qui est alloué dans la scope est drop
    /// automatiquement à la fin (auto-reset du bump).
    /// Use cases :
    ///   - **Science** : Monte Carlo trial où chaque trial alloue des
    ///     intermediaries drop ensemble (entre trials)
    ///   - **Trading** : backtesting où chaque jour de simulation
    ///     alloue positions/orders drop entre jours
    ///   - **Médical** : essais cliniques où chaque cohorte a des
    ///     state structures temporaires
    ///   - **Spatial** : trajectory simulation par segment
    ///   - **Chimie** : molécules candidates par round de docking
    ///
    /// Note : la closure ne peut pas garder de références aux objets
    /// alloués au-delà de son retour (lifetime borrow checker).
    pub fn with_arena_scope<R>(
        &self,
        f: impl FnOnce(&crate::monster::arena_lt::ArenaScope<'_>) -> R,
    ) -> R {
        let bump = self.scratch_bump.lock().unwrap();
        let scope = crate::monster::arena_lt::ArenaScope::new(&*bump);
        let result = f(&scope);
        drop(scope); // auto-reset bump
        drop(bump);
        result
    }

    /// Π.7 wire (static_pool) — take un slot du pool pré-alloué (0 alloc
    /// runtime). Retourne handle ou None si pool plein. LIFO reuse via
    /// `call_pool_release`.
    /// Use cases :
    ///   - Trading HFT : pool d'order IDs (0 alloc latence prévisible)
    ///   - Spatial/embedded : missions critiques RAM-strict
    ///   - Médical RT : sensor processing
    pub fn call_pool_take(
        &self,
        value: i64,
    ) -> Option<crate::monster::static_pool::PoolHandle> {
        self.call_pool.lock().unwrap().try_take(value)
    }

    /// Π.7 wire — release un slot du pool (LIFO reuse).
    pub fn call_pool_release(&self, handle: crate::monster::static_pool::PoolHandle) {
        self.call_pool.lock().unwrap().release(handle);
    }

    /// Π.7 wire — lecture du value au handle (sans take).
    pub fn call_pool_get(&self, handle: crate::monster::static_pool::PoolHandle) -> i64 {
        *self.call_pool.lock().unwrap().get(handle)
    }

    /// Π.7 wire — slots libres dans le pool (observabilité).
    pub fn call_pool_free_slots(&self) -> usize {
        self.call_pool.lock().unwrap().free()
    }

    /// Π.4 wire (disruptor) — publish un event i64 dans le ring SPSC
    /// lock-free. Retourne false si le ring est plein (caller backoff).
    /// Use cases :
    ///   - **Trading** : market feed publish tick → strategy consumer
    ///   - **Médical** : sensor publish reading → analytics consumer
    ///   - **Spatial** : telemetry publish state → ground consumer
    pub fn event_publish(&self, event: i64) -> bool {
        self.event_ring.try_publish(event)
    }

    /// Π.4 wire — consume le prochain event du ring. Retourne None si
    /// le ring est vide (caller poll/sleep).
    pub fn event_consume(&self) -> Option<i64> {
        self.event_ring.try_consume()
    }

    /// Π.4 wire — clone l'Arc<SpscRing> pour partage cross-thread.
    /// Producer thread + Consumer thread tiennent chacun un Arc.
    pub fn event_ring_handle(&self) -> std::sync::Arc<crate::monster::disruptor::SpscRing<i64>> {
        std::sync::Arc::clone(&self.event_ring)
    }

    /// Σ.23 wire (seqlock) — write le latest domain state. Single writer
    /// pattern : un seul thread doit appeler write ; many readers OK.
    /// Use cases :
    ///   - **Trading** : feed thread écrit latest price
    ///   - **Médical** : hardware thread écrit latest sensor reading
    ///   - **Spatial** : sensor thread écrit latest attitude
    pub fn domain_state_write(&self, state: i64) {
        self.domain_state.write(state);
    }

    /// Σ.23 wire — read le latest domain state SANS lock. Many readers
    /// peuvent appeler en parallèle sans contention. Le seqlock garantit
    /// la consistance via compteur de séquence (retry si write en cours).
    pub fn domain_state_read(&self) -> i64 {
        self.domain_state.read()
    }

    /// Σ.23 wire — sequence du seqlock (compteur de writes). Indique
    /// combien de fois le state a été mis à jour depuis boot.
    pub fn domain_state_sequence(&self) -> u32 {
        self.domain_state.sequence()
    }

    // ─── Tier 3 APIs : nnue + seminaive + cow_snapshot ──────────────────

    /// Π.1 wire (nnue) — predict via le NNUE int8 4→16→1 sur 4 features.
    /// Use cases :
    ///   - Trading : features = [order_size, mid_price_delta, spread,
    ///     orderbook_imbalance] → prédire impact
    ///   - Chimie : features = [bond_count, charge, ring_size, polarity]
    ///     → prédire énergie
    ///   - Médical : features = [age, vitals_zscore, lab_value, history]
    ///     → pathology score
    pub fn nnue_predict(
        &self,
        features: [i16; crate::monster::nnue::NNUE_INPUT_FEATURES],
    ) -> i64 {
        self.oracle_nnue.lock().unwrap().predict(features)
    }

    /// Π.1 wire (nnue) — incremental update. Quand UNE seule feature change
    /// (changer 1 atome dans une molécule, 1 patient feature, 1 ordre dans
    /// l'orderbook), recompute juste la delta du hidden layer. ~100× plus
    /// rapide que full predict.
    /// Le caller doit feed (slot, old_value, new_value) ; le NNUE met à
    /// jour son accumulator interne et retourne le nouveau output.
    pub fn nnue_incremental_update(&self, feature_idx: usize, old: i16, new: i16) -> i64 {
        self.oracle_nnue.lock().unwrap().incremental_update(feature_idx, old, new)
    }

    /// Π.1 wire (nnue) — encode (program_fingerprint, args) en 4 features
    /// pour le NNUE. Used pour wirer le NN comme oracle alternatif au
    /// 5-algo `infer_oracle` quand workload sature.
    pub fn nnue_encode_features(
        &self,
        prog_fingerprint: &[u8],
        args: &[u8],
    ) -> [i16; crate::monster::nnue::NNUE_INPUT_FEATURES] {
        crate::monster::nnue::NnueNetwork::encode_features(prog_fingerprint, args)
    }

    /// Π.8 wire (seminaive) — load les rules Datalog dans l'engine.
    /// Remplace l'engine existant si déjà loaded.
    /// Use case : "molécule M contient F, F implies reactivity R" →
    /// load comme Rule, puis run avec EDB facts (containments observés).
    pub fn seminaive_load_rules(&self, rules: Vec<crate::monster::seminaive::Rule>) {
        let engine = crate::monster::seminaive::SeminaiveEngine::new(rules);
        *self.seminaive_engine.write().unwrap() = Some(engine);
    }

    /// Π.8 wire (seminaive) — exécute Datalog avec les rules loadées + les
    /// facts EDB fournis. Retourne (IDB facts, stats). None si aucune
    /// rule loaded (caller doit appeler `seminaive_load_rules` avant).
    pub fn seminaive_run(
        &self,
        edb: Vec<crate::monster::seminaive::Fact>,
    ) -> Option<(Vec<crate::monster::seminaive::Fact>, crate::monster::seminaive::SeminaiveStats)> {
        let guard = self.seminaive_engine.read().unwrap();
        let engine = guard.as_ref()?;
        Some(engine.run(edb))
    }

    /// Π.8 wire (seminaive) — true si rules loadées.
    pub fn seminaive_has_rules(&self) -> bool {
        self.seminaive_engine.read().unwrap().is_some()
    }

    /// Π.31 wire (cow_snapshot) — initialise le state buffer (à appeler
    /// avant le premier snapshot). Le caller fournit son state sérialisé
    /// (par exemple atlas snapshot binary, portfolio state, etc.).
    pub fn state_init(&self, buffer: Vec<u8>, blob_count: usize) {
        let buf = buffer.into_boxed_slice();
        *self.state_snapshot.lock().unwrap() =
            crate::monster::cow_snapshot::CowSnapshotter::new(buf, blob_count);
    }

    /// Π.31 wire — take un snapshot O(1) via Arc::clone du backing
    /// courant. Le snapshot reste valide même si le state change après
    /// (la cause = Arc cloning, l'ancien backing reste vivant).
    /// Use case : avant un commit lab risqué, snapshot ; si régression
    /// mesurée → restore.
    pub fn state_take_snapshot(&self) -> crate::monster::cow_snapshot::CowSnapshot {
        self.state_snapshot.lock().unwrap().take_snapshot()
    }

    /// Π.31 wire — replace le state courant par un nouveau buffer.
    /// L'ancien backing reste vivant tant qu'un snapshot le référence.
    pub fn state_replace(&self, new_buffer: Vec<u8>, blob_count: usize) {
        let buf = new_buffer.into_boxed_slice();
        self.state_snapshot.lock().unwrap().replace_backing(buf, blob_count);
    }

    /// Π.31 wire — restaure le state depuis un snapshot. O(1) via Arc.
    /// Use case : régression détectée après commit → restore l'ancien.
    pub fn state_restore(&self, snapshot: &crate::monster::cow_snapshot::CowSnapshot) {
        self.state_snapshot.lock().unwrap().restore(snapshot);
    }

    /// Π.31 wire — copie le state courant en Vec owned (pour callers qui
    /// veulent observer sans tenir la lock).
    pub fn state_current(&self) -> Vec<u8> {
        self.state_snapshot.lock().unwrap().current_slice().to_vec()
    }

    /// Π.31 wire — stats du snapshotter (snapshots taken, restores, etc.).
    pub fn state_snapshot_stats(&self) -> crate::monster::cow_snapshot::SnapshotStats {
        self.state_snapshot.lock().unwrap().stats()
    }

    // ─── Tier 4 APIs : lua_table + walkforward + speed_ablation + mono_audit ─

    /// Π.13 wire (lua_table) — insert dans l'index hybrid array/hash.
    /// Use case : k-mer index avec clés denses 0..2^21 + special markers
    /// sparses → la part array est O(1) sans hash, la part hash gère
    /// les exceptions.
    pub fn domain_index_insert(&self, key: i64, value: u64) -> Option<u64> {
        self.domain_index.write().unwrap().insert(key, value)
    }

    /// Π.13 wire — get une valeur (note : LuaTable::get prend &mut self
    /// pour stats, donc on prend write lock).
    pub fn domain_index_get(&self, key: i64) -> Option<u64> {
        self.domain_index.write().unwrap().get(key).copied()
    }

    /// Π.13 wire — remove une clé. Retourne valeur si présente.
    pub fn domain_index_remove(&self, key: i64) -> Option<u64> {
        self.domain_index.write().unwrap().remove(key)
    }

    /// Π.13 wire — taille (nombre total de clés array+hash).
    pub fn domain_index_len(&self) -> usize {
        self.domain_index.read().unwrap().len()
    }

    /// Π.13 wire — stats (total_keys, array_hits, hash_hits) pour
    /// observer la distribution dense/sparse de l'index.
    pub fn domain_index_stats(&self) -> (u64, u64, u64) {
        self.domain_index.read().unwrap().stats()
    }

    /// Π.23 wire (walkforward) — exécute walk-forward analysis.
    /// Le caller fournit :
    ///   - config : tailles de fenêtres in-sample / out-of-sample
    ///   - candidate_params : paramètres à tester
    ///   - n_bars : taille totale de l'historique
    ///   - optimize_fn : trouve les meilleurs params sur in-sample
    ///   - eval_fn : évalue les params sur out-of-sample
    /// Retourne Vec de résultats par fenêtre.
    /// Use cases :
    ///   - **Trading** : valider une stratégie KASM générée par lab
    ///   - **Science** : valider modèle KASM ne overfit pas
    ///   - **Médical** : essais cliniques sliding window
    pub fn walkforward_run<P, F, G>(
        &self,
        config: crate::monster::walkforward::WalkForwardConfig,
        candidate_params: &[P],
        n_bars: usize,
        optimize_fn: F,
        eval_fn: G,
    ) -> Result<
        Vec<crate::monster::walkforward::WalkForwardResult<P>>,
        crate::monster::walkforward::WalkForwardError,
    >
    where
        P: Clone,
        F: Fn(std::ops::Range<usize>, &[P]) -> (P, i64),
        G: Fn(std::ops::Range<usize>, &P) -> i64,
    {
        crate::monster::walkforward::walk_forward(
            config,
            candidate_params,
            n_bars,
            optimize_fn,
            eval_fn,
        )
    }

    /// Π.23 wire — moyenne des scores out-of-sample. Utile comme
    /// metric finale pour décider d'adopter une stratégie/modèle.
    pub fn walkforward_avg_oos_score<P: Clone>(
        results: &[crate::monster::walkforward::WalkForwardResult<P>],
    ) -> i64 {
        crate::monster::walkforward::average_oos_score(results)
    }

    /// Σ.13 wire (speed_ablation) — audit du Speed Ablation : combien
    /// de #[inline(always)] appliqués, StackStr disponible, etc.
    /// Pour observabilité/diagnostic des optimisations.
    pub fn speed_ablation_audit(&self) -> crate::monster::speed_ablation::SpeedAblationAudit {
        crate::monster::speed_ablation::audit_report()
    }

    /// Σ.11 wire (mono_audit) — audit de la monomorphisation Rust.
    /// Estimated_savings_bytes = combien de KB de binary size
    /// économisés via stratégies dyn vs impl bien choisies.
    /// Pour diagnostic compile-time / binary bloat.
    pub fn mono_audit(&self) -> crate::monster::mono_audit::MonoAuditReport {
        crate::monster::mono_audit::audit_report()
    }

    /// Status du mmap view : (active, blob_count, backing_size_bytes).
    /// Pour observabilité Tauri / monitoring.
    pub fn mmap_view_status(&self) -> Option<(usize, usize)> {
        self.mmap_view.read().unwrap()
            .as_ref()
            .map(|m| (m.blob_count(), m.backing_size()))
    }

    /// Fast blob load via mmap view si activé. Retourne `None` si :
    /// - mmap view désactivé (caller doit fallback à `store().load()`)
    /// - blob absent du mmap (probable : ajouté après mmap build → fallback)
    ///
    /// Le caller décide du fallback : `node.load_blob_fast(h).or_else(|| node.store().load(&h))`.
    pub(in crate::monster) fn load_blob_fast(&self, hash: &Hash) -> Option<Vec<u8>> {
        let guard = self.mmap_view.read().unwrap();
        let mmap = guard.as_ref()?;
        let slice = mmap.lookup(hash)?;
        // Copie en owned Vec — l'API publique de Store::load retourne
        // Vec<u8>. Pour vraiment zero-copy, le caller doit utiliser
        // `mmap_view.lookup_owned()` (MmapBlobRef wrapper Arc).
        Some(slice.to_vec())
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Return a `Clone` of the underlying `Arc<Store>` so a caller can
    /// build additional `MonsterNode`s sharing the same persistent
    /// substrate.
    pub fn shared_store(&self) -> Arc<Store> {
        Arc::clone(&self.store)
    }

    pub fn governor(&self) -> &MemoryGovernor {
        &self.governor
    }

    /// First-install GPU bootstrap metadata.
    pub fn gpunode_bootstrap(&self) -> &GpuNodeBootstrap {
        self.gpunode_runtime.bootstrap()
    }

    /// Number of logical GPUnodes auto-deployed for this runtime.
    pub fn gpunode_count(&self) -> usize {
        self.gpunode_runtime.count()
    }

    /// Logical GPUnode handles currently active.
    pub fn gpunodes(&self) -> &[GpuNode] {
        self.gpunode_runtime.node_ids()
    }

    /// Phase 12.0 — Analyse structurale d'un programme par hash. Force
    /// le chargement du `HotProgram` (parse + verify + simplify) si pas
    /// encore en cache, puis retourne le résumé public utilisé par les
    /// callers UI (Tauri) pour afficher la taille + opérations
    /// récurrentes.
    pub fn analyze_program(&self, func: &Hash) -> std::io::Result<KasmStructure> {
        let hot = self.hot_program(func)?;
        Ok(KasmStructure {
            size_label: hot.structure.size.label(),
            node_count: hot.structure.node_count,
            recurring_ops: hot
                .structure
                .recurring_ops
                .iter()
                .map(|(op, c)| (format!("{:?}", op), *c))
                .collect(),
            is_decomposable: hot.structure.is_decomposable(),
        })
    }

    pub fn stats(&self) -> MonsterStats {
        self.stats_atomic.snapshot()
    }

    /// Number of distinct result OIDs currently tracked by the reverse
    /// index. Equals 0 when the node was built with `new`/`shared`
    /// (V7 default — reverse-index off).
    pub fn reverse_index_len(&self) -> usize {
        self.reverse.read().unwrap().len()
    }

    /// All `CallKey`s known to have produced `result`. Returns an empty
    /// vector when the result is unknown or the reverse index is
    /// disabled.
    pub fn calls_for_result(&self, result: &Hash) -> Vec<CallKey> {
        let map = self.reverse.read().unwrap();
        match map.get(result) {
            Some(set) => set.iter().map(|k| k.to_call_key()).collect(),
            None => Vec::new(),
        }
    }

    /// All distinct result OIDs ever observed for the program identified
    /// by `func` (resolved through the program cache to its semantic
    /// fingerprint). Returns an empty vector when the program is not in
    /// the cache or the reverse index is disabled.
    ///
    /// Falls back to a cache scan if the per-program index is empty for
    /// this fingerprint but the program is loaded; this keeps results
    /// consistent if the per-program index is ever evicted while the
    /// forward cache still has entries.
    pub fn results_for_program(&self, func: &Hash) -> Vec<Hash> {
        let hot = match self.lookup_program(func) {
            Some(h) => h,
            None => return Vec::new(),
        };
        let fp = hot.semantic_fingerprint;
        let primary = {
            let map = self.results_by_program.read().unwrap();
            map.get(&fp).map(|s| s.iter().copied().collect::<Vec<_>>())
        };
        if let Some(v) = primary {
            if !v.is_empty() {
                return v;
            }
        }
        // Fallback: scan the forward cache. O(N) in cache size but only
        // hit when the per-program index is missing this entry.
        let map = self.cache.read().unwrap();
        let mut out: HashSet<Hash> = HashSet::new();
        for (key, slot) in map.iter() {
            if let RamKey::Inline { program, .. } | RamKey::External { program, .. } = key {
                if program == &fp {
                    out.insert(slot.result);
                }
            }
        }
        out.into_iter().collect()
    }

    /// All `(CallKey, CallKey)` pairs that produced IDENTICAL results,
    /// capped at [`CONVERGENT_PAIRS_CAP`]. Each pair is emitted at most
    /// once (lexicographic order on the underlying 32-byte key).
    pub fn convergent_pairs(&self) -> Vec<(CallKey, CallKey)> {
        let map = self.reverse.read().unwrap();
        let mut out = Vec::new();
        for set in map.values() {
            if set.len() < 2 {
                continue;
            }
            let mut keys: Vec<CallKey> = set.iter().map(|k| k.to_call_key()).collect();
            // Stable order on the 32-byte CallKey representation so
            // pair output is deterministic across runs.
            keys.sort_by(|a, b| a.as_bytes().cmp(&b.as_bytes()));
            for i in 0..keys.len() {
                for j in (i + 1)..keys.len() {
                    if out.len() >= CONVERGENT_PAIRS_CAP {
                        return out;
                    }
                    out.push((keys[i].clone(), keys[j].clone()));
                }
            }
        }
        out
    }
}
