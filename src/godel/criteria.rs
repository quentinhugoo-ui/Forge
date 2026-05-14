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
use crate::{MonsterNode, MonsterTrainingConfig};

/// V8 Solution C — bench timing avec filtre RDTSC pour rejeter les
/// samples interrompus par le scheduler. Principe :
///
/// 1. On mesure simultanément le wall-time (ns) ET les cycles CPU
///    (RDTSC) autour de l'exécution.
/// 2. Le ratio cycles/ns approxime la fréquence CPU effective. Si
///    la fréquence apparente s'effondre (< 0.1 cycle/ns), c'est qu'un
///    interrupt + halt-state a "volé" du wall-time pendant que les
///    cycles n'avançaient pas (le CPU était endormi). Sample biaisé,
///    rejeté.
/// 3. On retente jusqu'à atteindre `target` samples valides ou
///    `target * 3` tentatives totales. La médiane des samples valides
///    est retournée — résistante aux outliers que ce filtre n'aurait
///    pas détectés.
///
/// Sur architecture non-x86_64, `read_cycles` retourne 0 → le filtre
/// est désactivé et on retombe sur le comportement V7 (5 samples,
/// médiane brute). Aucune régression.
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
        // Sur x86_64 : si cycles est non-zéro et que le ratio est
        // anormalement bas (< 0.1 cycle/ns soit < 100 cycles pour
        // 1000 ns), le sample est probablement interrompu — on jette.
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
        let examples = [(-4, -25), (-1, -4), (0, 3), (2, 17), (5, 38)];
        let config = MonsterTrainingConfig {
            max_nodes: 6,
            beam_width: 512,
            progress: None,
        };
        // V8 Solution C : measure_filtered rejette les samples
        // interrompus par le scheduler OS via détection RDTSC. La
        // médiane des samples valides remonte ; sur 5 samples cibles
        // avec ~15 % de bruit interruption, on récupère typiquement
        // 4-5 samples propres en 6-8 tentatives.
        measure_filtered(
            || {
                let _ = node.train_i64_program(&examples, config.clone());
            },
            5,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FabricResolveLatencyBench;

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
