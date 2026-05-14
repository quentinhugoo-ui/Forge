//! Unified cache (Sprint 1 refactor).
//!
//! Before: four `Mutex<VecDeque<...>>` caches with O(n) linear scans
//! and SHA-256 dedup of every arg blob.
//!
//! After: one `RwLock<HashMap<CallKey, CacheSlot>>` for memos+results,
//! one `RwLock<HashMap<Hash, Arc<HotProgram>>>` for verified programs,
//! one `Mutex<VecDeque<CallKey>>` keeping only the LRU keys for
//! eviction ordering, and an `AtomicStats` block for contention-free
//! counting. The `args` cache is gone entirely — for the i64 hot path
//! the args live on the stack, and for larger payloads the per-call
//! `Hash::for_blob` cost is dwarfed by what follows.
//!
//! ## Tâche A.2 — Absence invariant (codified 2026-05-01, Phase Ω.10)
//!
//! **Cache absence is `Option<T>`, never `Result<T, io::Error>`.**
//!
//! A cache miss is *not* an error — it is the absence of information.
//! Mapping it to `io::Error` would conflate "I never computed this" with
//! "I tried to compute it and the disk exploded". The atlas has the same
//! invariant: a hit returns `Some`, a miss returns `None`. No fake error.
//!
//! All lookup APIs in this module return `Option<T>`:
//! - `lookup_call`, `lookup_wire`         → `Option<CacheSlot>`
//! - `lookup_program`                     → `Option<Arc<HotProgram>>`
//! - `programs_recent_keys`               → `Vec<...>` (empty = absent)
//!
//! All write APIs are infallible: `remember_call`, `remember_calls`,
//! `remember_program` either insert or silently refuse on budget pressure
//! (logged as `cache_denials`). They never propagate an `io::Error` for
//! the simple reason that nothing they do can produce one — no syscalls,
//! no parsing, no peer messaging.
//!
//! The regression test `tache_a2_absence_returns_none_never_err` at the
//! bottom of this file locks the invariant in : adding a new lookup that
//! returns `Result<_, io::Error>` for absence would be a doctrine
//! violation that must instead surface as `Option<_>` here.
//!
//! Real `Result<_, io::Error>` semantics live one layer up
//! (`exec.rs::call`, `hot_program`, `swarm::import`) where parsing,
//! disk IO, or peer-consistency failures are the actual error sources —
//! those are *not* absence and stay `Result`.

use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::sync::Arc;

use crate::{CallKey, Hash};

use super::hotplan::{HotPlan, HotProgram, MemoizedSubProgram};
use super::stats::MonsterStats;
use super::MonsterNode;

pub(super) const SLOT_OVERHEAD: usize = 96;
pub(super) const PROGRAM_OVERHEAD: usize = 256;
pub(super) const INLINE_ARG_LIMIT: usize = 16;
pub(super) const RAMKEY_INITIAL_CAPACITY: usize = 1024;

/// Identity hasher for `RamKey`. The key bytes are already either
/// SHA-256 entropy (`program` field is a semantic fingerprint) or
/// hashed through SHA-1 (`args_hash`), so re-hashing through SipHash
/// would only add cycles without improving collision resistance.
///
/// This hasher just folds the first 8 bytes of whatever was written
/// into a `u64`. `RamKey::hash` is overridden below to feed it the
/// most-entropic 8 bytes (the start of `program`, which is already a
/// uniformly-distributed fingerprint).
#[derive(Default)]
pub(super) struct IdentityHasher {
    state: u64,
    /// We only honour the first 8 bytes; everything else is mixed in
    /// cheaply with xor-rotate so the rare collision on the prefix
    /// still produces a different bucket.
    extra: u64,
}

impl Hasher for IdentityHasher {
    #[inline]
    fn finish(&self) -> u64 {
        // xor-rotate makes the tail bytes contribute without an
        // expensive multiply. For the inline RamKey case the prefix is
        // already SHA-256 quality so `extra` is essentially free.
        self.state ^ self.extra.rotate_left(17)
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            let take = bytes.len().min(i + 8) - i;
            let mut buf = [0u8; 8];
            buf[..take].copy_from_slice(&bytes[i..i + take]);
            let chunk = u64::from_le_bytes(buf);
            if self.state == 0 {
                self.state = chunk;
            } else {
                self.extra ^= chunk.rotate_left((i as u32 & 63).wrapping_mul(7));
            }
            i += take;
        }
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        if self.state == 0 {
            self.state = n;
        } else {
            self.extra ^= n;
        }
    }
}

pub(super) type IdentityBuildHasher = BuildHasherDefault<IdentityHasher>;

/// In-RAM cache key. Avoids the SHA-1+SHA-256 chain that `CallKey`
/// requires by composing the program's semantic fingerprint with the
/// arg bytes directly. Combined with `IdentityHasher` the lookup path
/// is ~5 ns instead of the ~30 ns SipHash baseline.
///
/// `CallKey` is still computed on cache miss for git persistence so the
/// on-disk memo format is unchanged.
///
/// V7 γ.1: aligned on a 64-byte cache line so a `RamKey` lookup never
/// straddles two cache lines. The largest variant (`Inline`) is 32 + 16
/// + 1 = 49 bytes, so without alignment the layout could place keys
/// across line boundaries — a HashMap iteration would then double its
/// L1 footprint. The cost of the alignment is +14 B per key on average,
/// trivial vs the cache-locality win.
#[repr(align(64))]
#[derive(Clone, PartialEq, Eq)]
pub(super) enum RamKey {
    Inline {
        program: [u8; 32],
        args: [u8; INLINE_ARG_LIMIT],
        len: u8,
    },
    External {
        program: [u8; 32],
        args_hash: [u8; 20],
    },
    /// Used when seeding the cache from a remote `CallKey` (swarm
    /// import) where we don't have the original args. Equality is on
    /// the raw 32-byte CallKey.
    Wire([u8; 32]),
}

