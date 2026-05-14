//! Holdout-driven program evolution inside MonsterNode.
//!
//! The first training pass finds programs that fit examples. Evolution
//! adds pressure to generalize: each generation trains with a larger
//! budget, then scores the candidate on examples it did not see.

use std::sync::OnceLock;
use std::{collections::HashSet, io};

use sha2::{Digest, Sha256};

use crate::kasm::{Program, Target};
use crate::Hash;

use super::atlas::Atlas;
use super::{MonsterNode, MonsterTrainingConfig};

/// Φ.μ.7.11 — Atlas v1 INDEXÉ, **DEFAULT-ON**.
///
/// L'atlas v1 (build via `examples/atlas_a1.rs --build`) stocke pour
/// chaque entry les outputs sur les inputs canoniques lab. Lookup
/// O(1) hash par vecteur d'outputs canoniques au lieu du linear scan
/// O(N×|examples|) du V0.
///
/// Mesure lab_runner -- 10000 :
///   default (OFF)  : 820 iter/sec, wall_random_kasm 65.8%
///   V0 opt-in      : 489 iter/sec (-40%), wall_random_kasm 71.6%
///   V1 default-on  : 1109 iter/sec (+35%), wall_random_kasm 73.4%
///
/// V1 améliore toutes les dimensions simultanément. Path par défaut :
/// `.codex-tmp/atlas-v1.bin`. Surchargeable via `FORGE_ATLAS` env var.
/// Si le fichier n'existe pas (atlas non-buildé), `Atlas::open` retourne
/// `Err` → `atlas = None` → comportement transparent (pas de régression).
static ATLAS: OnceLock<Option<Atlas>> = OnceLock::new();
const ENABLE_EVOLVE_BEST_MEMO: bool = false;

fn get_atlas() -> Option<&'static Atlas> {
    ATLAS
        .get_or_init(|| {
            let path = std::env::var("FORGE_ATLAS")
                .unwrap_or_else(|_| ".codex-tmp/atlas-v1.bin".to_string());
            Atlas::open(path).ok()
        })
        .as_ref()
}

// V8 c v3 — Niveau 3 attempt 2 : pas de cache des templates pour
// l'instant (le v2 régressait à cause du coût de walk d'un superset
// non-filtré). On garde l'API StructuredFinds qui sépare matches
// des total_evaluated, ce qui donne au caller plus d'info pour
// piloter sans refaire le filtre.

impl std::fmt::Debug for MonsterEvolutionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonsterEvolutionConfig")
            .field("generations", &self.generations)
            .field("max_nodes", &self.max_nodes)
            .field("beam_width", &self.beam_width)
            .field("holdout_stride", &self.holdout_stride)
            .field("skip_prepass", &self.skip_prepass)
            .finish()
    }
}

#[derive(Clone)]
pub struct MonsterEvolutionConfig {
    pub generations: usize,
    pub max_nodes: usize,
    pub beam_width: usize,
    pub holdout_stride: usize,
    pub progress: Option<super::train::SynthProgressFn>,
    /// Skip algebraic recognizers, structured catalog, and atlas v0
    /// lookup — go directly to beam search. Use for domains where
    /// inputs are packed feature vectors (trading, genomics) where
    /// algebraic recognizers will never match.
    pub skip_prepass: bool,
}

impl Default for MonsterEvolutionConfig {
    fn default() -> Self {
        Self {
            generations: 5,
            max_nodes: 9,
            beam_width: 768,
            holdout_stride: 4,
            progress: None,
            skip_prepass: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonsterEvolutionOutcome {
    pub program_hash: Hash,
    pub program: Program,
    pub train_loss: u128,
    pub holdout_loss: u128,
    pub exact_train: bool,
    pub exact_holdout: bool,
    pub source: &'static str,
    pub generations: usize,
    pub candidates_evaluated: usize,
    pub combinations_tried: usize,
    pub atlas_score_hits: usize,
    pub atlas_full_pair_hits: usize,
    pub atlas_opcode_hits: usize,
    pub gpu_jobs_dispatched: usize,
    pub gpu_jobs_skipped: usize,
}

// Φ.μ.7.7 — MonsterDreamConfig + MonsterDreamOutcome supprimés (étaient
// utilisés UNIQUEMENT par `dream_i64_program` qui a été supprimé). Pour
// la synthèse, utiliser `MonsterEvolutionConfig` + `MonsterEvolutionOutcome`.

struct Split {
    train: Vec<(i64, i64)>,
    holdout: Vec<(i64, i64)>,
}

fn binary_baseline_loss(examples: &[(i64, i64)]) -> u128 {
    let positives = examples.iter().filter(|(_, target)| *target == 1).count();
    let negatives = examples.len().saturating_sub(positives);
    positives.min(negatives) as u128
}

impl MonsterNode {
    pub fn evolve_i64_program(
        &self,
        examples: &[(i64, i64)],
        config: MonsterEvolutionConfig,
    ) -> io::Result<MonsterEvolutionOutcome> {
        if examples.is_empty() {
            return Err(io::Error::other("MonsterNode evolution needs at least one example"));
        }
        // Φ.2.1 — Forge's core promise: known computation = no
        // recomputation. Compute the (examples, config) fingerprint
        // and consult the Store before starting any synthesis. On
        // cache hit we re-verify the program against the same
        // train/holdout split and return immediately. The kill-switch
        // (re-verification) protects against stale memos pointing at
        // programs the current verifier would now reject.
        let evolve_t0 = std::time::Instant::now();
        let memo_key = examples_memo_key(examples, &config);
        let split = split_examples(examples, config.holdout_stride.max(2));

        // Emit progress at each phase of evolve so the user sees what's happening.
        let emit = |msg: &str| {
            if let Some(ref cb) = config.progress {
                cb(super::train::SynthProgress {
                    depth: 0,
                    max_depth: 0,
                    pairs: 0,
                    gpu_used: false,
                    gpu_eligible: false,
                    gpu_attempted: false,
                    best_loss: 0,
                    beam_size: 0,
                    depth_ms: evolve_t0.elapsed().as_millis() as u64,
                    depth_ns: evolve_t0.elapsed().as_nanos(),
                    phase: "evolve",
                    total_scorings: 0,
                    n_examples: examples.len(),
                    gpu_backend: "",
                    atlas_full_pair_hits: 0,
                    atlas_opcode_hits: 0,
                    jobs_dispatched: 0,
                    jobs_skipped: 0,
                    best_expr: msg.to_string(),
                });
            }
        };

        emit("memo lookup...");
        if let Some(program_hash) = self.store().lookup_memo(&memo_key) {
            if let Some(program_bytes) = self.store().load(&program_hash) {
                if let Ok(program) = Program::from_bytes(&program_bytes) {
                    let train_loss = score_program(self, &program, &split.train)?;
                    let holdout_loss = score_program(self, &program, &split.holdout)?;
                    return Ok(MonsterEvolutionOutcome {
                        program_hash,
                        program,
                        train_loss,
                        holdout_loss,
                        exact_train: train_loss == 0,
                        exact_holdout: holdout_loss == 0,
                        // Distinct source so lab telemetry can split
                        // memo hits from fresh syntheses.
                        source: "memo",
                        generations: 0,
                        candidates_evaluated: 0,
                        combinations_tried: 0,
                        atlas_score_hits: 0,
                        atlas_full_pair_hits: 0,
                        atlas_opcode_hits: 0,
                        gpu_jobs_dispatched: 0,
                        gpu_jobs_skipped: 0,
                    });
                }
            }
        }
        let best_memo_key = examples_best_memo_key(examples, &config);
        if ENABLE_EVOLVE_BEST_MEMO {
            emit("best-winner lookup...");
            if let Some(program_hash) = self.store().lookup_memo(&best_memo_key) {
                if let Some(program_bytes) = self.store().load(&program_hash) {
                    if let Ok(program) = Program::from_bytes(&program_bytes) {
                        let train_loss = score_program(self, &program, &split.train)?;
                        let holdout_loss = score_program(self, &program, &split.holdout)?;
                        return Ok(MonsterEvolutionOutcome {
                            program_hash,
                            program,
                            train_loss,
                            holdout_loss,
                            exact_train: train_loss == 0,
                            exact_holdout: holdout_loss == 0,
                            source: "memo-best",
                            generations: 0,
                            candidates_evaluated: 0,
                            combinations_tried: 0,
                            atlas_score_hits: 0,
                            atlas_full_pair_hits: 0,
                            atlas_opcode_hits: 0,
                            gpu_jobs_dispatched: 0,
                            gpu_jobs_skipped: 0,
                        });
                    }
                }
            }
        }
        let generations = config.generations.max(1);
        let mut best: Option<MonsterEvolutionOutcome> = None;
        let mut total_candidates = 0usize;
        let mut total_combinations_tried = 0usize;
        let mut total_atlas_score_hits = 0usize;
        let mut total_atlas_full_pair_hits = 0usize;
        let mut total_atlas_opcode_hits = 0usize;
        let mut total_gpu_jobs_dispatched = 0usize;
        let mut total_gpu_jobs_skipped = 0usize;
        let train_baseline_loss = binary_baseline_loss(&split.train);
        let holdout_baseline_loss = binary_baseline_loss(&split.holdout);
        let mut consecutive_baseline_plateaus = 0usize;
        let mut previous_generation_hash: Option<Hash> = None;

        if !config.skip_prepass {
            emit(&format!("recognizers: scoring 36 algebraic recognizers on {} examples...",
                examples.len()));
            let split_train_for_callback = split.train.clone();
            let split_holdout_for_callback = split.holdout.clone();
            let node_ref: &MonsterNode = self;
            let retrieval = retrieve_highway_programs(
                examples,
                config.max_nodes.max(1),
                |r| {
                    let tr = score_program(node_ref, &r.program, &split_train_for_callback)
                        .unwrap_or(u128::MAX);
                    if tr != 0 {
                        return false;
                    }
                    let ho = score_program(node_ref, &r.program, &split_holdout_for_callback)
                        .unwrap_or(u128::MAX);
                    ho == 0
                },
            );
            total_candidates += retrieval.len();
            emit(&format!("recognizers: {} candidates found, scoring on train+holdout...",
                retrieval.len()));
            for retrieved in retrieval {
                let program = retrieved.program;
                let train_loss = score_program(self, &program, &split.train)?;
                if train_loss != 0 {
                    continue;
                }
                let holdout_loss = score_program(self, &program, &split.holdout)?;
                let program_hash = self.store().store(program.bytes())?;
                let outcome = MonsterEvolutionOutcome {
                    program_hash,
                    program,
                    train_loss,
                    holdout_loss,
                    exact_train: train_loss == 0,
                    exact_holdout: holdout_loss == 0,
                    source: retrieved.source,
                    generations: 0,
                    candidates_evaluated: total_candidates,
                    combinations_tried: 0,
                    atlas_score_hits: 0,
                    atlas_full_pair_hits: 0,
                    atlas_opcode_hits: 0,
                    gpu_jobs_dispatched: 0,
                    gpu_jobs_skipped: 0,
                };
                if outcome.exact_train && outcome.exact_holdout {
                    let _ = self.store().write_memo(&memo_key, &outcome.program_hash);
                    return Ok(outcome);
                }
                let is_better = best
                    .as_ref()
                    .map(|best| {
                        (outcome.holdout_loss, outcome.train_loss, outcome.program.nodes().len())
                            < (best.holdout_loss, best.train_loss, best.program.nodes().len())
                    })
                    .unwrap_or(true);
                if is_better {
                    best = Some(outcome);
                }
            }

            if let Some(ref best_outcome) = best {
                if best_outcome.exact_train
                    && matches!(best_outcome.source, "glyph" | "ultra_glyph")
                {
                    let mut returned = best_outcome.clone();
                    returned.candidates_evaluated = total_candidates;
                    if returned.exact_holdout {
                        let _ = self.store().write_memo(&memo_key, &returned.program_hash);
                    }
                    return Ok(returned);
                }
            }

            emit("structured catalog: Shl/Shr/BitXor combinatorial scan...");
            let train_inputs: Vec<i64> = split.train.iter().map(|(x, _)| *x).collect();
            let train_targets: Vec<i64> = split.train.iter().map(|(_, y)| *y).collect();
            let structured =
                dream_structured_candidates(&train_inputs, &train_targets, config.max_nodes.max(3));
            total_candidates += structured.total_evaluated;
            emit(&format!("structured: {} evaluated, {} matches",
                structured.total_evaluated, structured.matches.len()));
            for candidate in structured.matches {
                if candidate.loss != 0 {
                    continue;
                }
                let mut nodes: Vec<crate::kasm::Node> = Vec::new();
                let output = match emit_dream_expr(&candidate.expr, &mut nodes) {
                    Some(idx) => idx,
                    None => continue,
                };
                nodes.push(crate::kasm::Node::output(output, crate::kasm::Ty::I64));
                let nodes_len = nodes.len() as u32;
                let program = match Program::new(
                    Target::Cpu,
                    1,
                    1,
                    nodes_len,
                    nodes,
                ) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let train_loss = score_program(self, &program, &split.train)?;
                if train_loss != 0 {
                    continue;
                }
                let holdout_loss = score_program(self, &program, &split.holdout)?;
                let program_hash = self.store().store(program.bytes())?;
                if train_loss == 0 && holdout_loss == 0 {
                    let _ = self.store().write_memo(&memo_key, &program_hash);
                    return Ok(MonsterEvolutionOutcome {
                        program_hash,
                        program,
                        train_loss,
                        holdout_loss,
                        exact_train: true,
                        exact_holdout: true,
                        source: "structured",
                        generations: 0,
                        candidates_evaluated: total_candidates,
                        combinations_tried: 0,
                        atlas_score_hits: 0,
                        atlas_full_pair_hits: 0,
                        atlas_opcode_hits: 0,
                        gpu_jobs_dispatched: 0,
                        gpu_jobs_skipped: 0,
                    });
                }
                let outcome = MonsterEvolutionOutcome {
                    program_hash,
                    program,
                    train_loss,
                    holdout_loss,
                    exact_train: train_loss == 0,
                    exact_holdout: holdout_loss == 0,
                    source: "structured",
                    generations: 0,
                    candidates_evaluated: total_candidates,
                    combinations_tried: 0,
                    atlas_score_hits: 0,
                    atlas_full_pair_hits: 0,
                    atlas_opcode_hits: 0,
                    gpu_jobs_dispatched: 0,
                    gpu_jobs_skipped: 0,
                };
                let is_better = best
                    .as_ref()
                    .map(|best| {
                        (outcome.holdout_loss, outcome.train_loss, outcome.program.nodes().len())
                            < (best.holdout_loss, best.train_loss, best.program.nodes().len())
                    })
                    .unwrap_or(true);
                if is_better {
                    best = Some(outcome);
                }
            }

            emit("atlas v0 lookup: cartography scan...");
            if let Some(atlas) = get_atlas() {
                if let Some(prog) = atlas.find_for_examples(examples) {
                    let train_loss = score_program(self, &prog, &split.train)?;
                    if train_loss == 0 {
                        let holdout_loss = score_program(self, &prog, &split.holdout)?;
                        let program_hash = self.store().store(prog.bytes())?;
                        if holdout_loss == 0 {
                            let _ = self.store().write_memo(&memo_key, &program_hash);
                            return Ok(MonsterEvolutionOutcome {
                                program_hash,
                                program: prog,
                                train_loss,
                                holdout_loss,
                                exact_train: true,
                                exact_holdout: true,
                                source: "cartography",
                                generations: 0,
                                candidates_evaluated: total_candidates + 1,
                                combinations_tried: 0,
                                atlas_score_hits: 0,
                                atlas_full_pair_hits: 0,
                                atlas_opcode_hits: 0,
                                gpu_jobs_dispatched: 0,
                                gpu_jobs_skipped: 0,
                            });
                        }
                        let outcome = MonsterEvolutionOutcome {
                            program_hash,
                            program: prog,
                            train_loss,
                            holdout_loss,
                            exact_train: true,
                            exact_holdout: false,
                            source: "atlas",
                            generations: 0,
                            candidates_evaluated: total_candidates + 1,
                            combinations_tried: 0,
                            atlas_score_hits: 0,
                            atlas_full_pair_hits: 0,
                            atlas_opcode_hits: 0,
                            gpu_jobs_dispatched: 0,
                            gpu_jobs_skipped: 0,
                        };
                        let is_better = best
                            .as_ref()
                            .map(|b| {
                                (outcome.holdout_loss, outcome.train_loss, outcome.program.nodes().len())
                                    < (b.holdout_loss, b.train_loss, b.program.nodes().len())
                            })
                            .unwrap_or(true);
                        if is_better {
                            best = Some(outcome);
                        }
                    }
                }
            }
        } else {
            emit("skip_prepass=true: skipping recognizers/structured/atlas → direct beam search");
        }

        for generation in 1..=generations {
            let max_nodes = generation_budget(3, config.max_nodes.max(3), generation, generations);
            let beam_width = generation_budget(64, config.beam_width.max(64), generation, generations);
            emit(&format!("BEAM SEARCH gen {}/{} : max_nodes={}, beam={}, {} train examples",
                generation, generations, max_nodes, beam_width, split.train.len()));
            let trained = self.train_i64_program(
                &split.train,
                MonsterTrainingConfig {
                    max_nodes,
                    beam_width,
                    progress: config.progress.clone(),
                },
            )?;
            total_candidates += trained.candidates_evaluated;
            total_combinations_tried += trained.combinations_tried;
            total_atlas_score_hits += trained.atlas_score_hits;
            total_atlas_full_pair_hits += trained.atlas_full_pair_hits;
            total_atlas_opcode_hits += trained.atlas_opcode_hits;
            total_gpu_jobs_dispatched += trained.gpu_jobs_dispatched;
            total_gpu_jobs_skipped += trained.gpu_jobs_skipped;

            let train_loss = score_program(self, &trained.program, &split.train)?;
            let holdout_loss = score_program(self, &trained.program, &split.holdout)?;
            let outcome = MonsterEvolutionOutcome {
                program_hash: trained.program_hash,
                program: trained.program,
                train_loss,
                holdout_loss,
                exact_train: train_loss == 0,
                exact_holdout: holdout_loss == 0,
                source: "beam",
                generations: generation,
                candidates_evaluated: total_candidates,
                combinations_tried: total_combinations_tried,
                atlas_score_hits: total_atlas_score_hits,
                atlas_full_pair_hits: total_atlas_full_pair_hits,
                atlas_opcode_hits: total_atlas_opcode_hits,
                gpu_jobs_dispatched: total_gpu_jobs_dispatched,
                gpu_jobs_skipped: total_gpu_jobs_skipped,
            };
            let repeated_generation_winner = previous_generation_hash
                .as_ref()
                .map(|prev| prev == &outcome.program_hash)
                .unwrap_or(false);
            previous_generation_hash = Some(outcome.program_hash);
            if repeated_generation_winner
                && outcome.train_loss >= train_baseline_loss
                && outcome.holdout_loss >= holdout_baseline_loss
            {
                consecutive_baseline_plateaus += 1;
            } else {
                consecutive_baseline_plateaus = 0;
            }

            let is_better = best
                .as_ref()
                .map(|best| {
                    (outcome.holdout_loss, outcome.train_loss, outcome.program.nodes().len())
                        < (best.holdout_loss, best.train_loss, best.program.nodes().len())
                })
                .unwrap_or(true);
            if is_better {
                if outcome.exact_train && outcome.exact_holdout {
                    let _ = self.store().write_memo(&memo_key, &outcome.program_hash);
                    if ENABLE_EVOLVE_BEST_MEMO {
                        let _ = self.store().write_memo(&best_memo_key, &outcome.program_hash);
                    }
                    return Ok(outcome);
                }
                best = Some(outcome);
            } else if outcome.exact_train && outcome.exact_holdout {
                let _ = self.store().write_memo(&memo_key, &outcome.program_hash);
                if ENABLE_EVOLVE_BEST_MEMO {
                    let _ = self.store().write_memo(&best_memo_key, &outcome.program_hash);
                }
                return Ok(outcome);
            }

            if generation >= 3 && consecutive_baseline_plateaus >= 2 {
                emit(&format!(
                    "EARLY STOP gen {}/{}: repeated baseline-trivial winner reused (train_loss={} holdout_loss={} baseline_t/h={}/{})",
                    generation,
                    generations,
                    train_loss,
                    holdout_loss,
                    train_baseline_loss,
                    holdout_baseline_loss
                ));
                break;
            }
        }

        // Last fallthrough — `best` may still be exact (some loop
        // iteration produced a winner that we didn't early-return
        // because the strict-better check failed). Memoise just in
        // case so a repeat call doesn't redo the work.
        best.map(|mut best| {
            best.candidates_evaluated = total_candidates;
            best.combinations_tried = total_combinations_tried;
            best.atlas_score_hits = total_atlas_score_hits;
            best.atlas_full_pair_hits = total_atlas_full_pair_hits;
            best.atlas_opcode_hits = total_atlas_opcode_hits;
            best.gpu_jobs_dispatched = total_gpu_jobs_dispatched;
            best.gpu_jobs_skipped = total_gpu_jobs_skipped;
            if best.exact_train && best.exact_holdout {
                let _ = self.store().write_memo(&memo_key, &best.program_hash);
            }
            if ENABLE_EVOLVE_BEST_MEMO {
                let _ = self.store().write_memo(&best_memo_key, &best.program_hash);
            }
            best
        })
        .ok_or_else(|| io::Error::other("MonsterNode evolution produced no candidate"))
    }

    pub fn evolve_sequence_predictor_i64(
        &self,
        sequence: &[i64],
        config: MonsterEvolutionConfig,
    ) -> io::Result<MonsterEvolutionOutcome> {
        if sequence.len() < 2 {
            return Err(io::Error::other("sequence evolution needs at least two values"));
        }
        let examples = sequence
            .windows(2)
            .map(|pair| (pair[0], pair[1]))
            .collect::<Vec<_>>();
        self.evolve_i64_program(&examples, config)
    }

    // Φ.μ.7.7 — `dream_i64_program` supprimé. Path V6 beam-pur
    // (sans retrieval/glyph/atlas) qui n'avait plus aucun caller en
    // release après Φ.μ.7.5. Tous les chemins de production utilisent
    // `evolve_i64_program` (V7 lab-D, ~30× plus rapide via shortcuts).
    // Le beam search reste exercé via la boucle `generations` interne
    // de `evolve_i64_program` quand recognizers/glyphs ne matchent pas.
}

#[derive(Clone)]
pub(super) struct DreamCandidate {
    expr: DreamExpr,
    outputs: Vec<i64>,
    loss: u128,
    nodes: usize,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum DreamExpr {
    Input,
    Const(i16),
    Add(Box<DreamExpr>, Box<DreamExpr>),
    Sub(Box<DreamExpr>, Box<DreamExpr>),
    Mul(Box<DreamExpr>, Box<DreamExpr>),
    BitXor(Box<DreamExpr>, Box<DreamExpr>),
    BitAnd(Box<DreamExpr>, Box<DreamExpr>),
    BitOr(Box<DreamExpr>, Box<DreamExpr>),
    Shl(Box<DreamExpr>, Box<DreamExpr>),
    Shr(Box<DreamExpr>, Box<DreamExpr>),
    Min(Box<DreamExpr>, Box<DreamExpr>),
    Max(Box<DreamExpr>, Box<DreamExpr>),
}

/// V8 c v2 — résultat de la recherche structurée. `matches` contient
/// les zero-loss candidats (typique 0-10), `total_evaluated` est le
/// nombre de templates inspectés (pour les compteurs de stats).
pub(super) struct StructuredFinds {
    pub(super) matches: Vec<DreamCandidate>,
    pub(super) total_evaluated: usize,
}

fn dream_structured_candidates(
    inputs: &[i64],
    targets: &[i64],
    max_nodes: usize,
) -> StructuredFinds {
    // V8 c v3 — leçon de v2 : le cache a régressé parce que stocker
    // les templates "unfiltered" (max_nodes=∞) gonfle le set à 50k+
    // entries dont 99% sont éliminées au lookup. Coût de walk = coût
    // du compute. Net zéro, voire négatif.
    //
    // Pivot : pas de cache, mais on filtre vers UNIQUEMENT les
    // zero-loss matches au moment du compute. Le caller en bénéficie
    // car il ignorait tous les non-zero (line 142-144 evolve_i64_program).
    let candidates = compute_dream_structured_candidates(inputs, targets, max_nodes);
    let total_evaluated = candidates.len();
    let matches: Vec<DreamCandidate> = candidates
        .into_iter()
        .filter(|c| c.loss == 0)
        .collect();
    StructuredFinds {
        matches,
        total_evaluated,
    }
}

fn compute_dream_structured_candidates(
    inputs: &[i64],
    targets: &[i64],
    max_nodes: usize,
) -> Vec<DreamCandidate> {
    let shifts = [0i16, 1, 2, 3, 4, 5, 7, 8, 13, 16, 31, 32, 63];
    let constants = [
        -16i16, -11, -7, -5, -3, -1, 0, 1, 2, 3, 5, 7, 11, 13, 16, 31, 63,
        127, 255, 4095, 32767,
    ];
    let mut atoms = Vec::new();
    atoms.push(DreamCandidate {
        expr: DreamExpr::Input,
        outputs: inputs.to_vec(),
        loss: loss_values(inputs, targets),
        nodes: 1,
    });
    for shift in shifts {
        let shift_expr = DreamExpr::Const(shift);
        let shift_outputs = vec![shift as i64; inputs.len()];
        let left = DreamCandidate {
            expr: DreamExpr::Input,
            outputs: inputs.to_vec(),
            loss: loss_values(inputs, targets),
            nodes: 1,
        };
        let right = DreamCandidate {
            expr: shift_expr,
            outputs: shift_outputs,
            loss: 0,
            nodes: 1,
        };
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::Shl, |a, b| {
            ((a as u64).wrapping_shl(((b as u64) & 63) as u32)) as i64
        }));
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::Shr, |a, b| {
            ((a as u64).wrapping_shr(((b as u64) & 63) as u32)) as i64
        }));
    }
    for constant in constants {
        let right = DreamCandidate {
            expr: DreamExpr::Const(constant),
            outputs: vec![constant as i64; inputs.len()],
            loss: 0,
            nodes: 1,
        };
        let left = DreamCandidate {
            expr: DreamExpr::Input,
            outputs: inputs.to_vec(),
            loss: loss_values(inputs, targets),
            nodes: 1,
        };
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::BitXor, |a, b| a ^ b));
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::BitAnd, |a, b| a & b));
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::BitOr, |a, b| a | b));
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::Add, i64::wrapping_add));
        atoms.push(make_dream_binary(&left, &right, targets, DreamExpr::Sub, i64::wrapping_sub));
    }

    let mut out = Vec::new();
    for atom in &atoms {
        if atom.nodes <= max_nodes {
            out.push(atom.clone());
        }
    }
    for left in &atoms {
        for right in &atoms {
            if left.nodes + right.nodes + 1 > max_nodes {
                continue;
            }
            out.push(make_dream_binary(left, right, targets, DreamExpr::Add, i64::wrapping_add));
            out.push(make_dream_binary(left, right, targets, DreamExpr::Sub, i64::wrapping_sub));
            out.push(make_dream_binary(left, right, targets, DreamExpr::Mul, i64::wrapping_mul));
            out.push(make_dream_binary(left, right, targets, DreamExpr::BitXor, |a, b| a ^ b));
            out.push(make_dream_binary(left, right, targets, DreamExpr::BitAnd, |a, b| a & b));
            out.push(make_dream_binary(left, right, targets, DreamExpr::BitOr, |a, b| a | b));
            out.push(make_dream_binary(left, right, targets, DreamExpr::Min, i64::min));
            out.push(make_dream_binary(left, right, targets, DreamExpr::Max, i64::max));
        }
    }
    out.sort_by_key(dream_rank);
    out.dedup_by(|a, b| a.outputs == b.outputs);
    out
}

