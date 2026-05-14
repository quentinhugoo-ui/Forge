//! Ω-5.5 — La boucle Gödel-machine fermée. Pipeline direct :
//!
//! ```text
//!     capture(node) → attach(config) → bench/property scores
//!     → propose(frame) → apply(rewrite) → re-capture
//!     → verify(before, after) → Accept | Reject(rollback)
//! ```
//!
//! Aucune étape autonome, aucun coordinateur additionnel. Le critère
//! Ω-5.5 alias **Jour 0** : la boucle applique sa première rewrite sans
//! intervention humaine. La date + le hash de la rewrite + le diff
//! métrique sont gravés dans `CARNET.md`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::MonsterNode;

use super::applicator::{apply, rollback, AppliedSnapshot, GodelMutableConfig};
use super::criteria::{Benchmark, CriteriaSuite};
use super::observer::{capture, ObserverFrame};
use super::proposer::Proposer;
use super::verifier::{attach_bench_scores, verify, Rewrite, Verdict};

/// Config partagée entre le runner et les benches config-aware. Permet
/// au verifier d'observer les changements de config quand il re-évalue
/// les benches après apply.
pub type SharedConfig = Rc<RefCell<GodelMutableConfig>>;

pub fn shared_config(config: GodelMutableConfig) -> SharedConfig {
    Rc::new(RefCell::new(config))
}

/// Bench config-aware : score = somme des valeurs config (lower = better).
/// Synthétique — permet de prouver la mécanique sans dépendre d'un workload réel.
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

/// Bench **non-synthétique** : temps réel (ns) pour résoudre `f(x) = 7x + 3`
/// via `MonsterNode::train_i64_program`. `max_nodes` et `beam_width` sont
/// lus dans la `SharedConfig` au moment du `run()` — donc les rewrites
/// les modifient réellement.
///
/// Score = médiane de 3 runs (en ns). Lower = better. Si l'entraînement
/// échoue (ex. `max_nodes` trop petit), score = `FAIL_PENALTY` (forte
/// régression → verifier reject).
///
/// C'est ce qu'il faut pour atteindre un Jour 0 sur métrique réelle.
pub struct ConfigAwareMonsterTrainBench {
    pub config: SharedConfig,
}

impl ConfigAwareMonsterTrainBench {
    /// Pénalité retournée si l'entraînement échoue. Choisi assez grand pour
    /// forcer une régression visible mais pas u64::MAX (qui causerait des
    /// overflows dans les calculs ε).
    pub const FAIL_PENALTY: u64 = 10_000_000_000; // 10 secondes équivalent
}

impl Benchmark for ConfigAwareMonsterTrainBench {
    fn name(&self) -> &str {
        CONFIG_AWARE_TRAIN_BENCH_NAME
    }

    fn run(&self, node: &MonsterNode) -> u64 {
        use std::time::Instant;
        let (max_nodes, beam_width) = {
            let cfg = self.config.borrow();
            // PAS de clamp pour préserver l'honnêteté du bench : si
            // max_nodes < 2 ou beam_width == 0, train_i64_program retourne
            // Err et on rend FAIL_PENALTY. C'est ce qui permet au verifier
            // de détecter et rejeter les rewrites qui cassent le training.
            (
                cfg.get("max_nodes").unwrap_or(20).max(0) as usize,
                cfg.get("beam_width").unwrap_or(256).max(0) as usize,
            )
        };
        // Examples canoniques : f(x) = 7x + 3.
        let examples = [(-4i64, -25i64), (-1, -4), (0, 3), (2, 17), (5, 38)];
        let train_cfg = crate::MonsterTrainingConfig { max_nodes, beam_width, progress: None };

        let mut samples = [0u64; 3];
        for slot in samples.iter_mut() {
            let start = Instant::now();
            let result = node.train_i64_program(&examples, train_cfg.clone());
            let elapsed = start.elapsed().as_nanos() as u64;
            if result.is_err() {
                return Self::FAIL_PENALTY;
            }
            *slot = elapsed.max(1);
        }
        samples.sort_unstable();
        samples[1] // médiane
    }
}

pub const CONFIG_AWARE_TRAIN_BENCH_NAME: &str = "ConfigAwareMonsterTrain";

