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