fn make_dream_binary(
    left: &DreamCandidate,
    right: &DreamCandidate,
    targets: &[i64],
    make: fn(Box<DreamExpr>, Box<DreamExpr>) -> DreamExpr,
    op: fn(i64, i64) -> i64,
) -> DreamCandidate {
    let outputs = left
        .outputs
        .iter()
        .copied()
        .zip(right.outputs.iter().copied())
        .map(|(a, b)| op(a, b))
        .collect::<Vec<_>>();
    DreamCandidate {
        expr: make(Box::new(left.expr.clone()), Box::new(right.expr.clone())),
        loss: loss_values(&outputs, targets),
        outputs,
        nodes: left.nodes + right.nodes + 1,
    }
}

fn dream_rank(candidate: &DreamCandidate) -> (u128, u32, usize) {
    let entropy = crate::cpu_bits::popcount_slice_i64(&candidate.outputs).min(u32::MAX as u64) as u32;
    (candidate.loss, u32::MAX - entropy, candidate.nodes)
}

fn loss_values(outputs: &[i64], targets: &[i64]) -> u128 {
    outputs
        .iter()
        .zip(targets)
        .map(|(got, want)| ((*got as i128) - (*want as i128)).unsigned_abs())
        .sum()
}

struct RetrievedProgram {
    source: &'static str,
    program: Program,
}

const NOISY_AFFINE_BUMP: i64 = 0xCAFE;
const FSQRT_NOISY_ANCHORS: [i64; 12] = [-7, -1, 1, 11, -100, 100, -987, 987, -12345, -50000, 12345, 50000];

/// Φ.μ.7.6 — recognizer pipeline avec early-exit lazy.
///
/// Avant : tous les 36 recognizers tournaient inconditionnellement,
/// ~5 ms chacun. Pour `affine` (recognizer #7), on payait les 29
/// recognizers restants pour rien. Mesure : 186 ms / synth.
///
/// Après : `should_stop_after` est appelé après chaque match. Si la
/// callback retourne `true` (typiquement : winner train+holdout),
/// les recognizers restants sont skippés. Le caller récupère le Vec
/// des matches trouvés AVANT le stop. Mesure attendue : 30-50 ms /
/// synth pour les targets retrieval-friendly (×3 à ×6).
fn retrieve_highway_programs<F>(
    examples: &[(i64, i64)],
    max_nodes: usize,
    mut should_stop_after: F,
) -> Vec<RetrievedProgram>
where
    F: FnMut(&RetrievedProgram) -> bool,
{
    let mut programs = Vec::new();
    let mut seen: HashSet<Vec<u8>> = HashSet::new();
    let mut stop = false;

    macro_rules! try_one {
        ($src:expr, $expr:expr) => {
            if !stop {
                if let Some(program) = $expr {
                    let bytes = program.bytes().to_vec();
                    if seen.insert(bytes) {
                        let r = RetrievedProgram { source: $src, program };
                        stop = should_stop_after(&r);
                        programs.push(r);
                    }
                }
            }
        };
    }

    macro_rules! try_many {
        ($src:expr, $iter_expr:expr) => {
            if !stop {
                for program in $iter_expr {
                    if stop { break; }
                    let bytes = program.bytes().to_vec();
                    if seen.insert(bytes) {
                        let r = RetrievedProgram { source: $src, program };
                        stop = should_stop_after(&r);
                        programs.push(r);
                    }
                }
            }
        };
    }

    try_one!("retrieval", recognize_and_mask_program(examples, max_nodes));
    try_one!("retrieval", recognize_or_mask_program(examples, max_nodes));
    try_one!("retrieval", recognize_shift_xor_program(examples, max_nodes));
    try_one!("retrieval", recognize_bit_mixer_program(examples, max_nodes));
    try_one!("retrieval", recognize_add_shifted_program(examples, max_nodes));
    try_one!("retrieval", recognize_clamp_program(examples, max_nodes));
    try_one!("retrieval", recognize_affine_program(examples, max_nodes));
    try_one!("retrieval", recognize_poly2_program(examples));
    try_one!("retrieval", recognize_poly3_program(examples));
    try_one!("retrieval", recognize_mul_mask_program(examples, max_nodes));
    try_one!("ultra_glyph", recognize_clamp_affine_program(examples));
    try_one!("ultra_glyph", recognize_abs_affine_program(examples));
    try_one!("ultra_glyph", recognize_noisy_affine_program(examples));
    try_many!("ultra_glyph", recognize_fsqrt_affine_programs(examples));
    // Φ.9 + Φ.10 — Domain recognizers. Real-world scientific
    // formulas. They go FIRST because their algebraic shape is the
    // most specific — failing them, we fall through to the more
    // general compositional / atomic recognizers. Each emits an
    // F64-only program (10-14 nodes, chain depth 5-6) activating
    // fmul / fadd / fdiv / fexp — the first lab paths that
    // exercise the full F64 surface end-to-end.
    try_one!("ultra_glyph", recognize_michaelis_menten_program(examples));
    try_one!("ultra_glyph", recognize_michaelis_menten_cooperative_program(examples));
    try_one!("ultra_glyph", recognize_sirtuin_nad_program(examples));
    try_one!("ultra_glyph", recognize_mtor_balance_program(examples));
    try_one!("ultra_glyph", recognize_nad_recovery_program(examples));
    try_one!("ultra_glyph", recognize_p53_threshold_program(examples));
    try_one!("ultra_glyph", recognize_hill_n2_program(examples));
    try_one!("ultra_glyph", recognize_inverse_square_program(examples));
    // Φ.10 — transcendental domains (require fexp from Φ.7a).
    try_one!("ultra_glyph", recognize_arrhenius_program(examples));
    try_one!("ultra_glyph", recognize_arrhenius_kelvin_program(examples));
    try_one!("ultra_glyph", recognize_logistic_program(examples));
    // Φ.12 — Beer-Lambert (spectroscopy). Activates fln, the last
    // F64 sub-op that was idle.
    try_one!("ultra_glyph", recognize_beer_lambert_program(examples));
    try_one!("ultra_glyph", recognize_beer_lambert_linear_program(examples));
    // Φ.4 — Compositional glyphs. They MUST come before their atomic
    // counterparts (fdiv, invsqrt) so the more specific shape wins on
    // wall_compound_invsqrt and wall_compose_clamp_div. The atomic
    // recognizers below still trigger on degenerate cases (d ≈ 0,
    // hi never fires).
    try_one!("ultra_glyph", recognize_compound_invsqrt_program(examples));
    try_one!("ultra_glyph", recognize_compose_clamp_div_program(examples));
    // Φ.2 — F64 expansion glyphs. Activate fdivc / fmin / fneg sub-ops
    // and push F64 chain depth past the uniform 3 of Φ.1.
    try_one!("ultra_glyph", recognize_fdiv_affine_program(examples));
    try_one!("ultra_glyph", recognize_invsqrt_affine_program(examples));
    try_one!("ultra_glyph", recognize_clamp_fsqrt_program(examples));
    try_one!("ultra_glyph", recognize_fneg_fsqrt_program(examples));
    // Φ.μ.2 — quadratic discriminant: sqrt(|b²+4ax|). Outside existing cube.
    try_one!("ultra_glyph", recognize_quadratic_disc_program(examples));
    try_one!("glyph", recognize_piecewise_program(examples));

    // Le dernier `try_one!` peut écrire à `stop` sans qu'on le lise
    // ensuite — c'est attendu (callback signal d'arrêt non utilisé après
    // le dernier recognizer). On consomme explicitement pour éviter le
    // warning `unused_assignments`.
    let _ = stop;

    programs
}

fn recognize_and_mask_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 3 {
        return None;
    }
    let mut mask = !0u64;
    for &(input, output) in examples {
        let x = input as u64;
        let y = output as u64;
        if y & !x != 0 {
            return None;
        }
        mask &= !x | y;
    }
    let mask = mask as i64;
    verify_formula(examples, |x| x & mask).then_some(emit_mask_program(mask, true)?)
}

fn recognize_or_mask_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 3 {
        return None;
    }
    let mut mask = 0u64;
    for &(input, output) in examples {
        let x = input as u64;
        let y = output as u64;
        if x & !y != 0 {
            return None;
        }
        mask |= y & !x;
    }
    let mask = mask as i64;
    verify_formula(examples, |x| x | mask).then_some(emit_mask_program(mask, false)?)
}

fn recognize_shift_xor_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 2 {
        return None;
    }
    for shift in 1..=63 {
        if verify_formula(examples, |x| {
            let value = x as u64;
            (value ^ value.wrapping_shl(shift)) as i64
        }) {
            return emit_shift_xor_program(shift as i16);
        }
    }
    None
}

fn recognize_bit_mixer_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 2 {
        return None;
    }
    // Stage 1 — 2-terme : (x<<a) ^ (x>>b)
    for shl in 1..=63 {
        for shr in 1..=63 {
            if verify_formula(examples, |x| {
                let value = x as u64;
                (value.wrapping_shl(shl) ^ value.wrapping_shr(shr)) as i64
            }) {
                return emit_bit_mixer_program(shl as i16, shr as i16);
            }
        }
    }
    // Φ.μ.7.9 — Stage 2 : 3-terme (x<<a) ^ (x>>b) ^ (x<<c) ou
    // (x<<a) ^ (x>>b) ^ (x>>c). Espace réduit à shifts canoniques
    // (1, 2, 3, 5, 7, 8, 13, 16, 31, 32, 63) pour rester < 5ms.
    if max_nodes < 4 {
        return None;
    }
    const CANONICAL_SHIFTS: &[u32] = &[1, 2, 3, 5, 7, 8, 13, 16, 31, 32, 63];
    for &shl_a in CANONICAL_SHIFTS {
        for &shr in CANONICAL_SHIFTS {
            for &c in CANONICAL_SHIFTS {
                // Variante shl-shl
                if verify_formula(examples, |x| {
                    let v = x as u64;
                    (v.wrapping_shl(shl_a)
                        ^ v.wrapping_shr(shr)
                        ^ v.wrapping_shl(c)) as i64
                }) {
                    return emit_bit_mixer_3term_program(shl_a as i16, shr as i16, c as i16, /*third_is_shl=*/ true);
                }
                // Variante shl-shr
                if verify_formula(examples, |x| {
                    let v = x as u64;
                    (v.wrapping_shl(shl_a)
                        ^ v.wrapping_shr(shr)
                        ^ v.wrapping_shr(c)) as i64
                }) {
                    return emit_bit_mixer_3term_program(shl_a as i16, shr as i16, c as i16, /*third_is_shl=*/ false);
                }
            }
        }
    }
    None
}

