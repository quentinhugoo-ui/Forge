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