impl std::hash::Hash for RamKey {
    /// Hand-rolled to feed `IdentityHasher` exactly 8 bytes of
    /// pre-mixed entropy. Every variant carries a 32-byte fingerprint
    /// (program for Inline/External, the CallKey for Wire) whose first
    /// 8 bytes are already uniformly distributed — perfect input for
    /// an identity hash.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            RamKey::Inline { program, args, len } => {
                let mut prefix = [0u8; 8];
                prefix.copy_from_slice(&program[..8]);
                let mut p = u64::from_le_bytes(prefix);
                // Fold the first 8 arg bytes (always present, padded
                // with zeros for shorter args) so two calls to the
                // same program with different args land in different
                // buckets without a second hasher round-trip.
                let mut a = [0u8; 8];
                a.copy_from_slice(&args[..8]);
                p ^= u64::from_le_bytes(a).rotate_left(*len as u32 & 63);
                state.write_u64(p);
            }
            RamKey::External { program, args_hash } => {
                let mut prefix = [0u8; 8];
                prefix.copy_from_slice(&program[..8]);
                let mut p = u64::from_le_bytes(prefix);
                let mut a = [0u8; 8];
                a.copy_from_slice(&args_hash[..8]);
                p ^= u64::from_le_bytes(a).rotate_left(13);
                state.write_u64(p);
            }
            RamKey::Wire(bytes) => {
                let mut prefix = [0u8; 8];
                prefix.copy_from_slice(&bytes[..8]);
                state.write_u64(u64::from_le_bytes(prefix));
            }
        }
    }
}

impl RamKey {
    pub(super) fn for_args(program: [u8; 32], args: &[u8]) -> Self {
        if args.len() <= INLINE_ARG_LIMIT {
            let mut buf = [0u8; INLINE_ARG_LIMIT];
            buf[..args.len()].copy_from_slice(args);
            RamKey::Inline {
                program,
                args: buf,
                len: args.len() as u8,
            }
        } else {
            // External args still get the SHA-1 once, but only on the
            // big-payload path which is rare and dominated by I/O
            // anyway.
            RamKey::External {
                program,
                args_hash: *Hash::for_blob(args).as_bytes(),
            }
        }
    }

    pub(super) fn for_args_hash(program: [u8; 32], args_hash: &Hash) -> Self {
        RamKey::External {
            program,
            args_hash: *args_hash.as_bytes(),
        }
    }

    pub(super) fn to_call_key(&self) -> CallKey {
        match self {
            RamKey::Inline { program, args, len } => {
                let args_hash = Hash::for_blob(&args[..*len as usize]);
                CallKey::from_program_identity(program, &args_hash)
            }
            RamKey::External { program, args_hash } => {
                let hash = Hash::from_bytes(*args_hash);
                CallKey::from_program_identity(program, &hash)
            }
            RamKey::Wire(bytes) => CallKey::from_hex(
                &bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            )
            .expect("wire bytes are 32 valid hex bytes"),
        }
    }

    pub(super) fn from_call_key(key: &CallKey) -> Self {
        RamKey::Wire(key.as_bytes())
    }

    /// Returns the underlying program semantic fingerprint when known.
    /// `Wire`-keyed entries (swarm-imported memos) don't carry it, so
    /// they're excluded from the per-program result index.
    pub(super) fn program_identity(&self) -> Option<[u8; 32]> {
        match self {
            RamKey::Inline { program, .. } => Some(*program),
            RamKey::External { program, .. } => Some(*program),
            RamKey::Wire(_) => None,
        }
    }
}

/// V7 γ.1: aligned 64 B so a single cache slot fits in one L1 cache
/// line. The struct is ~40 B (Hash 20 + Arc 16 + u32 4 + padding) so
/// the alignment costs no extra memory in practice.
#[repr(align(64))]
#[derive(Clone)]
pub(super) struct CacheSlot {
    pub(super) result: Hash,
    pub(super) bytes: Arc<[u8]>,
    pub(super) charged: u32,
}

/// Σ.7 (Phase Ω.10, 2026-05-01) — `AtomicU64` padded to one full
/// cache line (64 bytes) to eliminate **false sharing** between
/// hot counters incremented by different threads.
///
/// Without padding, all 17 counters in `AtomicStats` pack into 17×8 =
/// 136 bytes ≈ 2-3 cache lines. When 8 threads each increment a
/// different counter, every write invalidates the line on all other
/// cores → MOESI cache thrashing, ~100 ns per `fetch_add` instead of
/// ~5 ns. With padding, each counter owns its own line — writes are
/// fully independent.
///
/// Cost : `AtomicStats` grows from ~136 B to ~1088 B (17×64). One
/// instance per `MonsterNode`, so the cache footprint inflation is
/// negligible compared to the contention win.
///
/// Methods auto-deref to the inner `AtomicU64` (`Deref` impl), so
/// callers don't change : `stats.executions.fetch_add(1, Relaxed)`
/// still works verbatim.
#[repr(align(64))]
#[derive(Default)]
pub(super) struct PaddedAtomicU64(pub(super) AtomicU64);