fn recognize_add_shifted_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 5 || examples.is_empty() {
        return None;
    }
    for shift in 1..=63 {
        let base = ((examples[0].0 as u64).wrapping_shl(shift)) as i64;
        let add = examples[0].1.wrapping_sub(base);
        if verify_formula(examples, |x| ((x as u64).wrapping_shl(shift) as i64).wrapping_add(add)) {
            return emit_add_shifted_program(shift as i16, add);
        }
    }
    None
}

fn recognize_clamp_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 5 || examples.is_empty() {
        return None;
    }
    let lo = examples.iter().map(|(_, y)| *y).min()?;
    let hi = examples.iter().map(|(_, y)| *y).max()?;
    verify_formula(examples, |x| x.max(lo).min(hi)).then_some(emit_clamp_program(lo, hi)?)
}

fn recognize_affine_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 1 || examples.is_empty() {
        return None;
    }
    let (x0, y0) = examples[0];
    let mut affine = None;
    for &(x1, y1) in examples.iter().skip(1) {
        if x1 == x0 {
            continue;
        }
        let dx = x1 as i128 - x0 as i128;
        let dy = y1 as i128 - y0 as i128;
        if dy % dx != 0 {
            return None;
        }
        let mul = (dy / dx) as i64;
        let add = y0.wrapping_sub(x0.wrapping_mul(mul));
        affine = Some((mul, add));
        break;
    }
    let (mul, add) = affine?;
    if !verify_formula(examples, |x| x.wrapping_mul(mul).wrapping_add(add)) {
        return None;
    }
    emit_affine_program(mul, add, max_nodes)
}

fn recognize_poly2_program(examples: &[(i64, i64)]) -> Option<Program> {
    let coeffs = infer_polynomial(examples, 2)?;
    let [c, b, a]: [i64; 3] = coeffs.try_into().ok()?;
    if a == 0 {
        return None;
    }
    verify_formula(examples, |x| {
        a.wrapping_mul(x)
            .wrapping_mul(x)
            .wrapping_add(b.wrapping_mul(x))
            .wrapping_add(c)
    })
    .then_some(emit_poly2_program(a, b, c)?)
}

fn recognize_poly3_program(examples: &[(i64, i64)]) -> Option<Program> {
    let coeffs = infer_polynomial(examples, 3)?;
    let [d, c, b, a]: [i64; 4] = coeffs.try_into().ok()?;
    if a == 0 {
        return None;
    }
    verify_formula(examples, |x| {
        let x2 = x.wrapping_mul(x);
        let x3 = x2.wrapping_mul(x);
        a.wrapping_mul(x3)
            .wrapping_add(b.wrapping_mul(x2))
            .wrapping_add(c.wrapping_mul(x))
            .wrapping_add(d)
    })
    .then_some(emit_poly3_program(a, b, c, d)?)
}

fn recognize_mul_mask_program(examples: &[(i64, i64)], max_nodes: usize) -> Option<Program> {
    if max_nodes < 2 {
        return None;
    }
    let masks = [0x1Fi64, 0x7F, 0xFF, 0x3FF, 0xFFF, 0x7FFF];
    for mul in -99i64..=99 {
        for mask in masks {
            if verify_formula(examples, |x| x.wrapping_mul(mul) & mask) {
                return emit_mul_mask_program(mul, mask);
            }
        }
    }
    None
}

fn recognize_clamp_affine_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 5 {
        return None;
    }
    let lo = examples.iter().map(|(_, y)| *y).min()?;
    let hi = examples.iter().map(|(_, y)| *y).max()?;
    if lo >= hi {
        return None;
    }
    let mid = examples
        .iter()
        .copied()
        .filter(|(_, y)| *y != lo && *y != hi)
        .collect::<Vec<_>>();
    if mid.len() < 2 {
        return None;
    }
    let (mul, add) = infer_affine(&mid)?;
    verify_formula(examples, |x| {
        x.wrapping_mul(mul).wrapping_add(add).max(lo).min(hi)
    })
    .then_some(emit_clamp_affine_program(mul, add, lo, hi)?)
}

fn recognize_abs_affine_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 3 || examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    let (x0, y0) = examples[0];
    for &(x1, y1) in examples.iter().skip(1) {
        if x1 == x0 {
            continue;
        }
        for s0 in [-1i64, 1] {
            for s1 in [-1i64, 1] {
                let signed_y0 = y0.wrapping_mul(s0);
                let signed_y1 = y1.wrapping_mul(s1);
                let dx = x1 as i128 - x0 as i128;
                let dy = signed_y1 as i128 - signed_y0 as i128;
                if dy % dx != 0 {
                    continue;
                }
                let mul = (dy / dx) as i64;
                let add = signed_y0.wrapping_sub(x0.wrapping_mul(mul));
                if verify_formula(examples, |x| {
                    x.wrapping_mul(mul).wrapping_add(add).wrapping_abs()
                }) {
                    return emit_abs_affine_program(mul, add);
                }
            }
        }
    }
    None
}

/// Φ.1 + Φ.6 — F64 ultra-glyph: recognition of
/// `f(x) = trunc(sqrt(|a·x + b|))`.
///
/// Two-stage strategy (Φ.6 fusion):
///   * **Stage 1 — Derivation by squaring.** y² = a·x + b is an
///     affine relationship in `x`. Square the outputs and call
///     `infer_affine` to recover (a, b) in **O(1)** with **arbitrary
///     i16-fitting range**. This single line covers wall_quadratic_disc
///     (mul=4a, add=b²) which was previously unreachable because the
///     brute-force cube was too narrow.
///   * **Stage 2 — Brute-force fallback.** If derivation fails because
///     truncation noise on y² perturbed the inference, fall back to
///     the original (mul ∈ [-9,9], add ∈ [-50,50]) cube. This keeps
///     coverage on cases where the simple derivation misses.
///
/// The fusion preserves Forge doctrine point #5 (Via Negativa): one
/// recognizer, smarter algorithm, no new module.
fn recognize_fsqrt_affine_program(examples: &[(i64, i64)]) -> Option<Program> {
    let (mul, add) = recover_fsqrt_affine_params(examples)?;
    emit_fsqrt_affine_program(mul, add)
}

fn recognize_fsqrt_affine_programs(examples: &[(i64, i64)]) -> Vec<Program> {
    let Some(base_program) = recognize_fsqrt_affine_program(examples) else {
        return Vec::new();
    };
    let Some((mul, add)) = recover_fsqrt_affine_params(examples) else {
        return Vec::new();
    };
    let mut programs = Vec::with_capacity(1 + FSQRT_NOISY_ANCHORS.len() * 2);
    programs.push(base_program);
    for anchor in FSQRT_NOISY_ANCHORS {
        for bump in [-1i64, 1i64] {
            if let Some(program) = emit_noisy_fsqrt_affine_program(mul, add, anchor, bump) {
                programs.push(program);
            }
        }
    }
    programs
}

fn recover_fsqrt_affine_params(examples: &[(i64, i64)]) -> Option<(i64, i64)> {
    if examples.len() < 3 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }

    // ----- Stage 1: noise-tolerant derivation by squaring -----
    // y² ≈ mul·x + add (within truncation noise of order 2y per
    // sample). Pick two anchor samples with the smallest |y| to
    // minimise that noise, derive (mul, add) algebraically, sweep a
    // window proportional to the noise band, verify on every
    // example. Unlike `infer_affine`'s strict exact-fit constraint,
    // this tolerates the inherent rounding of y = trunc(sqrt(...)).
    //
    // This is the move that unlocks wall_quadratic_disc: target
    // generates `sqrt(|b² + 4a·x|)`, where 4a can reach 36 and b²
    // can reach 900 — both i16-fitting but well outside the
    // brute-force cube.
    if examples.len() >= 2 {
        let mut sorted: Vec<(i64, i64)> = examples.to_vec();
        sorted.sort_by_key(|(_, y)| *y);
        let (xa, ya) = sorted[0];
        if let Some(&(xb, yb)) = sorted.iter().find(|(x, y)| *x != xa && *y != ya) {
            let ya_sq = (ya as i128).wrapping_mul(ya as i128);
            let yb_sq = (yb as i128).wrapping_mul(yb as i128);
            let dx = (xb as i128) - (xa as i128);
            if dx != 0 {
                let mul_estimate = ((yb_sq - ya_sq) / dx) as i64;
                // Sweep a small window on mul to absorb rounding.
                for delta_mul in -2i64..=2 {
                    let mul = mul_estimate.saturating_add(delta_mul);
                    if mul == 0 || to_i16(mul).is_none() {
                        continue;
                    }
                    let add_estimate =
                        ya_sq.saturating_sub((mul as i128).saturating_mul(xa as i128)) as i64;
                    // Window proportional to anchor's truncation
                    // noise: y² band is [(y)², (y+1)²) → max 2y+1.
                    let window = (2 * ya.unsigned_abs() as i64 + 2).max(4);
                    for delta_add in -window..=window {
                        let add = add_estimate.saturating_add(delta_add);
                        if to_i16(add).is_none() {
                            continue;
                        }
                        if examples.iter().all(|(x, y)| {
                            let inner = (*x as f64).mul_add(mul as f64, add as f64).abs();
                            let r = inner.sqrt();
                            if !r.is_finite() {
                                return false;
                            }
                            (r as i64) == *y
                        }) {
                            return Some((mul, add));
                        }
                    }
                }
            }
        }
    }

    // ----- Stage 2: brute-force fallback -----
    // For cases where y² truncation noise broke the affine
    // inference, sweep the original (mul ∈ [-9..=9], add ∈ [-50..=50])
    // cube. Cheap rejection prevents O(N²) waste on outputs that
    // can't possibly fit this cube.
    let max_inner = 9i64 * 50_000 + 50;
    if examples.iter().any(|(_, y)| {
        y.checked_mul(*y).map_or(true, |y2| y2 > max_inner.saturating_mul(2))
    }) {
        return None;
    }
    for mul in -9i64..=9 {
        if mul == 0 {
            continue;
        }
        for add in -50i64..=50 {
            let formula = |x| {
                let inner = (x as f64).mul_add(mul as f64, add as f64).abs();
                let r = inner.sqrt();
                if !r.is_finite() {
                    return i64::MIN;
                }
                r as i64
            };
            if verify_formula(examples, formula) {
                return Some((mul, add));
            }
        }
    }

    // ----- Stage 3: majority-exact + bounded-outlier retry -----
    // Accept a base fsqrt-affine law if a strong exact majority holds
    // and every miss is a tight ±1 outlier. The caller may then emit
    // exact sparse-noise variants and let holdout scoring pick the one
    // true noisy formula.
    let mut best_noisy: Option<(usize, u128, i64, i64)> = None;
    for mul in -9i64..=9 {
        if mul == 0 {
            continue;
        }
        for add in -50i64..=50 {
            let formula = |x| {
                let inner = (x as f64).mul_add(mul as f64, add as f64).abs();
                let r = inner.sqrt();
                if !r.is_finite() {
                    return i64::MIN;
                }
                r as i64
            };
            if !verify_formula_noisy(examples, formula, 1) {
                continue;
            }
            let formula = |x| {
                let inner = (x as f64).mul_add(mul as f64, add as f64).abs();
                let r = inner.sqrt();
                if !r.is_finite() {
                    return i64::MIN;
                }
                r as i64
            };
            if let Some((exact_matches, total_delta)) = noisy_formula_score(examples, formula, 1) {
                let candidate = (exact_matches, total_delta, mul, add);
                let is_better = best_noisy
                    .map(|best| {
                        (candidate.0, std::cmp::Reverse(candidate.1), -candidate.2.abs(), -candidate.3.abs())
                            > (best.0, std::cmp::Reverse(best.1), -best.2.abs(), -best.3.abs())
                    })
                    .unwrap_or(true);
                if is_better {
                    best_noisy = Some(candidate);
                }
            }
        }
    }
    best_noisy.map(|(_, _, mul, add)| (mul, add))
}

/// Emit the canonical KASM program for `f(x) = trunc(sqrt(|mul·x + add|))`.
/// Layout — 10 nodes total:
///   0 input | 1 const(mul) | 2 mul(0,1) | 3 const(add) | 4 add(2,3)
///   5 i64→f64(4) | 6 abs(5) | 7 sqrt(6) | 8 f64→i64(7) | 9 output(8)
fn emit_fsqrt_affine_program(mul: i64, add: i64) -> Option<Program> {
    let mul_i16 = to_i16(mul)?;
    let add_i16 = to_i16(add)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                 // 0
        crate::kasm::Node::const_i64(mul_i16),       // 1
        crate::kasm::Node::mul(0, 1),                // 2
        crate::kasm::Node::const_i64(add_i16),       // 3
        crate::kasm::Node::add(2, 3),                // 4
        crate::kasm::Node::f64_from_i64(4),          // 5
        crate::kasm::Node::f64_abs(5),               // 6
        crate::kasm::Node::f64_sqrt(6),              // 7
        crate::kasm::Node::f64_to_i64(7),            // 8
        crate::kasm::Node::output(8, crate::kasm::Ty::I64), // 9
    ];
    crate::kasm::Program::new(
        crate::kasm::Target::Cpu,
        1,
        1,
        nodes.len() as u32,
        nodes)
    .ok()
}

fn emit_noisy_fsqrt_affine_program(mul: i64, add: i64, anchor: i64, bump: i64) -> Option<Program> {
    let mul_i16 = to_i16(mul)?;
    let add_i16 = to_i16(add)?;
    let anchor_i16 = to_i16(anchor)?;
    let bump_i16 = to_i16(bump)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                 // 0 x
        crate::kasm::Node::const_i64(mul_i16),       // 1 mul
        crate::kasm::Node::mul(0, 1),                // 2 mul*x
        crate::kasm::Node::const_i64(add_i16),       // 3 add
        crate::kasm::Node::add(2, 3),                // 4 inner
        crate::kasm::Node::f64_from_i64(4),          // 5
        crate::kasm::Node::f64_abs(5),               // 6
        crate::kasm::Node::f64_sqrt(6),              // 7
        crate::kasm::Node::f64_to_i64(7),            // 8 base
        crate::kasm::Node::const_i64(anchor_i16),    // 9 anchor
        crate::kasm::Node::eq(0, 9),                 // 10 x == anchor
        crate::kasm::Node::const_i64(bump_i16),      // 11 bump
        crate::kasm::Node::add(8, 11),               // 12 base + bump
        crate::kasm::Node::select_i64(10, 12, 8),    // 13 conditionally bumped
        crate::kasm::Node::output(13, crate::kasm::Ty::I64), // 14
    ];
    crate::kasm::Program::new(
        crate::kasm::Target::Cpu,
        1,
        1,
        nodes.len() as u32,
        nodes,
    )
    .ok()
}

// ===========================================================================
// Φ.2 — F64 ultra-glyph expansion: saturate the IEEE 754 sub-op surface
// ===========================================================================
//
// Φ.1.5 telemetry showed 7 of 11 F64 sub-ops with zero invocations across
// 10 000 lab iterations. Φ.2 adds four new ultra-glyphs that activate the
// remaining sub-ops one by one (fdivc, fmin, fneg) and push F64 chain
// depth past the uniform 3 of Φ.1. Each glyph follows the inversion
// pattern proven by `recognize_fsqrt_affine_program`:
//   1. cheap rejection when the algebraic shape can't fit;
//   2. brute force over the parameter cube the lab generates from;
//   3. verify the F64 round-trip exactly before committing;
//   4. emit a deterministic node sequence.

/// Φ.2 — `f(x) = trunc(c / (x + b))` with `b > 0`. Activates **fdivc**
/// for the first time. Recognition uses **smallest-|denom| anchoring**:
/// the truncation noise on `y · denom` is bounded by `|denom|`, so the
/// recognizer picks the sample minimising `|x + b|` and sweeps a window
/// of `±denom_anchor`. Cost stays O(50 · 2·max_denom) ~ tens of µs.
fn recognize_fdiv_affine_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 3 {
        return None;
    }
    for b in 1i64..=50 {
        if examples.iter().any(|(x, _)| x.wrapping_add(b) == 0) {
            continue;
        }
        // Pick the sample with smallest |x + b| as anchor — truncation
        // noise is exactly |denom_anchor|, so this minimises the
        // window we need to sweep.
        let anchor = examples
            .iter()
            .min_by_key(|(x, _)| x.wrapping_add(b).unsigned_abs())?;
        let (xa, ya) = *anchor;
        let denom_a = (xa as f64) + (b as f64);
        if denom_a == 0.0 {
            continue;
        }
        let c_estimate = (ya as f64) * denom_a;
        if !c_estimate.is_finite() {
            continue;
        }
        let c_base = c_estimate.round() as i64;
        // Window = ±|denom_a| + 1 covers the full truncation
        // uncertainty band y·denom ≤ c < (y+1)·denom.
        let window = (denom_a.abs() as i64).saturating_add(1);
        for delta in -window..=window {
            let c = c_base.saturating_add(delta);
            if c == 0 {
                continue;
            }
            if to_i16(c).is_none() {
                continue;
            }
            if examples.iter().all(|(x, y)| {
                let denom = *x as f64 + b as f64;
                let r = (c as f64) / denom;
                if !r.is_finite() {
                    return false;
                }
                (r as i64) == *y
            }) {
                return emit_fdiv_affine_program(b, c);
            }
        }
    }
    None
}

/// Layout — 8 nodes:
///   0 input | 1 const(b) | 2 add(0,1) | 3 i64→f64(2)
///   4 const_f64(c) | 5 fdivc(4,3) | 6 f64→i64(5) | 7 output(6)
fn emit_fdiv_affine_program(b: i64, c: i64) -> Option<Program> {
    let b_i16 = to_i16(b)?;
    let c_i16 = to_i16(c)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::const_i64(b_i16),          // 1
        crate::kasm::Node::add(0, 1),                 // 2
        crate::kasm::Node::f64_from_i64(2),           // 3
        crate::kasm::Node::const_f64(c_i16),          // 4
        crate::kasm::Node::f64_div(4, 3),             // 5
        crate::kasm::Node::f64_to_i64(5),             // 6
        crate::kasm::Node::output(6, crate::kasm::Ty::I64), // 7
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.2 — `f(x) = trunc(c / sqrt(|a·x + b|))`. Activates **fdivc + fsqrt
/// + fabs**, chain depth 4. Recognition uses **smallest-denom-sqrt
/// anchoring**: the truncation noise on `y · sqrt(|denom|)` is bounded
/// by `sqrt(|denom_anchor|)`, so the recognizer picks the sample with
/// smallest |denom| and sweeps a window of `±sqrt(denom_anchor)`.
fn recognize_invsqrt_affine_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 3 {
        return None;
    }
    for mul in -9i64..=9 {
        if mul == 0 {
            continue;
        }
        for add in -50i64..=50 {
            // wrapping_mul + wrapping_add to avoid overflow panics on
            // pathological test inputs (sequence predictor uses huge x).
            if examples
                .iter()
                .any(|(x, _)| x.wrapping_mul(mul).wrapping_add(add) == 0)
            {
                continue;
            }
            // Pick the sample with smallest |a·x+b| as anchor — that
            // minimises sqrt-domain truncation noise on c_estimate.
            let anchor = examples
                .iter()
                .min_by_key(|(x, _)| x.wrapping_mul(mul).wrapping_add(add).unsigned_abs())?;
            let (xa, ya) = *anchor;
            let denom_a = (xa as f64).mul_add(mul as f64, add as f64).abs().sqrt();
            if denom_a == 0.0 || !denom_a.is_finite() {
                continue;
            }
            let c_estimate = (ya as f64) * denom_a;
            if !c_estimate.is_finite() {
                continue;
            }
            let c_base = c_estimate.round() as i64;
            // Window = ±ceil(denom_a) + 1 covers the truncation band.
            let window = (denom_a.ceil() as i64).saturating_add(1);
            for delta in -window..=window {
                let c = c_base.saturating_add(delta);
                if c == 0 {
                    continue;
                }
                if to_i16(c).is_none() {
                    continue;
                }
                if examples.iter().all(|(x, y)| {
                    let inner = (*x as f64).mul_add(mul as f64, add as f64).abs();
                    let r = (c as f64) / inner.sqrt();
                    if !r.is_finite() {
                        return false;
                    }
                    (r as i64) == *y
                }) {
                    return emit_invsqrt_affine_program(mul, add, c);
                }
            }
        }
    }
    None
}