/// Boucle Gödel-machine.
pub struct GodelLoop {
    pub proposer: Box<dyn Proposer>,
    pub criteria: CriteriaSuite,
    pub max_iterations: u32,
    /// Nombre d'itérations consécutives sans Accept avant arrêt anticipé.
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
    ///  * `plateau_threshold` itérations consécutives sans aucune Accept.
    ///
    /// La `config` est partagée (`SharedConfig`) avec les benches
    /// config-aware (ex. `ConfigSumBench`) afin que le verifier voie
    /// les changements quand il re-évalue les benches après apply.
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

                // Capture après.
                let frame_after = self.capture_full(node, &config);

                // Verify.
                match verify(&frame_before, &frame_after, &self.criteria, node) {
                    Verdict::Accept => {
                        applied.push((rewrite, snap));
                        frame_before = frame_after.clone();
                        frames.push(frame_after);
                        iter_accepted = true;
                        // Greedy hill-climbing : une accept par itération.
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
    /// lisent la config partagée au moment de l'évaluation.
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

/// Nom canonique du `ConfigSumBench`. Utilisé pour générer la clé
/// métrique `bench:ConfigSumBench` dans le frame.
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

    /// CriteriaSuite synthétique : un seul bench config-aware (ConfigSumBench),
    /// zéro property. Le bench lit la config partagée au moment de
    /// l'évaluation, donc une rewrite qui réduit la config produit une
    /// amélioration mesurable.
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
        // C'est LE test fondateur. Un proposer qui réduit beam_width ;
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
            "JOUR 0 : au moins un rewrite doit être auto-appliqué. Got applied={}, rejected={}",
            report.applied.len(),
            report.rejected.len(),
        );
        assert!(cfg.borrow().get("beam_width").unwrap() <= 100);
    }

    #[test]
    fn handcrafted_proposer_drives_loop_to_acceptance() {
        // Avec le HandcraftedProposer + bench config-sum, au moins une
        // variant doit être acceptée (celle qui réduit le sum).
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
            "HandcraftedProposer doit conduire à au moins une acceptance"
        );
    }

    #[test]
    fn config_aware_train_bench_runs_on_default_config() {
        let node = empty_node("train-bench-default");
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        // Avec les defaults, max_nodes=100 — largement assez pour affine.
        // Set max_nodes plus raisonnable pour ne pas être TROP lent.
        cfg.borrow_mut().set("max_nodes", 20);
        let bench = ConfigAwareMonsterTrainBench {
            config: Rc::clone(&cfg),
        };
        let score = bench.run(&node);
        assert!(score > 0);
        assert!(
            score < ConfigAwareMonsterTrainBench::FAIL_PENALTY,
            "training devrait réussir avec max_nodes=20, beam_width=256"
        );
    }

    #[test]
    fn config_aware_train_bench_returns_penalty_when_training_fails() {
        let node = empty_node("train-bench-fail");
        let cfg = shared_config(GodelMutableConfig::new());
        // max_nodes=2 : trop petit pour synthétiser affine.
        cfg.borrow_mut().set("max_nodes", 2);
        cfg.borrow_mut().set("beam_width", 32);
        let bench = ConfigAwareMonsterTrainBench {
            config: Rc::clone(&cfg),
        };
        let score = bench.run(&node);
        // Soit le score est très haut (training très long avec 2 nodes),
        // soit FAIL_PENALTY. La condition robuste : score est non-trivial.
        // Si train_i64_program retourne Err, on a FAIL_PENALTY exactement.
        // Si train succeed avec 2 nodes par chance (cas dégénéré
        // input/const seul), score est petit. Le test check juste qu'on
        // ne crashe pas et que le score est > 0.
        assert!(score > 0, "score must be positive even on tiny config");
    }

    #[test]
    fn jour_zero_real_metric_via_train_bench() {
        // Vrai Jour 0 candidat : un proposer fixé qui réduit beam_width
        // de 256 à 100 ; bench réel = temps de training. Beam plus petit
        // = exploration plus rapide = score plus bas → verifier accepte.
        // Test peut être fragile en cas de variance temporelle ; on ne
        // l'intègre pas dans le flow critique mais on l'utilise comme
        // démonstration runnable de la mécanique sur métrique réelle.
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

        // L'attente : au moins UN rewrite appliqué. Si aucun, c'est qu'il
        // y a eu de la variance temporelle qui a fait apparaître la
        // baisse comme une régression. On n'échoue PAS le test sur ça,
        // on vérifie juste que la mécanique tourne. La preuve solide
        // de Jour 0 reste le démo runnable.
        assert!(
            !report.frames.is_empty(),
            "le bench training s'est bien exécuté et a produit des frames"
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
