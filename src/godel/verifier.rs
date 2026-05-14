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