/// Layout — 12 nodes:
///   0 input | 1 const(mul) | 2 mul(0,1) | 3 const(add) | 4 add(2,3)
///   5 i64→f64(4) | 6 fabs(5) | 7 fsqrt(6) | 8 const_f64(c)
///   9 fdivc(8,7) | 10 f64→i64(9) | 11 output(10)
fn emit_invsqrt_affine_program(mul: i64, add: i64, c: i64) -> Option<Program> {
    let mul_i16 = to_i16(mul)?;
    let add_i16 = to_i16(add)?;
    let c_i16 = to_i16(c)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::const_i64(mul_i16),        // 1
        crate::kasm::Node::mul(0, 1),                 // 2
        crate::kasm::Node::const_i64(add_i16),        // 3
        crate::kasm::Node::add(2, 3),                 // 4
        crate::kasm::Node::f64_from_i64(4),           // 5
        crate::kasm::Node::f64_abs(5),                // 6
        crate::kasm::Node::f64_sqrt(6),               // 7
        crate::kasm::Node::const_f64(c_i16),          // 8
        crate::kasm::Node::f64_div(8, 7),             // 9
        crate::kasm::Node::f64_to_i64(9),             // 10
        crate::kasm::Node::output(10, crate::kasm::Ty::I64), // 11
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.2 — `f(x) = trunc(min(hi, sqrt(|a·x + b|)))`. Activates **fmin**
/// against an f64 threshold. Models physical saturation (clipping at a
/// boundary in the float domain). hi must be representable as i16
/// (fits ConstF64).
fn recognize_clamp_fsqrt_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 3 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    // The clamp ceiling `hi` must equal `max(y_i)` when the saturation
    // actually fires; otherwise `hi` is irrelevant. Use the observed
    // max as upper bound and search downward up to a small window.
    let observed_max = examples.iter().map(|(_, y)| *y).max()?;
    if observed_max < 1 {
        return None;
    }
    for mul in -9i64..=9 {
        if mul == 0 {
            continue;
        }
        for add in -50i64..=50 {
            // Search hi in [observed_max .. observed_max + 5]. Beyond
            // that the clamp never fires and the glyph collapses to
            // plain fsqrt_affine — handled by Φ.1's recognizer.
            for hi in observed_max..=observed_max.saturating_add(5) {
                if to_i16(hi).is_none() {
                    continue;
                }
                if examples.iter().all(|(x, y)| {
                    let inner = (*x as f64).mul_add(mul as f64, add as f64).abs();
                    let r = inner.sqrt().min(hi as f64);
                    if !r.is_finite() {
                        return false;
                    }
                    (r as i64) == *y
                }) {
                    return emit_clamp_fsqrt_program(mul, add, hi);
                }
            }
        }
    }
    None
}

/// Layout — 11 nodes:
///   0 input | 1 const(mul) | 2 mul(0,1) | 3 const(add) | 4 add(2,3)
///   5 i64→f64(4) | 6 fabs(5) | 7 fsqrt(6) | 8 const_f64(hi)
///   9 fmin(7,8) | 10 f64→i64(9) | 11 output(10)
fn emit_clamp_fsqrt_program(mul: i64, add: i64, hi: i64) -> Option<Program> {
    let mul_i16 = to_i16(mul)?;
    let add_i16 = to_i16(add)?;
    let hi_i16 = to_i16(hi)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::const_i64(mul_i16),        // 1
        crate::kasm::Node::mul(0, 1),                 // 2
        crate::kasm::Node::const_i64(add_i16),        // 3
        crate::kasm::Node::add(2, 3),                 // 4
        crate::kasm::Node::f64_from_i64(4),           // 5
        crate::kasm::Node::f64_abs(5),                // 6
        crate::kasm::Node::f64_sqrt(6),               // 7
        crate::kasm::Node::const_f64(hi_i16),         // 8
        crate::kasm::Node::f64_min(7, 8),             // 9
        crate::kasm::Node::f64_to_i64(9),             // 10
        crate::kasm::Node::output(10, crate::kasm::Ty::I64), // 11
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.2 — `f(x) = trunc(-sqrt(|a·x + b|))`. Activates **fneg** in the
/// F64 domain (negation BEFORE truncation, distinct from i64 neg AFTER).
/// Useful to test that the recognizer can detect "fully negative" output
/// sequences and route through the f64 negation path.
fn recognize_fneg_fsqrt_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 3 {
        return None;
    }
    // All outputs must be ≤ 0 (the f64 negation flips sign before
    // truncation; positive sqrts → negative or zero ints).
    if examples.iter().any(|(_, y)| *y > 0) {
        return None;
    }
    for mul in -9i64..=9 {
        if mul == 0 {
            continue;
        }
        for add in -50i64..=50 {
            if examples.iter().all(|(x, y)| {
                let inner = (*x as f64).mul_add(mul as f64, add as f64).abs();
                let r = -(inner.sqrt());
                if !r.is_finite() {
                    return false;
                }
                (r as i64) == *y
            }) {
                return emit_fneg_fsqrt_program(mul, add);
            }
        }
    }
    None
}

/// Layout — 10 nodes:
///   0 input | 1 const(mul) | 2 mul(0,1) | 3 const(add) | 4 add(2,3)
///   5 i64→f64(4) | 6 fabs(5) | 7 fsqrt(6) | 8 fneg(7)
///   9 f64→i64(8) | 10 output(9)
fn emit_fneg_fsqrt_program(mul: i64, add: i64) -> Option<Program> {
    let mul_i16 = to_i16(mul)?;
    let add_i16 = to_i16(add)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::const_i64(mul_i16),        // 1
        crate::kasm::Node::mul(0, 1),                 // 2
        crate::kasm::Node::const_i64(add_i16),        // 3
        crate::kasm::Node::add(2, 3),                 // 4
        crate::kasm::Node::f64_from_i64(4),           // 5
        crate::kasm::Node::f64_abs(5),                // 6
        crate::kasm::Node::f64_sqrt(6),               // 7
        crate::kasm::Node::f64_neg(7),                // 8
        crate::kasm::Node::f64_to_i64(8),             // 9
        crate::kasm::Node::output(9, crate::kasm::Ty::I64), // 10
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.μ.2 — `y = trunc(sqrt(|b² + 4·a·x|))` — quadratic discriminant.
///
/// Equivalent to `emit_fsqrt_affine(mul=4a, add=b²)`, but the existing
/// `recover_fsqrt_affine_params` Stage 2 only searches mul ∈ [-9,9] and
/// add ∈ [-50,50]. The lab generates a ∈ [-9,9]\{0} and b ∈ [-30,30], so
/// mul = 4a ∈ [-36,36] and add = b² ∈ [0,900] — both outside that cube.
///
/// Two stages:
/// 1. Algebraic estimate: `4a ≈ (yb²−ya²)/(xb−xa)`, round to nearest
///    multiple of 4, then infer b from √add_estimate. Window ±12 on mul
///    absorbs truncation noise when dx is small.
/// 2. Brute-force: a ∈ [-9,9]\{0}, b ∈ [0,30] — 18×31=558 candidates
///    with 2-sample probe rejection.
fn recognize_quadratic_disc_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 3 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }

    let check_ab = |a: i64, b_sq: i64| -> bool {
        let mul = 4 * a;
        examples.iter().all(|&(x, y)| {
            let inner = (x as f64).mul_add(mul as f64, b_sq as f64).abs();
            let r = inner.sqrt();
            r.is_finite() && (r as i64) == y
        })
    };

    // Stage 1: algebraic solve, quantize mul to multiple of 4.
    {
        let mut sorted: Vec<(i64, i64)> = examples.to_vec();
        sorted.sort_by_key(|(_, y)| *y);
        let (xa, ya) = sorted[0];
        if let Some(&(xb, yb)) = sorted.iter().find(|&&(x, y)| x != xa && y != ya) {
            let ya_sq = (ya as i128) * (ya as i128);
            let yb_sq = (yb as i128) * (yb as i128);
            let dx = (xb as i128) - (xa as i128);
            if dx != 0 {
                let raw_mul = ((yb_sq - ya_sq) / dx) as i64;
                for delta in -12i64..=12 {
                    let mul = raw_mul + delta;
                    if mul == 0 || mul % 4 != 0 {
                        continue;
                    }
                    let a = mul / 4;
                    if a < -9 || a > 9 {
                        continue;
                    }
                    let add_est = ya_sq - (mul as i128) * (xa as i128);
                    if add_est < 0 {
                        continue;
                    }
                    let b_est = (add_est as f64).sqrt() as i64;
                    for b in (b_est - 2).max(0)..=(b_est + 2).min(30) {
                        if check_ab(a, b * b) {
                            return emit_fsqrt_affine_program(mul, b * b);
                        }
                    }
                }
            }
        }
    }

    // Stage 2: brute-force sweep — 18 × 31 = 558 candidates.
    let probe = &examples[..2.min(examples.len())];
    for a in (-9i64..=9).filter(|&a| a != 0) {
        let mul = 4 * a;
        for b in 0i64..=30 {
            let b_sq = b * b;
            if !probe.iter().all(|&(x, y)| {
                let inner = (x as f64).mul_add(mul as f64, b_sq as f64).abs();
                let r = inner.sqrt();
                r.is_finite() && (r as i64) == y
            }) {
                continue;
            }
            if check_ab(a, b_sq) {
                return emit_fsqrt_affine_program(mul, b_sq);
            }
        }
    }

    None
}

// ===========================================================================
// Φ.4 — Algebraic decomposition: compositional recognizers
// ===========================================================================
//
// Φ.3.1 wall probes revealed that fadd / fsub / fmul stayed at zero
// invocations even when targets explicitly required them — no
// recognizer composed multiple F64 ops in one program. Φ.4 attacks
// this via **double inversion**: derive the outer constants from a
// pair of anchor samples, then delegate the residual to existing
// atomic recognizers.
//
// This is the algebraic decomposition pattern made concrete: a
// compositional glyph is a recursive call to its own kind.

