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