impl std::ops::Deref for PaddedAtomicU64 {
    type Target = AtomicU64;
    #[inline(always)]
    fn deref(&self) -> &AtomicU64 {
        &self.0
    }
}

/// Atomic mirror of `MonsterStats`. Snapshots are produced on demand,
/// so the hot path never takes a mutex to bump a counter.
///
/// Σ.7 — every counter is `PaddedAtomicU64` (64-byte aligned) to
/// avoid false sharing between threads bumping different counters.
#[derive(Default)]
pub(super) struct AtomicStats {
    pub(super) program_cache_hits: PaddedAtomicU64,
    pub(super) program_cache_misses: PaddedAtomicU64,
    pub(super) arg_cache_hits: PaddedAtomicU64,
    pub(super) arg_cache_stores: PaddedAtomicU64,
    pub(super) ram_memo_hits: PaddedAtomicU64,
    pub(super) ram_value_hits: PaddedAtomicU64,
    pub(super) git_memo_hits: PaddedAtomicU64,
    pub(super) executions: PaddedAtomicU64,
    pub(super) cache_denials: PaddedAtomicU64,
    pub(super) batch_dedupe_hits: PaddedAtomicU64,
    pub(super) rule_hits: PaddedAtomicU64,
    pub(super) oracle_hits: PaddedAtomicU64,
    /// Phase 12.1 — Hash64 op-level memoization hits (the new
    /// sub-scale cache). Incremented every time `execute_with_op_memo`
    /// finds a cached `(Op::Hash64, input) → output` entry instead of
    /// recomputing. Same `Relaxed` ordering as the rest of the
    /// counters — this is observability, not a happens-before edge.
    pub(super) op_memo_hits: PaddedAtomicU64,
    pub(super) op_memo_misses: PaddedAtomicU64,
    /// Counter for shadow-execution sampling. `n % SHADOW_PERIOD == 0`
    /// triggers one interpreter validation against the oracle output.
    pub(super) shadow_counter: PaddedAtomicU64,
    /// Bumped whenever a shadow-execution validation diverged from the
    /// oracle and the oracle was therefore invalidated.
    pub(super) shadow_invalidations: PaddedAtomicU64,
    /// Number of synthetic probes the always-on distillation daemon
    /// has fired into the node. Each probe is a `call_one_i64` (or
    /// equivalent) on a program lacking a learned oracle, so the
    /// usual `observe_execution` plumbing accumulates the sample.
    pub(super) distillations_attempted: PaddedAtomicU64,
    /// Number of times the daemon's probing produced a freshly-learned
    /// oracle. Should monotonically grow as the node runs longer; a
    /// stall means either every program already has an oracle or none
    /// of them admits one.
    pub(super) distillations_succeeded: PaddedAtomicU64,
}

impl AtomicStats {
    pub(super) fn snapshot(&self) -> MonsterStats {
        MonsterStats {
            program_cache_hits: self.program_cache_hits.load(Ordering::Relaxed),
            program_cache_misses: self.program_cache_misses.load(Ordering::Relaxed),
            arg_cache_hits: self.arg_cache_hits.load(Ordering::Relaxed),
            arg_cache_stores: self.arg_cache_stores.load(Ordering::Relaxed),
            ram_memo_hits: self.ram_memo_hits.load(Ordering::Relaxed),
            ram_value_hits: self.ram_value_hits.load(Ordering::Relaxed),
            git_memo_hits: self.git_memo_hits.load(Ordering::Relaxed),
            executions: self.executions.load(Ordering::Relaxed),
            cache_denials: self.cache_denials.load(Ordering::Relaxed),
            batch_dedupe_hits: self.batch_dedupe_hits.load(Ordering::Relaxed),
            rule_hits: self.rule_hits.load(Ordering::Relaxed),
            oracle_hits: self.oracle_hits.load(Ordering::Relaxed),
            op_memo_hits: self.op_memo_hits.load(Ordering::Relaxed),
            op_memo_misses: self.op_memo_misses.load(Ordering::Relaxed),
            shadow_invalidations: self.shadow_invalidations.load(Ordering::Relaxed),
            distillations_attempted: self.distillations_attempted.load(Ordering::Relaxed),
            distillations_succeeded: self.distillations_succeeded.load(Ordering::Relaxed),
        }
    }
}

/// FNV-1a 64-bit. Used for batch-dedup fingerprints — *not* for any
/// persisted identity. Roughly 30× faster than SHA-256 for tiny inputs
/// (the i64 case takes ~5 ns vs ~200 ns).
pub(super) fn fast_fingerprint(bytes: &[u8]) -> u64 {
    crate::fast_hash::fast_fingerprint64(bytes)
}

impl MonsterNode {
    pub fn program_cache_len(&self) -> usize {
        self.programs.read().unwrap().len()
    }

    /// Retired in Sprint 1 (the standalone arg cache is gone). Kept
    /// returning 0 so external observers don't break.
    pub fn arg_cache_len(&self) -> usize {
        0
    }