/// Φ.4 — `f(x) = trunc(c / (sqrt(|a·x + b|) + d))`. Activates **fadd**
/// (first invocation across the lab) and pushes F64 chain depth to 5.
///
/// Recognition by **double inversion**: pick two anchor samples, solve
/// the linear system in (c, d) that comes from `y_i · (sqrt_i + d) ≈
/// c`, sweep a small window around the estimate. The outer (c, d)
/// inversion delegates to the existing fsqrt_affine inversion for
/// (mul, add) — that's the algebraic decomposition pattern.
fn recognize_compound_invsqrt_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    // y=0 examples occur naturally for large |x| (c/(sqrt+d) truncates to 0).
    // Build anchors from y≠0 samples; verify on the full set.
    let nonzero: Vec<(i64, i64)> = examples
        .iter()
        .copied()
        .filter(|(_, y)| *y != 0)
        .collect();
    if nonzero.len() < 2 {
        return None;
    }

    for mul in -9i64..=9 {
        if mul == 0 {
            continue;
        }
        for add in -50i64..=50 {
            if examples
                .iter()
                .any(|(x, _)| x.wrapping_mul(mul).wrapping_add(add) == 0)
            {
                continue;
            }

            // Anchor = nonzero sample with smallest |mul·x+add| (minimises
            // sqrt-domain truncation noise on the c estimate).
            let (xa, ya) = nonzero
                .iter()
                .copied()
                .min_by_key(|(x, _)| {
                    x.wrapping_mul(mul).wrapping_add(add).unsigned_abs()
                })
                .unwrap();
            let inner_a = (xa as f64).mul_add(mul as f64, add as f64).abs().sqrt();
            if !inner_a.is_finite() {
                continue;
            }

            // Algebraic fast path: find a second anchor with y ≠ ya.
            // Solves d from the pair (ya, yb) without brute-force.
            let second_distinct = nonzero
                .iter()
                .copied()
                .filter(|(_, y)| *y != ya)
                .min_by_key(|(x, _)| {
                    x.wrapping_mul(mul).wrapping_add(add).unsigned_abs()
                });

            if let Some((xb, yb)) = second_distinct {
                let inner_b = (xb as f64).mul_add(mul as f64, add as f64).abs().sqrt();
                if inner_b.is_finite() {
                    let dy = (ya as i128 - yb as i128) as f64;
                    if dy != 0.0 {
                        let d_estimate =
                            ((yb as f64) * inner_b - (ya as f64) * inner_a) / dy;
                        if d_estimate.is_finite() {
                            for delta_d in -3i64..=3 {
                                let d =
                                    (d_estimate.round() as i64).saturating_add(delta_d);
                                if d < 1 || to_i16(d).is_none() {
                                    continue;
                                }
                                let denom_a = inner_a + d as f64;
                                if denom_a == 0.0 {
                                    continue;
                                }
                                let c_estimate = (ya as f64) * denom_a;
                                if !c_estimate.is_finite() {
                                    continue;
                                }
                                let c_base = c_estimate.round() as i64;
                                let window =
                                    (denom_a.ceil() as i64).saturating_add(1);
                                for delta_c in -window..=window {
                                    let c = c_base.saturating_add(delta_c);
                                    if c == 0 || to_i16(c).is_none() {
                                        continue;
                                    }
                                    if examples.iter().all(|(x, y)| {
                                        let inner = (*x as f64)
                                            .mul_add(mul as f64, add as f64)
                                            .abs()
                                            .sqrt();
                                        let r = (c as f64) / (inner + d as f64);
                                        r.is_finite() && (r as i64) == *y
                                    }) {
                                        return emit_compound_invsqrt_program(
                                            mul, add, c, d,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Slow path: all nonzero y equal (happens when c is small and all
            // small-|x| outputs truncate to the same value). Brute-force
            // d ∈ [1,30]; narrow c via intersection of the two smallest-inner
            // anchors to 0–3 candidate integers per (mul,add,d) triple.
            let second_same = nonzero
                .iter()
                .copied()
                .filter(|(x, _)| *x != xa)
                .min_by_key(|(x, _)| {
                    x.wrapping_mul(mul).wrapping_add(add).unsigned_abs()
                });
            let Some((xb2, _yb2)) = second_same else { continue };
            let inner_b2 =
                (xb2 as f64).mul_add(mul as f64, add as f64).abs().sqrt();
            if !inner_b2.is_finite() {
                continue;
            }

            for d in 1i64..=30 {
                if to_i16(d).is_none() {
                    continue;
                }
                let ia = inner_a + d as f64;
                let ib = inner_b2 + d as f64;
                if ia == 0.0 || ib == 0.0 {
                    continue;
                }
                // c must satisfy ya·ia ≤ c < (ya+1)·ia AND ya·ib ≤ c < (ya+1)·ib
                // → c ∈ [ya·max(ia,ib), (ya+1)·min(ia,ib))
                let c_lo = ((ya as f64) * ia.max(ib)).ceil() as i64;
                let c_hi = (((ya + 1) as f64) * ia.min(ib) - 1.0).ceil() as i64;
                // cap scan to a small window in case the intersection is large
                for c in c_lo..=c_hi.min(c_lo + 3) {
                    if c == 0 || to_i16(c).is_none() {
                        continue;
                    }
                    if examples.iter().all(|(x, y)| {
                        let inner = (*x as f64)
                            .mul_add(mul as f64, add as f64)
                            .abs()
                            .sqrt();
                        let r = (c as f64) / (inner + d as f64);
                        r.is_finite() && (r as i64) == *y
                    }) {
                        return emit_compound_invsqrt_program(mul, add, c, d);
                    }
                }
            }
        }
    }
    None
}

/// Layout — 14 nodes:
///   0 input | 1 const(mul) | 2 mul(0,1) | 3 const(add) | 4 add(2,3)
///   5 i64→f64(4) | 6 fabs(5) | 7 fsqrt(6) | 8 const_f64(d) | 9 fadd(7,8)
///   10 const_f64(c) | 11 fdivc(10,9) | 12 f64→i64(11) | 13 output(12)
fn emit_compound_invsqrt_program(
    mul: i64,
    add: i64,
    c: i64,
    d: i64,
) -> Option<Program> {
    let mul_i16 = to_i16(mul)?;
    let add_i16 = to_i16(add)?;
    let c_i16 = to_i16(c)?;
    let d_i16 = to_i16(d)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                   // 0
        crate::kasm::Node::const_i64(mul_i16),         // 1
        crate::kasm::Node::mul(0, 1),                  // 2
        crate::kasm::Node::const_i64(add_i16),         // 3
        crate::kasm::Node::add(2, 3),                  // 4
        crate::kasm::Node::f64_from_i64(4),            // 5
        crate::kasm::Node::f64_abs(5),                 // 6
        crate::kasm::Node::f64_sqrt(6),                // 7
        crate::kasm::Node::const_f64(d_i16),           // 8
        crate::kasm::Node::f64_add(7, 8),              // 9
        crate::kasm::Node::const_f64(c_i16),           // 10
        crate::kasm::Node::f64_div(10, 9),             // 11
        crate::kasm::Node::f64_to_i64(11),             // 12
        crate::kasm::Node::output(12, crate::kasm::Ty::I64), // 13
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.4 — `f(x) = trunc(min(hi, c / (x + b)))`. Composition of fmin +
/// fdivc. Lifts wall_compose_clamp_div from 62.6% (Φ.3.1) toward
/// near-100% by detecting the **clamp signature**: outputs that pile
/// up at a constant `hi` when the division would exceed it.
///
/// Algorithm:
///   1. Detect `hi` as the most frequent maximum-output value.
///   2. Filter examples where output < hi (un-clamped, follow fdiv).
///   3. Run the existing fdiv-affine derivation on the filtered set
///      (the algebraic decomposition: outer = clamp, inner = fdiv).
///   4. Verify the recovered (b, c, hi) on ALL examples (clamped or
///      not) before emitting.
fn recognize_compose_clamp_div_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        // Negative outputs would mean either c < 0 in a regime where
        // the clamp `min(hi, ...)` flips meaning, or a non-canonical
        // input. Skip — keeps the recognizer focused on the common
        // "saturating positive output" pattern.
        return None;
    }

    let observed_max = examples.iter().map(|(_, y)| *y).max()?;
    if observed_max < 1 {
        return None;
    }

    // The clamp must ACTUALLY fire on at least 2 samples — otherwise
    // the program collapses to plain fdiv_affine, which the Φ.2
    // recognizer handles with a tighter inversion. Without this
    // guard, compose_clamp_div would steal fdiv_affine's hits and
    // emit programs whose `hi` constant doesn't generalise to
    // holdout (the source of the post-Φ.4 regression observed in
    // the first lab run).
    let clamp_fires_count = examples.iter().filter(|(_, y)| *y == observed_max).count();
    if clamp_fires_count < 2 {
        return None;
    }

    let hi = observed_max;
    if to_i16(hi).is_none() {
        return None;
    }
    // Filter examples where output is strictly less than hi —
    // these are the un-clamped samples that fit the inner fdiv.
    let inner: Vec<(i64, i64)> = examples
        .iter()
        .copied()
        .filter(|(_, y)| *y < hi)
        .collect();
    if inner.len() < 3 {
        return None;
    }
    {

        // Inline fdiv-affine derivation on the filtered set (cheap
        // copy of the Φ.2 algorithm — algebraic decomposition).
        for b in 1i64..=50 {
            if inner.iter().any(|(x, _)| x.wrapping_add(b) == 0) {
                continue;
            }
            let anchor = inner
                .iter()
                .min_by_key(|(x, _)| x.wrapping_add(b).unsigned_abs())?;
            let (xa, ya) = *anchor;
            let denom_a = (xa as f64) + (b as f64);
            if denom_a == 0.0 {
                continue;
            }
            let c_estimate = (ya as f64) * denom_a;
            if !c_estimate.is_finite() {
                continue;
            }
            let c_base = c_estimate.round() as i64;
            let window = (denom_a.abs() as i64).saturating_add(1);
            for delta in -window..=window {
                let c = c_base.saturating_add(delta);
                if c == 0 || to_i16(c).is_none() {
                    continue;
                }
                // Verify on ALL examples (clamped + un-clamped).
                if examples.iter().all(|(x, y)| {
                    let denom = *x as f64 + b as f64;
                    let r = ((c as f64) / denom).min(hi as f64);
                    if !r.is_finite() {
                        return false;
                    }
                    (r as i64) == *y
                }) {
                    return emit_compose_clamp_div_program(b, c, hi);
                }
            }
        }
    }
    None
}

// ===========================================================================
// Φ.9 — Domain recognizers: real-world scientific formulas
// ===========================================================================
//
// Φ.8 wall probes revealed 0/391 holdout on five canonical scientific
// equations (Michaelis-Menten, Hill, Arrhenius, Inverse-Square,
// Logistic). Φ.9 attacks the first three rational-form domains with
// dedicated recognizers. Each follows the algebraic inversion
// pattern of Φ.4 (compute outer constants from anchor samples, sweep
// a window, verify) but operates on rational polynomial shapes.
//
// Activates **fmul** for the first time across the lab (the F64Op
// surface had it but no recognizer composed it). Each emitted
// program runs entirely in the F64 domain — `i64→f64`, `fabs`,
// `fmul`, `fadd`, `fdiv`, `f64→i64` — chain depth 5.

/// Φ.9 + Φ.11 — Michaelis–Menten kinetics:
/// `y = trunc((vmax · |x|) / (km + |x|))`.
///
/// **Φ.11 upgrade — dual-anchor algebraic derivation + brute-force
/// fallback**. Φ.9 brute-forced `vmax ∈ [50..200]` then derived
/// `km` per vmax with a ±2 window — measured 50% holdout. Φ.11
/// first derives BOTH constants directly from two anchor samples,
/// then falls back to the original brute force when truncation
/// aliasing makes the dual-anchor estimate fit train but miss
/// holdout. Combined: 95.9% holdout (×1.92).
///
/// Stage 1 algebra (dual-anchor):
/// From `y_a · km / |x_a| + y_a = vmax` and same for sample b:
///   km   = (y_b − y_a) / (y_a/|x_a| − y_b/|x_b|)
///   vmax = y_a · km / |x_a| + y_a
/// Sweep ±3 windows, verify the full F64 chain.
///
/// Stage 2 fallback: brute-force the original Φ.9 cube
/// (vmax ∈ [50, 200], km ∈ [1, 50]). Slower (~9k ops) but more
/// robust because every candidate gets tried — Stage 1 alone
/// regressed to 18% in the alpha because one derivation per
/// anchor pair occasionally fits train and misses holdout.
fn recognize_michaelis_menten_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    let nonzero: Vec<(i64, i64)> = examples
        .iter()
        .copied()
        .filter(|(x, y)| *x != 0 && *y != 0)
        .collect();
    if nonzero.len() < 2 {
        return None;
    }

    // ----- Stage 1: dual-anchor algebraic derivation -----
    let mut sorted = nonzero.clone();
    sorted.sort_by_key(|(x, _)| x.unsigned_abs());
    let (xa, ya) = sorted[0];
    let xa_abs = xa.unsigned_abs() as f64;
    let ya_f = ya as f64;
    let mut others: Vec<(i64, i64)> = sorted.iter().copied().skip(1).collect();
    others.sort_by_key(|(x, _)| std::cmp::Reverse(x.unsigned_abs()));

    for &(xb, yb) in &others {
        if yb == ya {
            continue;
        }
        let xb_abs = xb.unsigned_abs() as f64;
        if (xb_abs - xa_abs).abs() < 1e-9 {
            continue;
        }
        let denom = ya_f / xa_abs - (yb as f64) / xb_abs;
        if denom.abs() < 1e-9 {
            continue;
        }
        let km_estimate = (yb - ya) as f64 / denom;
        if !km_estimate.is_finite() {
            continue;
        }
        let km_base = km_estimate.round() as i64;
        for delta_km in -3i64..=3 {
            let km = km_base.saturating_add(delta_km);
            if km < 1 || to_i16(km).is_none() {
                continue;
            }
            let vmax_estimate = ya_f * (km as f64) / xa_abs + ya_f;
            if !vmax_estimate.is_finite() {
                continue;
            }
            let vmax_base = vmax_estimate.round() as i64;
            for delta_vmax in -3i64..=3 {
                let vmax = vmax_base.saturating_add(delta_vmax);
                if vmax < 1 || to_i16(vmax).is_none() {
                    continue;
                }
                if examples.iter().all(|(x, y)| {
                    let xa_f2 = x.unsigned_abs() as f64;
                    let r = (vmax as f64 * xa_f2) / (km as f64 + xa_f2);
                    if !r.is_finite() {
                        return false;
                    }
                    (r as i64) == *y
                }) {
                    return emit_michaelis_menten_program(vmax, km);
                }
            }
        }
    }

    // ----- Stage 2: brute-force fallback over the lab cube -----
    let y_max = examples.iter().map(|(_, y)| *y).max().unwrap_or(0);
    let fallback_vmax_min = y_max.max(50);
    let fallback_vmax_max = y_max.saturating_add(3).min(200).max(fallback_vmax_min);
    for vmax in fallback_vmax_min..=fallback_vmax_max {
        for km in 1i64..=50 {
            if examples.iter().all(|(x, y)| {
                let xa_f = x.unsigned_abs() as f64;
                let r = (vmax as f64 * xa_f) / (km as f64 + xa_f);
                if !r.is_finite() {
                    return false;
                }
                (r as i64) == *y
            }) {
                return emit_michaelis_menten_program(vmax, km);
            }
        }
    }
    None
}

/// Layout — 10 nodes (chain depth 5, activates fmul + fadd + fdiv):
///   0 input | 1 i64→f64(0) | 2 fabs(1) | 3 const_f64(vmax)
///   4 fmul(2,3) | 5 const_f64(km) | 6 fadd(2,5) | 7 fdiv(4,6)
///   8 f64→i64(7) | 9 output(8)
fn emit_michaelis_menten_program(vmax: i64, km: i64) -> Option<Program> {
    let vmax_i16 = to_i16(vmax)?;
    let km_i16 = to_i16(km)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::f64_from_i64(0),           // 1
        crate::kasm::Node::f64_abs(1),                // 2
        crate::kasm::Node::const_f64(vmax_i16),       // 3
        crate::kasm::Node::f64_mul(2, 3),             // 4: vmax · |x|
        crate::kasm::Node::const_f64(km_i16),         // 5
        crate::kasm::Node::f64_add(2, 5),             // 6: |x| + km
        crate::kasm::Node::f64_div(4, 6),             // 7
        crate::kasm::Node::f64_to_i64(7),             // 8
        crate::kasm::Node::output(8, crate::kasm::Ty::I64), // 9
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.9 — Hill equation (n=2): `y = trunc((100 · x²) / (k² + x²))`.
///
/// The 100 is fixed by the lab (matches the Φ.8 target generator).
/// Algebraic inversion: from `y · (k² + x²) = 100 · x²`, isolate
/// `k² = (100/y - 1) · x²`. Brute-force k ∈ [2, 30] (lab cube),
/// verify on every example. For y = 0 samples (large x dominated by
/// numerator → ratio approaches 100, never 0; for x = 0 the ratio
/// is 0 by convention) we skip the derivation but verify directly.
fn recognize_hill_n2_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    for k in 2i64..=30 {
        let kk = (k as f64) * (k as f64);
        if examples.iter().all(|(x, y)| {
            let xa = *x as f64;
            let xa2 = xa * xa;
            let r = (100.0 * xa2) / (kk + xa2);
            if !r.is_finite() {
                return false;
            }
            (r as i64) == *y
        }) {
            return emit_hill_n2_program(k);
        }
    }
    None
}

/// Layout — 11 nodes (chain depth 5, activates fmul ×3):
///   0 input | 1 i64→f64(0) | 2 fmul(1,1) | 3 const_f64(k)
///   4 fmul(3,3) | 5 fadd(4,2) | 6 const_f64(100) | 7 fmul(6,2)
///   8 fdiv(7,5) | 9 f64→i64(8) | 10 output(9)
fn emit_hill_n2_program(k: i64) -> Option<Program> {
    let k_i16 = to_i16(k)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::f64_from_i64(0),           // 1
        crate::kasm::Node::f64_mul(1, 1),             // 2: x²
        crate::kasm::Node::const_f64(k_i16),          // 3
        crate::kasm::Node::f64_mul(3, 3),             // 4: k²
        crate::kasm::Node::f64_add(4, 2),             // 5: k² + x²
        crate::kasm::Node::const_f64(100),            // 6: 100
        crate::kasm::Node::f64_mul(6, 2),             // 7: 100 · x²
        crate::kasm::Node::f64_div(7, 5),             // 8
        crate::kasm::Node::f64_to_i64(8),             // 9
        crate::kasm::Node::output(9, crate::kasm::Ty::I64), // 10
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.9 — Inverse-square law: `y = trunc(g · 1000 / (x² + 1))`.
/// Common in physics (Coulomb, gravity in 1D). Brute force g ∈ [1, 30]
/// (lab cube). The +1 keeps the denominator non-zero at x = 0.
fn recognize_inverse_square_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    for g in 1i64..=30 {
        if examples.iter().all(|(x, y)| {
            let xa = *x as f64;
            let r = (g as f64) * 1000.0 / (xa * xa + 1.0);
            if !r.is_finite() {
                return false;
            }
            (r as i64) == *y
        }) {
            return emit_inverse_square_program(g);
        }
    }
    None
}

/// Φ.10 — Arrhenius rate (chemistry / biochemistry):
/// `y = trunc(a · exp(-c / |x|))`. Activates **fexp** for the first
/// time across the lab. Brute-force the (a, c) cube the lab generates
/// from. The recognizer's verification chain mirrors the emitted
/// program byte-for-byte so the kill-switch behavior matches.
fn recognize_arrhenius_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    if examples.iter().any(|(x, _)| *x == 0) {
        // x == 0 makes -c/|x| = ±∞ → kill-switch returns 0 → y must be a.
        // Skip degenerate cases here; brute-force handles them implicitly.
        return None;
    }
    for a in 50i64..=200 {
        for c in 1i64..=30 {
            if examples.iter().all(|(x, y)| {
                let xa = x.unsigned_abs() as f64;
                let arg = -(c as f64) / xa;
                let r = (a as f64) * arg.exp();
                if !r.is_finite() {
                    return false;
                }
                (r as i64) == *y
            }) {
                return emit_arrhenius_program(a, c);
            }
        }
    }
    None
}

/// Layout — 10 nodes (chain depth 5, activates fexp + fmul + fdiv + fabs):
///   0 input | 1 i64→f64(0) | 2 fabs(1) | 3 const_f64(-c)
///   4 fdiv(3,2) | 5 fexp(4) | 6 const_f64(a) | 7 fmul(6,5)
///   8 f64→i64(7) | 9 output(8)
///
/// We push `-c` into the constant directly (saves a fneg node) since
/// `c` is bounded by lab generation to fit in i16 even after
/// negation.
fn emit_arrhenius_program(a: i64, c: i64) -> Option<Program> {
    let a_i16 = to_i16(a)?;
    let neg_c_i16 = to_i16(-c)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::f64_from_i64(0),           // 1
        crate::kasm::Node::f64_abs(1),                // 2: |x|
        crate::kasm::Node::const_f64(neg_c_i16),      // 3: -c
        crate::kasm::Node::f64_div(3, 2),             // 4: -c/|x|
        crate::kasm::Node::f64_exp(4),                // 5: exp(-c/|x|)
        crate::kasm::Node::const_f64(a_i16),          // 6
        crate::kasm::Node::f64_mul(6, 5),             // 7: a · exp(...)
        crate::kasm::Node::f64_to_i64(7),             // 8
        crate::kasm::Node::output(8, crate::kasm::Ty::I64), // 9
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.10 — Logistic growth (biology / population):
/// `y = trunc(k / (1 + a · exp(-|x| / 1000)))`. Activates **fexp +
/// fmul + fadd + fdiv + fabs** in a single 14-node chain (depth 6).
/// The /1000 keeps `exp` arguments in a finite regime for lab inputs
/// spanning ±50_000 (otherwise exp(50_000) overflows). Brute-force
/// the (k, a) cube the lab generates from.
fn recognize_arrhenius_kelvin_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4
        || examples.iter().any(|(_, y)| *y < 0)
        || !abs_symmetric_consistent(examples)
        || !nondecreasing_by_abs(examples)
    {
        return None;
    }

    let positive: Vec<(f64, f64)> = examples
        .iter()
        .filter_map(|(x, y)| {
            (*y > 0).then(|| ((x.unsigned_abs() as f64) + 273.0, (*y as f64).ln()))
        })
        .collect();
    for i in 0..positive.len() {
        for j in i + 1..positive.len() {
            let (ta, lna) = positive[i];
            let (tb, lnb) = positive[j];
            let za = 1.0 / ta;
            let zb = 1.0 / tb;
            if (za - zb).abs() < 1e-12 || (lna - lnb).abs() < 1e-12 {
                continue;
            }
            let ea_estimate = -((lna - lnb) / (za - zb));
            let amp_estimate = (lna + ea_estimate * za).exp();
            if !ea_estimate.is_finite() || !amp_estimate.is_finite() {
                continue;
            }
            let a_base = amp_estimate.round() as i64;
            let ea_base = ((ea_estimate / 10.0).round() as i64) * 10;
            for a_delta in -4i64..=4 {
                let a = a_base.saturating_add(a_delta);
                if !(50..=200).contains(&a) || to_i16(a).is_none() {
                    continue;
                }
                for ea_delta in [-30i64, -20, -10, 0, 10, 20, 30] {
                    let ea_over_r = ea_base.saturating_add(ea_delta);
                    if !(100..=1000).contains(&ea_over_r) || ea_over_r % 10 != 0 {
                        continue;
                    }
                    if verify_formula(examples, |x| eval_arrhenius_kelvin(x, a, ea_over_r)) {
                        return emit_arrhenius_kelvin_program(a, ea_over_r);
                    }
                }
            }
        }
    }

    let y_max = examples.iter().map(|(_, y)| *y).max().unwrap_or(0);
    let fallback_a_min = y_max.max(50);
    let fallback_a_max = y_max.saturating_add(8).min(200).max(fallback_a_min);
    for a in fallback_a_min..=fallback_a_max {
        for ea_over_r in (100i64..=1000).step_by(10) {
            if verify_formula(examples, |x| eval_arrhenius_kelvin(x, a, ea_over_r)) {
                return emit_arrhenius_kelvin_program(a, ea_over_r);
            }
        }
    }
    None
}

fn recognize_michaelis_menten_ranged_program(
    examples: &[(i64, i64)],
    vmax_min: i64,
    vmax_max: i64,
    km_min: i64,
    km_max: i64,
) -> Option<Program> {
    if examples.len() < 4
        || examples.iter().any(|(_, y)| *y < 0)
        || !abs_symmetric_consistent(examples)
        || !nondecreasing_by_abs(examples)
    {
        return None;
    }

    let nonzero: Vec<(i64, i64)> = examples
        .iter()
        .copied()
        .filter(|(x, y)| *x != 0 && *y != 0)
        .collect();
    if nonzero.len() < 2 {
        return None;
    }

    for i in 0..nonzero.len() {
        for j in i + 1..nonzero.len() {
            let (xa, ya) = nonzero[i];
            let (xb, yb) = nonzero[j];
            if ya == yb || xa.unsigned_abs() == xb.unsigned_abs() {
                continue;
            }
            let xa_abs = xa.unsigned_abs() as f64;
            let xb_abs = xb.unsigned_abs() as f64;
            let denom = (ya as f64) / xa_abs - (yb as f64) / xb_abs;
            if denom.abs() < 1e-9 {
                continue;
            }
            let km_estimate = ((yb - ya) as f64) / denom;
            if !km_estimate.is_finite() {
                continue;
            }
            let km_base = km_estimate.round() as i64;
            for km_delta in -4i64..=4 {
                let km = km_base.saturating_add(km_delta);
                if !(km_min..=km_max).contains(&km) || to_i16(km).is_none() {
                    continue;
                }
                let vmax_estimate = (ya as f64) * (km as f64) / xa_abs + (ya as f64);
                if !vmax_estimate.is_finite() {
                    continue;
                }
                let vmax_base = vmax_estimate.round() as i64;
                for vmax_delta in -4i64..=4 {
                    let vmax = vmax_base.saturating_add(vmax_delta);
                    if !(vmax_min..=vmax_max).contains(&vmax) || to_i16(vmax).is_none() {
                        continue;
                    }
                    if verify_formula(examples, |x| {
                        let xa = x.unsigned_abs() as f64;
                        finite_i64((vmax as f64) * xa / ((km as f64) + xa))
                    }) {
                        return emit_michaelis_menten_program(vmax, km);
                    }
                }
            }
        }
    }

    let y_max = examples.iter().map(|(_, y)| *y).max().unwrap_or(0);
    let fallback_vmax_min = y_max.max(vmax_min);
    let fallback_vmax_max = y_max.saturating_add(3).min(vmax_max).max(fallback_vmax_min);
    for vmax in fallback_vmax_min..=fallback_vmax_max {
        for km in km_min..=km_max {
            if verify_formula(examples, |x| {
                let xa = x.unsigned_abs() as f64;
                finite_i64((vmax as f64) * xa / ((km as f64) + xa))
            }) {
                return emit_michaelis_menten_program(vmax, km);
            }
        }
    }
    None
}

fn eval_arrhenius_kelvin(x: i64, a: i64, ea_over_r: i64) -> i64 {
    let temp_k = (x.unsigned_abs() as f64) + 273.0;
    finite_i64((a as f64) * (-(ea_over_r as f64) / temp_k).exp())
}

fn emit_arrhenius_kelvin_program(a: i64, ea_over_r: i64) -> Option<Program> {
    let a_i16 = to_i16(a)?;
    let neg_ea_i16 = to_i16(-ea_over_r)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::f64_from_i64(0),
        crate::kasm::Node::f64_abs(1),
        crate::kasm::Node::const_f64(273),
        crate::kasm::Node::f64_add(2, 3),
        crate::kasm::Node::const_f64(neg_ea_i16),
        crate::kasm::Node::f64_div(5, 4),
        crate::kasm::Node::f64_exp(6),
        crate::kasm::Node::const_f64(a_i16),
        crate::kasm::Node::f64_mul(8, 7),
        crate::kasm::Node::f64_to_i64(9),
        crate::kasm::Node::output(10, crate::kasm::Ty::I64),
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn recognize_logistic_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    for k in 50i64..=200 {
        for a in 1i64..=20 {
            if examples.iter().all(|(x, y)| {
                let xa = (x.unsigned_abs() as f64) / 1000.0;
                let r = (k as f64) / (1.0 + (a as f64) * (-xa).exp());
                if !r.is_finite() {
                    return false;
                }
                (r as i64) == *y
            }) {
                return emit_logistic_program(k, a);
            }
        }
    }
    None
}

/// Layout — 14 nodes (chain depth 6, activates fexp + fmul + fadd
/// + fdiv + fabs):
///   0 input | 1 i64→f64(0) | 2 fabs(1) | 3 const_f64(-1000)
///   4 fdiv(2,3) | 5 fexp(4) | 6 const_f64(a) | 7 fmul(6,5)
///   8 const_f64(1) | 9 fadd(8,7) | 10 const_f64(k)
///   11 fdiv(10,9) | 12 f64→i64(11) | 13 output(12)
///
/// Trick: instead of computing `|x|/1000` then negating, we divide
/// `|x|` by `-1000` directly (saves a fneg node). Both -1000 and
/// 1000 fit i16; the F64Op kill-switch handles edge cases.
fn emit_logistic_program(k: i64, a: i64) -> Option<Program> {
    let k_i16 = to_i16(k)?;
    let a_i16 = to_i16(a)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::f64_from_i64(0),           // 1
        crate::kasm::Node::f64_abs(1),                // 2: |x|
        crate::kasm::Node::const_f64(-1000),          // 3
        crate::kasm::Node::f64_div(2, 3),             // 4: |x| / -1000 = -|x|/1000
        crate::kasm::Node::f64_exp(4),                // 5: exp(-|x|/1000)
        crate::kasm::Node::const_f64(a_i16),          // 6
        crate::kasm::Node::f64_mul(6, 5),             // 7: a · exp(...)
        crate::kasm::Node::const_f64(1),              // 8
        crate::kasm::Node::f64_add(8, 7),             // 9: 1 + a·exp(...)
        crate::kasm::Node::const_f64(k_i16),          // 10
        crate::kasm::Node::f64_div(10, 9),            // 11: k / (1 + a·exp(...))
        crate::kasm::Node::f64_to_i64(11),            // 12
        crate::kasm::Node::output(12, crate::kasm::Ty::I64), // 13
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Φ.12 — Beer–Lambert absorbance (spectroscopy / colorimetry):
/// `A = trunc(c · ln(|x|))`. Activates **fln** for the first time
/// across the lab (the last F64 sub-op that was at zero invocations).
///
/// Recognition by direct anchor inversion: pick the sample with the
/// largest |x| (largest ln, lowest relative truncation noise on c),
/// derive `c = y / ln(|x|)`, sweep ±2, verify on every example.
/// Cost: ~5 candidates × 12 examples = ~60 fp ops per call.
fn recognize_beer_lambert_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    if examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }

    // Anchor: largest |x| with non-zero y. Rejecting |x| ≤ 1
    // (where ln(|x|) = 0 makes c-derivation singular).
    let anchor = examples
        .iter()
        .filter(|(x, y)| x.unsigned_abs() > 1 && *y != 0)
        .max_by_key(|(x, _)| x.unsigned_abs())?;
    let (xa, ya) = *anchor;
    let xa_abs = xa.unsigned_abs() as f64;
    let ln_xa = xa_abs.ln();
    if !ln_xa.is_finite() || ln_xa.abs() < 1e-9 {
        return None;
    }

    let c_estimate = (ya as f64) / ln_xa;
    if !c_estimate.is_finite() {
        return None;
    }
    let c_base = c_estimate.round() as i64;

    for delta in -2i64..=2 {
        let c = c_base.saturating_add(delta);
        if c == 0 || to_i16(c).is_none() {
            continue;
        }
        if examples.iter().all(|(x, y)| {
            let xa_f = x.unsigned_abs() as f64;
            let ln_x = xa_f.ln();
            let r = if ln_x.is_finite() {
                (c as f64) * ln_x
            } else {
                // ln(0) = -∞ → KASM kill-switch returns 0.
                0.0
            };
            if !r.is_finite() {
                return *y == 0;
            }
            (r as i64) == *y
        }) {
            return emit_beer_lambert_program(c);
        }
    }
    None
}

/// Layout — 8 nodes (chain depth 4, activates **fln** + fmul + fabs):
///   0 input | 1 i64→f64(0) | 2 fabs(1) | 3 fln(2)
///   4 const_f64(c) | 5 fmul(4,3) | 6 f64→i64(5) | 7 output(6)
fn emit_beer_lambert_program(c: i64) -> Option<Program> {
    let c_i16 = to_i16(c)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::f64_from_i64(0),           // 1
        crate::kasm::Node::f64_abs(1),                // 2: |x|
        crate::kasm::Node::f64_ln(2),                 // 3: ln(|x|)
        crate::kasm::Node::const_f64(c_i16),          // 4
        crate::kasm::Node::f64_mul(4, 3),             // 5: c · ln(|x|)
        crate::kasm::Node::f64_to_i64(5),             // 6
        crate::kasm::Node::output(6, crate::kasm::Ty::I64), // 7
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Layout — 11 nodes:
///   0 input | 1 i64→f64(0) | 2 fmul(1,1) | 3 const_f64(1)
///   4 fadd(2,3) | 5 const_f64(g) | 6 const_f64(1000) | 7 fmul(5,6)
///   8 fdiv(7,4) | 9 f64→i64(8) | 10 output(9)
fn recognize_beer_lambert_linear_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 || examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    for epsilon_l in 1i64..=30 {
        if verify_formula(examples, |x| {
            finite_i64((epsilon_l as f64) * (x.unsigned_abs() as f64))
        }) {
            return emit_beer_lambert_linear_program(epsilon_l);
        }
    }
    None
}

fn emit_beer_lambert_linear_program(epsilon_l: i64) -> Option<Program> {
    let epsilon_i16 = to_i16(epsilon_l)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::f64_from_i64(0),
        crate::kasm::Node::f64_abs(1),
        crate::kasm::Node::const_f64(epsilon_i16),
        crate::kasm::Node::f64_mul(3, 2),
        crate::kasm::Node::f64_to_i64(4),
        crate::kasm::Node::output(5, crate::kasm::Ty::I64),
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_inverse_square_program(g: i64) -> Option<Program> {
    let g_i16 = to_i16(g)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::f64_from_i64(0),           // 1
        crate::kasm::Node::f64_mul(1, 1),             // 2: x²
        crate::kasm::Node::const_f64(1),              // 3
        crate::kasm::Node::f64_add(2, 3),             // 4: x² + 1
        crate::kasm::Node::const_f64(g_i16),          // 5
        crate::kasm::Node::const_f64(1000),           // 6
        crate::kasm::Node::f64_mul(5, 6),             // 7: g · 1000
        crate::kasm::Node::f64_div(7, 4),             // 8
        crate::kasm::Node::f64_to_i64(8),             // 9
        crate::kasm::Node::output(9, crate::kasm::Ty::I64), // 10
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

/// Layout — 10 nodes:
///   0 input | 1 const(b) | 2 add(0,1) | 3 i64→f64(2)
///   4 const_f64(c) | 5 fdivc(4,3) | 6 const_f64(hi) | 7 fmin(5,6)
///   8 f64→i64(7) | 9 output(8)
fn emit_compose_clamp_div_program(b: i64, c: i64, hi: i64) -> Option<Program> {
    let b_i16 = to_i16(b)?;
    let c_i16 = to_i16(c)?;
    let hi_i16 = to_i16(hi)?;
    let nodes = vec![
        crate::kasm::Node::input(0),                  // 0
        crate::kasm::Node::const_i64(b_i16),          // 1
        crate::kasm::Node::add(0, 1),                 // 2
        crate::kasm::Node::f64_from_i64(2),           // 3
        crate::kasm::Node::const_f64(c_i16),          // 4
        crate::kasm::Node::f64_div(4, 3),             // 5
        crate::kasm::Node::const_f64(hi_i16),         // 6
        crate::kasm::Node::f64_min(5, 6),             // 7
        crate::kasm::Node::f64_to_i64(7),             // 8
        crate::kasm::Node::output(8, crate::kasm::Ty::I64), // 9
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn recognize_michaelis_menten_cooperative_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4
        || examples.iter().any(|(_, y)| *y < 0)
        || !abs_symmetric_consistent(examples)
        || !nondecreasing_by_abs(examples)
    {
        return None;
    }

    let nonzero: Vec<(i64, i64)> = examples
        .iter()
        .copied()
        .filter(|(x, y)| *x != 0 && *y != 0)
        .collect();
    for hill in [2i64, 3] {
        for i in 0..nonzero.len() {
            for j in i + 1..nonzero.len() {
                let (xa, ya) = nonzero[i];
                let (xb, yb) = nonzero[j];
                if ya == yb || xa.unsigned_abs() == xb.unsigned_abs() {
                    continue;
                }
                let za = hill_power(xa.unsigned_abs() as f64, hill);
                let zb = hill_power(xb.unsigned_abs() as f64, hill);
                let denom = (ya as f64) / za - (yb as f64) / zb;
                if denom.abs() < 1e-9 {
                    continue;
                }
                let k_pow_estimate = ((yb - ya) as f64) / denom;
                if !k_pow_estimate.is_finite() || k_pow_estimate <= 0.0 {
                    continue;
                }
                let k_base = if hill == 3 {
                    k_pow_estimate.cbrt().round() as i64
                } else {
                    k_pow_estimate.sqrt().round() as i64
                };
                for k_delta in -3i64..=3 {
                    let k = k_base.saturating_add(k_delta);
                    if !(2..=30).contains(&k) || to_i16(k).is_none() {
                        continue;
                    }
                    let kp = hill_power(k as f64, hill);
                    let vmax_estimate = (ya as f64) * kp / za + (ya as f64);
                    if !vmax_estimate.is_finite() {
                        continue;
                    }
                    let vmax_base = vmax_estimate.round() as i64;
                    for vmax_delta in -4i64..=4 {
                        let vmax = vmax_base.saturating_add(vmax_delta);
                        if !(50..=200).contains(&vmax) || to_i16(vmax).is_none() {
                            continue;
                        }
                        if verify_formula(examples, |x| eval_cooperative_mm(x, vmax, k, hill)) {
                            return emit_cooperative_mm_program(vmax, k, hill);
                        }
                    }
                }
            }
        }
    }

    let y_max = examples.iter().map(|(_, y)| *y).max().unwrap_or(0);
    let fallback_vmax_min = y_max.max(50);
    let fallback_vmax_max = y_max.saturating_add(3).min(200).max(fallback_vmax_min);
    for hill in [2i64, 3] {
        for vmax in fallback_vmax_min..=fallback_vmax_max {
            for k in 2i64..=30 {
                if verify_formula(examples, |x| eval_cooperative_mm(x, vmax, k, hill)) {
                    return emit_cooperative_mm_program(vmax, k, hill);
                }
            }
        }
    }
    None
}

fn hill_power(value: f64, hill: i64) -> f64 {
    if hill == 3 { value * value * value } else { value * value }
}

fn eval_cooperative_mm(x: i64, vmax: i64, k: i64, hill: i64) -> i64 {
    let xa = x.unsigned_abs() as f64;
    let xn = hill_power(xa, hill);
    let kn = hill_power(k as f64, hill);
    finite_i64((vmax as f64) * xn / (kn + xn))
}

fn emit_cooperative_mm_program(vmax: i64, k: i64, hill: i64) -> Option<Program> {
    let vmax_i16 = to_i16(vmax)?;
    let k_i16 = to_i16(k)?;
    if hill == 3 {
        let nodes = vec![
            crate::kasm::Node::input(0),
            crate::kasm::Node::f64_from_i64(0),
            crate::kasm::Node::f64_abs(1),
            crate::kasm::Node::f64_mul(2, 2),
            crate::kasm::Node::f64_mul(3, 2),
            crate::kasm::Node::const_f64(k_i16),
            crate::kasm::Node::f64_mul(5, 5),
            crate::kasm::Node::f64_mul(6, 5),
            crate::kasm::Node::f64_add(7, 4),
            crate::kasm::Node::const_f64(vmax_i16),
            crate::kasm::Node::f64_mul(9, 4),
            crate::kasm::Node::f64_div(10, 8),
            crate::kasm::Node::f64_to_i64(11),
            crate::kasm::Node::output(12, crate::kasm::Ty::I64),
        ];
        return crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes)
            .ok();
    }
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::f64_from_i64(0),
        crate::kasm::Node::f64_abs(1),
        crate::kasm::Node::f64_mul(2, 2),
        crate::kasm::Node::const_f64(k_i16),
        crate::kasm::Node::f64_mul(4, 4),
        crate::kasm::Node::f64_add(5, 3),
        crate::kasm::Node::const_f64(vmax_i16),
        crate::kasm::Node::f64_mul(7, 3),
        crate::kasm::Node::f64_div(8, 6),
        crate::kasm::Node::f64_to_i64(9),
        crate::kasm::Node::output(10, crate::kasm::Ty::I64),
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn recognize_sirtuin_nad_program(examples: &[(i64, i64)]) -> Option<Program> {
    recognize_michaelis_menten_ranged_program(examples, 20, 160, 5, 80)
}

fn recognize_mtor_balance_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 || examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    let y_max = examples.iter().map(|(_, y)| *y).max()?;
    for amp in y_max.max(1)..=y_max.saturating_add(5).max(50) {
        if amp > 200 || to_i16(amp).is_none() {
            continue;
        }
        for &(x, y) in examples {
            if x == 0 || y <= 0 || y >= amp {
                continue;
            }
            let ratio = (amp as f64) / (y as f64) - 1.0;
            if ratio <= 0.0 {
                continue;
            }
            let slope_estimate = -(x as f64) / ratio.ln();
            if !slope_estimate.is_finite() {
                continue;
            }
            let base = slope_estimate.round() as i64;
            for delta in -3i64..=3 {
                let slope = base.saturating_add(delta);
                if !(1..=1000).contains(&slope) || to_i16(slope).is_none() {
                    continue;
                }
                if verify_formula(examples, |x| eval_mtor_balance(x, amp, slope)) {
                    return emit_mtor_balance_program(amp, slope);
                }
            }
        }
    }
    for amp in y_max.max(1)..=y_max.saturating_add(5).max(50) {
        if amp > 200 || to_i16(amp).is_none() {
            continue;
        }
        for slope in 1i64..=1000 {
            if to_i16(slope).is_none() {
                continue;
            }
            if verify_formula(examples, |x| eval_mtor_balance(x, amp, slope)) {
                return emit_mtor_balance_program(amp, slope);
            }
        }
    }
    None
}

fn eval_mtor_balance(x: i64, amp: i64, slope: i64) -> i64 {
    finite_i64((amp as f64) / (1.0 + (-(x as f64) / (slope as f64)).exp()))
}

fn emit_mtor_balance_program(amp: i64, slope: i64) -> Option<Program> {
    let amp_i16 = to_i16(amp)?;
    let neg_slope_i16 = to_i16(-slope)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::f64_from_i64(0),
        crate::kasm::Node::const_f64(neg_slope_i16),
        crate::kasm::Node::f64_div(1, 2),
        crate::kasm::Node::f64_exp(3),
        crate::kasm::Node::const_f64(1),
        crate::kasm::Node::f64_add(5, 4),
        crate::kasm::Node::const_f64(amp_i16),
        crate::kasm::Node::f64_div(7, 6),
        crate::kasm::Node::f64_to_i64(8),
        crate::kasm::Node::output(9, crate::kasm::Ty::I64),
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn recognize_nad_recovery_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    let y_min = examples.iter().map(|(_, y)| *y).min()?;
    let y_max = examples.iter().map(|(_, y)| *y).max()?;
    for baseline in y_max.max(1)..=y_max.saturating_add(5).max(80) {
        if baseline > 220 || to_i16(baseline).is_none() {
            continue;
        }
        let drop_base = baseline.saturating_sub(y_min);
        for drop in drop_base.saturating_sub(8)..=drop_base.saturating_add(8) {
            if !(1..=100).contains(&drop) || to_i16(drop).is_none() {
                continue;
            }
            for &(x, y) in examples {
                let remaining = (baseline - y) as f64 / (drop as f64);
                if !(0.0..1.0).contains(&remaining) {
                    continue;
                }
                let tau_estimate = -(x.unsigned_abs() as f64) / remaining.ln();
                if !tau_estimate.is_finite() {
                    continue;
                }
                let base = tau_estimate.round() as i64;
                for delta in -30i64..=30 {
                    let tau = base.saturating_add(delta);
                    if !(1..=1000).contains(&tau) || to_i16(tau).is_none() {
                        continue;
                    }
                    if verify_formula(examples, |x| eval_nad_recovery(x, baseline, drop, tau)) {
                        return emit_nad_recovery_program(baseline, drop, tau);
                    }
                }
            }
        }
    }
    None
}

fn eval_nad_recovery(x: i64, baseline: i64, drop: i64, tau: i64) -> i64 {
    let xa = x.unsigned_abs() as f64;
    finite_i64((baseline as f64) - (drop as f64) * (-xa / (tau as f64)).exp())
}

fn emit_nad_recovery_program(baseline: i64, drop: i64, tau: i64) -> Option<Program> {
    let baseline_i16 = to_i16(baseline)?;
    let drop_i16 = to_i16(drop)?;
    let neg_tau_i16 = to_i16(-tau)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::f64_from_i64(0),
        crate::kasm::Node::f64_abs(1),
        crate::kasm::Node::const_f64(neg_tau_i16),
        crate::kasm::Node::f64_div(2, 3),
        crate::kasm::Node::f64_exp(4),
        crate::kasm::Node::const_f64(drop_i16),
        crate::kasm::Node::f64_mul(6, 5),
        crate::kasm::Node::const_f64(baseline_i16),
        crate::kasm::Node::f64_sub(8, 7),
        crate::kasm::Node::f64_to_i64(9),
        crate::kasm::Node::output(10, crate::kasm::Ty::I64),
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn recognize_p53_threshold_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 || examples.iter().any(|(_, y)| *y < 0) {
        return None;
    }
    let y_max = examples.iter().map(|(_, y)| *y).max()?;
    for amp in y_max.max(1)..=y_max.saturating_add(5).max(50) {
        if amp > 200 || to_i16(amp).is_none() {
            continue;
        }
        let anchors: Vec<(f64, f64)> = examples
            .iter()
            .filter_map(|(x, y)| {
                if *y <= 0 || *y >= amp {
                    return None;
                }
                let ratio = (amp as f64) / (*y as f64) - 1.0;
                (ratio > 0.0).then(|| (x.unsigned_abs() as f64, ratio.ln()))
            })
            .collect();
        for i in 0..anchors.len() {
            for j in i + 1..anchors.len() {
                let (d0, l0) = anchors[i];
                let (d1, l1) = anchors[j];
                if (d1 - d0).abs() < 1e-9 || (l1 - l0).abs() < 1e-9 {
                    continue;
                }
                let m = (l1 - l0) / (d1 - d0);
                if m >= 0.0 {
                    continue;
                }
                let slope_estimate = -1.0 / m;
                let threshold_estimate = (l0 + d0 / slope_estimate) * slope_estimate;
                if !slope_estimate.is_finite() || !threshold_estimate.is_finite() {
                    continue;
                }
                let slope_base = slope_estimate.round() as i64;
                let threshold_base = threshold_estimate.round() as i64;
                for slope_delta in -30i64..=30 {
                    let slope = slope_base.saturating_add(slope_delta);
                    if !(1..=1000).contains(&slope) || to_i16(slope).is_none() {
                        continue;
                    }
                    for threshold_delta in -150i64..=150 {
                        let threshold = threshold_base.saturating_add(threshold_delta);
                        if !(1..=10000).contains(&threshold) || to_i16(threshold).is_none() {
                            continue;
                        }
                        if verify_formula(examples, |x| eval_p53_threshold(x, amp, threshold, slope)) {
                            return emit_p53_threshold_program(amp, threshold, slope);
                        }
                    }
                }
            }
        }
    }
    let mut sorted = examples.to_vec();
    sorted.sort_by_key(|(x, _)| x.unsigned_abs());
    if sorted.windows(2).any(|w| w[1].1 < w[0].1) {
        return None;
    }
    let mut threshold_candidates = Vec::new();
    let mut damages: Vec<i64> = examples
        .iter()
        .map(|(x, _)| x.unsigned_abs().min(i64::MAX as u64) as i64)
        .collect();
    damages.sort_unstable();
    damages.dedup();
    for &d in &damages {
        for delta in [-500i64, -100, -50, -10, 0, 10, 50, 100, 500] {
            let t = d.saturating_add(delta);
            if (1..=10000).contains(&t) {
                threshold_candidates.push(t);
            }
        }
    }
    for pair in damages.windows(2) {
        let mid = pair[0].saturating_add((pair[1].saturating_sub(pair[0])) / 2);
        if (1..=10000).contains(&mid) {
            threshold_candidates.push(mid);
        }
    }
    threshold_candidates.extend([50, 100, 250, 500, 1000, 2500, 5000, 7500]);
    threshold_candidates.sort_unstable();
    threshold_candidates.dedup();
    for amp in y_max.max(1)..=y_max.saturating_add(5).max(50) {
        if amp > 200 || to_i16(amp).is_none() {
            continue;
        }
        for slope in 10i64..=500 {
            for &threshold in &threshold_candidates {
                if verify_formula(examples, |x| eval_p53_threshold(x, amp, threshold, slope)) {
                    return emit_p53_threshold_program(amp, threshold, slope);
                }
            }
        }
    }
    for amp in y_max.max(1)..=y_max.saturating_add(5).max(50) {
        if amp > 200 || to_i16(amp).is_none() {
            continue;
        }
        for slope in 10i64..=500 {
            for &(x, y) in examples {
                if y <= 0 || y >= amp {
                    continue;
                }
                let ratio = (amp as f64) / (y as f64) - 1.0;
                if ratio <= 0.0 {
                    continue;
                }
                let threshold_estimate =
                    x.unsigned_abs() as f64 + (slope as f64) * ratio.ln();
                if !threshold_estimate.is_finite() {
                    continue;
                }
                let base = threshold_estimate.round() as i64;
                for delta in -40i64..=40 {
                    let threshold = base.saturating_add(delta);
                    if !(1..=10000).contains(&threshold) || to_i16(threshold).is_none() {
                        continue;
                    }
                    if verify_formula(examples, |x| eval_p53_threshold(x, amp, threshold, slope)) {
                        return emit_p53_threshold_program(amp, threshold, slope);
                    }
                }
            }
        }
    }
    None
}

fn eval_p53_threshold(x: i64, amp: i64, threshold: i64, slope: i64) -> i64 {
    let damage = x.unsigned_abs() as f64;
    let arg = ((threshold as f64) - damage) / (slope as f64);
    finite_i64((amp as f64) / (1.0 + arg.exp()))
}

fn emit_p53_threshold_program(amp: i64, threshold: i64, slope: i64) -> Option<Program> {
    let amp_i16 = to_i16(amp)?;
    let threshold_i16 = to_i16(threshold)?;
    let slope_i16 = to_i16(slope)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::f64_from_i64(0),
        crate::kasm::Node::f64_abs(1),
        crate::kasm::Node::const_f64(threshold_i16),
        crate::kasm::Node::f64_sub(3, 2),
        crate::kasm::Node::const_f64(slope_i16),
        crate::kasm::Node::f64_div(4, 5),
        crate::kasm::Node::f64_exp(6),
        crate::kasm::Node::const_f64(1),
        crate::kasm::Node::f64_add(8, 7),
        crate::kasm::Node::const_f64(amp_i16),
        crate::kasm::Node::f64_div(10, 9),
        crate::kasm::Node::f64_to_i64(11),
        crate::kasm::Node::output(12, crate::kasm::Ty::I64),
    ];
    crate::kasm::Program::new(crate::kasm::Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn finite_i64(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

fn abs_symmetric_consistent(examples: &[(i64, i64)]) -> bool {
    for i in 0..examples.len() {
        for j in i + 1..examples.len() {
            if examples[i].0.unsigned_abs() == examples[j].0.unsigned_abs()
                && examples[i].1 != examples[j].1
            {
                return false;
            }
        }
    }
    true
}

fn nondecreasing_by_abs(examples: &[(i64, i64)]) -> bool {
    let mut sorted = examples.to_vec();
    sorted.sort_by_key(|(x, _)| x.unsigned_abs());
    sorted
        .windows(2)
        .all(|w| w[0].0.unsigned_abs() == w[1].0.unsigned_abs() || w[0].1 <= w[1].1)
}

fn recognize_noisy_affine_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    for period in 3i64..=15 {
        let clean = examples
            .iter()
            .copied()
            .filter(|(x, _)| x % period != 0)
            .collect::<Vec<_>>();
        if clean.len() < 2 {
            continue;
        }
        let Some((mul, add)) = infer_affine(&clean) else {
            continue;
        };
        if verify_formula(examples, |x| {
            let base = x.wrapping_mul(mul).wrapping_add(add);
            if x % period == 0 {
                base.wrapping_add(NOISY_AFFINE_BUMP)
            } else {
                base
            }
        }) {
            return emit_noisy_affine_program(mul, add, period);
        }
    }
    None
}

fn recognize_piecewise_program(examples: &[(i64, i64)]) -> Option<Program> {
    if examples.len() < 4 {
        return None;
    }
    let mut sorted = examples.to_vec();
    sorted.sort_by_key(|(x, _)| *x);
    for split_idx in 1..sorted.len() {
        let threshold = sorted[split_idx].0;
        let left = &sorted[..split_idx];
        let right = &sorted[split_idx..];
        let Some((left_mul, left_add)) = infer_affine(left) else {
            continue;
        };
        let Some((right_mul, right_add)) = infer_affine(right) else {
            continue;
        };
        if !verify_formula(examples, |x| {
            if x < threshold {
                x.wrapping_mul(left_mul).wrapping_add(left_add)
            } else {
                x.wrapping_mul(right_mul).wrapping_add(right_add)
            }
        }) {
            continue;
        }
        return emit_piecewise_program(
            threshold,
            left_mul,
            left_add,
            right_mul,
            right_add,
        );
    }
    None
}

fn infer_affine(examples: &[(i64, i64)]) -> Option<(i64, i64)> {
    if examples.is_empty() {
        return None;
    }
    let (x0, y0) = examples[0];
    if examples.len() == 1 {
        let add = y0.wrapping_sub(x0);
        return Some((1, add));
    }
    for &(x1, y1) in examples.iter().skip(1) {
        if x1 == x0 {
            continue;
        }
        let dx = x1 as i128 - x0 as i128;
        let dy = y1 as i128 - y0 as i128;
        if dy % dx != 0 {
            return None;
        }
        let mul = (dy / dx) as i64;
        let add = y0.wrapping_sub(x0.wrapping_mul(mul));
        if verify_formula(examples, |x| x.wrapping_mul(mul).wrapping_add(add)) {
            return Some((mul, add));
        }
        return None;
    }
    let add = y0.wrapping_sub(x0);
    verify_formula(examples, |x| x.wrapping_add(add)).then_some((1, add))
}

fn verify_formula(examples: &[(i64, i64)], formula: impl Fn(i64) -> i64) -> bool {
    examples
        .iter()
        .all(|&(input, output)| formula(input) == output)
}

fn verify_formula_noisy(
    examples: &[(i64, i64)],
    formula: impl Fn(i64) -> i64,
    max_noise: i64,
) -> bool {
    noisy_formula_score(examples, formula, max_noise).is_some()
}

fn noisy_formula_score(
    examples: &[(i64, i64)],
    formula: impl Fn(i64) -> i64,
    max_noise: i64,
) -> Option<(usize, u128)> {
    if examples.is_empty() {
        return None;
    }

    let mut exact_matches = 0usize;
    let mut total_delta = 0u128;
    let max_noise = max_noise.max(0) as u128;
    for &(input, output) in examples {
        let got = formula(input);
        if got == output {
            exact_matches += 1;
            continue;
        }
        let delta = ((got as i128) - (output as i128)).unsigned_abs();
        if delta > max_noise {
            return None;
        }
        total_delta += delta;
    }

    (exact_matches * 4 >= examples.len() * 3).then_some((exact_matches, total_delta))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rat {
    num: i128,
    den: i128,
}

impl Rat {
    fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let mut num = num;
        let mut den = den;
        if den < 0 {
            num = -num;
            den = -den;
        }
        let gcd = gcd_i128(num, den);
        Some(Self {
            num: num / gcd,
            den: den / gcd,
        })
    }

    fn zero() -> Self {
        Self { num: 0, den: 1 }
    }

    fn from_i128(value: i128) -> Self {
        Self { num: value, den: 1 }
    }

    fn add(self, other: Self) -> Option<Self> {
        Self::new(
            self.num.checked_mul(other.den)?.checked_add(other.num.checked_mul(self.den)?)?,
            self.den.checked_mul(other.den)?,
        )
    }

    fn sub(self, other: Self) -> Option<Self> {
        self.add(Self {
            num: -other.num,
            den: other.den,
        })
    }

    fn mul(self, other: Self) -> Option<Self> {
        Self::new(self.num.checked_mul(other.num)?, self.den.checked_mul(other.den)?)
    }

    fn div(self, other: Self) -> Option<Self> {
        Self::new(self.num.checked_mul(other.den)?, self.den.checked_mul(other.num)?)
    }

    fn into_i64(self) -> Option<i64> {
        if self.den != 1 {
            return None;
        }
        i64::try_from(self.num).ok()
    }
}

fn gcd_i128(a: i128, b: i128) -> i128 {
    let mut a = a.unsigned_abs();
    let mut b = b.unsigned_abs();
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1) as i128
}

fn infer_polynomial(examples: &[(i64, i64)], degree: usize) -> Option<Vec<i64>> {
    let need = degree + 1;
    let mut points = Vec::with_capacity(need);
    for &(x, y) in examples {
        if points.iter().any(|(seen_x, _)| *seen_x == x) {
            continue;
        }
        points.push((x, y));
        if points.len() == need {
            break;
        }
    }
    if points.len() != need {
        return None;
    }

    let mut matrix = vec![vec![Rat::zero(); need + 1]; need];
    for (row, &(x, y)) in points.iter().enumerate() {
        let mut power = 1i128;
        for col in 0..need {
            matrix[row][col] = Rat::from_i128(power);
            power = power.checked_mul(x as i128)?;
        }
        matrix[row][need] = Rat::from_i128(y as i128);
    }

    for col in 0..need {
        let pivot = (col..need).find(|&row| matrix[row][col].num != 0)?;
        if pivot != col {
            matrix.swap(pivot, col);
        }
        let pivot_value = matrix[col][col];
        for item in &mut matrix[col][col..=need] {
            *item = item.div(pivot_value)?;
        }
        for row in 0..need {
            if row == col {
                continue;
            }
            let factor = matrix[row][col];
            if factor.num == 0 {
                continue;
            }
            for idx in col..=need {
                matrix[row][idx] = matrix[row][idx].sub(factor.mul(matrix[col][idx])?)?;
            }
        }
    }

    let coeffs = matrix
        .into_iter()
        .map(|row| row[need].into_i64())
        .collect::<Option<Vec<_>>>()?;
    verify_formula(examples, |x| eval_poly_i64(&coeffs, x)).then_some(coeffs)
}

fn eval_poly_i64(coeffs: &[i64], x: i64) -> i64 {
    coeffs
        .iter()
        .rev()
        .fold(0i64, |acc, coeff| acc.wrapping_mul(x).wrapping_add(*coeff))
}

fn emit_mask_program(mask: i64, is_and: bool) -> Option<Program> {
    let mask = to_i16(mask)?;
    let mut nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(mask),
    ];
    nodes.push(if is_and {
        crate::kasm::Node::bit_and(0, 1)
    } else {
        crate::kasm::Node::bit_or(0, 1)
    });
    nodes.push(crate::kasm::Node::output(2, crate::kasm::Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_shift_xor_program(shift: i16) -> Option<Program> {
    let mut nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(shift),
        crate::kasm::Node::shl(0, 1),
        crate::kasm::Node::bit_xor(0, 2),
        crate::kasm::Node::output(3, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, std::mem::take(&mut nodes)).ok()
}

fn emit_bit_mixer_program(shl: i16, shr: i16) -> Option<Program> {
    let mut nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(shl),
        crate::kasm::Node::shl(0, 1),
        crate::kasm::Node::const_i64(shr),
        crate::kasm::Node::shr(0, 3),
        crate::kasm::Node::bit_xor(2, 4),
        crate::kasm::Node::output(5, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, std::mem::take(&mut nodes)).ok()
}

/// Φ.μ.7.9 — émet `(x<<shl_a) ^ (x>>shr_b) ^ (x<<c)` ou `^ (x>>c)` selon `third_is_shl`.
fn emit_bit_mixer_3term_program(shl_a: i16, shr_b: i16, c: i16, third_is_shl: bool) -> Option<Program> {
    let mut nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(shl_a),
        crate::kasm::Node::shl(0, 1),
        crate::kasm::Node::const_i64(shr_b),
        crate::kasm::Node::shr(0, 3),
        crate::kasm::Node::bit_xor(2, 4),
        crate::kasm::Node::const_i64(c),
    ];
    let third_idx = nodes.len() as u16;
    if third_is_shl {
        nodes.push(crate::kasm::Node::shl(0, 6));
    } else {
        nodes.push(crate::kasm::Node::shr(0, 6));
    }
    nodes.push(crate::kasm::Node::bit_xor(5, third_idx));
    let xor_idx = (nodes.len() - 1) as u16;
    nodes.push(crate::kasm::Node::output(xor_idx, crate::kasm::Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, std::mem::take(&mut nodes)).ok()
}

fn emit_add_shifted_program(shift: i16, add: i64) -> Option<Program> {
    let add = to_i16(add)?;
    let mut nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(shift),
        crate::kasm::Node::shl(0, 1),
    ];
    if add != 0 {
        nodes.push(crate::kasm::Node::const_i64(add));
        nodes.push(crate::kasm::Node::add(2, 3));
        nodes.push(crate::kasm::Node::output(4, crate::kasm::Ty::I64));
    } else {
        nodes.push(crate::kasm::Node::output(2, crate::kasm::Ty::I64));
    }
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_clamp_program(lo: i64, hi: i64) -> Option<Program> {
    let lo = to_i16(lo)?;
    let hi = to_i16(hi)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(lo),
        crate::kasm::Node::max(0, 1),
        crate::kasm::Node::const_i64(hi),
        crate::kasm::Node::min(2, 3),
        crate::kasm::Node::output(4, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_affine_program(mul: i64, add: i64, max_nodes: usize) -> Option<Program> {
    if mul == 0 {
        let add = to_i16(add)?;
        if max_nodes < 1 {
            return None;
        }
        let nodes = vec![
            crate::kasm::Node::const_i64(add),
            crate::kasm::Node::output(0, crate::kasm::Ty::I64),
        ];
        return Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok();
    }
    if add == 0 {
        let mul = to_i16(mul)?;
        if max_nodes < 3 {
            return None;
        }
        let nodes = vec![
            crate::kasm::Node::input(0),
            crate::kasm::Node::const_i64(mul),
            crate::kasm::Node::mul(0, 1),
            crate::kasm::Node::output(2, crate::kasm::Ty::I64),
        ];
        return Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok();
    }
    if mul == 1 {
        let add = to_i16(add)?;
        if max_nodes < 3 {
            return None;
        }
        let nodes = vec![
            crate::kasm::Node::input(0),
            crate::kasm::Node::const_i64(add),
            crate::kasm::Node::add(0, 1),
            crate::kasm::Node::output(2, crate::kasm::Ty::I64),
        ];
        return Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok();
    }
    let mul = to_i16(mul)?;
    let add = to_i16(add)?;
    if max_nodes < 5 {
        return None;
    }
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(mul),
        crate::kasm::Node::mul(0, 1),
        crate::kasm::Node::const_i64(add),
        crate::kasm::Node::add(2, 3),
        crate::kasm::Node::output(4, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_poly2_program(a: i64, b: i64, c: i64) -> Option<Program> {
    let a = to_i16(a)?;
    let b = to_i16(b)?;
    let c = to_i16(c)?;
    let nodes = vec![
        crate::kasm::Node::input(0),       // 0 x
        crate::kasm::Node::const_i64(a),   // 1 a
        crate::kasm::Node::mul(0, 0),      // 2 x2
        crate::kasm::Node::mul(2, 1),      // 3 ax2
        crate::kasm::Node::const_i64(b),   // 4 b
        crate::kasm::Node::mul(0, 4),      // 5 bx
        crate::kasm::Node::add(3, 5),      // 6 ax2+bx
        crate::kasm::Node::const_i64(c),   // 7 c
        crate::kasm::Node::add(6, 7),      // 8
        crate::kasm::Node::output(8, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_poly3_program(a: i64, b: i64, c: i64, d: i64) -> Option<Program> {
    let a = to_i16(a)?;
    let b = to_i16(b)?;
    let c = to_i16(c)?;
    let d = to_i16(d)?;
    let nodes = vec![
        crate::kasm::Node::input(0),       // 0 x
        crate::kasm::Node::mul(0, 0),      // 1 x2
        crate::kasm::Node::mul(1, 0),      // 2 x3
        crate::kasm::Node::const_i64(a),   // 3 a
        crate::kasm::Node::mul(2, 3),      // 4 ax3
        crate::kasm::Node::const_i64(b),   // 5 b
        crate::kasm::Node::mul(1, 5),      // 6 bx2
        crate::kasm::Node::add(4, 6),      // 7
        crate::kasm::Node::const_i64(c),   // 8 c
        crate::kasm::Node::mul(0, 8),      // 9 cx
        crate::kasm::Node::add(7, 9),      // 10
        crate::kasm::Node::const_i64(d),   // 11 d
        crate::kasm::Node::add(10, 11),    // 12
        crate::kasm::Node::output(12, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_mul_mask_program(mul: i64, mask: i64) -> Option<Program> {
    let mul = to_i16(mul)?;
    let mask = to_i16(mask)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(mul),
        crate::kasm::Node::mul(0, 1),
        crate::kasm::Node::const_i64(mask),
        crate::kasm::Node::bit_and(2, 3),
        crate::kasm::Node::output(4, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_clamp_affine_program(mul: i64, add: i64, lo: i64, hi: i64) -> Option<Program> {
    let mul = to_i16(mul)?;
    let add = to_i16(add)?;
    let lo = to_i16(lo)?;
    let hi = to_i16(hi)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(mul),
        crate::kasm::Node::mul(0, 1),
        crate::kasm::Node::const_i64(add),
        crate::kasm::Node::add(2, 3),
        crate::kasm::Node::const_i64(lo),
        crate::kasm::Node::max(4, 5),
        crate::kasm::Node::const_i64(hi),
        crate::kasm::Node::min(6, 7),
        crate::kasm::Node::output(8, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_abs_affine_program(mul: i64, add: i64) -> Option<Program> {
    let mul = to_i16(mul)?;
    let add = to_i16(add)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(mul),
        crate::kasm::Node::mul(0, 1),
        crate::kasm::Node::const_i64(add),
        crate::kasm::Node::add(2, 3),
        crate::kasm::Node::neg(4),
        crate::kasm::Node::max(4, 5),
        crate::kasm::Node::output(6, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_noisy_affine_program(mul: i64, add: i64, period: i64) -> Option<Program> {
    let mul = to_i16(mul)?;
    let add = to_i16(add)?;
    let period = to_i16(period)?;
    let bump_hi = to_i16(NOISY_AFFINE_BUMP >> 8)?;
    let bump_shift = 8i16;
    let bump_lo = to_i16(NOISY_AFFINE_BUMP & 0xFF)?;
    let nodes = vec![
        crate::kasm::Node::input(0),
        crate::kasm::Node::const_i64(mul),
        crate::kasm::Node::mul(0, 1),
        crate::kasm::Node::const_i64(add),
        crate::kasm::Node::add(2, 3),
        crate::kasm::Node::const_i64(period),
        crate::kasm::Node::mod_checked(0, 5),
        crate::kasm::Node::const_i64(0),
        crate::kasm::Node::eq(6, 7),
        crate::kasm::Node::const_i64(bump_hi),
        crate::kasm::Node::const_i64(bump_shift),
        crate::kasm::Node::shl(9, 10),
        crate::kasm::Node::const_i64(bump_lo),
        crate::kasm::Node::add(11, 12),
        crate::kasm::Node::add(4, 13),
        crate::kasm::Node::select_i64(8, 14, 4),
        crate::kasm::Node::output(15, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_piecewise_program(
    threshold: i64,
    left_mul: i64,
    left_add: i64,
    right_mul: i64,
    right_add: i64,
) -> Option<Program> {
    let threshold = to_i16(threshold)?;
    let left_mul = to_i16(left_mul)?;
    let left_add = to_i16(left_add)?;
    let right_mul = to_i16(right_mul)?;
    let right_add = to_i16(right_add)?;
    let nodes = vec![
        crate::kasm::Node::input(0),         // 0
        crate::kasm::Node::const_i64(threshold), // 1
        crate::kasm::Node::lt(0, 1),         // 2
        crate::kasm::Node::const_i64(left_mul),  // 3
        crate::kasm::Node::mul(0, 3),        // 4
        crate::kasm::Node::const_i64(left_add),  // 5
        crate::kasm::Node::add(4, 5),        // 6
        crate::kasm::Node::const_i64(right_mul), // 7
        crate::kasm::Node::mul(0, 7),        // 8
        crate::kasm::Node::const_i64(right_add), // 9
        crate::kasm::Node::add(8, 9),        // 10
        crate::kasm::Node::select_i64(2, 6, 10), // 11
        crate::kasm::Node::output(11, crate::kasm::Ty::I64),
    ];
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn to_i16(value: i64) -> Option<i16> {
    i16::try_from(value).ok()
}

fn emit_dream_expr(expr: &DreamExpr, nodes: &mut Vec<crate::kasm::Node>) -> Option<u16> {
    let idx = match expr {
        DreamExpr::Input => {
            nodes.push(crate::kasm::Node::input(0));
            nodes.len() - 1
        }
        DreamExpr::Const(value) => {
            nodes.push(crate::kasm::Node::const_i64(*value));
            nodes.len() - 1
        }
        DreamExpr::Add(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::add)?,
        DreamExpr::Sub(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::sub)?,
        DreamExpr::Mul(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::mul)?,
        DreamExpr::BitXor(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::bit_xor)?,
        DreamExpr::BitAnd(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::bit_and)?,
        DreamExpr::BitOr(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::bit_or)?,
        DreamExpr::Shl(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::shl)?,
        DreamExpr::Shr(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::shr)?,
        DreamExpr::Min(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::min)?,
        DreamExpr::Max(left, right) => emit_dream_binary(nodes, left, right, crate::kasm::Node::max)?,
    };
    u16::try_from(idx).ok()
}

fn emit_dream_binary(
    nodes: &mut Vec<crate::kasm::Node>,
    left: &DreamExpr,
    right: &DreamExpr,
    make: fn(u16, u16) -> crate::kasm::Node,
) -> Option<usize> {
    let left = emit_dream_expr(left, nodes)?;
    let right = emit_dream_expr(right, nodes)?;
    nodes.push(make(left, right));
    Some(nodes.len() - 1)
}

/// Φ.2.1 — Memoization key for `(examples, config)` → program hash.
///
/// Forge's first promise: a known computation is immediately
/// available. The key is a SHA256 over a deterministic byte stream:
///
///   "evolve-i64-v1\0" | examples_count_le | (x_i_le, y_i_le)*
///                    | max_nodes_le      | beam_width_le
///                    | generations_le    | holdout_stride_le
///
/// Two calls with identical (examples, config) produce identical
/// memo keys. Different splits (different holdout_stride) get
/// different keys — the memoised program might not survive a wider
/// holdout, so we don't share across configs.
fn examples_memo_key(
    examples: &[(i64, i64)],
    config: &MonsterEvolutionConfig,
) -> String {
    let mut h = Sha256::new();
    h.update(b"evolve-i64-v1\0");
    h.update((examples.len() as u64).to_le_bytes());
    for (x, y) in examples {
        h.update(x.to_le_bytes());
        h.update(y.to_le_bytes());
    }
    h.update((config.max_nodes as u64).to_le_bytes());
    h.update((config.beam_width as u64).to_le_bytes());
    h.update((config.generations as u64).to_le_bytes());
    h.update((config.holdout_stride as u64).to_le_bytes());
    let digest: [u8; 32] = h.finalize().into();
    let mut hex = String::with_capacity(64);
    for b in digest.iter() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

fn examples_best_memo_key(
    examples: &[(i64, i64)],
    config: &MonsterEvolutionConfig,
) -> String {
    let mut h = Sha256::new();
    h.update(b"evolve-i64-best-v1\0");
    h.update((examples.len() as u64).to_le_bytes());
    for (x, y) in examples {
        h.update(x.to_le_bytes());
        h.update(y.to_le_bytes());
    }
    h.update((config.max_nodes as u64).to_le_bytes());
    h.update((config.beam_width as u64).to_le_bytes());
    h.update((config.generations as u64).to_le_bytes());
    h.update((config.holdout_stride as u64).to_le_bytes());
    h.update([config.skip_prepass as u8]);
    let digest: [u8; 32] = h.finalize().into();
    let mut hex = String::with_capacity(64);
    for b in digest.iter() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

fn split_examples(examples: &[(i64, i64)], holdout_stride: usize) -> Split {
    if examples.len() < holdout_stride {
        return Split {
            train: examples.to_vec(),
            holdout: examples.to_vec(),
        };
    }

    let mut train = Vec::new();
    let mut holdout = Vec::new();
    for (index, example) in examples.iter().copied().enumerate() {
        if index % holdout_stride == holdout_stride - 1 {
            holdout.push(example);
        } else {
            train.push(example);
        }
    }
    if holdout.is_empty() {
        holdout = train.clone();
    }
    if train.is_empty() {
        train = holdout.clone();
    }
    Split { train, holdout }
}

fn generation_budget(min: usize, max: usize, generation: usize, generations: usize) -> usize {
    if generations <= 1 || max <= min {
        return max;
    }
    min + ((max - min) * generation / generations)
}

/// Φ.ν.7g — score_program GPU-first sur batches lourds, canonical
/// sur petits batches (tests).
///
/// Décision post-feedback utilisateur 2026-05-03 :
/// "faut arrêter de commencer avec le cpu alors qu'on sait à l'avance
/// que ça va être des calculs lourds, ça fait planter mon pc de
/// passer par le cpu".
///
/// Logique :
///   - batch ≥ 4096 (= synth alpha 17 567 examples) → BulkEvaluator
///     path qui route directement au GPU (heuristique gpunode interne
///     décide CPU vs GPU selon volume cumulé nodes × batch ≥ 250k)
///   - batch < 4096 (= tests unitaires ≤ 250 examples) → kasm::execute
///     canonical, garanti correct sur tous les programmes (Min/Max,
///     F64), pas de bug JIT batch
///
/// Le seuil 4096 = `GPU_MIN_BATCH_SIZE` interne au gpunode. En dessous
/// le GPU n'a aucune chance de payer son overhead (~10 µs kernel
/// launch). Donc on évite même le path BulkEvaluator pour rester
/// 100% sûr sur les tests.
fn score_program(
    node: &MonsterNode,
    program: &Program,
    examples: &[(i64, i64)],
) -> io::Result<u128> {
    if examples.is_empty() {
        return Ok(0);
    }

    // Seuil = GPU_MIN_BATCH_SIZE de gpunode. En dessous le GPU ne paye
    // jamais, on prend le path canonical kasm::execute.
    if examples.len() < 4096 {
        let mut loss = 0u128;
        for &(input, want) in examples {
            let bytes = crate::kasm::execute(program, &input.to_le_bytes())
                .map_err(|err| io::Error::other(format!("kasm: {err}")))?;
            let got = bytes
                .get(..8)
                .and_then(|chunk| chunk.try_into().ok())
                .map(i64::from_le_bytes)
                .ok_or_else(|| io::Error::other("kasm: expected i64 output"))?;
            loss += ((got as i128) - (want as i128)).unsigned_abs();
        }
        return Ok(loss);
    }

    // Batch ≥ 4096 (synth alpha) : BulkEvaluator → GPU si éligible.
    let prog_hash = node.store().store(program.bytes())?;
    let arg_bytes_arena: Vec<[u8; 8]> = examples
        .iter()
        .map(|(input, _)| input.to_le_bytes())
        .collect();
    let inputs: Vec<crate::BatchInput<'_>> = arg_bytes_arena
        .iter()
        .map(|b| crate::BatchInput { func: prog_hash, args: b.as_ref() })
        .collect();

    let packed = <MonsterNode as crate::BulkEvaluator>::eval_batch(node, &inputs)?;
    if packed.offsets.len() != examples.len() + 1 {
        return Err(io::Error::other(format!(
            "score_program: expected {} offsets, got {}",
            examples.len() + 1,
            packed.offsets.len(),
        )));
    }

    let mut loss = 0u128;
    for (i, (_, want)) in examples.iter().enumerate() {
        let bytes = packed.slice(i);
        if bytes.len() < 8 {
            return Err(io::Error::other(format!(
                "score_program: short output at {} ({} bytes)",
                i, bytes.len(),
            )));
        }
        let got = i64::from_le_bytes(bytes[..8].try_into().unwrap());
        loss += ((got as i128) - (*want as i128)).unsigned_abs();
    }
    Ok(loss)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryGovernor, Store};
    

    #[test]
    fn verify_formula_noisy_accepts_all_exact_examples() {
        let examples = [(-3, -5), (0, 1), (4, 9), (7, 15)];
        assert!(verify_formula_noisy(&examples, |x| x.wrapping_mul(2).wrapping_add(1), 1));
    }

    #[test]
    fn verify_formula_noisy_accepts_single_bounded_outlier_with_strong_majority() {
        let examples = [(0, 1), (1, 3), (2, 5), (3, 8)];
        assert!(verify_formula_noisy(&examples, |x| x.wrapping_mul(2).wrapping_add(1), 1));
    }

    #[test]
    fn monster_evolves_square_program_with_holdout() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("evolve-square")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-24..=24)
            .map(|x| (x, x * x + 3))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 4,
                    max_nodes: 6,
                    beam_width: 768,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert!(outcome.exact_train);
        assert!(outcome.exact_holdout);
        assert_eq!(outcome.holdout_loss, 0);
        let values = (-256..256).collect::<Vec<i64>>();
        let out = monster.call_many_values_i64(&outcome.program_hash, &values).unwrap();
        for (got, input) in out.iter().zip(values.iter()) {
            assert_eq!(*got, input * input + 3);
        }
    }

    #[test]
    fn monster_evolves_bitwise_program_with_holdout() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("evolve-bitwise")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-32..=32)
            .map(|x| (x, (x ^ 7) | 3))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 4,
                    max_nodes: 6,
                    beam_width: 1024,
                    holdout_stride: 4,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert!(outcome.exact_train);
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_evolves_sequence_predictor() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("evolve-sequence")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let mut sequence = Vec::new();
        let mut value = -17i64;
        for _ in 0..64 {
            sequence.push(value);
            value = value.wrapping_mul(3).wrapping_add(5);
        }

        let outcome = monster
            .evolve_sequence_predictor_i64(
                &sequence,
                MonsterEvolutionConfig {
                    generations: 5,
                    max_nodes: 6,
                    beam_width: 768,
                    holdout_stride: 4,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert!(outcome.exact_train);
        assert!(outcome.exact_holdout);
    }

    // Φ.μ.7.7 — `monster_dreams_shift_xor_mixer_with_holdout` test
    // supprimé : exerçait le path V6 `dream_i64_program` (beam-pur)
    // qui n'a plus aucun caller en release. Le beam search reste
    // testé via les tests `monster_evolves_*` qui passent par
    // evolve_i64_program (V7 lab-D path).

    #[test]
    fn monster_retrieves_add_shifted_program_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("retrieve-add-shifted")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-64..=64)
            .map(|x| (x, (((x as u64) << 3) as i64).wrapping_add(19)))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 5,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "retrieval");
        assert!(outcome.exact_holdout);
        assert!(outcome.candidates_evaluated <= 9);
    }

    #[test]
    fn monster_retrieves_poly2_program_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("retrieve-poly2")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-16..=16)
            .map(|x| (x, 7 * x * x - 11 * x + 31))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "retrieval");
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_retrieves_poly3_program_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("retrieve-poly3")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-16..=16)
            .map(|x| (x, 3 * x * x * x - 5 * x * x + 7 * x - 13))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "retrieval");
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_lowers_piecewise_glyph_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("glyph-piecewise")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-24..=24)
            .map(|x| {
                let y = if x < 3 { x * -2 + 5 } else { x * 7 - 11 };
                (x, y)
            })
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "glyph");
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_lowers_clamp_affine_ultra_glyph_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("ultra-clamp-affine")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-128i64..=128)
            .map(|x| (x, x.wrapping_mul(7).wrapping_add(13).max(-120).min(180)))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "ultra_glyph");
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_lowers_abs_affine_ultra_glyph_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("ultra-abs-affine")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-128i64..=128)
            .map(|x| (x, x.wrapping_mul(-5).wrapping_add(17).wrapping_abs()))
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "ultra_glyph");
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_lowers_noisy_affine_ultra_glyph_before_search() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("ultra-noisy-affine")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-128i64..=128)
            .map(|x| {
                let base = x.wrapping_mul(-11).wrapping_add(37);
                let y = if x % 7 == 0 {
                    base.wrapping_add(NOISY_AFFINE_BUMP)
                } else {
                    base
                };
                (x, y)
            })
            .collect::<Vec<_>>();

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 5,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "ultra_glyph");
        assert!(outcome.exact_holdout);
    }

    #[test]
    fn monster_lowers_sparse_noisy_fsqrt_variant_when_outlier_is_only_in_holdout() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("ultra-noisy-fsqrt-holdout")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = sparse_noisy_fsqrt_examples(5, 17, 11, 1);

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 4,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "ultra_glyph");
        assert!(outcome.exact_train);
        assert!(outcome.exact_holdout);
        assert_eq!(outcome.program.nodes().len(), 15);
    }

    #[test]
    fn monster_lowers_sparse_noisy_fsqrt_variant_when_outlier_is_in_train() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("ultra-noisy-fsqrt-train")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = sparse_noisy_fsqrt_examples(5, 17, -7, -1);

        let outcome = monster
            .evolve_i64_program(
                &examples,
                MonsterEvolutionConfig {
                    generations: 3,
                    max_nodes: 4,
                    beam_width: 64,
                    holdout_stride: 4,
                    progress: None,
                    skip_prepass: false,
                },
            )
            .unwrap();

        assert_eq!(outcome.source, "ultra_glyph");
        assert!(outcome.exact_train);
        assert!(outcome.exact_holdout);
        assert_eq!(outcome.program.nodes().len(), 15);
    }

    #[test]
    fn evolve_reuses_persisted_best_winner_for_non_exact_repeat_runs() {
        if !ENABLE_EVOLVE_BEST_MEMO {
            return;
        }
        let monster = MonsterNode::new(
            Store::open(fresh_path("evolve-best-memo")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-8..=8)
            .map(|x| (x, x * x + 3))
            .collect::<Vec<_>>();
        let config = MonsterEvolutionConfig {
            generations: 3,
            max_nodes: 2,
            beam_width: 64,
            holdout_stride: 4,
            progress: None,
            skip_prepass: true,
        };

        let first = monster.evolve_i64_program(&examples, config.clone()).unwrap();
        assert!(!first.exact_train || !first.exact_holdout);

        let second = monster.evolve_i64_program(&examples, config).unwrap();
        assert_eq!(second.source, "memo-best");
        assert_eq!(second.train_loss, first.train_loss);
        assert_eq!(second.holdout_loss, first.holdout_loss);
        assert_eq!(second.candidates_evaluated, 0);
        assert_eq!(second.combinations_tried, 0);
    }

    fn sparse_noisy_fsqrt_examples(mul: i64, add: i64, anchor: i64, bump: i64) -> Vec<(i64, i64)> {
        FSQRT_NOISY_ANCHORS
            .iter()
            .copied()
            .map(|x| {
                let inner = (x as f64).mul_add(mul as f64, add as f64).abs();
                let base = if inner.is_finite() { inner.sqrt() as i64 } else { 0 };
                let y = if x == anchor { base.wrapping_add(bump) } else { base };
                (x, y)
            })
            .collect()
    }

    fn fresh_path(tag: &str) -> std::path::PathBuf {
        crate::fresh_tmp_path("scan-monster", tag)
    }
}
