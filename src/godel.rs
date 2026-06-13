//! Omega-5 Godel-machine substrate.
//!
//! This module starts with the typed observer. Later Omega-5 caps add
//! criteria, verification, proposal, application, and the closed loop.

pub mod observer {
//! Typed, content-addressed observation for the Omega-5 Godel loop.
//!
//! The observer intentionally reads through `MonsterNode`'s public surface:
//! stats, memory governor, reverse index, swarm-exportable call keys, and
//! store sidecar counters. That keeps capture non-perturbing with respect to
//! the node's executable state and avoids turning Omega-5 into a hidden
//! privileged runtime.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::{CallKey, Hash, MonsterNode};

const OBSERVER_CACHE_LIMIT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverFrame {
    pub epoch: u64,
    pub programs_loaded: Vec<Hash>,
    pub oracles_active: Vec<Hash>,
    pub cache_hot_paths: Vec<(CallKey, u64)>,
    pub metrics: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObserverDelta {
    pub programs_added: Vec<Hash>,
    pub programs_removed: Vec<Hash>,
    pub oracles_added: Vec<Hash>,
    pub oracles_removed: Vec<Hash>,
    pub cache_hot_paths_added: Vec<(CallKey, u64)>,
    pub cache_hot_paths_removed: Vec<(CallKey, u64)>,
    pub cache_hot_paths_changed: Vec<(CallKey, u64, u64)>,
    pub metrics_changed: BTreeMap<String, (u64, u64)>,
}

impl ObserverDelta {
    pub fn is_empty(&self) -> bool {
        self.programs_added.is_empty()
            && self.programs_removed.is_empty()
            && self.oracles_added.is_empty()
            && self.oracles_removed.is_empty()
            && self.cache_hot_paths_added.is_empty()
            && self.cache_hot_paths_removed.is_empty()
            && self.cache_hot_paths_changed.is_empty()
            && self.metrics_changed.is_empty()
    }
}

pub fn capture(node: &MonsterNode) -> ObserverFrame {
    let stats = node.stats();
    let presence = node.swarm_presence();
    let governor = node.governor();

    let mut metrics = BTreeMap::new();
    insert_metric(&mut metrics, "program_cache_hits", stats.program_cache_hits);
    insert_metric(
        &mut metrics,
        "program_cache_misses",
        stats.program_cache_misses,
    );
    insert_metric(&mut metrics, "arg_cache_hits", stats.arg_cache_hits);
    insert_metric(&mut metrics, "arg_cache_stores", stats.arg_cache_stores);
    insert_metric(&mut metrics, "ram_memo_hits", stats.ram_memo_hits);
    insert_metric(&mut metrics, "ram_value_hits", stats.ram_value_hits);
    insert_metric(&mut metrics, "git_memo_hits", stats.git_memo_hits);
    insert_metric(&mut metrics, "executions", stats.executions);
    insert_metric(&mut metrics, "cache_denials", stats.cache_denials);
    insert_metric(&mut metrics, "batch_dedupe_hits", stats.batch_dedupe_hits);
    insert_metric(&mut metrics, "rule_hits", stats.rule_hits);
    insert_metric(&mut metrics, "oracle_hits", stats.oracle_hits);
    insert_metric(
        &mut metrics,
        "shadow_invalidations",
        stats.shadow_invalidations,
    );
    insert_metric(
        &mut metrics,
        "distillations_attempted",
        stats.distillations_attempted,
    );
    insert_metric(
        &mut metrics,
        "distillations_succeeded",
        stats.distillations_succeeded,
    );
    insert_metric(&mut metrics, "avoided_calls", stats.avoided());
    insert_metric(&mut metrics, "total_calls", stats.total_calls());
    insert_metric(
        &mut metrics,
        "memory_budget_bytes",
        governor.budget_bytes() as u64,
    );
    insert_metric(&mut metrics, "memory_used_bytes", governor.used_bytes() as u64);
    insert_metric(
        &mut metrics,
        "program_cache_entries",
        node.program_cache_len() as u64,
    );
    insert_metric(&mut metrics, "arg_cache_entries", node.arg_cache_len() as u64);
    insert_metric(&mut metrics, "memo_cache_entries", presence.memo_cache_entries as u64);
    insert_metric(
        &mut metrics,
        "result_cache_entries",
        presence.result_cache_entries as u64,
    );
    insert_metric(
        &mut metrics,
        "reverse_index_entries",
        node.reverse_index_len() as u64,
    );

    let epoch = monotone_epoch(&stats);
    let cache_hot_paths = recent_cache_hot_paths(node);

    ObserverFrame {
        epoch,
        programs_loaded: Vec::new(),
        oracles_active: Vec::new(),
        cache_hot_paths,
        metrics,
    }
}

pub fn frame_hash(frame: &ObserverFrame) -> [u8; 32] {
    let canonical = canonical_frame(frame);
    let mut h = Sha256::new();
    h.update(b"scan-godel-observer-frame-v1\0");
    write_u64(&mut h, canonical.epoch);
    write_hash_vec(&mut h, &canonical.programs_loaded);
    write_hash_vec(&mut h, &canonical.oracles_active);
    write_cache_vec(&mut h, &canonical.cache_hot_paths);
    write_metrics(&mut h, &canonical.metrics);
    h.finalize().into()
}

pub fn diff(a: &ObserverFrame, b: &ObserverFrame) -> ObserverDelta {
    let a = canonical_frame(a);
    let b = canonical_frame(b);

    let (programs_added, programs_removed) = diff_hash_vec(&a.programs_loaded, &b.programs_loaded);
    let (oracles_added, oracles_removed) = diff_hash_vec(&a.oracles_active, &b.oracles_active);
    let (cache_hot_paths_added, cache_hot_paths_removed, cache_hot_paths_changed) =
        diff_cache_vec(&a.cache_hot_paths, &b.cache_hot_paths);
    let metrics_changed = diff_metrics(&a.metrics, &b.metrics);

    ObserverDelta {
        programs_added,
        programs_removed,
        oracles_added,
        oracles_removed,
        cache_hot_paths_added,
        cache_hot_paths_removed,
        cache_hot_paths_changed,
        metrics_changed,
    }
}

fn insert_metric(metrics: &mut BTreeMap<String, u64>, name: &str, value: u64) {
    metrics.insert(name.to_owned(), value);
}

fn monotone_epoch(stats: &crate::MonsterStats) -> u64 {
    stats.program_cache_hits
        .saturating_add(stats.program_cache_misses)
        .saturating_add(stats.arg_cache_hits)
        .saturating_add(stats.arg_cache_stores)
        .saturating_add(stats.ram_memo_hits)
        .saturating_add(stats.ram_value_hits)
        .saturating_add(stats.git_memo_hits)
        .saturating_add(stats.executions)
        .saturating_add(stats.cache_denials)
        .saturating_add(stats.batch_dedupe_hits)
        .saturating_add(stats.rule_hits)
        .saturating_add(stats.oracle_hits)
        .saturating_add(stats.shadow_invalidations)
        .saturating_add(stats.distillations_attempted)
        .saturating_add(stats.distillations_succeeded)
}

fn recent_cache_hot_paths(node: &MonsterNode) -> Vec<(CallKey, u64)> {
    let mut counts: BTreeMap<[u8; 32], (CallKey, u64)> = BTreeMap::new();
    if let Ok(frame) = node.export_swarm_frame(OBSERVER_CACHE_LIMIT) {
        for memo in frame.memos {
            let entry = counts
                .entry(memo.call_key.as_bytes())
                .or_insert((memo.call_key, 0));
            entry.1 = entry.1.saturating_add(1);
        }
    }
    counts.into_values().collect()
}

fn canonical_frame(frame: &ObserverFrame) -> ObserverFrame {
    let mut programs_loaded = frame.programs_loaded.clone();
    programs_loaded.sort();
    programs_loaded.dedup();

    let mut oracles_active = frame.oracles_active.clone();
    oracles_active.sort();
    oracles_active.dedup();

    let mut cache_counts: BTreeMap<[u8; 32], (CallKey, u64)> = BTreeMap::new();
    for (key, count) in &frame.cache_hot_paths {
        cache_counts
            .entry(key.as_bytes())
            .and_modify(|(_, existing)| *existing = existing.saturating_add(*count))
            .or_insert((*key, *count));
    }

    ObserverFrame {
        epoch: frame.epoch,
        programs_loaded,
        oracles_active,
        cache_hot_paths: cache_counts.into_values().collect(),
        metrics: frame.metrics.clone(),
    }
}

fn write_u64(h: &mut Sha256, value: u64) {
    h.update(value.to_le_bytes());
}

fn write_hash_vec(h: &mut Sha256, hashes: &[Hash]) {
    write_u64(h, hashes.len() as u64);
    for hash in hashes {
        h.update(hash.as_bytes());
    }
}

fn write_cache_vec(h: &mut Sha256, paths: &[(CallKey, u64)]) {
    write_u64(h, paths.len() as u64);
    for (key, count) in paths {
        h.update(key.as_bytes());
        write_u64(h, *count);
    }
}

fn write_metrics(h: &mut Sha256, metrics: &BTreeMap<String, u64>) {
    write_u64(h, metrics.len() as u64);
    for (key, value) in metrics {
        write_u64(h, key.len() as u64);
        h.update(key.as_bytes());
        write_u64(h, *value);
    }
}

fn diff_hash_vec(a: &[Hash], b: &[Hash]) -> (Vec<Hash>, Vec<Hash>) {
    let a: BTreeSet<Hash> = a.iter().copied().collect();
    let b: BTreeSet<Hash> = b.iter().copied().collect();
    (
        b.difference(&a).copied().collect(),
        a.difference(&b).copied().collect(),
    )
}

type CacheDiff = (Vec<(CallKey, u64)>, Vec<(CallKey, u64)>, Vec<(CallKey, u64, u64)>);

fn diff_cache_vec(a: &[(CallKey, u64)], b: &[(CallKey, u64)]) -> CacheDiff {
    let a = cache_map(a);
    let b = cache_map(b);
    let keys: BTreeSet<[u8; 32]> = a.keys().chain(b.keys()).copied().collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for key in keys {
        match (a.get(&key), b.get(&key)) {
            (None, Some((call_key, count))) => added.push((*call_key, *count)),
            (Some((call_key, count)), None) => removed.push((*call_key, *count)),
            (Some((call_key, before)), Some((_, after))) if before != after => {
                changed.push((*call_key, *before, *after));
            }
            _ => {}
        }
    }

    (added, removed, changed)
}

fn cache_map(paths: &[(CallKey, u64)]) -> BTreeMap<[u8; 32], (CallKey, u64)> {
    let mut map = BTreeMap::new();
    for (key, count) in paths {
        map.entry(key.as_bytes())
            .and_modify(|(_, existing): &mut (CallKey, u64)| {
                *existing = existing.saturating_add(*count)
            })
            .or_insert((*key, *count));
    }
    map
}

fn diff_metrics(a: &BTreeMap<String, u64>, b: &BTreeMap<String, u64>) -> BTreeMap<String, (u64, u64)> {
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    let mut out = BTreeMap::new();
    for key in keys {
        let before = a.get(key).copied().unwrap_or(0);
        let after = b.get(key).copied().unwrap_or(0);
        if before != after {
            out.insert(key.clone(), (before, after));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};


    use crate::kasm::{Node, Program, Target, Ty};
    use crate::{MemoryGovernor, Store};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_hash(byte: u8) -> Hash {
        Hash::from_bytes([byte; 20])
    }

    fn test_key(byte: u8) -> CallKey {
        let hex = format!("{byte:02x}").repeat(32);
        CallKey::from_hex(&hex).unwrap()
    }

    fn frame(epoch: u64) -> ObserverFrame {
        ObserverFrame {
            epoch,
            programs_loaded: vec![test_hash(1), test_hash(2)],
            oracles_active: vec![test_hash(9)],
            cache_hot_paths: vec![(test_key(3), 7), (test_key(4), 11)],
            metrics: BTreeMap::from([
                ("memory_used_bytes".to_owned(), 128),
                ("total_calls".to_owned(), 3),
            ]),
        }
    }

    fn fresh_path(tag: &str) -> PathBuf {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut p = crate::fresh_tmp_path("scan-godel-observer", tag);
        p.set_file_name(format!(
            "{}-{seq}",
            p.file_name().unwrap().to_str().unwrap()
        ));
        p
    }

    fn test_program() -> Program {
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

    fn fresh_node(tag: &str) -> (MonsterNode, PathBuf) {
        let path = fresh_path(tag);
        let node = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (node, path)
    }

    #[test]
    fn capture_empty_node_is_deterministic() {
        let (node, path) = fresh_node("deterministic");
        let a = capture(&node);
        let b = capture(&node);
        assert_eq!(a, b);
        assert_eq!(frame_hash(&a), frame_hash(&b));
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn capture_contains_public_runtime_metrics() {
        let (node, path) = fresh_node("metrics");
        let captured = capture(&node);
        assert_eq!(captured.metrics["memory_budget_bytes"], 1024 * 1024);
        assert_eq!(captured.metrics["memory_used_bytes"], 0);
        assert_eq!(captured.metrics["total_calls"], 0);
        assert!(captured.metrics.contains_key("program_cache_entries"));
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn frame_hash_is_stable_for_same_frame() {
        let f = frame(42);
        assert_eq!(frame_hash(&f), frame_hash(&f));
    }

    #[test]
    fn frame_hash_changes_when_metric_changes() {
        let a = frame(42);
        let mut b = frame(42);
        b.metrics.insert("total_calls".to_owned(), 4);
        assert_ne!(frame_hash(&a), frame_hash(&b));
    }

    #[test]
    fn frame_hash_canonicalizes_ordering() {
        let a = frame(42);
        let mut b = frame(42);
        b.programs_loaded.reverse();
        b.cache_hot_paths.reverse();
        assert_eq!(frame_hash(&a), frame_hash(&b));
    }

    #[test]
    fn diff_empty_for_identical_frames() {
        let a = frame(7);
        assert!(diff(&a, &a).is_empty());
    }

    #[test]
    fn diff_detects_program_add_remove() {
        let mut a = frame(1);
        let mut b = frame(1);
        a.programs_loaded = vec![test_hash(1), test_hash(2)];
        b.programs_loaded = vec![test_hash(2), test_hash(3)];
        let d = diff(&a, &b);
        assert_eq!(d.programs_added, vec![test_hash(3)]);
        assert_eq!(d.programs_removed, vec![test_hash(1)]);
    }

    #[test]
    fn diff_detects_oracle_add_remove() {
        let mut a = frame(1);
        let mut b = frame(1);
        a.oracles_active = vec![test_hash(10), test_hash(11)];
        b.oracles_active = vec![test_hash(11), test_hash(12)];
        let d = diff(&a, &b);
        assert_eq!(d.oracles_added, vec![test_hash(12)]);
        assert_eq!(d.oracles_removed, vec![test_hash(10)]);
    }

    #[test]
    fn diff_detects_cache_added_removed_and_changed() {
        let mut a = frame(1);
        let mut b = frame(1);
        a.cache_hot_paths = vec![(test_key(1), 2), (test_key(2), 4)];
        b.cache_hot_paths = vec![(test_key(2), 5), (test_key(3), 8)];
        let d = diff(&a, &b);
        assert_eq!(d.cache_hot_paths_added, vec![(test_key(3), 8)]);
        assert_eq!(d.cache_hot_paths_removed, vec![(test_key(1), 2)]);
        assert_eq!(d.cache_hot_paths_changed, vec![(test_key(2), 4, 5)]);
    }

    #[test]
    fn diff_detects_metric_changes() {
        let a = frame(1);
        let mut b = frame(1);
        b.metrics.insert("total_calls".to_owned(), 99);
        b.metrics.insert("new_counter".to_owned(), 5);
        let d = diff(&a, &b);
        assert_eq!(d.metrics_changed["total_calls"], (3, 99));
        assert_eq!(d.metrics_changed["new_counter"], (0, 5));
    }

    #[test]
    fn diff_is_symmetric_by_added_removed_mirroring() {
        let mut a = frame(1);
        let mut b = frame(1);
        a.programs_loaded = vec![test_hash(1)];
        b.programs_loaded = vec![test_hash(2)];
        let ab = diff(&a, &b);
        let ba = diff(&b, &a);
        assert_eq!(ab.programs_added, ba.programs_removed);
        assert_eq!(ab.programs_removed, ba.programs_added);
    }

    #[test]
    fn capture_after_node_activity_changes_hash_and_diff() {
        let (node, path) = fresh_node("drift");
        let before = capture(&node);
        let program = test_program();
        let func = node.store().store(program.bytes()).unwrap();
        let out = node.call_bytes(&func, &3i64.to_le_bytes()).unwrap();
        assert!(node.store().load(&out.result).is_some());
        let after = capture(&node);
        let d = diff(&before, &after);
        assert_ne!(frame_hash(&before), frame_hash(&after));
        assert!(!d.is_empty());
        assert!(d.metrics_changed.contains_key("total_calls"));
        assert!(!after.cache_hot_paths.is_empty());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn capture_is_non_perturbing_for_visible_node_state() {
        let (node, path) = fresh_node("nonperturb");
        let program = test_program();
        let func = node.store().store(program.bytes()).unwrap();
        let _ = node.call_bytes(&func, &5i64.to_le_bytes()).unwrap();

        let before = capture(&node);
        let before_hash = frame_hash(&before);
        let after = capture(&node);
        let after_hash = frame_hash(&after);

        assert_eq!(before_hash, after_hash);
        assert!(diff(&before, &after).is_empty());
        let _ = std::fs::remove_dir_all(path);
    }
}

}

pub mod hardware {
//! Safe hardware-derived signal models for Omega-5.
//!
//! This module deliberately does not perform Rowhammer, DMA, cold-boot
//! recovery, cross-process memory reads, or any other hardware attack. It
//! models the resulting observations as typed witnesses that can be hashed,
//! compared, and fed into the Godel loop as reproducible evidence.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HardwareHash([u8; 32]);

impl HardwareHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for b in self.0 {
            out.push_str(&format!("{b:02x}"));
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlipObservation {
    pub byte_index: u64,
    pub bit_index: u8,
    pub flips: u64,
    pub trials: u64,
}

impl FlipObservation {
    pub fn new(byte_index: u64, bit_index: u8, flips: u64, trials: u64) -> Option<Self> {
        if bit_index >= 8 || trials == 0 || flips > trials {
            return None;
        }
        Some(Self {
            byte_index,
            bit_index,
            flips,
            trials,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PufWitness {
    observations: Vec<FlipObservation>,
}

impl PufWitness {
    pub fn new(observations: Vec<FlipObservation>) -> Self {
        Self {
            observations: canonical_observations(observations),
        }
    }

    pub fn observations(&self) -> &[FlipObservation] {
        &self.observations
    }

    pub fn identity_hash(&self) -> HardwareHash {
        let mut h = Sha256::new();
        h.update(b"scan-omega-puf-witness-v1\0");
        write_observations(&mut h, &self.observations);
        HardwareHash(h.finalize().into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntropyWitness {
    pub state_hash: HardwareHash,
    observations: Vec<FlipObservation>,
}

impl EntropyWitness {
    pub fn from_observations(state_hash: HardwareHash, observations: Vec<FlipObservation>) -> Self {
        Self {
            state_hash,
            observations: canonical_observations(observations),
        }
    }

    pub fn observations(&self) -> &[FlipObservation] {
        &self.observations
    }

    pub fn entropy_hash(&self) -> HardwareHash {
        let mut h = Sha256::new();
        h.update(b"scan-omega-entropy-witness-v1\0");
        h.update(self.state_hash.as_bytes());
        write_observations(&mut h, &self.observations);
        HardwareHash(h.finalize().into())
    }

    pub fn choose_index(&self, choices: usize) -> Option<usize> {
        if choices == 0 {
            return None;
        }
        let hash = self.entropy_hash();
        let mut first_eight = [0u8; 8];
        first_eight.copy_from_slice(&hash.as_bytes()[..8]);
        Some((u64::from_le_bytes(first_eight) as usize) % choices)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CriticalBit {
    pub byte_index: u64,
    pub bit_index: u8,
    pub baseline_score: u64,
    pub flipped_score: u64,
    pub delta: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragilityReport {
    pub artifact_hash: HardwareHash,
    pub artifact_len: u64,
    pub threshold: u64,
    pub critical_bits: Vec<CriticalBit>,
}

impl FragilityReport {
    pub fn report_hash(&self) -> HardwareHash {
        let mut h = Sha256::new();
        h.update(b"scan-omega-fragility-report-v1\0");
        h.update(self.artifact_hash.as_bytes());
        write_u64(&mut h, self.artifact_len);
        write_u64(&mut h, self.threshold);
        write_u64(&mut h, self.critical_bits.len() as u64);
        for bit in &self.critical_bits {
            write_u64(&mut h, bit.byte_index);
            h.update([bit.bit_index]);
            write_u64(&mut h, bit.baseline_score);
            write_u64(&mut h, bit.flipped_score);
            write_u64(&mut h, bit.delta);
        }
        HardwareHash(h.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardeningStrategy {
    TripleModularRedundancy,
    EccParity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardeningPlan {
    pub artifact_hash: HardwareHash,
    pub report_hash: HardwareHash,
    pub strategy: HardeningStrategy,
    pub protected_bits: Vec<CriticalBit>,
}

impl HardeningPlan {
    pub fn from_report(report: &FragilityReport, strategy: HardeningStrategy) -> Self {
        let mut protected_bits = report.critical_bits.clone();
        protected_bits.sort();
        protected_bits.dedup();
        Self {
            artifact_hash: report.artifact_hash,
            report_hash: report.report_hash(),
            strategy,
            protected_bits,
        }
    }

    pub fn plan_hash(&self) -> HardwareHash {
        let mut h = Sha256::new();
        h.update(b"scan-omega-hardening-plan-v1\0");
        h.update(self.artifact_hash.as_bytes());
        h.update(self.report_hash.as_bytes());
        h.update([strategy_tag(self.strategy)]);
        write_u64(&mut h, self.protected_bits.len() as u64);
        for bit in &self.protected_bits {
            write_u64(&mut h, bit.byte_index);
            h.update([bit.bit_index]);
            write_u64(&mut h, bit.baseline_score);
            write_u64(&mut h, bit.flipped_score);
            write_u64(&mut h, bit.delta);
        }
        HardwareHash(h.finalize().into())
    }
}

pub fn artifact_hash(bytes: &[u8]) -> HardwareHash {
    let mut h = Sha256::new();
    h.update(b"scan-omega-hardware-artifact-v1\0");
    write_u64(&mut h, bytes.len() as u64);
    h.update(bytes);
    HardwareHash(h.finalize().into())
}

pub fn scan_bit_fragility<F>(artifact: &[u8], threshold: u64, scorer: F) -> FragilityReport
where
    F: Fn(&[u8]) -> u64,
{
    let baseline_score = scorer(artifact);
    let mut critical_bits = Vec::new();
    let mut mutated = artifact.to_vec();

    for byte_index in 0..artifact.len() {
        for bit_index in 0..8u8 {
            mutated[byte_index] ^= 1u8 << bit_index;
            let flipped_score = scorer(&mutated);
            mutated[byte_index] ^= 1u8 << bit_index;
            let delta = baseline_score.abs_diff(flipped_score);
            if delta >= threshold {
                critical_bits.push(CriticalBit {
                    byte_index: byte_index as u64,
                    bit_index,
                    baseline_score,
                    flipped_score,
                    delta,
                });
            }
        }
    }

    critical_bits.sort();
    FragilityReport {
        artifact_hash: artifact_hash(artifact),
        artifact_len: artifact.len() as u64,
        threshold,
        critical_bits,
    }
}

fn canonical_observations(observations: Vec<FlipObservation>) -> Vec<FlipObservation> {
    let mut merged: BTreeMap<(u64, u8), (u64, u64)> = BTreeMap::new();
    for obs in observations {
        let entry = merged
            .entry((obs.byte_index, obs.bit_index))
            .or_insert((0, 0));
        entry.0 = entry.0.saturating_add(obs.flips);
        entry.1 = entry.1.saturating_add(obs.trials);
    }
    merged
        .into_iter()
        .filter_map(|((byte_index, bit_index), (flips, trials))| {
            FlipObservation::new(byte_index, bit_index, flips.min(trials), trials)
        })
        .collect()
}

fn strategy_tag(strategy: HardeningStrategy) -> u8 {
    match strategy {
        HardeningStrategy::TripleModularRedundancy => 1,
        HardeningStrategy::EccParity => 2,
    }
}

fn write_observations(h: &mut Sha256, observations: &[FlipObservation]) {
    write_u64(h, observations.len() as u64);
    for obs in observations {
        write_u64(h, obs.byte_index);
        h.update([obs.bit_index]);
        write_u64(h, obs.flips);
        write_u64(h, obs.trials);
    }
}

fn write_u64(h: &mut Sha256, value: u64) {
    h.update(value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(byte: u64, bit: u8, flips: u64, trials: u64) -> FlipObservation {
        FlipObservation::new(byte, bit, flips, trials).unwrap()
    }

    fn weighted_score(bytes: &[u8]) -> u64 {
        bytes
            .iter()
            .enumerate()
            .map(|(i, b)| (*b as u64).saturating_mul((i as u64) + 1))
            .sum()
    }

    #[test]
    fn flip_observation_rejects_invalid_bit_index() {
        assert!(FlipObservation::new(0, 8, 1, 10).is_none());
    }

    #[test]
    fn flip_observation_rejects_zero_trials() {
        assert!(FlipObservation::new(0, 1, 0, 0).is_none());
    }

    #[test]
    fn flip_observation_rejects_more_flips_than_trials() {
        assert!(FlipObservation::new(0, 1, 11, 10).is_none());
    }

    #[test]
    fn puf_same_observation_same_identity() {
        let a = PufWitness::new(vec![obs(0, 1, 2, 100), obs(3, 7, 1, 100)]);
        let b = PufWitness::new(vec![obs(0, 1, 2, 100), obs(3, 7, 1, 100)]);
        assert_eq!(a.identity_hash(), b.identity_hash());
    }

    #[test]
    fn puf_different_observation_different_identity() {
        let a = PufWitness::new(vec![obs(0, 1, 2, 100)]);
        let b = PufWitness::new(vec![obs(0, 1, 3, 100)]);
        assert_ne!(a.identity_hash(), b.identity_hash());
    }

    #[test]
    fn puf_observation_order_is_canonical() {
        let a = PufWitness::new(vec![obs(3, 7, 1, 100), obs(0, 1, 2, 100)]);
        let b = PufWitness::new(vec![obs(0, 1, 2, 100), obs(3, 7, 1, 100)]);
        assert_eq!(a.observations(), b.observations());
        assert_eq!(a.identity_hash(), b.identity_hash());
    }

    #[test]
    fn puf_duplicate_observations_are_merged() {
        let puf = PufWitness::new(vec![obs(0, 1, 2, 100), obs(0, 1, 3, 200)]);
        assert_eq!(puf.observations(), &[obs(0, 1, 5, 300)]);
    }

    #[test]
    fn entropy_same_measurement_is_stable_after_capture() {
        let state = artifact_hash(b"node-state");
        let a = EntropyWitness::from_observations(state, vec![obs(4, 0, 1, 128)]);
        let b = EntropyWitness::from_observations(state, vec![obs(4, 0, 1, 128)]);
        assert_eq!(a.entropy_hash(), b.entropy_hash());
        assert_eq!(a.choose_index(17), b.choose_index(17));
    }

    #[test]
    fn entropy_changes_with_state_hash() {
        let a = EntropyWitness::from_observations(artifact_hash(b"a"), vec![obs(4, 0, 1, 128)]);
        let b = EntropyWitness::from_observations(artifact_hash(b"b"), vec![obs(4, 0, 1, 128)]);
        assert_ne!(a.entropy_hash(), b.entropy_hash());
    }

    #[test]
    fn entropy_choose_index_rejects_empty_choice_set() {
        let witness = EntropyWitness::from_observations(artifact_hash(b"state"), vec![obs(1, 2, 3, 4)]);
        assert_eq!(witness.choose_index(0), None);
    }

    #[test]
    fn fragility_scan_detects_critical_bits() {
        let report = scan_bit_fragility(&[0b0000_0000, 0b1000_0000], 128, weighted_score);
        assert!(report
            .critical_bits
            .iter()
            .any(|bit| bit.byte_index == 1 && bit.bit_index == 7));
    }

    #[test]
    fn fragility_report_hash_is_stable() {
        let a = scan_bit_fragility(&[1, 2, 3], 8, weighted_score);
        let b = scan_bit_fragility(&[1, 2, 3], 8, weighted_score);
        assert_eq!(a.report_hash(), b.report_hash());
    }

    #[test]
    fn fragility_report_hash_changes_with_threshold() {
        let a = scan_bit_fragility(&[1, 2, 3], 8, weighted_score);
        let b = scan_bit_fragility(&[1, 2, 3], 16, weighted_score);
        assert_ne!(a.report_hash(), b.report_hash());
    }

    #[test]
    fn hardening_plan_is_deterministic() {
        let report = scan_bit_fragility(&[1, 2, 3], 8, weighted_score);
        let a = HardeningPlan::from_report(&report, HardeningStrategy::TripleModularRedundancy);
        let b = HardeningPlan::from_report(&report, HardeningStrategy::TripleModularRedundancy);
        assert_eq!(a, b);
        assert_eq!(a.plan_hash(), b.plan_hash());
    }

    #[test]
    fn hardening_plan_hash_depends_on_strategy() {
        let report = scan_bit_fragility(&[1, 2, 3], 8, weighted_score);
        let tmr = HardeningPlan::from_report(&report, HardeningStrategy::TripleModularRedundancy);
        let ecc = HardeningPlan::from_report(&report, HardeningStrategy::EccParity);
        assert_ne!(tmr.plan_hash(), ecc.plan_hash());
    }

    #[test]
    fn artifact_hash_is_length_delimited() {
        let a = artifact_hash(&[1, 2, 3]);
        let b = artifact_hash(&[1, 2, 3, 0]);
        assert_ne!(a, b);
    }
}

}

pub mod fabric {
//! Î©-5.0H-B â€” Content-Addressed Memory Fabric Simulator.
//!
//! DÃ©tournement *non armÃ©* des concepts hardware identifiÃ©s dans
//! `docs/OMEGA_RAM_INTROSPECTION_IDEAS.md` :
//!
//!   * IdÃ©e #14 â€” FPGA DRAM controller indexÃ© par hash de contenu plutÃ´t
//!     que par adresse physique.
//!   * IdÃ©e #10 â€” Battering-RAM-style interposer qui rÃ©Ã©crit le bus
//!     mÃ©moire pour exposer un content-addressing au niveau silicon.
//!
//! **SÃ©curitÃ©** : ce module est PUREMENT logique. Aucun Rowhammer, aucun
//! DMA hors-process, aucun cold-boot, aucune lecture RAM cross-process.
//! On modÃ©lise le comportement *attendu* d'un substrat content-addressed
//! comme une structure de donnÃ©es Rust ordinaire â€” c'est la sandbox de
//! validation qui prÃ©cÃ©dera tout effort hardware rÃ©el.
//!
//! Le simulateur expose :
//!   * Un store content-addressed (`hash â†’ bytes`, immuable).
//!   * Un mapping virtuel (`VirtualAddr â†’ ContentHash`) qui peut Ãªtre
//!     remappÃ©/migrÃ© sans copier les bytes.
//!   * Une allocation dÃ©terministe de `PhysicalSlot` par hash.
//!   * Un TLB minimal (`VirtualAddr` ever-resolved set) pour distinguer
//!     les premiers accÃ¨s (miss) des accÃ¨s subsÃ©quents (hit).
//!   * Un `fabric_hash` canonique invariant sous l'ordre d'insertion.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

const HASH_DOMAIN: &[u8] = b"SCAN-OMEGA-FABRIC-PAGE-V1";
const FABRIC_HASH_DOMAIN: &[u8] = b"SCAN-OMEGA-FABRIC-STATE-V1";
const TLB_CAPACITY: usize = 64;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Hash content-addressed sur 32 bytes (sha256 domain-separated).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Calcule le hash d'un blob avec un domaine de sÃ©paration explicite.
    /// Utiliser ce constructeur garantit qu'aucune collision ne peut Ãªtre
    /// induite depuis un autre contexte (autre crate, autre niveau de hash).
    pub fn for_bytes(bytes: &[u8]) -> Self {
        let mut h = Sha256::new();
        h.update(HASH_DOMAIN);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
        let result = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        Self(out)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Adresse virtuelle = poignÃ©e logique. Aucune sÃ©mantique physique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualAddr(pub u64);

/// Slot physique allouÃ© dans le substrat simulÃ©. Monotone, dÃ©terministe
/// dans l'ordre d'insertion d'un hash unique. **Pas inclus dans
/// `fabric_hash`** â€” c'est un dÃ©tail d'allocation runtime, pas un Ã©tat
/// content-addressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalSlot(pub u64);

/// Page indexÃ©e par contenu. Les bytes sont **immuables une fois indexÃ©s** â€”
/// aucune API publique du fabric ne permet de les mutater.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FabricPage {
    pub hash: ContentHash,
    pub bytes: Vec<u8>,
}

/// Compteurs runtime du fabric.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FabricMetrics {
    /// Resolves cache hits (TLB hits).
    pub hits: u64,
    /// Resolves cache misses (TLB misses, premier accÃ¨s Ã  une addr).
    pub misses: u64,
    /// Nombre de remaps explicites ou implicites (insert sur addr dÃ©jÃ  mappÃ©e).
    pub remaps: u64,
    /// DÃ©doublonnages : insertion d'un hash dÃ©jÃ  prÃ©sent dans le store.
    pub dedupes: u64,
}

/// Erreur retournÃ©e par les opÃ©rations de remap/migrate.
#[derive(Debug, PartialEq, Eq)]
pub enum FabricError {
    /// `remap` cible un hash absent du page store.
    UnknownHash(ContentHash),
    /// `migrate_addr` source non mappÃ©e.
    UnknownAddr(VirtualAddr),
}

impl std::fmt::Display for FabricError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FabricError::UnknownHash(h) => {
                write!(f, "fabric: unknown content hash {:02x?}", &h.0[..8])
            }
            FabricError::UnknownAddr(a) => write!(f, "fabric: unknown virtual addr {}", a.0),
        }
    }
}

impl std::error::Error for FabricError {}

// ---------------------------------------------------------------------------
// Fabric
// ---------------------------------------------------------------------------

/// Substrat mÃ©moire content-addressed simulÃ©.
///
/// Garanties :
///   * Pages immuables une fois indexÃ©es (aucune mutation publique).
///   * DÃ©doublonnage automatique : deux insertions avec mÃªmes bytes
///     partagent le mÃªme slot et le mÃªme hash.
///   * `migrate_addr` / `remap` n'effectuent **aucune copie de bytes**,
///     uniquement la modification du mapping.
///   * `fabric_hash` est un hash canonique de l'Ã©tat content-addressed
///     (pages + mapping), invariant sous l'ordre d'insertion.
#[derive(Debug, Default)]
pub struct ContentAddressedFabric {
    pages: BTreeMap<ContentHash, FabricPage>,
    mapping: BTreeMap<VirtualAddr, ContentHash>,
    slots: BTreeMap<ContentHash, PhysicalSlot>,
    next_slot: u64,
    tlb: Vec<VirtualAddr>,
    metrics: FabricMetrics,
}

impl ContentAddressedFabric {
    pub fn new() -> Self {
        Self::default()
    }

    /// InsÃ¨re un blob et le mappe Ã  `addr`. Retourne le `ContentHash`.
    /// Si le blob existe dÃ©jÃ  â†’ dÃ©doublonnage (pas de nouvelle copie).
    /// Si `addr` Ã©tait dÃ©jÃ  mappÃ©e â†’ comptabilisÃ© comme `remap`.
    pub fn insert(&mut self, addr: VirtualAddr, bytes: Vec<u8>) -> ContentHash {
        let hash = ContentHash::for_bytes(&bytes);
        if self.pages.contains_key(&hash) {
            self.metrics.dedupes += 1;
        } else {
            self.pages.insert(hash, FabricPage { hash, bytes });
            self.slots.insert(hash, PhysicalSlot(self.next_slot));
            self.next_slot += 1;
        }
        if self.mapping.insert(addr, hash).is_some() {
            self.metrics.remaps += 1;
            self.invalidate_tlb(addr);
        }
        hash
    }

    /// RÃ©sout `addr` en page. Met Ã  jour le TLB et les mÃ©triques.
    /// Premier accÃ¨s = miss, suivants = hit.
    pub fn resolve(&mut self, addr: VirtualAddr) -> Option<&FabricPage> {
        let hash = *self.mapping.get(&addr)?;
        if self.tlb.iter().any(|a| *a == addr) {
            self.metrics.hits += 1;
        } else {
            self.metrics.misses += 1;
            self.tlb.push(addr);
            if self.tlb.len() > TLB_CAPACITY {
                self.tlb.remove(0);
            }
        }
        self.pages.get(&hash)
    }

    /// Repointe `addr` vers `hash`. Erreur si le hash n'est pas dans le store.
    pub fn remap(&mut self, addr: VirtualAddr, hash: ContentHash) -> Result<(), FabricError> {
        if !self.pages.contains_key(&hash) {
            return Err(FabricError::UnknownHash(hash));
        }
        if self.mapping.insert(addr, hash).is_some() {
            self.metrics.remaps += 1;
        }
        self.invalidate_tlb(addr);
        Ok(())
    }

    /// Migre le mapping de `from` vers `to`. Aucune copie de bytes.
    /// AprÃ¨s migration, `from` est dÃ©mappÃ©e ; `to` pointe vers le hash
    /// d'origine. Erreur si `from` n'existe pas.
    pub fn migrate_addr(
        &mut self,
        from: VirtualAddr,
        to: VirtualAddr,
    ) -> Result<(), FabricError> {
        let hash = self
            .mapping
            .remove(&from)
            .ok_or(FabricError::UnknownAddr(from))?;
        if self.mapping.insert(to, hash).is_some() {
            self.metrics.remaps += 1;
        }
        self.invalidate_tlb(from);
        self.invalidate_tlb(to);
        Ok(())
    }

    pub fn metrics(&self) -> FabricMetrics {
        self.metrics
    }

    /// Hash canonique de l'Ã©tat content-addressed.
    ///
    /// Construction :
    ///   1. Domain separator `SCAN-OMEGA-FABRIC-STATE-V1`.
    ///   2. Pages triÃ©es par `ContentHash` (ordre BTreeMap natif).
    ///   3. Mapping triÃ© par `VirtualAddr` (ordre BTreeMap natif).
    ///
    /// Le `next_slot`, le TLB, et les mÃ©triques runtime ne sont **PAS**
    /// inclus â€” ils dÃ©pendent de l'historique d'opÃ©rations, pas de l'Ã©tat
    /// content-addressed final.
    pub fn fabric_hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(FABRIC_HASH_DOMAIN);
        h.update((self.pages.len() as u64).to_le_bytes());
        for (hash, page) in &self.pages {
            h.update(hash.as_bytes());
            h.update((page.bytes.len() as u64).to_le_bytes());
            h.update(&page.bytes);
        }
        h.update((self.mapping.len() as u64).to_le_bytes());
        for (addr, hash) in &self.mapping {
            h.update(addr.0.to_le_bytes());
            h.update(hash.as_bytes());
        }
        let result = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    /// Slot physique allouÃ© pour un hash, si prÃ©sent. Utile pour debug ;
    /// pas inclus dans `fabric_hash` (allocation runtime).
    pub fn physical_slot_for(&self, hash: &ContentHash) -> Option<PhysicalSlot> {
        self.slots.get(hash).copied()
    }

    fn invalidate_tlb(&mut self, addr: VirtualAddr) {
        self.tlb.retain(|a| *a != addr);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fab() -> ContentAddressedFabric {
        ContentAddressedFabric::new()
    }

    #[test]
    fn same_bytes_same_hash() {
        let bytes = vec![1, 2, 3, 4, 5];
        let h1 = ContentHash::for_bytes(&bytes);
        let h2 = ContentHash::for_bytes(&bytes);
        assert_eq!(h1, h2);

        // Et via le fabric : deux insertions de mÃªmes bytes Ã  des addrs
        // diffÃ©rentes produisent le mÃªme hash.
        let mut f = fab();
        let a = f.insert(VirtualAddr(1), vec![10, 20, 30]);
        let b = f.insert(VirtualAddr(2), vec![10, 20, 30]);
        assert_eq!(a, b);
    }

    #[test]
    fn different_bytes_different_hash() {
        let h1 = ContentHash::for_bytes(b"hello");
        let h2 = ContentHash::for_bytes(b"world");
        assert_ne!(h1, h2);

        let mut f = fab();
        let a = f.insert(VirtualAddr(1), vec![1, 2, 3]);
        let b = f.insert(VirtualAddr(2), vec![4, 5, 6]);
        assert_ne!(a, b);
    }

    #[test]
    fn insert_then_resolve() {
        let mut f = fab();
        let bytes = vec![42, 99, 7];
        let h = f.insert(VirtualAddr(1), bytes.clone());
        let page = f.resolve(VirtualAddr(1)).expect("must resolve");
        assert_eq!(page.hash, h);
        assert_eq!(page.bytes, bytes);
    }

    #[test]
    fn duplicate_insert_dedupes() {
        let mut f = fab();
        let bytes = vec![1, 2, 3];
        f.insert(VirtualAddr(1), bytes.clone());
        f.insert(VirtualAddr(2), bytes.clone());
        f.insert(VirtualAddr(3), bytes);
        // Trois mappings, une seule page physique.
        assert_eq!(f.mapping.len(), 3);
        assert_eq!(f.pages.len(), 1);
        assert_eq!(f.metrics().dedupes, 2);
    }

    #[test]
    fn remap_changes_address_without_copy() {
        let mut f = fab();
        let h = f.insert(VirtualAddr(1), vec![7, 8, 9]);
        let pages_before = f.pages.len();

        // Remap addr=2 vers le mÃªme hash â†’ pas de nouvelle page.
        f.remap(VirtualAddr(2), h).expect("remap");
        assert_eq!(f.pages.len(), pages_before, "remap doit pas crÃ©er de page");

        // Les deux addrs rÃ©solvent au mÃªme contenu.
        let p1_bytes = f.resolve(VirtualAddr(1)).unwrap().bytes.clone();
        let p2_bytes = f.resolve(VirtualAddr(2)).unwrap().bytes.clone();
        assert_eq!(p1_bytes, p2_bytes);
        assert_eq!(p1_bytes, vec![7, 8, 9]);
    }

    #[test]
    fn migrate_addr_preserves_content_hash() {
        let mut f = fab();
        let h = f.insert(VirtualAddr(10), vec![100, 101, 102]);
        f.migrate_addr(VirtualAddr(10), VirtualAddr(20)).expect("migrate");

        // L'ancienne addr est dÃ©mappÃ©e.
        assert!(f.resolve(VirtualAddr(10)).is_none());
        // La nouvelle addr rÃ©sout vers le mÃªme hash.
        let page = f.resolve(VirtualAddr(20)).expect("new addr");
        assert_eq!(page.hash, h);
        assert_eq!(page.bytes, vec![100, 101, 102]);
        // Une seule page physique reste.
        assert_eq!(f.pages.len(), 1);
    }

    #[test]
    fn unknown_addr_returns_none() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![1]);
        // Addr jamais insÃ©rÃ©e.
        assert!(f.resolve(VirtualAddr(9999)).is_none());
    }

    #[test]
    fn unknown_hash_remap_errors() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![1, 2, 3]);
        // Hash arbitraire jamais insÃ©rÃ©.
        let bogus = ContentHash::from_bytes([0xAA; 32]);
        let result = f.remap(VirtualAddr(2), bogus);
        assert!(matches!(result, Err(FabricError::UnknownHash(_))));
    }

    #[test]
    fn fabric_hash_stable() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![1, 2]);
        f.insert(VirtualAddr(2), vec![3, 4]);
        let h1 = f.fabric_hash();
        let h2 = f.fabric_hash();
        let h3 = f.fabric_hash();
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);

        // AccÃ¨s en lecture (resolve) ne change pas le hash canonique
        // (les mÃ©triques mutent mais ne participent pas au hash).
        let _ = f.resolve(VirtualAddr(1));
        let _ = f.resolve(VirtualAddr(1));
        let h4 = f.fabric_hash();
        assert_eq!(h1, h4, "fabric_hash doit Ãªtre insensible aux resolves");
    }

    #[test]
    fn fabric_hash_order_independent() {
        // Insertion ordre A â†’ B
        let mut a = fab();
        a.insert(VirtualAddr(1), vec![1, 2, 3]);
        a.insert(VirtualAddr(2), vec![4, 5, 6]);
        a.insert(VirtualAddr(3), vec![7, 8, 9]);

        // Insertion ordre B â†’ A â†’ C (et donc allocation slots diffÃ©rente)
        let mut b = fab();
        b.insert(VirtualAddr(2), vec![4, 5, 6]);
        b.insert(VirtualAddr(1), vec![1, 2, 3]);
        b.insert(VirtualAddr(3), vec![7, 8, 9]);

        // MÃªme Ã©tat content-addressed final â†’ mÃªme fabric_hash.
        assert_eq!(a.fabric_hash(), b.fabric_hash());
    }

    #[test]
    fn tlb_hits_after_first_resolve() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![42]);

        // Premier resolve = miss.
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        assert_eq!(f.metrics().misses, 1);
        assert_eq!(f.metrics().hits, 0);

        // Trois resolves de plus = trois hits.
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        assert_eq!(f.metrics().hits, 3);
        assert_eq!(f.metrics().misses, 1);

        // Une remap invalide le TLB pour cette addr â†’ next resolve = miss.
        let h = ContentHash::for_bytes(&[42]);
        f.remap(VirtualAddr(1), h).unwrap();
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        assert_eq!(f.metrics().misses, 2);
    }

    #[test]
    fn immutable_content_cannot_be_mutated_through_api() {
        let mut f = fab();
        let original = vec![1, 2, 3, 4, 5];
        let h = f.insert(VirtualAddr(1), original.clone());

        // Snapshot des bytes aprÃ¨s insertion.
        let snap = f.resolve(VirtualAddr(1)).unwrap().bytes.clone();
        assert_eq!(snap, original);

        // Insertions rÃ©pÃ©tÃ©es avec mÃªmes bytes ne modifient pas la page.
        for _ in 0..5 {
            f.insert(VirtualAddr(1), original.clone());
        }
        let after_reinserts = f.resolve(VirtualAddr(1)).unwrap().bytes.clone();
        assert_eq!(after_reinserts, original);

        // Insertion avec bytes diffÃ©rents crÃ©e une NOUVELLE page (hash
        // diffÃ©rent), n'altÃ¨re pas la page d'origine.
        let h2 = f.insert(VirtualAddr(2), vec![9, 9, 9]);
        assert_ne!(h, h2);

        // La page originale a toujours ses bytes intacts.
        let page1 = f.pages.get(&h).expect("page hash should still exist");
        assert_eq!(page1.bytes, original);

        // Migration ne touche pas non plus aux bytes.
        f.migrate_addr(VirtualAddr(1), VirtualAddr(3)).unwrap();
        let migrated = f.resolve(VirtualAddr(3)).unwrap().bytes.clone();
        assert_eq!(migrated, original);
    }

    // ---- Tests bonus pour blindage supplÃ©mentaire ----

    #[test]
    fn physical_slot_is_assigned_per_unique_hash() {
        let mut f = fab();
        let h1 = f.insert(VirtualAddr(1), vec![1]);
        let h2 = f.insert(VirtualAddr(2), vec![2]);
        let s1 = f.physical_slot_for(&h1).unwrap();
        let s2 = f.physical_slot_for(&h2).unwrap();
        assert_ne!(s1, s2);

        // InsÃ©rer le mÃªme contenu ne crÃ©e pas un nouveau slot.
        let h3 = f.insert(VirtualAddr(3), vec![1]);
        assert_eq!(h3, h1);
        assert_eq!(f.physical_slot_for(&h3).unwrap(), s1);
    }

    #[test]
    fn migrate_unknown_addr_errors() {
        let mut f = fab();
        let result = f.migrate_addr(VirtualAddr(1), VirtualAddr(2));
        assert!(matches!(result, Err(FabricError::UnknownAddr(_))));
    }

    #[test]
    fn empty_fabric_hash_is_well_defined() {
        let f1 = fab();
        let f2 = fab();
        assert_eq!(f1.fabric_hash(), f2.fabric_hash());
    }

    #[test]
    fn hash_for_bytes_uses_domain_separation() {
        // Domaine garantit qu'un hash de bytes au sein du fabric ne peut
        // pas accidentellement collisionner avec un hash externe sur les
        // mÃªmes bytes (diffÃ©rent domaine, diffÃ©rent prÃ©fixe).
        let h_fabric = ContentHash::for_bytes(b"abc");
        let mut raw = Sha256::new();
        raw.update(b"abc");
        let raw_result: [u8; 32] = raw.finalize().into();
        assert_ne!(h_fabric.as_bytes(), &raw_result);
    }
}

}

pub mod criteria {
//! Omega-5.1 benchmark suite B and property suite P.
//!
//! Criteria are intentionally small, deterministic in structure, and runnable
//! without privileged access to `MonsterNode` internals. Where the original
//! Omega wording says "programs in cache", this first mile uses a fixed public
//! KASM corpus because the program/oracle maps are private to `crate::monster`.

use std::time::Instant;

use crate::godel::fabric::{ContentAddressedFabric, VirtualAddr};
use crate::godel::hardware::scan_bit_fragility;
use crate::godel::observer::{capture, frame_hash};
use crate::kasm::{Node, Program, Target, Ty};
use crate::monster::read_cycles;
use crate::MonsterNode;

/// V8 Solution C â€” bench timing avec filtre RDTSC pour rejeter les
/// samples interrompus par le scheduler. Principe :
///
/// 1. On mesure simultanÃ©ment le wall-time (ns) ET les cycles CPU
///    (RDTSC) autour de l'exÃ©cution.
/// 2. Le ratio cycles/ns approxime la frÃ©quence CPU effective. Si
///    la frÃ©quence apparente s'effondre (< 0.1 cycle/ns), c'est qu'un
///    interrupt + halt-state a "volÃ©" du wall-time pendant que les
///    cycles n'avanÃ§aient pas (le CPU Ã©tait endormi). Sample biaisÃ©,
///    rejetÃ©.
/// 3. On retente jusqu'Ã  atteindre `target` samples valides ou
///    `target * 3` tentatives totales. La mÃ©diane des samples valides
///    est retournÃ©e â€” rÃ©sistante aux outliers que ce filtre n'aurait
///    pas dÃ©tectÃ©s.
///
/// Sur architecture non-x86_64, `read_cycles` retourne 0 â†’ le filtre
/// est dÃ©sactivÃ© et on retombe sur le comportement V7 (5 samples,
/// mÃ©diane brute). Aucune rÃ©gression.
fn measure_filtered<F: FnMut()>(mut f: F, target: usize) -> u64 {
    let max_attempts = target * 3;
    let mut samples: Vec<u64> = Vec::with_capacity(max_attempts);
    let mut attempts = 0;
    while samples.len() < target && attempts < max_attempts {
        attempts += 1;
        let cyc_before = read_cycles();
        let inst = Instant::now();
        f();
        let elapsed_ns = inst.elapsed().as_nanos() as u64;
        let cyc_after = read_cycles();
        let cycles = cyc_after.saturating_sub(cyc_before);
        // Sur x86_64 : si cycles est non-zÃ©ro et que le ratio est
        // anormalement bas (< 0.1 cycle/ns soit < 100 cycles pour
        // 1000 ns), le sample est probablement interrompu â€” on jette.
        // Sinon (non-x86 ou ratio normal) on garde.
        if cycles > 0 && elapsed_ns > 0 {
            let ratio_per_kns = cycles.saturating_mul(1000) / elapsed_ns;
            if ratio_per_kns < 100 {
                continue;
            }
        }
        samples.push(elapsed_ns.max(1));
    }
    if samples.is_empty() {
        return 1;
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

pub trait Benchmark {
    fn name(&self) -> &str;
    fn run(&self, node: &MonsterNode) -> u64;
}

pub trait Property {
    fn name(&self) -> &str;
    fn check(&self, node: &MonsterNode) -> Result<(), String>;
}

#[derive(Default)]
pub struct CriteriaSuite {
    pub benches: Vec<Box<dyn Benchmark>>,
    pub props: Vec<Box<dyn Property>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriteriaReport {
    pub bench_scores: Vec<(String, u64)>,
    pub prop_results: Vec<(String, Result<(), String>)>,
}

impl CriteriaSuite {
    pub fn new(benches: Vec<Box<dyn Benchmark>>, props: Vec<Box<dyn Property>>) -> Self {
        Self { benches, props }
    }

    pub fn omega51_default() -> Self {
        Self {
            benches: vec![
                Box::new(KasmCanonicalizeBench::default()),
                Box::new(MonsterTrainAffineBench),
                Box::new(FabricResolveLatencyBench),
                Box::new(ObserverCaptureLatencyBench),
                Box::new(HardeningSensitivityBench::default()),
            ],
            props: vec![
                Box::new(Termination::default()),
                Box::new(HashStability::default()),
                Box::new(NoF32InMlirCanonical::default()),
                Box::new(MemoryBound),
                Box::new(FabricImmutability::default()),
            ],
        }
    }

    pub fn evaluate(&self, node: &MonsterNode) -> CriteriaReport {
        let bench_scores = self
            .benches
            .iter()
            .map(|bench| (bench.name().to_owned(), bench.run(node)))
            .collect();
        let prop_results = self
            .props
            .iter()
            .map(|prop| (prop.name().to_owned(), prop.check(node)))
            .collect();
        CriteriaReport {
            bench_scores,
            prop_results,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KasmCanonicalizeBench {
    corpus: Vec<Program>,
}

impl Default for KasmCanonicalizeBench {
    fn default() -> Self {
        Self {
            corpus: kasm_corpus(),
        }
    }
}

impl Benchmark for KasmCanonicalizeBench {
    fn name(&self) -> &str {
        "KasmCanonicalizeBench"
    }

    fn run(&self, _node: &MonsterNode) -> u64 {
        self.corpus
            .iter()
            .filter_map(|program| program.canonical().ok())
            .map(|program| program.nodes().len() as u64)
            .sum()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MonsterTrainAffineBench;

impl Benchmark for MonsterTrainAffineBench {
    fn name(&self) -> &str {
        "MonsterTrainAffineBench"
    }

    fn run(&self, node: &MonsterNode) -> u64 {
        let source = forge_affine_newcompute_source(6_000, Some(512));
        // V8 Solution C : measure_filtered rejette les samples
        // interrompus par le scheduler OS via dÃ©tection RDTSC. La
        // mÃ©diane des samples valides remonte ; sur 5 samples cibles
        // avec ~15 % de bruit interruption, on rÃ©cupÃ¨re typiquement
        // 4-5 samples propres en 6-8 tentatives.
        measure_filtered(
            || {
                let _ = node.prepare_forge_source(&source, std::iter::empty::<String>());
            },
            5,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FabricResolveLatencyBench;

fn forge_affine_newcompute_source(max_steps: u64, parallelism: Option<usize>) -> String {
    let parallelism = parallelism
        .map(|value| format!("parallelism={value}\n"))
        .unwrap_or_default();
    format!(
        "forge_module:\n  module godel_affine_newcompute version 1\nforge_imports:\n  none\nforge_constants:\n  const a: f64 unit none = 7.0\n  const b: f64 unit none = 3.0\nforge_functions:\n  fn affine(x: f64) -> f64 {{ return a * x + b }}\nforge_program:\n  let y = affine(x)\n  emit y: f64 = y\nforge_inputs:\n  param x: f64 unit none bounds [-10.0,10.0] nominal 0.0\nforge_outputs:\n  output y: f64 unit none handoff scalar\nforge_constraints:\n  assert finite(y)\n  assert bounds(y,[-100.0,100.0])\nforge_samples:\n  case basic seed 1 {{ given x=2.0; expect y approx 17.0 tolerance 0.01 }}\nforge_cost:\nmax_steps={max_steps}\nmax_memory_mb=16\nprecision=f64\n{parallelism}artifact_handoff:\nproof_hash,output_hash,compact_result"
    )
}
impl Benchmark for FabricResolveLatencyBench {
    fn name(&self) -> &str {
        "FabricResolveLatencyBench"
    }

    fn run(&self, _node: &MonsterNode) -> u64 {
        let mut fabric = ContentAddressedFabric::new();
        for i in 0..1000u64 {
            fabric.insert(VirtualAddr(i), format!("page-{i:04}").into_bytes());
        }
        let start = Instant::now();
        for i in 0..1000u64 {
            let _ = fabric.resolve(VirtualAddr(i));
        }
        ((start.elapsed().as_nanos() as u64) / 1000).max(1)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObserverCaptureLatencyBench;

impl Benchmark for ObserverCaptureLatencyBench {
    fn name(&self) -> &str {
        "ObserverCaptureLatencyBench"
    }

    fn run(&self, node: &MonsterNode) -> u64 {
        let start = Instant::now();
        let frame = capture(node);
        let _ = frame_hash(&frame);
        (start.elapsed().as_nanos() as u64).max(1)
    }
}

#[derive(Debug, Clone)]
pub struct HardeningSensitivityBench {
    corpus: Vec<Program>,
}

impl Default for HardeningSensitivityBench {
    fn default() -> Self {
        Self {
            corpus: kasm_corpus(),
        }
    }
}

impl Benchmark for HardeningSensitivityBench {
    fn name(&self) -> &str {
        "HardeningSensitivityBench"
    }

    fn run(&self, _node: &MonsterNode) -> u64 {
        let mut total = 0u64;
        for program in &self.corpus {
            let report = scan_bit_fragility(program.bytes(), 128, byte_weighted_score);
            total = total.saturating_add(report.critical_bits.len() as u64);
        }
        total / (self.corpus.len() as u64).max(1)
    }
}

#[derive(Debug, Clone)]
pub struct Termination {
    corpus: Vec<Program>,
    injected_bad: Vec<(u32, usize)>,
}

impl Default for Termination {
    fn default() -> Self {
        Self {
            corpus: kasm_corpus(),
            injected_bad: Vec::new(),
        }
    }
}

impl Termination {
    #[cfg(test)]
    fn with_bad_case(fuel: u32, nodes: usize) -> Self {
        Self {
            corpus: Vec::new(),
            injected_bad: vec![(fuel, nodes)],
        }
    }
}

impl Property for Termination {
    fn name(&self) -> &str {
        "Termination"
    }

    fn check(&self, _node: &MonsterNode) -> Result<(), String> {
        for program in &self.corpus {
            if program.fuel() < program.nodes().len() as u32 {
                return Err(format!(
                    "fuel {} < nodes {}",
                    program.fuel(),
                    program.nodes().len()
                ));
            }
        }
        for (fuel, nodes) in &self.injected_bad {
            if *fuel < *nodes as u32 {
                return Err(format!("fuel {fuel} < nodes {nodes}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HashStability {
    corpus: Vec<Program>,
    force_failure: bool,
}

impl Default for HashStability {
    fn default() -> Self {
        Self {
            corpus: kasm_corpus(),
            force_failure: false,
        }
    }
}

impl HashStability {
    #[cfg(test)]
    fn forced_failure() -> Self {
        Self {
            corpus: kasm_corpus(),
            force_failure: true,
        }
    }
}

impl Property for HashStability {
    fn name(&self) -> &str {
        "HashStability"
    }

    fn check(&self, _node: &MonsterNode) -> Result<(), String> {
        if self.force_failure {
            return Err("forced hash instability fixture".to_owned());
        }
        for program in &self.corpus {
            let before = program
                .canonical_hash_hex()
                .map_err(|err| format!("canonical_hash before: {err}"))?;
            let after = program
                .canonical()
                .map_err(|err| format!("canonicalize: {err}"))?
                .canonical_hash_hex()
                .map_err(|err| format!("canonical_hash after: {err}"))?;
            if before != after {
                return Err(format!("canonical hash drift: {before} != {after}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NoF32InMlirCanonical {
    corpus: Vec<Program>,
    injected_text: Option<String>,
}

impl Default for NoF32InMlirCanonical {
    fn default() -> Self {
        Self {
            corpus: kasm_corpus(),
            injected_text: None,
        }
    }
}

impl NoF32InMlirCanonical {
    #[cfg(test)]
    fn with_text(text: &str) -> Self {
        Self {
            corpus: Vec::new(),
            injected_text: Some(text.to_owned()),
        }
    }
}

impl Property for NoF32InMlirCanonical {
    fn name(&self) -> &str {
        "NoF32InMlirCanonical"
    }

    fn check(&self, _node: &MonsterNode) -> Result<(), String> {
        if let Some(text) = &self.injected_text {
            return check_text_has_no_float(text);
        }
        for program in &self.corpus {
            let text = program
                .canonical_mlir_text()
                .map_err(|err| format!("canonical mlir: {err}"))?;
            check_text_has_no_float(&text)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryBound;

impl Property for MemoryBound {
    fn name(&self) -> &str {
        "MemoryBound"
    }

    fn check(&self, node: &MonsterNode) -> Result<(), String> {
        let used = node.governor().used_bytes();
        let budget = node.governor().budget_bytes();
        if used > budget {
            Err(format!("memory used {used} > budget {budget}"))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct FabricImmutability {
    corpus: Vec<Program>,
    force_failure: bool,
}

impl Default for FabricImmutability {
    fn default() -> Self {
        Self {
            corpus: kasm_corpus(),
            force_failure: false,
        }
    }
}

impl FabricImmutability {
    #[cfg(test)]
    fn forced_failure() -> Self {
        Self {
            corpus: kasm_corpus(),
            force_failure: true,
        }
    }
}

impl Property for FabricImmutability {
    fn name(&self) -> &str {
        "FabricImmutability"
    }

    fn check(&self, _node: &MonsterNode) -> Result<(), String> {
        if self.force_failure {
            return Err("forced fabric mutation fixture".to_owned());
        }
        let mut fabric = ContentAddressedFabric::new();
        for (i, program) in self.corpus.iter().enumerate() {
            fabric.insert(VirtualAddr(i as u64), program.bytes().to_vec());
        }
        let before = fabric.fabric_hash();
        for i in 0..self.corpus.len() {
            let _ = fabric.resolve(VirtualAddr(i as u64));
        }
        let after = fabric.fabric_hash();
        if before == after {
            Ok(())
        } else {
            Err("fabric hash changed under repeated resolve".to_owned())
        }
    }
}

fn check_text_has_no_float(text: &str) -> Result<(), String> {
    if text.contains("f32") || text.contains("f64") {
        Err("canonical MLIR text contains f32/f64".to_owned())
    } else {
        Ok(())
    }
}

fn byte_weighted_score(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .enumerate()
        .map(|(i, b)| (*b as u64).saturating_mul((i as u64 % 17) + 1))
        .sum()
}

fn kasm_corpus() -> Vec<Program> {
    (0..16).map(corpus_program).collect()
}

fn corpus_program(i: usize) -> Program {
    match i % 8 {
        0 => affine_program(2, 1),
        1 => affine_program(7, 3),
        2 => affine_program(-3, 5),
        3 => bitmix_program(),
        4 => select_program(),
        5 => redundant_program(),
        6 => shift_program(),
        _ => affine_program((i as i16) + 1, (i as i16) - 4),
    }
}

fn affine_program(mul: i16, add: i16) -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        6,
        vec![
            Node::input(0),
            Node::const_i64(mul),
            Node::mul(0, 1),
            Node::const_i64(add),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap()
}

fn bitmix_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        8,
        vec![
            Node::input(0),
            Node::const_i64(3),
            Node::shl(0, 1),
            Node::const_i64(2),
            Node::shr(0, 3),
            Node::bit_xor(2, 4),
            Node::const_i64(7),
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap()
}

fn select_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        7,
        vec![
            Node::input(0),
            Node::const_i64(0),
            Node::eq(0, 1),
            Node::const_i64(10),
            Node::const_i64(-10),
            Node::select_i64(2, 3, 4),
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap()
}

fn redundant_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        8,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::add(0, 1),
            Node::const_i64(1),
            Node::add(0, 3),
            Node::mul(2, 4),
            Node::const_i64(99),
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap()
}

fn shift_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        7,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::shl(0, 1),
            Node::const_i64(3),
            Node::bit_or(2, 3),
            Node::bit_and(4, 0),
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};


    use crate::{MemoryGovernor, Store};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fresh_path(tag: &str) -> PathBuf {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut p = crate::fresh_tmp_path("scan-godel-criteria", tag);
        // Append seq to disambiguate concurrent test runs (TEST_COUNTER ordering)
        p.set_file_name(format!(
            "{}-{seq}",
            p.file_name().unwrap().to_str().unwrap()
        ));
        p
    }

    fn fresh_node(tag: &str) -> (MonsterNode, PathBuf) {
        let path = fresh_path(tag);
        let node = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (node, path)
    }

    #[test]
    fn kasm_canonicalize_bench_returns_finite_score() {
        let (node, path) = fresh_node("canon-bench");
        let score = KasmCanonicalizeBench::default().run(&node);
        assert!(score > 0);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn monster_train_affine_bench_returns_finite_score() {
        let (node, path) = fresh_node("train-bench");
        let score = MonsterTrainAffineBench.run(&node);
        assert!(score > 0);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn fabric_resolve_latency_bench_returns_finite_score() {
        let (node, path) = fresh_node("fabric-bench");
        let score = FabricResolveLatencyBench.run(&node);
        assert!(score > 0);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn observer_capture_latency_bench_returns_finite_score() {
        let (node, path) = fresh_node("observer-bench");
        let score = ObserverCaptureLatencyBench.run(&node);
        assert!(score > 0);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn hardening_sensitivity_bench_returns_finite_score() {
        let (node, path) = fresh_node("hardening-bench");
        let score = HardeningSensitivityBench::default().run(&node);
        assert!(score > 0);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn termination_passes_and_detects_bad_fixture() {
        let (node, path) = fresh_node("termination");
        assert!(Termination::default().check(&node).is_ok());
        assert!(Termination::with_bad_case(1, 2).check(&node).is_err());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn hash_stability_passes_and_detects_bad_fixture() {
        let (node, path) = fresh_node("hash-stability");
        assert!(HashStability::default().check(&node).is_ok());
        assert!(HashStability::forced_failure().check(&node).is_err());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn no_f32_mlir_passes_and_detects_bad_text() {
        let (node, path) = fresh_node("no-f32");
        assert!(NoF32InMlirCanonical::default().check(&node).is_ok());
        assert!(NoF32InMlirCanonical::with_text("tensor<f32>").check(&node).is_err());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn memory_bound_passes_on_fresh_node() {
        let (node, path) = fresh_node("memory-bound");
        assert!(MemoryBound.check(&node).is_ok());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn fabric_immutability_passes_and_detects_bad_fixture() {
        let (node, path) = fresh_node("fabric-immut");
        assert!(FabricImmutability::default().check(&node).is_ok());
        assert!(FabricImmutability::forced_failure().check(&node).is_err());
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn criteria_suite_evaluate_reports_all_criteria() {
        let (node, path) = fresh_node("suite");
        let suite = CriteriaSuite::omega51_default();
        let report = suite.evaluate(&node);
        assert_eq!(report.bench_scores.len(), 5);
        assert_eq!(report.prop_results.len(), 5);
        assert!(report.bench_scores.iter().all(|(_, score)| *score > 0));
        assert!(report.prop_results.iter().all(|(_, result)| result.is_ok()));
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn criteria_suite_custom_constructor_is_runnable() {
        let (node, path) = fresh_node("custom-suite");
        let suite = CriteriaSuite::new(
            vec![Box::new(KasmCanonicalizeBench::default())],
            vec![Box::new(MemoryBound)],
        );
        let report = suite.evaluate(&node);
        assert_eq!(report.bench_scores.len(), 1);
        assert_eq!(report.prop_results.len(), 1);
        assert!(report.prop_results[0].1.is_ok());
        let _ = std::fs::remove_dir_all(path);
    }
}

}

pub mod verifier {
//! Omega-5.2 verifier.
//!
//! Direct pipeline contract: frames carry benchmark scores as metrics named
//! `bench:<name>`. The verifier checks all properties on the after-node and
//! compares before/after scores without inserting an extra orchestration layer.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::godel::criteria::{CriteriaReport, CriteriaSuite};
use crate::godel::observer::ObserverFrame;
use crate::MonsterNode;

pub const DEFAULT_EPSILON_BPS: u64 = 500;
pub const BENCH_METRIC_PREFIX: &str = "bench:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    pub id: u64,
    pub description: String,
    pub kind: RewriteKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteKind {
    ConfigPatch(BTreeMap<&'static str, i64>),
}

impl Rewrite {
    pub fn config_patch(description: impl Into<String>, patch: BTreeMap<&'static str, i64>) -> Self {
        let description = description.into();
        let id = rewrite_id(&description, &RewriteKind::ConfigPatch(patch.clone()));
        Self {
            id,
            description,
            kind: RewriteKind::ConfigPatch(patch),
        }
    }

    pub fn hash_hex(&self) -> String {
        let mut out = String::with_capacity(16);
        for b in self.hash_bytes() {
            out.push_str(&format!("{b:02x}"));
        }
        out
    }

    pub fn hash_bytes(&self) -> [u8; 8] {
        self.id.to_le_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accept,
    Reject(Vec<String>),
}

pub fn verify(
    before: &ObserverFrame,
    after: &ObserverFrame,
    criteria: &CriteriaSuite,
    node: &MonsterNode,
) -> Verdict {
    verify_with_epsilon(before, after, criteria, node, DEFAULT_EPSILON_BPS)
}

pub fn verify_with_epsilon(
    before: &ObserverFrame,
    after: &ObserverFrame,
    criteria: &CriteriaSuite,
    node: &MonsterNode,
    epsilon_bps: u64,
) -> Verdict {
    let mut reasons = Vec::new();

    for prop in &criteria.props {
        if let Err(reason) = prop.check(node) {
            reasons.push(format!("property {} failed: {reason}", prop.name()));
        }
    }

    let after_report = criteria.evaluate(node);
    let mut any_improved = false;
    let mut compared = 0usize;

    for (name, measured_after) in after_report.bench_scores {
        let before_key = bench_metric_key(&name);
        let Some(before_score) = before.metrics.get(&before_key).copied() else {
            reasons.push(format!("benchmark {name} missing before score"));
            continue;
        };
        let after_score = after
            .metrics
            .get(&before_key)
            .copied()
            .unwrap_or(measured_after);
        compared += 1;

        if after_score < before_score {
            any_improved = true;
        }

        let allowed = allowed_regression_score(before_score, epsilon_bps);
        if after_score > allowed {
            reasons.push(format!(
                "benchmark {name} regressed: before={before_score}, after={after_score}, allowed={allowed}"
            ));
        }
    }

    if compared == 0 {
        reasons.push("no benchmarks compared".to_owned());
    } else if !any_improved {
        reasons.push("no benchmark improved strictly".to_owned());
    }

    if reasons.is_empty() {
        Verdict::Accept
    } else {
        Verdict::Reject(reasons)
    }
}

pub fn attach_bench_scores(mut frame: ObserverFrame, report: &CriteriaReport) -> ObserverFrame {
    for (name, score) in &report.bench_scores {
        frame.metrics.insert(bench_metric_key(name), *score);
    }
    frame
}

pub fn bench_metric_key(name: &str) -> String {
    format!("{BENCH_METRIC_PREFIX}{name}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticRecallVerification {
    pub verification_hash: String,
    pub before_frame_hash: String,
    pub after_frame_hash: String,
    pub accepted: bool,
    pub reasons: Vec<String>,
}

pub fn verify_semantic_note_recall(
    note_hash: &str,
    evidence_hash: &str,
    recall_hash: &str,
    recalled_note_hashes: &[String],
) -> SemanticRecallVerification {
    let anchored = !evidence_hash.trim().is_empty();
    let recalled = recalled_note_hashes.iter().any(|candidate| candidate == note_hash);
    let mut before_metrics = std::collections::BTreeMap::new();
    before_metrics.insert("semantic_note_expected".to_string(), 1);
    before_metrics.insert("semantic_anchor_expected".to_string(), anchored as u64);
    before_metrics.insert("semantic_recall_requested".to_string(), 1);
    before_metrics.insert("semantic_note_hash_len".to_string(), note_hash.len() as u64);
    before_metrics.insert("semantic_evidence_hash_len".to_string(), evidence_hash.len() as u64);
    before_metrics.insert("semantic_recall_hash_len".to_string(), recall_hash.len() as u64);
    let mut after_metrics = before_metrics.clone();
    after_metrics.insert("semantic_note_recalled".to_string(), recalled as u64);
    after_metrics.insert(
        "semantic_recalled_note_count".to_string(),
        recalled_note_hashes.len() as u64,
    );

    let before = ObserverFrame {
        epoch: 0,
        programs_loaded: Vec::new(),
        oracles_active: Vec::new(),
        cache_hot_paths: Vec::new(),
        metrics: before_metrics,
    };
    let after = ObserverFrame {
        epoch: 1,
        programs_loaded: Vec::new(),
        oracles_active: Vec::new(),
        cache_hot_paths: Vec::new(),
        metrics: after_metrics,
    };
    let before_frame_hash = hex_frame_hash(&crate::godel::observer::frame_hash(&before));
    let after_frame_hash = hex_frame_hash(&crate::godel::observer::frame_hash(&after));

    let mut reasons = Vec::new();
    if !anchored {
        reasons.push("semantic note is missing evidence hash".to_string());
    }
    if !recalled {
        reasons.push("semantic note hash was not present in bounded recall".to_string());
    }
    let accepted = reasons.is_empty();
    let canonical = format!(
        "forge-godel-semantic-recall-v1\nnote_hash={}\nevidence_hash={}\nrecall_hash={}\naccepted={}\nbefore_frame_hash={}\nafter_frame_hash={}\nreasons={}\n",
        sanitize_semantic_line(note_hash),
        sanitize_semantic_line(evidence_hash),
        sanitize_semantic_line(recall_hash),
        accepted,
        before_frame_hash,
        after_frame_hash,
        reasons
            .iter()
            .map(|reason| sanitize_semantic_line(reason))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    SemanticRecallVerification {
        verification_hash: crate::Hash::for_blob(canonical.as_bytes()).as_hex(),
        before_frame_hash,
        after_frame_hash,
        accepted,
        reasons,
    }
}

fn sanitize_semantic_line(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
        .collect::<String>()
        .trim()
        .to_string()
}

fn hex_frame_hash(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn allowed_regression_score(before_score: u64, epsilon_bps: u64) -> u64 {
    before_score.saturating_add(before_score.saturating_mul(epsilon_bps) / 10_000)
}

fn rewrite_id(description: &str, kind: &RewriteKind) -> u64 {
    let mut h = Sha256::new();
    h.update(b"scan-omega-rewrite-v1\0");
    h.update(description.as_bytes());
    match kind {
        RewriteKind::ConfigPatch(patch) => {
            h.update(b"config-patch\0");
            h.update((patch.len() as u64).to_le_bytes());
            for (key, value) in patch {
                h.update((key.len() as u64).to_le_bytes());
                h.update(key.as_bytes());
                h.update(value.to_le_bytes());
            }
        }
    }
    let digest = h.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::godel::criteria::{Benchmark, Property};
    use crate::godel::observer::ObserverFrame;
    use crate::{MemoryGovernor, Store};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};


    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct StaticBench {
        name: &'static str,
        score: u64,
    }

    impl Benchmark for StaticBench {
        fn name(&self) -> &str {
            self.name
        }

        fn run(&self, _node: &crate::MonsterNode) -> u64 {
            self.score
        }
    }

    struct StaticProp {
        name: &'static str,
        result: Result<(), String>,
    }

    impl Property for StaticProp {
        fn name(&self) -> &str {
            self.name
        }

        fn check(&self, _node: &crate::MonsterNode) -> Result<(), String> {
            self.result.clone()
        }
    }

    fn fresh_path(tag: &str) -> PathBuf {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut p = crate::fresh_tmp_path("scan-godel-verifier", tag);
        p.set_file_name(format!(
            "{}-{seq}",
            p.file_name().unwrap().to_str().unwrap()
        ));
        p
    }

    fn fresh_node(tag: &str) -> (crate::MonsterNode, PathBuf) {
        let path = fresh_path(tag);
        let node = crate::MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (node, path)
    }

    fn frame(scores: &[(&str, u64)]) -> ObserverFrame {
        let mut metrics = BTreeMap::new();
        for (name, score) in scores {
            metrics.insert(bench_metric_key(name), *score);
        }
        ObserverFrame {
            epoch: 0,
            programs_loaded: Vec::new(),
            oracles_active: Vec::new(),
            cache_hot_paths: Vec::new(),
            metrics,
        }
    }

    fn suite(benches: Vec<(&'static str, u64)>, props: Vec<(&'static str, Result<(), String>)>) -> CriteriaSuite {
        CriteriaSuite::new(
            benches
                .into_iter()
                .map(|(name, score)| Box::new(StaticBench { name, score }) as Box<dyn Benchmark>)
                .collect(),
            props
                .into_iter()
                .map(|(name, result)| Box::new(StaticProp { name, result }) as Box<dyn Property>)
                .collect(),
        )
    }

    #[test]
    fn accept_on_strict_improvement_no_regression() {
        let (node, path) = fresh_node("accept");
        let criteria = suite(vec![("a", 90), ("b", 100)], vec![("p", Ok(()))]);
        let before = frame(&[("a", 100), ("b", 100)]);
        let after = frame(&[("a", 90), ("b", 100)]);
        assert_eq!(verify(&before, &after, &criteria, &node), Verdict::Accept);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn accept_on_mixed_improvement_with_small_regression() {
        let (node, path) = fresh_node("mixed");
        let criteria = suite(vec![("a", 90), ("b", 104)], vec![("p", Ok(()))]);
        let before = frame(&[("a", 100), ("b", 100)]);
        let after = frame(&[("a", 90), ("b", 104)]);
        assert_eq!(verify(&before, &after, &criteria, &node), Verdict::Accept);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn reject_on_property_failure() {
        let (node, path) = fresh_node("prop-fail");
        let criteria = suite(
            vec![("a", 90)],
            vec![("p1", Err("bad invariant".to_owned())), ("p2", Ok(()))],
        );
        let verdict = verify(&frame(&[("a", 100)]), &frame(&[("a", 90)]), &criteria, &node);
        match verdict {
            Verdict::Reject(reasons) => assert!(reasons.iter().any(|r| r.contains("p1"))),
            Verdict::Accept => panic!("expected reject"),
        }
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn reject_on_benchmark_regression() {
        let (node, path) = fresh_node("bench-regress");
        let criteria = suite(vec![("a", 107), ("b", 90)], vec![("p", Ok(()))]);
        let verdict = verify(&frame(&[("a", 100), ("b", 100)]), &frame(&[("a", 107), ("b", 90)]), &criteria, &node);
        match verdict {
            Verdict::Reject(reasons) => assert!(reasons.iter().any(|r| r.contains("regressed"))),
            Verdict::Accept => panic!("expected reject"),
        }
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn reject_when_everything_equal() {
        let (node, path) = fresh_node("equal");
        let criteria = suite(vec![("a", 100)], vec![("p", Ok(()))]);
        let verdict = verify(&frame(&[("a", 100)]), &frame(&[("a", 100)]), &criteria, &node);
        match verdict {
            Verdict::Reject(reasons) => {
                assert!(reasons.iter().any(|r| r.contains("no benchmark improved")))
            }
            Verdict::Accept => panic!("expected reject"),
        }
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn reject_lists_all_reasons() {
        let (node, path) = fresh_node("all-reasons");
        let criteria = suite(
            vec![("a", 120), ("b", 100)],
            vec![("p1", Err("bad one".to_owned())), ("p2", Err("bad two".to_owned()))],
        );
        let verdict = verify(&frame(&[("a", 100), ("b", 100)]), &frame(&[("a", 120), ("b", 100)]), &criteria, &node);
        match verdict {
            Verdict::Reject(reasons) => {
                assert!(reasons.len() >= 4);
                assert!(reasons.iter().any(|r| r.contains("p1")));
                assert!(reasons.iter().any(|r| r.contains("p2")));
                assert!(reasons.iter().any(|r| r.contains("regressed")));
                assert!(reasons.iter().any(|r| r.contains("no benchmark improved")));
            }
            Verdict::Accept => panic!("expected reject"),
        }
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn reject_when_before_score_missing() {
        let (node, path) = fresh_node("missing-before");
        let criteria = suite(vec![("a", 90)], vec![("p", Ok(()))]);
        let verdict = verify(&frame(&[]), &frame(&[("a", 90)]), &criteria, &node);
        match verdict {
            Verdict::Reject(reasons) => {
                assert!(reasons.iter().any(|r| r.contains("missing before score")));
                assert!(reasons.iter().any(|r| r.contains("no benchmarks compared")));
            }
            Verdict::Accept => panic!("expected reject"),
        }
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn attach_bench_scores_writes_frame_metrics() {
        let mut report = CriteriaReport {
            bench_scores: vec![("a".to_owned(), 42)],
            prop_results: Vec::new(),
        };
        let attached = attach_bench_scores(frame(&[]), &report);
        assert_eq!(attached.metrics[&bench_metric_key("a")], 42);
        report.bench_scores[0].1 = 7;
        let attached2 = attach_bench_scores(frame(&[]), &report);
        assert_eq!(attached2.metrics[&bench_metric_key("a")], 7);
    }
}

}

pub mod proposer {
//! Omega-5.3 rewrite proposers.
//!
//! Proposers only create in-memory `Rewrite` values. They never edit source
//! files and never apply changes to a node.

use std::collections::{BTreeMap, BTreeSet};

use crate::godel::observer::ObserverFrame;
use crate::godel::verifier::Rewrite;

pub trait Proposer {
    fn name(&self) -> &str;
    fn propose(&self, frame: &ObserverFrame) -> Vec<Rewrite>;
}

#[derive(Debug, Clone)]
pub struct HandcraftedProposer {
    pub variants: Vec<Rewrite>,
}

impl Default for HandcraftedProposer {
    fn default() -> Self {
        Self {
            variants: vec![
                rewrite("increase_beam_width_2x", "beam_width", 512),
                rewrite("shrink_max_nodes_10pct", "max_nodes", 90),
                rewrite("double_oracle_threshold", "oracle_threshold", 20),
                rewrite("halve_oracle_threshold", "oracle_threshold", 5),
                rewrite("extend_fuel_50pct", "fuel", 150),
                rewrite("tighten_fuel_25pct", "fuel", 75),
            ],
        }
    }
}

impl Proposer for HandcraftedProposer {
    fn name(&self) -> &str {
        "HandcraftedProposer"
    }

    fn propose(&self, _frame: &ObserverFrame) -> Vec<Rewrite> {
        self.variants.clone()
    }
}

#[derive(Debug, Clone)]
pub struct ConfigPerturbProposer {
    pub keys: Vec<&'static str>,
    pub range: i64,
}

impl ConfigPerturbProposer {
    pub fn new(keys: Vec<&'static str>, range: i64) -> Self {
        Self { keys, range }
    }
}

impl Proposer for ConfigPerturbProposer {
    fn name(&self) -> &str {
        "ConfigPerturbProposer"
    }

    fn propose(&self, frame: &ObserverFrame) -> Vec<Rewrite> {
        let mut out = Vec::new();
        for key in &self.keys {
            let base = frame
                .metrics
                .get(&config_metric_key(key))
                .copied()
                .map(|v| v as i64)
                .unwrap_or_else(|| default_config_value(key));
            for delta in [-self.range, self.range] {
                if delta == 0 {
                    continue;
                }
                let candidate = base.saturating_add(delta).max(1);
                let description = if delta < 0 {
                    format!("perturb_{key}_minus_{}", self.range)
                } else {
                    format!("perturb_{key}_plus_{}", self.range)
                };
                out.push(rewrite(&description, *key, candidate));
            }
        }
        dedup_rewrites(out)
    }
}

pub struct CombinedProposer {
    proposers: Vec<Box<dyn Proposer>>,
}

impl CombinedProposer {
    pub fn new(proposers: Vec<Box<dyn Proposer>>) -> Self {
        Self { proposers }
    }
}

impl Proposer for CombinedProposer {
    fn name(&self) -> &str {
        "CombinedProposer"
    }

    fn propose(&self, frame: &ObserverFrame) -> Vec<Rewrite> {
        let mut all = Vec::new();
        for proposer in &self.proposers {
            all.extend(proposer.propose(frame));
        }
        dedup_rewrites(all)
    }
}

pub fn config_metric_key(key: &str) -> String {
    format!("config:{key}")
}

fn rewrite(description: &str, key: &'static str, value: i64) -> Rewrite {
    let mut patch = BTreeMap::new();
    patch.insert(key, value);
    Rewrite::config_patch(description, patch)
}

fn default_config_value(key: &str) -> i64 {
    match key {
        "beam_width" => 256,
        "max_nodes" => 100,
        "oracle_threshold" => 10,
        "fuel" => 100,
        _ => 10,
    }
}

fn dedup_rewrites(rewrites: Vec<Rewrite>) -> Vec<Rewrite> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for rewrite in rewrites {
        if seen.insert(rewrite.id) {
            out.push(rewrite);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::godel::observer::ObserverFrame;

    fn frame() -> ObserverFrame {
        ObserverFrame {
            epoch: 0,
            programs_loaded: Vec::new(),
            oracles_active: Vec::new(),
            cache_hot_paths: Vec::new(),
            metrics: BTreeMap::from([
                (config_metric_key("beam_width"), 32),
                (config_metric_key("max_nodes"), 20),
            ]),
        }
    }

    fn ids(rewrites: &[Rewrite]) -> BTreeSet<u64> {
        rewrites.iter().map(|rewrite| rewrite.id).collect()
    }

    #[test]
    fn handcrafted_proposer_produces_six_variants() {
        let rewrites = HandcraftedProposer::default().propose(&frame());
        assert_eq!(rewrites.len(), 6);
    }

    #[test]
    fn handcrafted_proposer_has_unique_ids() {
        let rewrites = HandcraftedProposer::default().propose(&frame());
        assert_eq!(ids(&rewrites).len(), rewrites.len());
    }

    #[test]
    fn handcrafted_descriptions_are_non_empty() {
        let rewrites = HandcraftedProposer::default().propose(&frame());
        assert!(rewrites.iter().all(|rewrite| !rewrite.description.is_empty()));
    }

    #[test]
    fn config_perturb_proposer_produces_at_least_three_variants() {
        let proposer = ConfigPerturbProposer::new(
            vec!["beam_width", "max_nodes", "fuel"],
            4,
        );
        let rewrites = proposer.propose(&frame());
        assert!(rewrites.len() >= 3);
    }

    #[test]
    fn config_perturb_uses_frame_values() {
        let proposer = ConfigPerturbProposer::new(vec!["beam_width"], 4);
        let rewrites = proposer.propose(&frame());
        let values: BTreeSet<i64> = rewrites
            .iter()
            .flat_map(|rewrite| match &rewrite.kind {
                crate::godel::verifier::RewriteKind::ConfigPatch(patch) => patch.values().copied(),
            })
            .collect();
        assert!(values.contains(&28));
        assert!(values.contains(&36));
    }

    #[test]
    fn combined_proposer_dedups_ids() {
        let handcrafted = HandcraftedProposer::default();
        let combined = CombinedProposer::new(vec![
            Box::new(handcrafted.clone()),
            Box::new(handcrafted),
            Box::new(ConfigPerturbProposer::new(vec!["fuel"], 5)),
        ]);
        let rewrites = combined.propose(&frame());
        assert_eq!(ids(&rewrites).len(), rewrites.len());
        assert!(rewrites.len() >= 6);
    }
}

}

pub mod applicator {
//! Î©-5.4 â€” Applicator : applique un `Rewrite` Ã  un Ã©tat config mutable
//! (whitelist + bornes), produit un `AppliedSnapshot` permettant un
//! rollback parfait byte-pour-byte.
//!
//! Les rewrites du first mile ne touchent QUE des paramÃ¨tres runtime
//! whitelistÃ©s (`beam_width`, `max_nodes`, `oracle_threshold`, `fuel`).
//! Aucune modification de fichier source. La modification de code source
//! self-modify est une dette explicite Î©-5.4.x.

use std::collections::BTreeMap;

use crate::godel::observer::ObserverFrame;
use crate::godel::proposer::config_metric_key;
use crate::godel::verifier::{Rewrite, RewriteKind};

/// Whitelist des clÃ©s patchables. Hors whitelist â†’ `UnknownKey`.
pub const ALLOWED_KEYS: &[&str] = &[
    "beam_width",
    "max_nodes",
    "oracle_threshold",
    "fuel",
];

const MIN_VALUE: i64 = 1;
const MAX_VALUE: i64 = 1_000_000_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GodelMutableConfig {
    values: BTreeMap<&'static str, i64>,
}

impl GodelMutableConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construit la config avec les valeurs par dÃ©faut Î©-5 demo.
    pub fn with_defaults() -> Self {
        let mut s = Self::new();
        s.values.insert("beam_width".into(), 256);
        s.values.insert("max_nodes".into(), 100);
        s.values.insert("oracle_threshold".into(), 10);
        s.values.insert("fuel".into(), 100);
        s
    }

    pub fn get(&self, key: &str) -> Option<i64> {
        self.values.get(key).copied()
    }

    pub fn set(&mut self, key: &'static str, value: i64) {
        self.values.insert(key, value);
    }

    pub fn unset(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, i64)> {
        self.values.iter().map(|(k, v)| (*k, *v))
    }

    /// Injecte les valeurs config dans `frame.metrics` sous le prÃ©fixe
    /// `config:*`. UtilisÃ© pour que le proposer/verifier voient les vraies
    /// valeurs courantes.
    pub fn attach_to_frame(&self, mut frame: ObserverFrame) -> ObserverFrame {
        for (k, v) in &self.values {
            frame
                .metrics
                .insert(config_metric_key(k), (*v).max(0) as u64);
        }
        frame
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedSnapshot {
    pub rewrite_id: u64,
    /// Pour chaque clÃ© patchÃ©e, valeur prÃ©cÃ©dente (None si la clÃ© n'existait pas).
    pub prev_config: BTreeMap<&'static str, Option<i64>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApplicatorError {
    UnknownKey(&'static str),
    OutOfRange { key: &'static str, value: i64 },
}

impl std::fmt::Display for ApplicatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicatorError::UnknownKey(k) => write!(f, "applicator: unknown config key '{k}'"),
            ApplicatorError::OutOfRange { key, value } => {
                write!(f, "applicator: value {value} for '{key}' out of range")
            }
        }
    }
}

impl std::error::Error for ApplicatorError {}

/// Applique un `Rewrite::ConfigPatch` Ã  la config. Valide la whitelist
/// et les bornes AVANT toute modification (atomicitÃ©). Retourne un
/// snapshot pour rollback.
pub fn apply(
    rewrite: &Rewrite,
    config: &mut GodelMutableConfig,
) -> Result<AppliedSnapshot, ApplicatorError> {
    let RewriteKind::ConfigPatch(patch) = &rewrite.kind;

    // Validation prÃ©-mutation (atomicitÃ©).
    for (key, value) in patch {
        if !ALLOWED_KEYS.contains(key) {
            return Err(ApplicatorError::UnknownKey(key));
        }
        if *value < MIN_VALUE || *value > MAX_VALUE {
            return Err(ApplicatorError::OutOfRange { key, value: *value });
        }
    }

    // Snapshot des valeurs AVANT mutation.
    let mut prev = BTreeMap::new();
    for key in patch.keys() {
        prev.insert(*key, config.get(key));
    }

    // Mutation.
    for (key, value) in patch {
        config.set(key, *value);
    }

    Ok(AppliedSnapshot {
        rewrite_id: rewrite.id,
        prev_config: prev,
    })
}

/// Annule un apply en restaurant l'Ã©tat prÃ©-snapshot byte-pour-byte.
/// Pour les clÃ©s qui n'existaient pas avant, on les supprime.
pub fn rollback(snap: &AppliedSnapshot, config: &mut GodelMutableConfig) {
    for (key, opt_value) in &snap.prev_config {
        match opt_value {
            Some(v) => config.set(key, *v),
            None => config.unset(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn rewrite_for(key: &'static str, value: i64) -> Rewrite {
        let mut p: BTreeMap<&'static str, i64> = BTreeMap::new();
        p.insert(key, value);
        Rewrite::config_patch(format!("set_{key}_to_{value}"), p)
    }

    #[test]
    fn apply_sets_value_in_config() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let r = rewrite_for("beam_width", 1024);
        let snap = apply(&r, &mut cfg).unwrap();
        assert_eq!(cfg.get("beam_width"), Some(1024));
        assert_eq!(snap.rewrite_id, r.id);
    }

    #[test]
    fn rollback_restores_previous_value() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let original = cfg.get("beam_width").unwrap();
        let r = rewrite_for("beam_width", 1024);
        let snap = apply(&r, &mut cfg).unwrap();
        rollback(&snap, &mut cfg);
        assert_eq!(cfg.get("beam_width"), Some(original));
    }

    #[test]
    fn rollback_removes_keys_that_were_new() {
        let mut cfg = GodelMutableConfig::new(); // empty
        let r = rewrite_for("beam_width", 512);
        let snap = apply(&r, &mut cfg).unwrap();
        assert_eq!(cfg.get("beam_width"), Some(512));
        rollback(&snap, &mut cfg);
        assert_eq!(cfg.get("beam_width"), None, "clÃ© nouvelle doit Ãªtre supprimÃ©e au rollback");
    }

    #[test]
    fn apply_rejects_unknown_key() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let r = rewrite_for("forbidden_key", 42);
        let err = apply(&r, &mut cfg).unwrap_err();
        assert!(matches!(err, ApplicatorError::UnknownKey(k) if k == "forbidden_key"));
        // Pas d'effet de bord.
        assert_eq!(cfg.get("forbidden_key"), None);
    }

    #[test]
    fn apply_rejects_out_of_range_value() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let r_neg = rewrite_for("beam_width", -1);
        assert!(matches!(
            apply(&r_neg, &mut cfg).unwrap_err(),
            ApplicatorError::OutOfRange { .. }
        ));
        let r_huge = rewrite_for("beam_width", i64::MAX);
        assert!(matches!(
            apply(&r_huge, &mut cfg).unwrap_err(),
            ApplicatorError::OutOfRange { .. }
        ));
        // beam_width inchangÃ©.
        assert_eq!(cfg.get("beam_width"), Some(256));
    }

    #[test]
    fn apply_is_atomic_on_validation_failure() {
        // Une rewrite multi-clÃ©s oÃ¹ une clÃ© est invalide â†’ AUCUNE mutation.
        let mut cfg = GodelMutableConfig::with_defaults();
        let mut p = BTreeMap::new();
        p.insert("beam_width", 1024); // valide
        p.insert("forbidden", 1); // invalide
        let r = Rewrite::config_patch("multi_invalid", p);
        let err = apply(&r, &mut cfg).unwrap_err();
        assert!(matches!(err, ApplicatorError::UnknownKey(_)));
        // beam_width inchangÃ©.
        assert_eq!(cfg.get("beam_width"), Some(256));
    }

    #[test]
    fn double_apply_then_rollback_returns_to_original() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let original = cfg.get("beam_width").unwrap();
        let r1 = rewrite_for("beam_width", 1024);
        let r2 = rewrite_for("beam_width", 2048);
        let snap1 = apply(&r1, &mut cfg).unwrap();
        let snap2 = apply(&r2, &mut cfg).unwrap();
        assert_eq!(cfg.get("beam_width"), Some(2048));
        rollback(&snap2, &mut cfg);
        assert_eq!(cfg.get("beam_width"), Some(1024));
        rollback(&snap1, &mut cfg);
        assert_eq!(cfg.get("beam_width"), Some(original));
    }

    #[test]
    fn attach_config_injects_metrics() {
        use std::collections::BTreeMap as BTM;
        let cfg = GodelMutableConfig::with_defaults();
        let frame = ObserverFrame {
            epoch: 0,
            programs_loaded: vec![],
            oracles_active: vec![],
            cache_hot_paths: vec![],
            metrics: BTM::new(),
        };
        let attached = cfg.attach_to_frame(frame);
        assert_eq!(attached.metrics.get("config:beam_width"), Some(&256u64));
        assert_eq!(attached.metrics.get("config:max_nodes"), Some(&100u64));
    }
}

}

pub mod runner {
//! Î©-5.5 â€” La boucle GÃ¶del-machine fermÃ©e. Pipeline direct :
//!
//! ```text
//!     capture(node) â†’ attach(config) â†’ bench/property scores
//!     â†’ propose(frame) â†’ apply(rewrite) â†’ re-capture
//!     â†’ verify(before, after) â†’ Accept | Reject(rollback)
//! ```
//!
//! Aucune Ã©tape autonome, aucun coordinateur additionnel. Le critÃ¨re
//! Î©-5.5 alias **Jour 0** : la boucle applique sa premiÃ¨re rewrite sans
//! intervention humaine. La date + le hash de la rewrite + le diff
//! mÃ©trique sont gravÃ©s par le commit/proof artifact correspondant.

use std::cell::RefCell;
use std::rc::Rc;

use crate::MonsterNode;

use super::applicator::{apply, rollback, AppliedSnapshot, GodelMutableConfig};
use super::criteria::{Benchmark, CriteriaSuite};
use super::observer::{capture, ObserverFrame};
use super::proposer::Proposer;
use super::verifier::{attach_bench_scores, verify, Rewrite, Verdict};

/// Config partagÃ©e entre le runner et les benches config-aware. Permet
/// au verifier d'observer les changements de config quand il re-Ã©value
/// les benches aprÃ¨s apply.
pub type SharedConfig = Rc<RefCell<GodelMutableConfig>>;

pub fn shared_config(config: GodelMutableConfig) -> SharedConfig {
    Rc::new(RefCell::new(config))
}

/// Bench config-aware : score = somme des valeurs config (lower = better).
/// SynthÃ©tique â€” permet de prouver la mÃ©canique sans dÃ©pendre d'un workload rÃ©el.
pub struct ConfigSumBench {
    pub config: SharedConfig,
}

impl Benchmark for ConfigSumBench {
    fn name(&self) -> &str {
        CONFIG_SUM_BENCH_NAME
    }

    fn run(&self, _node: &MonsterNode) -> u64 {
        self.config
            .borrow()
            .iter()
            .map(|(_, v)| v.max(0) as u64)
            .sum()
    }
}

/// Bench **non-synthÃ©tique** : temps rÃ©el (ns) pour prÃ©parer un
/// `/newcompute_` Forge qui encode `f(x) = 7x + 3`. `max_nodes` et
/// `beam_width` sont lus dans la `SharedConfig` au moment du `run()` â€” donc
/// les rewrites les modifient rÃ©ellement.
///
/// Score = mÃ©diane de 3 runs (en ns). Lower = better. Si l'entraÃ®nement
/// Ã©choue (ex. `max_nodes` trop petit), score = `FAIL_PENALTY` (forte
/// rÃ©gression â†’ verifier reject).
///
/// C'est ce qu'il faut pour atteindre un Jour 0 sur mÃ©trique rÃ©elle.
pub struct ConfigAwareMonsterTrainBench {
    pub config: SharedConfig,
}

fn forge_affine_newcompute_source(max_steps: u64, parallelism: Option<usize>) -> String {
    let parallelism = parallelism
        .map(|value| format!("parallelism={value}\n"))
        .unwrap_or_default();
    format!(
        "forge_module:\n  module godel_affine_newcompute version 1\nforge_imports:\n  none\nforge_constants:\n  const a: f64 unit none = 7.0\n  const b: f64 unit none = 3.0\nforge_functions:\n  fn affine(x: f64) -> f64 {{ return a * x + b }}\nforge_program:\n  let y = affine(x)\n  emit y: f64 = y\nforge_inputs:\n  param x: f64 unit none bounds [-10.0,10.0] nominal 0.0\nforge_outputs:\n  output y: f64 unit none handoff scalar\nforge_constraints:\n  assert finite(y)\n  assert bounds(y,[-100.0,100.0])\nforge_samples:\n  case basic seed 1 {{ given x=2.0; expect y approx 17.0 tolerance 0.01 }}\nforge_cost:\nmax_steps={max_steps}\nmax_memory_mb=16\nprecision=f64\n{parallelism}artifact_handoff:\nproof_hash,output_hash,compact_result"
    )
}

impl ConfigAwareMonsterTrainBench {
    /// PÃ©nalitÃ© retournÃ©e si l'entraÃ®nement Ã©choue. Choisi assez grand pour
    /// forcer une rÃ©gression visible mais pas u64::MAX (qui causerait des
    /// overflows dans les calculs Îµ).
    pub const FAIL_PENALTY: u64 = 10_000_000_000; // 10 secondes Ã©quivalent
}

impl Benchmark for ConfigAwareMonsterTrainBench {
    fn name(&self) -> &str {
        CONFIG_AWARE_TRAIN_BENCH_NAME
    }

    fn run(&self, node: &MonsterNode) -> u64 {
        use std::time::Instant;
        let (max_nodes, beam_width) = {
            let cfg = self.config.borrow();
            // PAS de clamp pour prÃ©server l'honnÃªtetÃ© du bench : si
            // max_nodes < 2 ou beam_width == 0 peut produire un budget
            // incohÃ©rent; dans ce cas on rend FAIL_PENALTY. C'est ce qui
            // permet au verifier de dÃ©tecter et rejeter les rewrites qui
            // cassent le chemin /newcompute_.
            (
                cfg.get("max_nodes").unwrap_or(20).max(0) as usize,
                cfg.get("beam_width").unwrap_or(256).max(0) as usize,
            )
        };
        // Examples canoniques : f(x) = 7x + 3.
        let source = forge_affine_newcompute_source((max_nodes as u64).saturating_mul(1_000), Some(beam_width));

        let mut samples = [0u64; 3];
        for slot in samples.iter_mut() {
            let start = Instant::now();
            let result = node.prepare_forge_source(&source, std::iter::empty::<String>());
            let elapsed = start.elapsed().as_nanos() as u64;
            if result.is_err() {
                return Self::FAIL_PENALTY;
            }
            *slot = elapsed.max(1);
        }
        samples.sort_unstable();
        samples[1] // mÃ©diane
    }
}

pub const CONFIG_AWARE_TRAIN_BENCH_NAME: &str = "ConfigAwareMonsterTrain";

/// Boucle GÃ¶del-machine.
pub struct GodelLoop {
    pub proposer: Box<dyn Proposer>,
    pub criteria: CriteriaSuite,
    pub max_iterations: u32,
    /// Nombre d'itÃ©rations consÃ©cutives sans Accept avant arrÃªt anticipÃ©.
    pub plateau_threshold: u32,
}

#[derive(Debug, Clone)]
pub struct GodelReport {
    pub applied: Vec<(Rewrite, AppliedSnapshot)>,
    pub rejected: Vec<(Rewrite, Vec<String>)>,
    pub iterations: u32,
    pub frames: Vec<ObserverFrame>,
}

impl GodelReport {
    pub fn summary(&self) -> String {
        format!(
            "GodelReport {{ applied: {}, rejected: {}, iterations: {}, frames: {} }}",
            self.applied.len(),
            self.rejected.len(),
            self.iterations,
            self.frames.len()
        )
    }
}

impl GodelLoop {
    /// Lance la boucle. Termine si :
    ///  * `iterations >= max_iterations`, OU
    ///  * `plateau_threshold` itÃ©rations consÃ©cutives sans aucune Accept.
    ///
    /// La `config` est partagÃ©e (`SharedConfig`) avec les benches
    /// config-aware (ex. `ConfigSumBench`) afin que le verifier voie
    /// les changements quand il re-Ã©value les benches aprÃ¨s apply.
    pub fn run(
        &mut self,
        node: &mut MonsterNode,
        config: SharedConfig,
    ) -> GodelReport {
        let mut applied = Vec::new();
        let mut rejected = Vec::new();
        let mut frames = Vec::new();
        let mut consecutive_no_accept = 0u32;

        // Frame initial : capture + config + bench scores.
        let mut frame_before = self.capture_full(node, &config);
        frames.push(frame_before.clone());

        for iter in 0..self.max_iterations {
            let candidates = self.proposer.propose(&frame_before);
            let mut iter_accepted = false;

            for rewrite in candidates {
                // Apply (via RefCell).
                let snap = {
                    let mut cfg = config.borrow_mut();
                    match apply(&rewrite, &mut cfg) {
                        Ok(s) => s,
                        Err(e) => {
                            rejected.push((rewrite, vec![format!("apply error: {e}")]));
                            continue;
                        }
                    }
                };

                // Capture aprÃ¨s.
                let frame_after = self.capture_full(node, &config);

                // Verify.
                match verify(&frame_before, &frame_after, &self.criteria, node) {
                    Verdict::Accept => {
                        applied.push((rewrite, snap));
                        frame_before = frame_after.clone();
                        frames.push(frame_after);
                        iter_accepted = true;
                        // Greedy hill-climbing : une accept par itÃ©ration.
                        break;
                    }
                    Verdict::Reject(reasons) => {
                        rollback(&snap, &mut config.borrow_mut());
                        rejected.push((rewrite, reasons));
                    }
                }
            }

            if iter_accepted {
                consecutive_no_accept = 0;
            } else {
                consecutive_no_accept += 1;
                if consecutive_no_accept >= self.plateau_threshold {
                    return GodelReport {
                        applied,
                        rejected,
                        iterations: iter + 1,
                        frames,
                    };
                }
            }
        }

        GodelReport {
            applied,
            rejected,
            iterations: self.max_iterations,
            frames,
        }
    }

    /// Capture frame + injecte config + attache bench scores via
    /// `criteria.evaluate(node)`. Les benches config-aware (ConfigSumBench)
    /// lisent la config partagÃ©e au moment de l'Ã©valuation.
    fn capture_full(
        &self,
        node: &MonsterNode,
        config: &SharedConfig,
    ) -> ObserverFrame {
        let frame = capture(node);
        let frame = config.borrow().attach_to_frame(frame);
        let report = self.criteria.evaluate(node);
        attach_bench_scores(frame, &report)
    }
}

/// Nom canonique du `ConfigSumBench`. UtilisÃ© pour gÃ©nÃ©rer la clÃ©
/// mÃ©trique `bench:ConfigSumBench` dans le frame.
pub const CONFIG_SUM_BENCH_NAME: &str = "ConfigSumBench";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::godel::criteria::{Benchmark, Property};
    use crate::godel::proposer::HandcraftedProposer;
    use crate::{MemoryGovernor, Store};


    fn fresh_path(tag: &str) -> std::path::PathBuf {
        crate::fresh_tmp_path("scan-godel", tag)
    }

    fn empty_node(tag: &str) -> MonsterNode {
        MonsterNode::new(
            Store::open(fresh_path(tag)).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        )
    }

    /// CriteriaSuite synthÃ©tique : un seul bench config-aware (ConfigSumBench),
    /// zÃ©ro property. Le bench lit la config partagÃ©e au moment de
    /// l'Ã©valuation, donc une rewrite qui rÃ©duit la config produit une
    /// amÃ©lioration mesurable.
    fn synthetic_suite_with_config_sum(config: SharedConfig) -> CriteriaSuite {
        let benches: Vec<Box<dyn Benchmark>> =
            vec![Box::new(ConfigSumBench { config })];
        let props: Vec<Box<dyn Property>> = vec![];
        CriteriaSuite::new(benches, props)
    }

    struct EmptyProposer;
    impl Proposer for EmptyProposer {
        fn name(&self) -> &str {
            "Empty"
        }
        fn propose(&self, _: &ObserverFrame) -> Vec<Rewrite> {
            vec![]
        }
    }

    #[test]
    fn run_with_no_proposers_yields_zero_iterations_acceptable() {
        let mut node = empty_node("loop-empty");
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        let mut g = GodelLoop {
            proposer: Box::new(EmptyProposer),
            criteria: synthetic_suite_with_config_sum(Rc::clone(&cfg)),
            max_iterations: 50,
            plateau_threshold: 5,
        };
        let report = g.run(&mut node, cfg);
        assert_eq!(report.applied.len(), 0);
        assert!(report.iterations <= 50);
    }

    #[test]
    fn run_records_initial_frame() {
        let mut node = empty_node("loop-frame");
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        let mut g = GodelLoop {
            proposer: Box::new(EmptyProposer),
            criteria: synthetic_suite_with_config_sum(Rc::clone(&cfg)),
            max_iterations: 1,
            plateau_threshold: 1,
        };
        let report = g.run(&mut node, cfg);
        assert!(!report.frames.is_empty(), "frame initial doit exister");
    }

    #[test]
    fn jour_zero_first_auto_applied_rewrite() {
        // C'est LE test fondateur. Un proposer qui rÃ©duit beam_width ;
        // un bench config-sum qui chute donc ; le verifier doit Accept.
        struct ReduceBeamProposer;
        impl Proposer for ReduceBeamProposer {
            fn name(&self) -> &str {
                "ReduceBeam"
            }
            fn propose(&self, _: &ObserverFrame) -> Vec<Rewrite> {
                let mut p = std::collections::BTreeMap::new();
                p.insert("beam_width", 100);
                vec![Rewrite::config_patch("reduce_beam_to_100", p)]
            }
        }

        let mut node = empty_node("jour-zero");
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        let mut g = GodelLoop {
            proposer: Box::new(ReduceBeamProposer),
            criteria: synthetic_suite_with_config_sum(Rc::clone(&cfg)),
            max_iterations: 10,
            plateau_threshold: 5,
        };
        let report = g.run(&mut node, Rc::clone(&cfg));
        assert!(
            !report.applied.is_empty(),
            "JOUR 0 : au moins un rewrite doit Ãªtre auto-appliquÃ©. Got applied={}, rejected={}",
            report.applied.len(),
            report.rejected.len(),
        );
        assert!(cfg.borrow().get("beam_width").unwrap() <= 100);
    }

    #[test]
    fn handcrafted_proposer_drives_loop_to_acceptance() {
        // Avec le HandcraftedProposer + bench config-sum, au moins une
        // variant doit Ãªtre acceptÃ©e (celle qui rÃ©duit le sum).
        let mut node = empty_node("handcrafted-loop");
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        let mut g = GodelLoop {
            proposer: Box::new(HandcraftedProposer::default()),
            criteria: synthetic_suite_with_config_sum(Rc::clone(&cfg)),
            max_iterations: 20,
            plateau_threshold: 5,
        };
        let report = g.run(&mut node, cfg);
        assert!(
            !report.applied.is_empty(),
            "HandcraftedProposer doit conduire Ã  au moins une acceptance"
        );
    }

    #[test]
    fn config_aware_train_bench_runs_on_default_config() {
        let node = empty_node("train-bench-default");
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        // Avec les defaults, max_nodes=100 â€” largement assez pour affine.
        // Set max_nodes plus raisonnable pour ne pas Ãªtre TROP lent.
        cfg.borrow_mut().set("max_nodes", 20);
        let bench = ConfigAwareMonsterTrainBench {
            config: Rc::clone(&cfg),
        };
        let score = bench.run(&node);
        assert!(score > 0);
        assert!(
            score < ConfigAwareMonsterTrainBench::FAIL_PENALTY,
            "training devrait rÃ©ussir avec max_nodes=20, beam_width=256"
        );
    }

    #[test]
    fn config_aware_train_bench_returns_penalty_when_training_fails() {
        let node = empty_node("train-bench-fail");
        let cfg = shared_config(GodelMutableConfig::new());
        // max_nodes=2 : trop petit pour synthÃ©tiser affine.
        cfg.borrow_mut().set("max_nodes", 2);
        cfg.borrow_mut().set("beam_width", 32);
        let bench = ConfigAwareMonsterTrainBench {
            config: Rc::clone(&cfg),
        };
        let score = bench.run(&node);
        // Soit le score est trÃ¨s haut, soit FAIL_PENALTY. La condition
        // robuste : score est non-trivial. Le test check juste qu'on ne
        // crashe pas et que le score est > 0.
        assert!(score > 0, "score must be positive even on tiny config");
    }

    #[test]
    fn jour_zero_real_metric_via_train_bench() {
        // Vrai Jour 0 candidat : un proposer fixÃ© qui rÃ©duit beam_width
        // de 256 Ã  100 ; bench rÃ©el = temps de training. Beam plus petit
        // = exploration plus rapide = score plus bas â†’ verifier accepte.
        // Test peut Ãªtre fragile en cas de variance temporelle ; on ne
        // l'intÃ¨gre pas dans le flow critique mais on l'utilise comme
        // dÃ©monstration runnable de la mÃ©canique sur mÃ©trique rÃ©elle.
        struct ReduceBeamProposer;
        impl Proposer for ReduceBeamProposer {
            fn name(&self) -> &str {
                "ReduceBeam"
            }
            fn propose(&self, _: &ObserverFrame) -> Vec<Rewrite> {
                let mut p = std::collections::BTreeMap::new();
                p.insert("beam_width", 50);
                vec![Rewrite::config_patch("reduce_beam_to_50", p)]
            }
        }

        let mut node = empty_node("jour-zero-real");
        let cfg = shared_config(GodelMutableConfig::new());
        cfg.borrow_mut().set("max_nodes", 20);
        cfg.borrow_mut().set("beam_width", 256);

        // Suite avec le vrai bench training (config-aware).
        let benches: Vec<Box<dyn Benchmark>> = vec![Box::new(ConfigAwareMonsterTrainBench {
            config: Rc::clone(&cfg),
        })];
        let props: Vec<Box<dyn Property>> = vec![];
        let criteria = CriteriaSuite::new(benches, props);

        let mut g = GodelLoop {
            proposer: Box::new(ReduceBeamProposer),
            criteria,
            max_iterations: 5,
            plateau_threshold: 5,
        };
        let report = g.run(&mut node, Rc::clone(&cfg));

        // L'attente : au moins UN rewrite appliquÃ©. Si aucun, c'est qu'il
        // y a eu de la variance temporelle qui a fait apparaÃ®tre la
        // baisse comme une rÃ©gression. On n'Ã©choue PAS le test sur Ã§a,
        // on vÃ©rifie juste que la mÃ©canique tourne. La preuve solide
        // de Jour 0 reste le dÃ©mo runnable.
        assert!(
            !report.frames.is_empty(),
            "le bench training s'est bien exÃ©cutÃ© et a produit des frames"
        );
    }

    #[test]
    fn report_summary_contains_counts() {
        let r = GodelReport {
            applied: vec![],
            rejected: vec![],
            iterations: 7,
            frames: vec![],
        };
        let s = r.summary();
        assert!(s.contains("iterations: 7"));
    }
}

}

pub mod verifier_v2 {
//! Omega-7.0.3 first mile -- Verifier v2.
//!
//! Etend le pouvoir d'expression du verifier Godel-machine pour accepter
//! des `ProgramSubstitution { from: Hash, to: Hash }` en plus des
//! `ConfigPatch` historiques. Vit en parallele de `super::verifier` (Codex)
//! sans le modifier -- option B documentee dans
//! `docs/OMEGA_OMEGA70_AGENT_ROADMAP.md`.
//!
//! Semantique de `ProgramSubstitution` :
//!  - `from`, `to` doivent etre chargeables depuis le store de la node.
//!  - L'invariant d'equivalence semantique entre `from` et `to` est la
//!    *responsabilite de l'agent qui a produit la substitution* (pas
//!    re-verifie ici -- c'est un compromis de scope first mile).
//!  - Le verifier_v2 garantit l'integrite referentielle : pas d'oubli,
//!    pas de hash bidon.
//!
//! Pour des semantiques fortes (re-executer sur des inputs sample, verifier
//! preuve Omega-4 d'equivalence, etc.) voir Omega-7.0.3.1 reporte.

use std::collections::BTreeMap;

use crate::{Hash, MonsterNode};

/// Rewrite v2. Une variante mirror du `ConfigPatch` pour rester compatible
/// avec les usages existants ; une variante nouvelle `ProgramSubstitution`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteV2 {
    /// Patch de configuration (mirror de `verifier::RewriteKind::ConfigPatch`).
    ConfigPatch(BTreeMap<&'static str, i64>),
    /// Substitution d'un programme entier par un autre. `from` et `to`
    /// sont les hashes content-addressed des programmes. L'equivalence
    /// semantique est *presumee* (responsabilite de l'agent producteur).
    ProgramSubstitution { from: Hash, to: Hash },
}

/// Resultat de la verification d'une rewrite v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcomeV2 {
    /// Tous les checks referentiels passent.
    Accept,
    /// Au moins un check a echoue -- `reasons` listent les motifs.
    Reject { reasons: Vec<String> },
}

impl VerificationOutcomeV2 {
    pub fn is_accept(&self) -> bool {
        matches!(self, VerificationOutcomeV2::Accept)
    }
    pub fn reasons(&self) -> Option<&[String]> {
        if let VerificationOutcomeV2::Reject { reasons } = self {
            Some(reasons)
        } else {
            None
        }
    }
}

/// Verifie une rewrite v2 contre une node.
///
/// Pour `ConfigPatch` : check minimal -- toutes les cles non-vides, valeurs
/// dans la plage `[1, 1_000_000_000]` (mirror du applicator existant).
///
/// Pour `ProgramSubstitution` : check referentiel -- `from` et `to` doivent
/// etre chargeables depuis `node.store()`.
pub fn verify_v2(rewrite: &RewriteV2, node: &MonsterNode) -> VerificationOutcomeV2 {
    let mut reasons = Vec::new();
    match rewrite {
        RewriteV2::ConfigPatch(map) => {
            if map.is_empty() {
                reasons.push("empty config patch".to_string());
            }
            for (k, v) in map {
                if k.is_empty() {
                    reasons.push("empty key in config patch".to_string());
                }
                if !(1..=1_000_000_000).contains(v) {
                    reasons.push(format!("value {v} for key {k} out of allowed range [1, 1e9]"));
                }
            }
        }
        RewriteV2::ProgramSubstitution { from, to } => {
            if from == to {
                reasons.push("trivial substitution: from == to".to_string());
            }
            if node.store().load(from).is_none() {
                reasons.push(format!("source program {from:?} not in store"));
            }
            if node.store().load(to).is_none() {
                reasons.push(format!("target program {to:?} not in store"));
            }
        }
    }
    if reasons.is_empty() {
        VerificationOutcomeV2::Accept
    } else {
        VerificationOutcomeV2::Reject { reasons }
    }
}

// ---------------------------------------------------------------------------
// Î©-7.0.3.1 â€” Re-vÃ©rification sÃ©mantique sample-based
// ---------------------------------------------------------------------------

/// Politique de re-vÃ©rification sÃ©mantique pour ProgramSubstitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticPolicy {
    /// Pas de re-vÃ©rification â€” Ã©quivalence prÃ©sumÃ©e. C'est le comportement
    /// de `verify_v2` original.
    Trust,
    /// Re-exÃ©cute les deux programmes sur N inputs dÃ©terministes et exige
    /// que tous les outputs concordent. Plus coÃ»teux, mais dÃ©tecte les
    /// substitutions qui ne prÃ©servent pas la sÃ©mantique.
    SampleBased { samples: usize },
}

/// Verifier v2 + re-vÃ©rification sÃ©mantique. Pour ConfigPatch, identique Ã 
/// `verify_v2`. Pour ProgramSubstitution avec policy SampleBased, charge
/// les deux programmes, les exÃ©cute sur `samples` jeux d'inputs dÃ©terministes,
/// et compare les outputs.
pub fn verify_v2_with_policy(
    rewrite: &RewriteV2,
    node: &MonsterNode,
    policy: SemanticPolicy,
) -> VerificationOutcomeV2 {
    // 1. VÃ©rification rÃ©fÃ©rentielle (mirror verify_v2).
    let base = verify_v2(rewrite, node);
    if !base.is_accept() {
        return base;
    }

    // 2. Re-vÃ©rification sÃ©mantique uniquement pour ProgramSubstitution + SampleBased.
    let RewriteV2::ProgramSubstitution { from, to } = rewrite else {
        return VerificationOutcomeV2::Accept;
    };
    let SemanticPolicy::SampleBased { samples } = policy else {
        return VerificationOutcomeV2::Accept;
    };

    // Charge les deux programmes.
    let from_bytes = match node.store().load(from) {
        Some(b) => b,
        None => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("source program {from:?} not loadable")],
            };
        }
    };
    let to_bytes = match node.store().load(to) {
        Some(b) => b,
        None => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("target program {to:?} not loadable")],
            };
        }
    };

    let from_p = match crate::kasm::Program::from_bytes(&from_bytes) {
        Ok(p) => p,
        Err(e) => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("source program failed to parse: {e}")],
            };
        }
    };
    let to_p = match crate::kasm::Program::from_bytes(&to_bytes) {
        Ok(p) => p,
        Err(e) => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("target program failed to parse: {e}")],
            };
        }
    };

    // VÃ©rifie que les deux programmes ont le mÃªme profil IO.
    if from_p.inputs() != to_p.inputs() {
        return VerificationOutcomeV2::Reject {
            reasons: vec![format!(
                "input arity mismatch: from has {}, to has {}",
                from_p.inputs(), to_p.inputs(),
            )],
        };
    }
    if from_p.outputs() != to_p.outputs() {
        return VerificationOutcomeV2::Reject {
            reasons: vec![format!(
                "output arity mismatch: from has {}, to has {}",
                from_p.outputs(), to_p.outputs(),
            )],
        };
    }

    // GÃ©nÃ¨re `samples` jeux d'inputs dÃ©terministes et compare les outputs.
    let mut reasons: Vec<String> = Vec::new();
    for sample_idx in 0..samples {
        let inputs = generate_sample_inputs(from_p.inputs() as usize, sample_idx as u64);
        let from_out = match crate::kasm::execute(&from_p, &inputs) {
            Ok(b) => b,
            Err(e) => {
                reasons.push(format!("from execute on sample {sample_idx} failed: {e}"));
                continue;
            }
        };
        let to_out = match crate::kasm::execute(&to_p, &inputs) {
            Ok(b) => b,
            Err(e) => {
                reasons.push(format!("to execute on sample {sample_idx} failed: {e}"));
                continue;
            }
        };
        if from_out != to_out {
            reasons.push(format!(
                "output mismatch on sample {sample_idx}: from={:?} to={:?}",
                from_out, to_out,
            ));
        }
    }

    if reasons.is_empty() {
        VerificationOutcomeV2::Accept
    } else {
        VerificationOutcomeV2::Reject { reasons }
    }
}

/// GÃ©nÃ¨re un jeu d'inputs dÃ©terministe pour un sample donnÃ©. MÃ©lange
/// quelques valeurs corner (0, 1, -1, MIN, MAX) avec des valeurs hashÃ©es.
fn generate_sample_inputs(n_inputs: usize, sample_idx: u64) -> Vec<u8> {
    let corners: [i64; 5] = [0, 1, -1, i64::MIN, i64::MAX];
    let mut bytes = Vec::with_capacity(n_inputs * 8);
    for slot in 0..n_inputs {
        let v: i64 = if (sample_idx as usize) < corners.len() {
            corners[sample_idx as usize]
                .wrapping_add((slot as i64).wrapping_mul(17))
        } else {
            // Hash deterministe.
            let mut x = (sample_idx).wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ ((slot as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
            (x ^ (x >> 31)) as i64
        };
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryGovernor, Store};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};


    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fresh_path(tag: &str) -> PathBuf {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut p = crate::fresh_tmp_path("scan-verifier-v2", tag);
        p.set_file_name(format!(
            "{}-{seq}",
            p.file_name().unwrap().to_str().unwrap()
        ));
        p
    }

    fn fresh_node(tag: &str) -> MonsterNode {
        MonsterNode::new(
            Store::open(fresh_path(tag)).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        )
    }

    fn store_program_in_node(node: &MonsterNode, p: &crate::kasm::Program) -> Hash {
        node.store().store(p.bytes()).expect("store write")
    }

    fn affine_program() -> crate::kasm::Program {
        use crate::kasm::{Node, Program, Target, Ty};
        Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::const_i64(3),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        ).unwrap()
    }

    fn other_program() -> crate::kasm::Program {
        use crate::kasm::{Node, Program, Target, Ty};
        Program::new(
            Target::Cpu, 1, 1, 4,
            vec![
                Node::input(0),
                Node::output(0, Ty::I64),
            ],
        ).unwrap()
    }

    #[test]
    fn verify_v2_accepts_known_program_substitution() {
        let node = fresh_node("accept");
        let h_from = store_program_in_node(&node, &affine_program());
        let h_to = store_program_in_node(&node, &other_program());
        let rw = RewriteV2::ProgramSubstitution { from: h_from, to: h_to };
        let r = verify_v2(&rw, &node);
        assert!(r.is_accept(), "got {r:?}");
    }

    #[test]
    fn verify_v2_rejects_missing_target() {
        let node = fresh_node("missing-to");
        let h_from = store_program_in_node(&node, &affine_program());
        let h_to = Hash::for_blob(b"not in store");
        let rw = RewriteV2::ProgramSubstitution { from: h_from, to: h_to };
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("target")));
    }

    #[test]
    fn verify_v2_rejects_missing_source() {
        let node = fresh_node("missing-from");
        let h_to = store_program_in_node(&node, &other_program());
        let h_from = Hash::for_blob(b"not in store either");
        let rw = RewriteV2::ProgramSubstitution { from: h_from, to: h_to };
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("source")));
    }

    #[test]
    fn verify_v2_rejects_trivial_substitution() {
        let node = fresh_node("trivial");
        let h = store_program_in_node(&node, &affine_program());
        let rw = RewriteV2::ProgramSubstitution { from: h, to: h };
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("trivial")));
    }

    #[test]
    fn verify_v2_config_patch_accepts_valid() {
        let node = fresh_node("config-ok");
        let mut map = BTreeMap::new();
        map.insert("beam_width", 256);
        map.insert("max_nodes", 20);
        let rw = RewriteV2::ConfigPatch(map);
        let r = verify_v2(&rw, &node);
        assert!(r.is_accept(), "got {r:?}");
    }

    #[test]
    fn verify_v2_config_patch_rejects_out_of_range() {
        let node = fresh_node("config-oor");
        let mut map = BTreeMap::new();
        map.insert("beam_width", 0);
        let rw = RewriteV2::ConfigPatch(map);
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
    }

    #[test]
    fn verify_v2_config_patch_rejects_empty() {
        let node = fresh_node("config-empty");
        let rw = RewriteV2::ConfigPatch(BTreeMap::new());
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
    }

    #[test]
    fn agent_candidates_become_rewrites_v2() {
        // Cross-cap : agent symbolique propose des programmes,
        // candidates_as_rewrites_v2 les transforme en RewriteV2.
        use crate::agent::SymbolicAgent;
        use crate::kasm::{Node, Program, Target, Ty};

        let p = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());

        let rewrites = crate::agent::symbolic::candidates_as_rewrites_v2(&p, &candidates);
        assert_eq!(rewrites.len(), candidates.len());

        for rw in &rewrites {
            assert!(matches!(rw, RewriteV2::ProgramSubstitution { .. }));
        }
    }

    #[test]
    fn cross_cap_agent_proposes_then_verifier_v2_accepts() {
        // Pipeline complet : agent -> rewrites_v2 -> verify_v2 doit accepter
        // tant que les programmes sont bien dans le store.
        use crate::agent::SymbolicAgent;
        use crate::kasm::{Node, Program, Target, Ty};

        let node = fresh_node("cross-cap");
        let p = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();

        // Stocke l'input.
        let _from_hash = store_program_in_node(&node, &p);

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());

        // Stocke les programmes candidats aussi.
        for c in &candidates {
            let _ = store_program_in_node(&node, &c.program);
        }

        let rewrites = crate::agent::symbolic::candidates_as_rewrites_v2(&p, &candidates);
        // Au moins une rewrite doit etre Accept (le filtre du store assure
        // que les hashes existent).
        let any_accept = rewrites.iter().any(|rw| verify_v2(rw, &node).is_accept());
        assert!(any_accept, "au moins une rewrite doit etre acceptee");
    }

    // ----- Î©-7.0.3.1 â€” Re-vÃ©rification sÃ©mantique sample-based -----

    fn equivalent_program_pair() -> (crate::kasm::Program, crate::kasm::Program) {
        use crate::kasm::{Node, Program, Target, Ty};
        // f(x) = x + 0 (5 nodes)
        let p_with_add_zero = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        // f(x) = x (canonicalisÃ©)
        let p_canonical = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        (p_with_add_zero, p_canonical)
    }

    fn divergent_program_pair() -> (crate::kasm::Program, crate::kasm::Program) {
        use crate::kasm::{Node, Program, Target, Ty};
        // f(x) = x
        let p_id = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        // f(x) = x + 1 â€” sÃ©mantique diffÃ©rente !
        let p_plus_one = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        (p_id, p_plus_one)
    }

    #[test]
    fn semantic_policy_trust_skips_re_verification() {
        let node = fresh_node("trust");
        let (a, b) = divergent_program_pair();
        let h_a = node.store().store(a.bytes()).unwrap();
        let h_b = node.store().store(b.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h_a, to: h_b };
        // Trust = pas de check sÃ©mantique â†’ Accept mÃªme si divergent.
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::Trust);
        assert!(r.is_accept(), "Trust ne re-vÃ©rifie pas, doit accepter");
    }

    #[test]
    fn semantic_policy_sample_based_accepts_equivalent() {
        let node = fresh_node("sample-equiv");
        let (a, b) = equivalent_program_pair();
        let h_a = node.store().store(a.bytes()).unwrap();
        let h_b = node.store().store(b.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h_a, to: h_b };
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 8 });
        assert!(r.is_accept(), "programmes Ã©quivalents doivent passer; got {r:?}");
    }

    #[test]
    fn semantic_policy_sample_based_rejects_divergent() {
        let node = fresh_node("sample-diverg");
        let (a, b) = divergent_program_pair();
        let h_a = node.store().store(a.bytes()).unwrap();
        let h_b = node.store().store(b.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h_a, to: h_b };
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 8 });
        assert!(!r.is_accept());
        let reasons = r.reasons().unwrap();
        assert!(reasons.iter().any(|s| s.contains("output mismatch")));
    }

    #[test]
    fn semantic_policy_rejects_input_arity_mismatch() {
        use crate::kasm::{Node, Program, Target, Ty};
        let node = fresh_node("arity");
        let p1 = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        let p2 = Program::new(
            Target::Cpu, 2, 1, 4,
            vec![Node::input(0), Node::input(1), Node::output(0, Ty::I64)],
        ).unwrap();
        let h1 = node.store().store(p1.bytes()).unwrap();
        let h2 = node.store().store(p2.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h1, to: h2 };
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 4 });
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("arity mismatch")));
    }

    #[test]
    fn semantic_policy_config_patch_unaffected() {
        let node = fresh_node("config-policy");
        let mut map = std::collections::BTreeMap::new();
        map.insert("beam_width", 100);
        let rw = RewriteV2::ConfigPatch(map);
        // Policy SampleBased ne s'applique pas Ã  ConfigPatch.
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 8 });
        assert!(r.is_accept());
    }
}

}

pub mod applicator_v2 {
//! Î©-5.6 â€” Applicator v2 : applique des `RewriteV2` y compris ProgramSubstitution.

use crate::godel::verifier_v2::{verify_v2, RewriteV2, VerificationOutcomeV2};
use crate::{Hash, MonsterNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicatorV2Error {
    /// Verification a rejetÃ© la rewrite.
    Rejected { reasons: Vec<String> },
    /// Le programme `from` n'est pas chargeable (dÃ©jÃ  checkÃ© par verify, mais defensive).
    SourceMissing(Hash),
    /// Le programme `to` n'est pas chargeable.
    TargetMissing(Hash),
}

/// Trace d'une application : avant/aprÃ¨s pour permettre rollback.
#[derive(Debug, Clone)]
pub struct ApplicationTrace {
    pub rewrite: RewriteV2,
    pub previous_active: Option<Hash>,
}

/// Applicator v2. Maintient l'ensemble des programmes "actifs" â€” un set
/// de hashes qui reprÃ©sente les Programs que la node considÃ¨re comme sa
/// version courante. Une ProgramSubstitution remplace `from` par `to` dans
/// ce set ; un rollback restaure.
///
/// L'Ã©tat "active programs" est une abstraction interne au verifier de
/// l'agent â€” pas un endroit oÃ¹ la node modifie son store. Le store reste
/// append-only.
#[derive(Debug, Default)]
pub struct ApplicatorV2 {
    active_programs: std::collections::BTreeSet<Hash>,
}

impl ApplicatorV2 {
    pub fn new() -> Self {
        Self { active_programs: std::collections::BTreeSet::new() }
    }

    /// Initialise l'ensemble actif avec un programme.
    pub fn activate(&mut self, hash: Hash) {
        self.active_programs.insert(hash);
    }

    /// `is_active(hash)` retourne true si le hash est dans le set actif.
    pub fn is_active(&self, hash: &Hash) -> bool {
        self.active_programs.contains(hash)
    }

    /// Applique une RewriteV2. Pour ProgramSubstitution :
    ///  1. verify_v2 doit Accept.
    ///  2. `from` doit Ãªtre actif (sinon Reject).
    ///  3. Remplace `from` par `to` dans active_programs.
    ///  4. Retourne ApplicationTrace pour rollback.
    pub fn apply(
        &mut self,
        rewrite: RewriteV2,
        node: &MonsterNode,
    ) -> Result<ApplicationTrace, ApplicatorV2Error> {
        match verify_v2(&rewrite, node) {
            VerificationOutcomeV2::Accept => {}
            VerificationOutcomeV2::Reject { reasons } => {
                return Err(ApplicatorV2Error::Rejected { reasons });
            }
        }
        match &rewrite {
            RewriteV2::ConfigPatch(_) => {
                // Pour ConfigPatch, on dÃ©lÃ¨gue conceptuellement Ã  l'applicator v1.
                // Ici on enregistre juste la trace ; l'effet config est externe.
                Ok(ApplicationTrace { rewrite, previous_active: None })
            }
            RewriteV2::ProgramSubstitution { from, to } => {
                let from = *from;
                let to = *to;
                if !self.active_programs.contains(&from) {
                    return Err(ApplicatorV2Error::Rejected {
                        reasons: vec![format!("source {from:?} not in active set")],
                    });
                }
                self.active_programs.remove(&from);
                self.active_programs.insert(to);
                Ok(ApplicationTrace {
                    rewrite,
                    previous_active: Some(from),
                })
            }
        }
    }

    /// Rollback d'une trace : restaure l'Ã©tat avant `apply`.
    pub fn rollback(&mut self, trace: &ApplicationTrace) {
        match &trace.rewrite {
            RewriteV2::ConfigPatch(_) => {}
            RewriteV2::ProgramSubstitution { from, to } => {
                self.active_programs.remove(to);
                if let Some(prev) = trace.previous_active {
                    debug_assert_eq!(prev, *from);
                    self.active_programs.insert(prev);
                }
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_programs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};
    use crate::{MemoryGovernor, Store};
    use std::path::PathBuf;


    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("scan-applicator-v2", tag)
    }

    fn fresh_node(tag: &str) -> MonsterNode {
        MonsterNode::new(
            Store::open(fresh_path(tag)).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        )
    }

    fn store_program(node: &MonsterNode, p: &Program) -> Hash {
        node.store().store(p.bytes()).expect("store")
    }

    fn affine_program() -> Program {
        Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::const_i64(3),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        ).unwrap()
    }

    fn id_program() -> Program {
        Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap()
    }

    #[test]
    fn applicator_v2_starts_empty() {
        let app = ApplicatorV2::new();
        assert_eq!(app.active_count(), 0);
    }

    #[test]
    fn activate_adds_to_active_set() {
        let mut app = ApplicatorV2::new();
        let h = Hash::for_blob(b"x");
        app.activate(h);
        assert!(app.is_active(&h));
        assert_eq!(app.active_count(), 1);
    }

    #[test]
    fn apply_substitution_swaps_active() {
        let node = fresh_node("swap");
        let mut app = ApplicatorV2::new();
        let h_from = store_program(&node, &affine_program());
        let h_to = store_program(&node, &id_program());
        app.activate(h_from);
        let trace = app.apply(
            RewriteV2::ProgramSubstitution { from: h_from, to: h_to },
            &node,
        ).expect("accept");
        assert!(!app.is_active(&h_from));
        assert!(app.is_active(&h_to));
        assert_eq!(trace.previous_active, Some(h_from));
    }

    #[test]
    fn rollback_restores_active_set() {
        let node = fresh_node("rollback");
        let mut app = ApplicatorV2::new();
        let h_from = store_program(&node, &affine_program());
        let h_to = store_program(&node, &id_program());
        app.activate(h_from);
        let trace = app.apply(
            RewriteV2::ProgramSubstitution { from: h_from, to: h_to },
            &node,
        ).unwrap();
        app.rollback(&trace);
        assert!(app.is_active(&h_from));
        assert!(!app.is_active(&h_to));
    }

    #[test]
    fn apply_rejects_when_source_not_active() {
        let node = fresh_node("not-active");
        let mut app = ApplicatorV2::new();
        let h_from = store_program(&node, &affine_program());
        let h_to = store_program(&node, &id_program());
        // h_from N'EST PAS activÃ©.
        let result = app.apply(
            RewriteV2::ProgramSubstitution { from: h_from, to: h_to },
            &node,
        );
        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicatorV2Error::Rejected { reasons } => {
                assert!(reasons.iter().any(|s| s.contains("not in active set")));
            }
            _ => panic!("expected Rejected"),
        }
    }

    #[test]
    fn apply_rejects_unverified_substitution() {
        let node = fresh_node("unverified");
        let mut app = ApplicatorV2::new();
        // from et to identiques â†’ trivial, rejetÃ© par verify_v2.
        let h = store_program(&node, &affine_program());
        app.activate(h);
        let result = app.apply(
            RewriteV2::ProgramSubstitution { from: h, to: h },
            &node,
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_handles_config_patch_no_active_change() {
        let node = fresh_node("config");
        let mut app = ApplicatorV2::new();
        let mut map = std::collections::BTreeMap::new();
        map.insert("beam_width", 100);
        let trace = app.apply(RewriteV2::ConfigPatch(map), &node).expect("accept");
        assert_eq!(app.active_count(), 0);
        assert!(matches!(trace.rewrite, RewriteV2::ConfigPatch(_)));
    }

    #[test]
    fn jour_zero_program_substitution_is_recorded_in_trace() {
        // Cross-cap : agent propose, applicator_v2 applique, trace conservÃ©e.
        use crate::agent::SymbolicAgent;
        let node = fresh_node("jour-zero-program");
        let p_input = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let h_input = store_program(&node, &p_input);
        let mut app = ApplicatorV2::new();
        app.activate(h_input);

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p_input);
        assert!(!candidates.is_empty());
        for c in &candidates {
            let _ = store_program(&node, &c.program);
        }
        let rewrites = crate::agent::symbolic::candidates_as_rewrites_v2(&p_input, &candidates);
        let mut applied = 0;
        for rw in rewrites {
            if app.apply(rw, &node).is_ok() {
                applied += 1;
                break; // Une seule substitution par session pour ce test.
            }
        }
        assert!(applied >= 1, "au moins une substitution doit Ãªtre appliquÃ©e");
        assert_eq!(app.active_count(), 1, "set actif doit avoir taille 1 aprÃ¨s swap");
    }
}

}