    pub fn memo_cache_len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    pub fn result_cache_len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    pub(super) fn lookup_call(&self, key: &RamKey) -> Option<CacheSlot> {
        self.cache.read().unwrap().get(key).cloned()
    }

    /// Cache miss fallback used when the cheap `RamKey` lookup fails:
    /// computes the persistent `CallKey` and probes the `Wire` variant.
    /// Only callers that already paid the SHA-256 (e.g. swarm import or
    /// the cold `dispatch_call` path) should use this — the regular hot
    /// path should not.
    pub(super) fn lookup_wire(&self, call_key: &CallKey) -> Option<CacheSlot> {
        let wire = RamKey::Wire(call_key.as_bytes());
        self.cache.read().unwrap().get(&wire).cloned()
    }

    pub(super) fn remember_call(&self, key: RamKey, result: Hash, bytes: Arc<[u8]>) {
        let charged = bytes.len() + SLOT_OVERHEAD;
        if !self.reserve_with_eviction(charged) {
            self.stats_atomic.cache_denials.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let mut map = self.cache.write().unwrap();
        if map.contains_key(&key) {
            // Released the seat we reserved — the existing entry already
            // owns the budget.
            self.governor.release(charged);
            return;
        }
        map.insert(
            key.clone(),
            CacheSlot {
                result,
                bytes,
                charged: charged as u32,
            },
        );
        drop(map);
        self.lru.lock().unwrap().push_back(key.clone());

        // Reverse index bookkeeping. Skipped entirely on V7 default
        // (`new`/`shared`) so benches isolate the per-call overhead.
        if self.reverse_enabled {
            self.reverse
                .write()
                .unwrap()
                .entry(result)
                .or_insert_with(std::collections::HashSet::new)
                .insert(key.clone());
            if let Some(program) = key.program_identity() {
                self.results_by_program
                    .write()
                    .unwrap()
                    .entry(program)
                    .or_insert_with(std::collections::HashSet::new)
                    .insert(result);
            }
        }
    }

    /// Batched version of `remember_call` — single `cache.write()` and
    /// single `lru.lock()` for the whole input. Each entry is
    /// individually budget-checked (governor charges/releases stay
    /// per-entry), but the lock-acquisition overhead drops from N to 2.
    /// Used by `dispatch_batch` to ingest evaluator outputs.
    pub(super) fn remember_calls(&self, entries: Vec<(RamKey, Hash, Arc<[u8]>)>) {
        if entries.is_empty() {
            return;
        }

        let mut admitted: Vec<(RamKey, CacheSlot)> = Vec::with_capacity(entries.len());
        let mut admitted_for_lru: Vec<RamKey> = Vec::with_capacity(entries.len());
        let mut admitted_for_reverse: Vec<(RamKey, Hash)> = Vec::new();

        for (key, result, bytes) in entries.into_iter() {
            let charged = bytes.len() + SLOT_OVERHEAD;
            if !self.reserve_with_eviction(charged) {
                self.stats_atomic.cache_denials.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            admitted.push((
                key.clone(),
                CacheSlot {
                    result,
                    bytes,
                    charged: charged as u32,
                },
            ));
            admitted_for_lru.push(key.clone());
            if self.reverse_enabled {
                admitted_for_reverse.push((key, result));
            }
        }

        if admitted.is_empty() {
            return;
        }

        // Single write lock for all admitted entries. Pre-existing keys
        // refund their reservation — same semantics as `remember_call`.
        {
            let mut map = self.cache.write().unwrap();
            admitted.retain(|(key, slot)| {
                if map.contains_key(key) {
                    self.governor.release(slot.charged as usize);
                    false
                } else {
                    true
                }
            });
            for (key, slot) in admitted {
                map.insert(key, slot);
            }
        }

        {
            let mut lru = self.lru.lock().unwrap();
            for key in admitted_for_lru {
                lru.push_back(key);
            }
        }

        if self.reverse_enabled {
            let mut reverse = self.reverse.write().unwrap();
            let mut by_program = self.results_by_program.write().unwrap();
            for (key, result) in admitted_for_reverse {
                reverse
                    .entry(result)
                    .or_insert_with(std::collections::HashSet::new)
                    .insert(key.clone());
                if let Some(program) = key.program_identity() {
                    by_program
                        .entry(program)
                        .or_insert_with(std::collections::HashSet::new)
                        .insert(result);
                }
            }
        }
    }

    pub(super) fn lookup_program(&self, func: &Hash) -> Option<Arc<HotProgram>> {
        self.programs.read().unwrap().get(func).cloned()
    }

    pub(super) fn remember_program(
        &self,
        raw_hash: Hash,
        semantic_fingerprint: [u8; 32],
        plan: HotPlan,
        explicit_memos: Arc<[MemoizedSubProgram]>,
        program: Arc<crate::kasm::Program>,
        charged: usize,
    ) -> Arc<HotProgram> {
        let structure = super::hotplan::StructuralAnalysis::analyze(&program);
        let hot = Arc::new(HotProgram {
            semantic_fingerprint,
            program,
            plan,
            explicit_memos,
            jit: Mutex::new(None),
            jit_disabled: AtomicBool::new(false),
            charged: charged as u32,
            structure,
            inline_cache: AdaptiveInlineCache::new(),
        });
        if self.reserve_with_eviction(charged) {
            self.programs.write().unwrap().insert(raw_hash, Arc::clone(&hot));
        } else {
            self.stats_atomic.cache_denials.fetch_add(1, Ordering::Relaxed);
        }
        self.stats_atomic
            .program_cache_misses
            .fetch_add(1, Ordering::Relaxed);
        hot
    }

    fn reserve_with_eviction(&self, bytes: usize) -> bool {
        loop {
            if self.governor.try_reserve(bytes) {
                return true;
            }
            if !self.evict_one() {
                return false;
            }
        }
    }

    /// Drop one slot from the LRU. Programs are last-resort because
    /// they're more expensive to rebuild (verify + simplify + plan).
    fn evict_one(&self) -> bool {
        let evicted_key = self.lru.lock().unwrap().pop_front();
        if let Some(key) = evicted_key {
            let mut map = self.cache.write().unwrap();
            if let Some(slot) = map.remove(&key) {
                self.governor.release(slot.charged as usize);
                drop(map);
                if self.reverse_enabled {
                    let mut reverse = self.reverse.write().unwrap();
                    let drop_entry = if let Some(set) = reverse.get_mut(&slot.result) {
                        set.remove(&key);
                        set.is_empty()
                    } else {
                        false
                    };
                    if drop_entry {
                        reverse.remove(&slot.result);
                    }
                    drop(reverse);
                    if let Some(program) = key.program_identity() {
                        // Cheap consistency check: only purge the
                        // per-program result entry if no other live
                        // forward-cache entry for that program still
                        // produces this result. Skipped on the
                        // assumption that the cache is large compared
                        // to the eviction rate; over-purging is
                        // recovered by the `results_for_program`
                        // fallback scan.
                        let still_referenced = {
                            let cache = self.cache.read().unwrap();
                            cache.iter().any(|(k, s)| {
                                k.program_identity() == Some(program)
                                    && s.result == slot.result
                            })
                        };
                        if !still_referenced {
                            let mut by_prog = self.results_by_program.write().unwrap();
                            let drop_program = if let Some(set) = by_prog.get_mut(&program)
                            {
                                set.remove(&slot.result);
                                set.is_empty()
                            } else {
                                false
                            };
                            if drop_program {
                                by_prog.remove(&program);
                            }
                        }
                    }
                }
                return true;
            }
        }
        // Last resort: drop the oldest program. There is no LRU on
        // programs yet, so we just take the smallest.
        let mut programs = self.programs.write().unwrap();
        if let Some(key) = programs.keys().next().cloned() {
            if let Some(prog) = programs.remove(&key) {
                self.governor.release(prog.charged as usize);
                return true;
            }
        }
        false
    }

    pub(super) fn programs_recent_keys(&self, limit: usize) -> Vec<(CallKey, Hash)> {
        let map = self.cache.read().unwrap();
        self.lru
            .lock()
            .unwrap()
            .iter()
            .rev()
            .take(limit)
            .filter_map(|k| map.get(k).map(|slot| (k.to_call_key(), slot.result)))
            .collect()
    }
}

// ----- Φ.μ.7 — InlineCache L0 (5-10 ns hit) + StridePredictor -----
//
// `InlineCache` est un cache direct-mapped 64 slots × 64 octets (4 KB
// par programme, fit L1) protégé par un protocole SeqLock lock-free
// sur `tag.fetch_add()`. Le hot path est `try_match_i64` :
// 1 charge cache-line + 1 comparaison u64 + 2 charges atomiques sur
// le tag = ~5-10 ns quand L1-resident.
//
// Tag `0` réservé pour "jamais armé" (évite faux match sur args=0).
// Writer : tag.fetch_add(1) → odd (in-flight) → store args+result →
// tag.fetch_add(1) → even (stable, ≥ 2). Reader : tag1, args, result,
// tag2 (Acquire) ; reject si odd ou si tag1 != tag2.

pub const INLINE_SLOTS: usize = 64;
pub const INLINE_MASK: u64 = (INLINE_SLOTS as u64) - 1;

#[repr(C, align(64))]
pub struct PredictedSlot {
    tag: AtomicU64,
    predicted_args: AtomicU64,
    predicted_result: AtomicU64,
    _pad: [u8; 40],
}

impl PredictedSlot {
    const fn new() -> Self {
        Self {
            tag: AtomicU64::new(0),
            predicted_args: AtomicU64::new(0),
            predicted_result: AtomicU64::new(0),
            _pad: [0; 40],
        }
    }
}

/// Cache direct-mapped 64 slots, lock-free, ~5-10 ns par lookup.
/// Une instance par `HotProgram` ; les programmes ne se polluent pas
/// entre eux. Sequential / short-cycle workloads y trouvent leur place
/// sans collision.
pub struct InlineCache {
    slots: Box<[PredictedSlot]>,
}

impl InlineCache {
    pub fn new() -> Self {
        let mut v: Vec<PredictedSlot> = Vec::with_capacity(INLINE_SLOTS);
        for _ in 0..INLINE_SLOTS {
            v.push(PredictedSlot::new());
        }
        Self {
            slots: v.into_boxed_slice(),
        }
    }

    #[inline(always)]
    pub fn try_match_i64(&self, args: u64) -> Option<u64> {
        let slot = unsafe { self.slots.get_unchecked((args & INLINE_MASK) as usize) };
        let tag1 = slot.tag.load(Ordering::Acquire);
        if tag1 == 0 || (tag1 & 1) != 0 {
            return None;
        }
        let pred_args = slot.predicted_args.load(Ordering::Relaxed);
        if pred_args != args {
            return None;
        }
        let result = slot.predicted_result.load(Ordering::Relaxed);
        let tag2 = slot.tag.load(Ordering::Acquire);
        if tag1 != tag2 {
            return None;
        }
        Some(result)
    }

    #[inline]
    pub fn arm_i64(&self, args: u64, result: u64) {
        let slot = unsafe { self.slots.get_unchecked((args & INLINE_MASK) as usize) };
        slot.tag.fetch_add(1, Ordering::AcqRel);
        slot.predicted_args.store(args, Ordering::Relaxed);
        slot.predicted_result.store(result, Ordering::Relaxed);
        slot.tag.fetch_add(1, Ordering::AcqRel);
    }
}

impl Default for InlineCache {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Φ.ν.7g — AdaptiveInlineCache : L0 avec auto-disable ────────────
//
// Wrapper autour d'`InlineCache` qui apprend en ligne SI ce L0 paye
// pour un programme donné. Mécanisme :
//
//   1. Pendant le warmup (les WARMUP_CALLS premières exécutions), on
//      probe + arm comme un L0 normal. On compte (probes, hits).
//   2. Après warmup, on calcule hit_rate = hits / probes.
//   3. Si hit_rate >= ENABLE_THRESHOLD (5%), le L0 reste actif
//      indéfiniment (les workloads à args répétés gagnent).
//   4. Si hit_rate < ENABLE_THRESHOLD, on DÉSACTIVE complètement le
//      L0 pour ce programme : plus aucun probe ni arm. Hot path
//      retourne immédiatement à la cascade existante (zéro overhead).
//
// Ça résout le piège mesuré sur DNA bench (commit b1d6bb7) : 100k
// args uniques → 0% hit → +30% latence due au probe wasted. Avec
// auto-disable, après 100 calls on désactive et le bypass HashChain
// reprend sa pleine vitesse.
//
// Sur le workload reverse_synth (millions de candidats KASM évalués
// sur les MÊMES features_i64 d'une bougie de décision), le hit rate
// devrait être très haut (50-90%) → L0 reste actif et accélère.

const WARMUP_CALLS: u64 = 100;
const ENABLE_THRESHOLD_NUMERATOR: u64 = 5; // 5% = 5/100
const ENABLE_THRESHOLD_DENOMINATOR: u64 = 100;

/// État adaptatif du L0 : 0 = warmup, 1 = active, 2 = disabled.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L0Mode {
    Warmup = 0,
    Active = 1,
    Disabled = 2,
}

pub struct AdaptiveInlineCache {
    inner: InlineCache,
    /// Compteur (probes_low_32 | hits_high_32). Un seul AtomicU64 pour
    /// éviter false sharing sur 2 atomics adjacents.
    counters: AtomicU64,
    /// Mode courant. 0=warmup, 1=active, 2=disabled. Lit en Relaxed
    /// (drift acceptable, on évalue à chaque call).
    mode: std::sync::atomic::AtomicU8,
}

impl AdaptiveInlineCache {
    pub fn new() -> Self {
        Self {
            inner: InlineCache::new(),
            counters: AtomicU64::new(0),
            mode: std::sync::atomic::AtomicU8::new(L0Mode::Warmup as u8),
        }
    }

    /// Probe le L0 si actif. Retourne `Some(result)` sur hit, `None`
    /// sinon (miss ou disabled). Met à jour les compteurs en warmup.
    #[inline]
    pub fn try_match_i64(&self, args: u64) -> Option<u64> {
        let mode = self.mode.load(Ordering::Relaxed);
        if mode == L0Mode::Disabled as u8 {
            return None;
        }
        let result = self.inner.try_match_i64(args);
        if mode == L0Mode::Warmup as u8 {
            // Atomically bump (probes++, hits += result.is_some())
            let delta = if result.is_some() { (1u64 << 32) | 1 } else { 1 };
            let prev = self.counters.fetch_add(delta, Ordering::Relaxed);
            let probes = (prev & 0xFFFF_FFFF) + 1;
            // Évaluation à warmup_done (single transition)
            if probes == WARMUP_CALLS {
                let hits = (prev >> 32) + if result.is_some() { 1 } else { 0 };
                let active = hits * ENABLE_THRESHOLD_DENOMINATOR
                    >= probes * ENABLE_THRESHOLD_NUMERATOR;
                let new_mode = if active { L0Mode::Active } else { L0Mode::Disabled };
                self.mode.store(new_mode as u8, Ordering::Relaxed);
            }
        }
        result
    }

    /// Arm le L0 si actif. No-op si disabled (évite tout coût atomique
    /// inutile). En warmup ou active, le slot est mis à jour.
    #[inline]
    pub fn arm_i64(&self, args: u64, result: u64) {
        let mode = self.mode.load(Ordering::Relaxed);
        if mode == L0Mode::Disabled as u8 {
            return;
        }
        self.inner.arm_i64(args, result);
    }

    #[cfg(test)]
    pub fn mode(&self) -> L0Mode {
        match self.mode.load(Ordering::Relaxed) {
            0 => L0Mode::Warmup,
            1 => L0Mode::Active,
            _ => L0Mode::Disabled,
        }
    }
}

impl Default for AdaptiveInlineCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod adaptive_inline_cache_tests {
    use super::*;

    #[test]
    fn warmup_then_active_when_hit_rate_high() {
        let c = AdaptiveInlineCache::new();
        // Arm one slot, then probe it WARMUP_CALLS times → 100% hit rate
        c.arm_i64(42, 1729);
        for _ in 0..WARMUP_CALLS {
            assert_eq!(c.try_match_i64(42), Some(1729));
        }
        // Mode should now be Active (was reset on the WARMUP_CALLS-th probe)
        assert_eq!(c.mode(), L0Mode::Active);
    }

    #[test]
    fn warmup_then_disabled_when_no_hits() {
        let c = AdaptiveInlineCache::new();
        // Probe with always-different args, never armed → 0% hit rate
        for i in 0..WARMUP_CALLS {
            assert_eq!(c.try_match_i64(0x1000_0000_0000_0000 + i), None);
        }
        assert_eq!(c.mode(), L0Mode::Disabled);
        // Arm + probe : disabled L0 ignores arm, returns None on probe
        c.arm_i64(99, 1234);
        assert_eq!(c.try_match_i64(99), None);
    }

    #[test]
    fn disabled_persists_after_arm_attempts() {
        let c = AdaptiveInlineCache::new();
        // Force disabled via 100 misses
        for i in 0..WARMUP_CALLS {
            c.try_match_i64(0xDEAD_BEEF_0000_0000 + i);
        }
        assert_eq!(c.mode(), L0Mode::Disabled);
        // Even after 1000 arms, mode stays disabled
        for i in 0..1000 {
            c.arm_i64(i, i * 10);
        }
        assert_eq!(c.mode(), L0Mode::Disabled);
        assert_eq!(c.try_match_i64(0), None);
    }
}

/// Détecteur de stride : observe les deltas successifs ; quand deux
/// deltas consécutifs s'accordent, prédit le prochain accès comme
/// `current + delta`. Généralise les progressions arithmétiques que
/// le 1er-ordre Markov ne capte qu'après transition répétée. ~30 lignes,
/// zéro alloc, consultable depuis le hot path Atlas.
pub struct StridePredictor {
    last: Option<i64>,
    delta: Option<i64>,
    run: u32,
}

impl StridePredictor {
    pub fn new() -> Self {
        Self {
            last: None,
            delta: None,
            run: 0,
        }
    }

    /// Observe `value`, met à jour l'état, et retourne la prédiction du
    /// PROCHAIN appel quand `run >= min_run`. `min_run = 2` est un bon
    /// défaut (stride confirmé par 3 valeurs consécutives).
    pub fn observe(&mut self, value: i64, min_run: u32) -> Option<i64> {
        let prediction = match (self.last, self.delta) {
            (Some(_), Some(d)) if self.run >= min_run => Some(value.wrapping_add(d)),
            _ => None,
        };
        if let Some(prev) = self.last {
            let new_delta = value.wrapping_sub(prev);
            if Some(new_delta) == self.delta {
                self.run = self.run.saturating_add(1);
            } else {
                self.delta = Some(new_delta);
                self.run = 1;
            }
        }
        self.last = Some(value);
        prediction
    }

    pub fn reset(&mut self) {
        self.last = None;
        self.delta = None;
        self.run = 0;
    }
}

impl Default for StridePredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod inline_cache_tests {
    use super::*;

    #[test]
    fn fresh_slot_misses_on_zero_args() {
        // Régression : un slot non-armé (tag=0) ne doit JAMAIS retourner
        // Some pour args=0, sinon le default-zero des champs ferait un
        // faux hit.
        let cache = InlineCache::new();
        assert!(cache.try_match_i64(0).is_none());
    }

    #[test]
    fn arm_then_match_returns_armed_value() {
        let cache = InlineCache::new();
        cache.arm_i64(42, 1729);
        assert_eq!(cache.try_match_i64(42), Some(1729));
    }

    #[test]
    fn miss_on_wrong_args_same_slot() {
        let cache = InlineCache::new();
        cache.arm_i64(42, 1729);
        // 42 + INLINE_SLOTS atterrit dans le même slot direct-mapped.
        assert!(cache.try_match_i64(42 + INLINE_SLOTS as u64).is_none());
    }

    #[test]
    fn distinct_slots_dont_collide() {
        let cache = InlineCache::new();
        cache.arm_i64(1, 100);
        cache.arm_i64(2, 200);
        assert_eq!(cache.try_match_i64(1), Some(100));
        assert_eq!(cache.try_match_i64(2), Some(200));
    }

    #[test]
    fn stride_predicts_arithmetic_progression() {
        // 3 valeurs nécessaires pour confirmer le delta (run >= 2),
        // donc la première prédiction non-None apparaît à la 4e
        // observation. C'est précisément la généralisation que Markov
        // 1er-ordre ne peut pas faire avec min_obs=2.
        let mut p = StridePredictor::new();
        assert_eq!(p.observe(0, 2), None);
        assert_eq!(p.observe(1, 2), None); // delta=1, run=1
        assert_eq!(p.observe(2, 2), None); // delta=1, run=2 (mais check avant update)
        assert_eq!(p.observe(3, 2), Some(4)); // run=2 entrée → prédit 3+1
        assert_eq!(p.observe(4, 2), Some(5));
    }

    #[test]
    fn stride_resets_on_pattern_break() {
        let mut p = StridePredictor::new();
        p.observe(0, 2);
        p.observe(2, 2);
        p.observe(4, 2); // run=2 sur delta=2
        // observe(5) : check passe avec ancien delta=2 → prédit 5+2=7 ;
        // après l'update, new_delta=1 != Some(2), reset run=1.
        assert_eq!(p.observe(5, 2), Some(7));
        assert_eq!(p.observe(6, 2), None); // delta=1, run=1 < 2
    }
}

#[cfg(test)]
mod memoize_tests {
    use std::path::PathBuf;
    

    use crate::kasm::{Node, Program, Target, Ty};
    use crate::{Hash, MemoryGovernor, Store};

    use super::*;

    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("scan-cache", tag)
    }

    #[test]
    fn test_memoize_forces_cache() {
        let path = fresh_path("memoize");
        let monster = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let program = Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(2),
                Node::mul(0, 1),
                Node::memoize(2),
                Node::const_i64(5),
                Node::add(3, 4),
                Node::output(5, Ty::I64),
            ],
        )
        .unwrap();
        let func = monster.store().store(program.bytes()).unwrap();
        let args = 17i64.to_le_bytes();

        let before = monster.memo_cache_len();
        let _ = monster.call_bytes(&func, &args).unwrap();
        let after = monster.memo_cache_len();

        let memoized = program.memoize_subprograms().unwrap().pop().unwrap();
        let memoized_fp = if super::super::hotplan::should_semantic_fingerprint(&memoized) {
            memoized.semantic_fingerprint().unwrap()
        } else {
            let canonical_hash = Hash::for_blob(memoized.bytes());
            super::super::hotplan::exact_program_identity(&memoized, &canonical_hash)
        };
        let memoized_key = RamKey::for_args(memoized_fp, &args);

        assert!(
            monster.lookup_call(&memoized_key).is_some(),
            "Op::Memoize must force a RamMemo entry for the wrapped slot"
        );
        assert!(
            after >= before + 2,
            "expected final result + explicit memoized subprogram to be cached"
        );

        let _ = std::fs::remove_dir_all(&path);
    }

    /// Tâche A.2 (Phase Ω.10) — locks in the invariant that cache absence
    /// surfaces as `None`, never as a synthetic `io::Error`. Every public
    /// lookup in this module must return `Option<T>` for "not cached" so
    /// callers can branch cheaply without parsing an error message.
    ///
    /// If a future refactor changes a `lookup_*` to return
    /// `Result<_, io::Error>` for the miss case, this test still passes
    /// (because `Result<Option<_>, _>` would still let `None` through the
    /// Ok arm), but the *type signatures asserted below* would refuse to
    /// compile — that's the actual lock-in. The runtime asserts double as
    /// smoke tests for the no-side-effect contract.
    #[test]
    fn tache_a2_absence_returns_none_never_err() {
        let path = fresh_path("absence");
        let monster = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );

        // Inputs that are deliberately *not* in any cache.
        let absent_program: [u8; 32] = [0xAB; 32];
        let absent_args = 99_999_999i64.to_le_bytes();
        let absent_ram_key = RamKey::for_args(absent_program, &absent_args);
        let absent_call_key = absent_ram_key.to_call_key();
        let absent_func_hash = Hash::from_bytes([0xCD; 20]);

        // 1. lookup_call → Option<CacheSlot>, miss = None.
        //    Compile-time: `let _: Option<CacheSlot> = ...;` would fail if
        //    the signature ever became `Result<...>`.
        let result_call: Option<CacheSlot> = monster.lookup_call(&absent_ram_key);
        assert!(
            result_call.is_none(),
            "Tâche A.2: lookup_call on absent RamKey must yield None"
        );

        // 2. lookup_wire → Option<CacheSlot>, miss = None.
        let result_wire: Option<CacheSlot> = monster.lookup_wire(&absent_call_key);
        assert!(
            result_wire.is_none(),
            "Tâche A.2: lookup_wire on absent CallKey must yield None"
        );

        // 3. lookup_program → Option<Arc<HotProgram>>, miss = None.
        let result_program: Option<Arc<HotProgram>> = monster.lookup_program(&absent_func_hash);
        assert!(
            result_program.is_none(),
            "Tâche A.2: lookup_program on absent func hash must yield None"
        );

        // 4. programs_recent_keys → Vec<...>, no entries = empty Vec.
        //    Empty vec is the absence representation here (also Option-y
        //    in spirit — collection cardinality 0 = absence).
        let recent: Vec<(crate::CallKey, Hash)> = monster.programs_recent_keys(16);
        assert!(
            recent.is_empty(),
            "Tâche A.2: programs_recent_keys on empty cache must return empty Vec"
        );

        // 5. Counters all zero = no side effects from absence probing.
        //    A Result-flavoured impl might bump cache_denials or worse;
        //    the Option contract guarantees lookups are read-only.
        let stats = monster.stats();
        assert_eq!(stats.cache_denials, 0);
        assert_eq!(stats.program_cache_hits, 0);
        assert_eq!(stats.program_cache_misses, 0);

        let _ = std::fs::remove_dir_all(&path);
    }
}

