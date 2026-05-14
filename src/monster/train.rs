//! Internal program synthesis for MonsterNode.
//!
//! This is deliberately small and deterministic: given i64 -> i64
//! examples, MonsterNode searches a bounded symbolic KASM space and
//! stores the best program it can synthesize.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};

use crate::kasm::{Node, Program, Target, Ty};
use crate::Hash;

use super::{swiss_table::SwissMap, MonsterNode};

/// Per-depth progress report emitted during beam search.
#[derive(Debug, Clone)]
pub struct SynthProgress {
    pub depth: usize,
    pub max_depth: usize,
    pub pairs: usize,
    pub gpu_used: bool,
    pub gpu_eligible: bool,
    pub gpu_attempted: bool,
    pub best_loss: u128,
    pub beam_size: usize,
    pub depth_ms: u64,
    pub depth_ns: u128,
    /// "start" = about to compute this depth, "done" = depth completed.
    pub phase: &'static str,
    /// Total scorings dispatched this depth (pairs × ops).
    pub total_scorings: usize,
    /// Number of examples per scoring (size of the target vector).
    pub n_examples: usize,
    /// GPU backend that actually executed ("cuda+wgpu", "cuda", "wgpu", "").
    pub gpu_backend: &'static str,
    /// Number of full pair bundles served from Atlas this depth.
    pub atlas_full_pair_hits: usize,
    /// Number of individual opcode scores served from Atlas this depth.
    pub atlas_opcode_hits: usize,
    /// Number of jobs actually sent to GPU/CPU scorer this depth.
    pub jobs_dispatched: usize,
    /// Number of jobs skipped thanks to Atlas memoization this depth.
    pub jobs_skipped: usize,
    /// String representation of the best expression found so far.
    pub best_expr: String,
}

pub type SynthProgressFn = Arc<dyn Fn(SynthProgress) + Send + Sync>;

#[derive(Clone)]
pub struct MonsterTrainingConfig {
    pub max_nodes: usize,
    pub beam_width: usize,
    pub progress: Option<SynthProgressFn>,
}

impl std::fmt::Debug for MonsterTrainingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonsterTrainingConfig")
            .field("max_nodes", &self.max_nodes)
            .field("beam_width", &self.beam_width)
            .field("progress", &self.progress.is_some())
            .finish()
    }
}

impl Default for MonsterTrainingConfig {
    fn default() -> Self {
        Self {
            max_nodes: 9,
            beam_width: 256,
            progress: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonsterTrainingOutcome {
    pub program_hash: Hash,
    pub program: Program,
    pub loss: u128,
    pub exact: bool,
    pub candidates_evaluated: usize,
    /// Total (left × right × op) triples attempted by push_binary,
    /// including those filtered out early (constant, duplicate, etc.).
    pub combinations_tried: usize,
    /// push_binary calls where the score came from the atlas RESULT
    /// cache instead of being computed fresh against all training examples.
    pub atlas_score_hits: usize,
    pub atlas_full_pair_hits: usize,
    pub atlas_opcode_hits: usize,
    pub gpu_jobs_dispatched: usize,
    pub gpu_jobs_skipped: usize,
}

#[derive(Clone)]
struct Candidate {
    expr: Expr,
    outputs: Vec<i64>,
    loss: u128,
    nodes: usize,
}

#[derive(Clone)]
struct PendingCandidate {
    left_idx: usize,
    right_idx: usize,
    op_idx: usize,
    loss: u128,
    nodes: usize,
    outputs: Option<Vec<i64>>,
}

struct GpuDepthOutcome {
    pending: Vec<PendingCandidate>,
    atlas_score_hits: usize,
    atlas_full_pair_hits: usize,
    atlas_opcode_hits: usize,
    jobs_dispatched: usize,
    jobs_skipped: usize,
    gpu_used: bool,
}


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PairHotKey {
    left_fp: u64,
    right_fp: u64,
    targets_fp: u64,
    n_examples: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PairOpHotKey {
    pair: PairHotKey,
    op: u8,
}

#[derive(Clone)]
struct HotPairBundle {
    entries: [PairScoreEntry; NUM_OPS],
    outputs: Option<Arc<Vec<Vec<i64>>>>,
}

const DEPTH_FRONTIER_CACHE_MAGIC: &[u8; 8] = b"DFCACHE1";

static DEPTH_FRONTIER_CACHE: OnceLock<Mutex<HashMap<String, Vec<Candidate>>>> = OnceLock::new();
const DEPTH_FRONTIER_CACHE_MAX_ENTRIES: usize = 256;
static HOT_PAIR_BUNDLE_CACHE: OnceLock<Mutex<SwissMap<PairHotKey, HotPairBundle>>> = OnceLock::new();
static HOT_PAIR_OP_CACHE: OnceLock<Mutex<SwissMap<PairOpHotKey, PairScoreEntry>>> = OnceLock::new();
const HOT_PAIR_BUNDLE_CACHE_MAX_ENTRIES: usize = 16_384;
const HOT_PAIR_OP_CACHE_MAX_ENTRIES: usize = 131_072;
const ENABLE_TRAIN_WINNER_MEMO: bool = false;
const ENABLE_DEPTH_FRONTIER_RAM_CACHE: bool = true;
const ENABLE_DEPTH_FRONTIER_PERSIST_CACHE: bool = true;
const ENABLE_HOT_PAIR_CACHES: bool = false;

#[derive(Clone, PartialEq, Eq, Hash)]
enum Expr {
    Input,
    Const(i16),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    BitXor(Box<Expr>, Box<Expr>),
    BitAnd(Box<Expr>, Box<Expr>),
    BitOr(Box<Expr>, Box<Expr>),
    CmpGt(Box<Expr>, Box<Expr>),
    CmpLt(Box<Expr>, Box<Expr>),
    Select(Box<Expr>, Box<Expr>),
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Input => write!(f, "x"),
            Expr::Const(v) => write!(f, "{v}"),
            Expr::Add(a, b) => write!(f, "({a}+{b})"),
            Expr::Sub(a, b) => write!(f, "({a}-{b})"),
            Expr::Mul(a, b) => write!(f, "({a}*{b})"),
            Expr::BitXor(a, b) => write!(f, "({a}^{b})"),
            Expr::BitAnd(a, b) => write!(f, "({a}&{b})"),
            Expr::BitOr(a, b) => write!(f, "({a}|{b})"),
            Expr::CmpGt(a, b) => write!(f, "({a}>{b})"),
            Expr::CmpLt(a, b) => write!(f, "({a}<{b})"),
            Expr::Select(a, b) => write!(f, "sel({a},{b})"),
        }
    }
}

impl MonsterNode {
    pub fn train_i64_program(
        &self,
        examples: &[(i64, i64)],
        config: MonsterTrainingConfig,
    ) -> io::Result<MonsterTrainingOutcome> {
        let memo_key = training_memo_key(examples, &config);
        if ENABLE_TRAIN_WINNER_MEMO {
            if let Some(program_hash) = self.store().lookup_memo(&memo_key) {
                if let Some(program_bytes) = self.store().load(&program_hash) {
                    if let Ok(program) = Program::from_bytes(&program_bytes) {
                        let loss = score_program_examples(self, &program_hash, examples)?;
                        if let Some(ref cb) = config.progress {
                            cb(SynthProgress {
                                depth: 0,
                                max_depth: config.max_nodes.saturating_sub(1),
                                pairs: 0,
                                gpu_used: false,
                                gpu_eligible: false,
                                gpu_attempted: false,
                                best_loss: loss,
                                beam_size: 0,
                                depth_ms: 0,
                                depth_ns: 0,
                                phase: "evolve",
                                total_scorings: 0,
                                n_examples: examples.len(),
                                gpu_backend: "",
                                atlas_full_pair_hits: 0,
                                atlas_opcode_hits: 0,
                                jobs_dispatched: 0,
                                jobs_skipped: 0,
                                best_expr: format!(
                                    "beam winner cache hit: persisted train winner reused (loss={loss})"
                                ),
                            });
                        }
                        return Ok(MonsterTrainingOutcome {
                            program_hash,
                            program,
                            loss,
                            exact: loss == 0,
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

        let atlas = self.atlas();
        let outcome = synthesize_i64(examples, &config, Some(self.store()), atlas.as_deref())
            .ok_or_else(|| io::Error::other("MonsterNode could not synthesize any KASM candidate"))?;
        let program_hash = self.store().store(outcome.program.bytes())?;
        let training = MonsterTrainingOutcome {
            program_hash,
            program: outcome.program,
            loss: outcome.loss,
            exact: outcome.loss == 0,
            candidates_evaluated: outcome.candidates_evaluated,
            combinations_tried: outcome.combinations_tried,
            atlas_score_hits: outcome.atlas_score_hits,
            atlas_full_pair_hits: outcome.atlas_full_pair_hits,
            atlas_opcode_hits: outcome.atlas_opcode_hits,
            gpu_jobs_dispatched: outcome.gpu_jobs_dispatched,
            gpu_jobs_skipped: outcome.gpu_jobs_skipped,
        };
        if ENABLE_TRAIN_WINNER_MEMO {
            let _ = self.store().write_memo(&memo_key, &training.program_hash);
        }
        Ok(training)
    }
}

struct SynthesisOutcome {
    program: Program,
    loss: u128,
    candidates_evaluated: usize,
    combinations_tried: usize,
    atlas_score_hits: usize,
    atlas_full_pair_hits: usize,
    atlas_opcode_hits: usize,
    gpu_jobs_dispatched: usize,
    gpu_jobs_skipped: usize,
}

fn synthesize_i64(
    examples: &[(i64, i64)],
    config: &MonsterTrainingConfig,
    store: Option<&crate::Store>,
    atlas: Option<&crate::atlas::Atlas>,
) -> Option<SynthesisOutcome> {
    if examples.is_empty() || config.max_nodes < 2 || config.beam_width == 0 {
        return None;
    }

    let inputs = examples.iter().map(|(input, _)| *input).collect::<Vec<_>>();
    let targets = examples.iter().map(|(_, output)| *output).collect::<Vec<_>>();
    let targets_fp = outputs_fingerprint(&targets);
    let mut candidates = seed_candidates(&inputs, &targets);
    let mut candidates_evaluated = candidates.len();
    let mut combinations_tried = candidates.len();
    let mut atlas_score_hits = 0usize;
    let mut atlas_full_pair_hits = 0usize;
    let mut atlas_opcode_hits = 0usize;
    let mut gpu_jobs_dispatched = 0usize;
    let mut gpu_jobs_skipped = 0usize;
    let mut best = candidates.iter().min_by_key(|candidate| (candidate.loss, candidate.nodes)).cloned()?;
    if best.loss == 0 {
        return finish(
            best,
            candidates_evaluated,
            combinations_tried,
            atlas_score_hits,
            atlas_full_pair_hits,
            atlas_opcode_hits,
            gpu_jobs_dispatched,
            gpu_jobs_skipped,
        );
    }

    let mut scratch: Vec<i64> = Vec::with_capacity(targets.len());

    // Cap pairs per depth to avoid combinatorial explosion on deep beams.
    // With 14k examples, 4096 pairs × 9 ops = 36k scorings is plenty.
    const MAX_PAIRS_PER_DEPTH: usize = 4096;

    let max_depth = config.max_nodes.saturating_sub(1);
    for nodes in 2..=max_depth {
        let depth_t0 = std::time::Instant::now();
        let depth_cache_key = depth_frontier_key(&candidates, targets_fp, nodes, config.beam_width);
        if ENABLE_DEPTH_FRONTIER_RAM_CACHE {
            if let Some(cached_frontier) = depth_frontier_cache()
                .lock()
                .expect("depth frontier cache poisoned")
                .get(&depth_cache_key)
                .cloned()
            {
                candidates = cached_frontier;
                if let Some(local_best) = candidates.first() {
                    best = local_best.clone();
                    if let Some(ref cb) = config.progress {
                        cb(SynthProgress {
                            depth: nodes,
                            max_depth,
                            pairs: 0,
                            gpu_used: false,
                            gpu_eligible: false,
                            gpu_attempted: false,
                            best_loss: best.loss,
                            beam_size: candidates.len(),
                            depth_ms: 0,
                            depth_ns: 0,
                            phase: "done",
                            total_scorings: 0,
                            n_examples: targets.len(),
                            gpu_backend: "CACHE",
                            atlas_full_pair_hits: 0,
                            atlas_opcode_hits: 0,
                            jobs_dispatched: 0,
                            jobs_skipped: 0,
                            best_expr: format!("{}", best.expr),
                        });
                    }
                    if best.loss == 0 {
                        return finish(
                            best,
                            candidates_evaluated,
                            combinations_tried,
                            atlas_score_hits,
                            atlas_full_pair_hits,
                            atlas_opcode_hits,
                            gpu_jobs_dispatched,
                            gpu_jobs_skipped,
                        );
                    }
                    continue;
                }
            }
        }
        if ENABLE_DEPTH_FRONTIER_PERSIST_CACHE {
            if let Some(store) = store {
                let persisted_key = depth_frontier_persist_memo_key(&depth_cache_key);
                if let Some(blob_hash) = store.lookup_memo(&persisted_key) {
                    if let Some(blob_bytes) = store.load(&blob_hash) {
                        if let Some(cached_frontier) = decode_depth_frontier_candidates(&blob_bytes, &inputs) {
                            candidates = cached_frontier;
                            if let Some(local_best) = candidates.first() {
                                best = local_best.clone();
                                {
                                    let mut cache = depth_frontier_cache()
                                        .lock()
                                        .expect("depth frontier cache poisoned");
                                    if cache.len() >= DEPTH_FRONTIER_CACHE_MAX_ENTRIES {
                                        cache.clear();
                                    }
                                    cache.insert(depth_cache_key.clone(), candidates.clone());
                                }
                                if let Some(ref cb) = config.progress {
                                    cb(SynthProgress {
                                        depth: nodes,
                                        max_depth,
                                        pairs: 0,
                                        gpu_used: false,
                                        gpu_eligible: false,
                                        gpu_attempted: false,
                                        best_loss: best.loss,
                                        beam_size: candidates.len(),
                                        depth_ms: 0,
                                        depth_ns: 0,
                                        phase: "done",
                                        total_scorings: 0,
                                        n_examples: targets.len(),
                                        gpu_backend: "CACHE-PERSIST",
                                        atlas_full_pair_hits: 0,
                                        atlas_opcode_hits: 0,
                                        jobs_dispatched: 0,
                                        jobs_skipped: 0,
                                        best_expr: format!("{}", best.expr),
                                    });
                                }
                                if best.loss == 0 {
                                    return finish(
                                        best,
                                        candidates_evaluated,
                                        combinations_tried,
                                        atlas_score_hits,
                                        atlas_full_pair_hits,
                                        atlas_opcode_hits,
                                        gpu_jobs_dispatched,
                                        gpu_jobs_skipped,
                                    );
                                }
                                continue;
                            }
                        }
                    }
                }
            }
        }
        let mut valid_pairs: Vec<(usize, usize)> = Vec::new();
        for (li, left) in candidates.iter().enumerate() {
            if left.nodes >= nodes { continue; }
            for (ri, right) in candidates.iter().enumerate() {
                if right.nodes >= nodes { continue; }
                if left.nodes + right.nodes + 1 == nodes {
                    valid_pairs.push((li, ri));
                    if valid_pairs.len() >= MAX_PAIRS_PER_DEPTH { break; }
                }
            }
            if valid_pairs.len() >= MAX_PAIRS_PER_DEPTH { break; }
        }
        if valid_pairs.is_empty() {
            continue;
        }

        // Emit START of depth — fires immediately so user sees activity.
        if let Some(ref cb) = config.progress {
            cb(SynthProgress {
                depth: nodes,
                max_depth,
                pairs: valid_pairs.len(),
                gpu_used: false,
                gpu_eligible: false,
                gpu_attempted: false,
                best_loss: best.loss,
                beam_size: candidates.len(),
                depth_ms: 0,
                depth_ns: 0,
                phase: "start",
                total_scorings: valid_pairs.len() * NUM_OPS,
                n_examples: targets.len(),
                gpu_backend: "",
                atlas_full_pair_hits: 0,
                atlas_opcode_hits: 0,
                jobs_dispatched: 0,
                jobs_skipped: 0,
                best_expr: format!("{}", best.expr),
            });
        }

        combinations_tried += valid_pairs.len() * NUM_OPS;

        // Try GPU scoring — threshold: total work (pairs × ops × examples)
        // must justify GPU transfer overhead (~50k ops minimum).
        let gpu_work = valid_pairs.len() * NUM_OPS * targets.len();
        let gpu_eligible = gpu_work >= 50_000;
        let gpu_outcome = if gpu_eligible {
            try_gpu_score_depth(
                &candidates,
                &targets,
                &valid_pairs,
                config.beam_width,
                targets_fp,
                store,
                atlas,
            )
        } else {
            None
        };
        let gpu_used_for_progress = gpu_outcome.as_ref().is_some_and(|outcome| outcome.gpu_used);
        let gpu_backend_for_progress = if gpu_used_for_progress {
            super::gpu_synth::last_gpu_backend()
        } else {
            "CPU"
        };
        let atlas_full_pair_hits_for_progress = gpu_outcome
            .as_ref()
            .map(|outcome| outcome.atlas_full_pair_hits)
            .unwrap_or(0);
        let atlas_opcode_hits_for_progress = gpu_outcome
            .as_ref()
            .map(|outcome| outcome.atlas_opcode_hits)
            .unwrap_or(0);
        let jobs_dispatched_for_progress = gpu_outcome
            .as_ref()
            .map(|outcome| outcome.jobs_dispatched)
            .unwrap_or(valid_pairs.len() * NUM_OPS);
        let jobs_skipped_for_progress = gpu_outcome
            .as_ref()
            .map(|outcome| outcome.jobs_skipped)
            .unwrap_or(0);

        let mut next = Vec::new();
        let mut seen: HashMap<u64, u128> = HashMap::with_capacity(valid_pairs.len() * 6);

        if let Some(gpu_depth) = gpu_outcome {
            atlas_score_hits += gpu_depth.atlas_score_hits;
            atlas_full_pair_hits += gpu_depth.atlas_full_pair_hits;
            atlas_opcode_hits += gpu_depth.atlas_opcode_hits;
            gpu_jobs_dispatched += gpu_depth.jobs_dispatched;
            gpu_jobs_skipped += gpu_depth.jobs_skipped;
            let pending = gpu_depth.pending;
            next = build_next_from_pending(&candidates, &pending, config.beam_width);
        } else {
            for &(li, ri) in &valid_pairs {
                let left = &candidates[li];
                let right = &candidates[ri];
                for op_idx in 0..NUM_OPS {
                    atlas_score_hits += push_binary(&mut next, &mut seen, &targets, left, right, OP_TO_EXPR[op_idx], OP_TO_FN[op_idx], targets_fp, atlas, &mut scratch) as usize;
                }
            }
        }

        candidates_evaluated += next.len();
        if next.is_empty() {
            continue;
        }
        next.sort_by_key(|candidate| (candidate.loss, candidate.nodes));
        next.truncate(config.beam_width);
        if let Some(local_best) = next.first() {
            if (local_best.loss, local_best.nodes) < (best.loss, best.nodes) {
                best = local_best.clone();
                if best.loss == 0 {
                    return finish(
                        best,
                        candidates_evaluated,
                        combinations_tried,
                        atlas_score_hits,
                        atlas_full_pair_hits,
                        atlas_opcode_hits,
                        gpu_jobs_dispatched,
                        gpu_jobs_skipped,
                    );
                }
            }
        }
        candidates.extend(next);
        candidates.sort_by_key(|candidate| (candidate.loss, candidate.nodes));
        candidates.truncate(config.beam_width);
        if ENABLE_DEPTH_FRONTIER_RAM_CACHE {
            let mut cache = depth_frontier_cache()
                .lock()
                .expect("depth frontier cache poisoned");
            if cache.len() >= DEPTH_FRONTIER_CACHE_MAX_ENTRIES {
                cache.clear();
            }
            cache.insert(depth_cache_key.clone(), candidates.clone());
        }
        if ENABLE_DEPTH_FRONTIER_PERSIST_CACHE {
            if let Some(store) = store {
                if let Ok(blob_hash) = store.store(&encode_depth_frontier_candidates(&candidates)) {
                    let _ = store.write_memo(
                        &depth_frontier_persist_memo_key(&depth_cache_key),
                        &blob_hash,
                    );
                }
            }
        }

        if let Some(ref cb) = config.progress {
            cb(SynthProgress {
                depth: nodes,
                max_depth,
                pairs: valid_pairs.len(),
                gpu_used: gpu_used_for_progress,
                gpu_eligible,
                gpu_attempted: gpu_eligible,
                best_loss: best.loss,
                beam_size: candidates.len(),
                depth_ms: depth_t0.elapsed().as_millis() as u64,
                depth_ns: depth_t0.elapsed().as_nanos(),
                phase: "done",
                total_scorings: valid_pairs.len() * NUM_OPS,
                n_examples: targets.len(),
                gpu_backend: gpu_backend_for_progress,
                atlas_full_pair_hits: atlas_full_pair_hits_for_progress,
                atlas_opcode_hits: atlas_opcode_hits_for_progress,
                jobs_dispatched: jobs_dispatched_for_progress,
                jobs_skipped: jobs_skipped_for_progress,
                best_expr: format!("{}", best.expr),
            });
        }
    }

    finish(
        best,
        candidates_evaluated,
        combinations_tried,
        atlas_score_hits,
        atlas_full_pair_hits,
        atlas_opcode_hits,
        gpu_jobs_dispatched,
        gpu_jobs_skipped,
    )
}

#[inline]
fn build_next_from_pending(
    candidates: &[Candidate],
    pending: &[PendingCandidate],
    beam_width: usize,
) -> Vec<Candidate> {
    let mut next = Vec::with_capacity(pending.len().min(beam_width));
    for pending_candidate in pending {
        if next.len() >= beam_width {
            break;
        }
        let left = &candidates[pending_candidate.left_idx];
        let right = &candidates[pending_candidate.right_idx];
        let outputs = if let Some(outputs) = pending_candidate.outputs.clone() {
            outputs
        } else {
            compute_pair_outputs(left, right, pending_candidate.op_idx)
        };
        if outputs.len() > 1 && outputs.windows(2).all(|w| w[0] == w[1]) {
            continue;
        }
        let make_expr = OP_TO_EXPR[pending_candidate.op_idx];
        let expr = make_expr(Box::new(left.expr.clone()), Box::new(right.expr.clone()));
        next.push(Candidate {
            expr,
            outputs,
            loss: pending_candidate.loss,
            nodes: pending_candidate.nodes,
        });
    }
    next
}

fn seed_candidates(inputs: &[i64], targets: &[i64]) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut constants = (-16..=16).collect::<Vec<i16>>();
    for target in targets {
        if let Ok(value) = i16::try_from(*target) {
            constants.push(value);
        }
    }
    constants.sort_unstable();
    constants.dedup();

    let input_outputs = inputs.to_vec();
    out.push(Candidate {
        loss: loss(&input_outputs, targets),
        outputs: input_outputs,
        expr: Expr::Input,
        nodes: 1,
    });
    for value in constants {
        let outputs = vec![value as i64; inputs.len()];
        out.push(Candidate {
            loss: loss(&outputs, targets),
            outputs,
            expr: Expr::Const(value),
            nodes: 1,
        });
    }
    out.sort_by_key(|candidate| (candidate.loss, candidate.nodes));
    out
}

/// V8 c — FNV-1a 64-bit fingerprint sur outputs. Bit-stable et rapide :
/// ~50 ns pour 12 i64 vs ~200 ns pour `Vec<i64>::hash` via SipHash.
#[inline(always)]
fn outputs_fingerprint(outputs: &[i64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &v in outputs {
        h ^= v as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Atlas SCORE key : 32 bytes = `[outputs_fp:8][targets_fp:8][zero:16]`.
/// Stable across sessions for any synth that hits the same (outputs,
/// targets) pair.
#[inline]
fn score_key(outputs_fp: u64, targets_fp: u64) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&outputs_fp.to_le_bytes());
    key[8..16].copy_from_slice(&targets_fp.to_le_bytes());
    key
}

/// Pack a loss u128 into the 20-byte SCORE value slot. Layout:
/// `[loss:u128 LE = 16][zero:4]`.
#[inline]
fn pack_score_loss(loss: u128) -> [u8; 20] {
    let mut out = [0u8; 20];
    out[..16].copy_from_slice(&loss.to_le_bytes());
    out
}

#[inline]
fn unpack_score_loss(packed: &[u8; 20]) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&packed[..16]);
    u128::from_le_bytes(bytes)
}

fn training_memo_key(examples: &[(i64, i64)], config: &MonsterTrainingConfig) -> String {
    let mut h = Sha256::new();
    h.update(b"train-i64-v1\0");
    h.update((examples.len() as u64).to_le_bytes());
    for (x, y) in examples {
        h.update(x.to_le_bytes());
        h.update(y.to_le_bytes());
    }
    h.update((config.max_nodes as u64).to_le_bytes());
    h.update((config.beam_width as u64).to_le_bytes());
    let digest: [u8; 32] = h.finalize().into();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

fn score_program_examples(
    node: &MonsterNode,
    program_hash: &Hash,
    examples: &[(i64, i64)],
) -> io::Result<u128> {
    let mut loss = 0u128;
    for &(x, want) in examples {
        let got = node.call_one_i64(program_hash, x)?;
        loss += ((got as i128) - (want as i128)).unsigned_abs();
    }
    Ok(loss)
}

fn depth_frontier_cache() -> &'static Mutex<HashMap<String, Vec<Candidate>>> {
    DEPTH_FRONTIER_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hot_pair_bundle_cache() -> &'static Mutex<SwissMap<PairHotKey, HotPairBundle>> {
    HOT_PAIR_BUNDLE_CACHE.get_or_init(|| Mutex::new(SwissMap::with_capacity(2048)))
}

fn hot_pair_op_cache() -> &'static Mutex<SwissMap<PairOpHotKey, PairScoreEntry>> {
    HOT_PAIR_OP_CACHE.get_or_init(|| Mutex::new(SwissMap::with_capacity(8192)))
}

#[inline]
fn pair_hot_key(left_fp: u64, right_fp: u64, targets_fp: u64, n_examples: u32) -> PairHotKey {
    PairHotKey {
        left_fp,
        right_fp,
        targets_fp,
        n_examples,
    }
}

fn hot_pair_bundle_get(key: PairHotKey) -> Option<HotPairBundle> {
    hot_pair_bundle_cache()
        .lock()
        .expect("hot pair bundle cache poisoned")
        .get(&key)
        .cloned()
}

fn hot_pair_bundle_insert(key: PairHotKey, value: HotPairBundle) {
    let mut cache = hot_pair_bundle_cache()
        .lock()
        .expect("hot pair bundle cache poisoned");
    if cache.len() >= HOT_PAIR_BUNDLE_CACHE_MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(key, value);
}

fn hot_pair_op_get(key: PairOpHotKey) -> Option<PairScoreEntry> {
    hot_pair_op_cache()
        .lock()
        .expect("hot pair op cache poisoned")
        .get(&key)
        .copied()
}

fn hot_pair_op_insert(key: PairOpHotKey, value: PairScoreEntry) {
    let mut cache = hot_pair_op_cache()
        .lock()
        .expect("hot pair op cache poisoned");
    if cache.len() >= HOT_PAIR_OP_CACHE_MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(key, value);
}

fn expr_structural_fingerprint(expr: &Expr, h: &mut Sha256) {
    match expr {
        Expr::Input => h.update([0]),
        Expr::Const(v) => {
            h.update([1]);
            h.update(v.to_le_bytes());
        }
        Expr::Add(a, b) => {
            h.update([2]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::Sub(a, b) => {
            h.update([3]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::Mul(a, b) => {
            h.update([4]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::BitXor(a, b) => {
            h.update([5]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::BitAnd(a, b) => {
            h.update([6]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::BitOr(a, b) => {
            h.update([7]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::CmpGt(a, b) => {
            h.update([8]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::CmpLt(a, b) => {
            h.update([9]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
        Expr::Select(a, b) => {
            h.update([10]);
            expr_structural_fingerprint(a, h);
            expr_structural_fingerprint(b, h);
        }
    }
}

fn depth_frontier_key(
    candidates: &[Candidate],
    targets_fp: u64,
    depth: usize,
    beam_width: usize,
) -> String {
    let mut h = Sha256::new();
    h.update(b"depth-frontier-v1\0");
    h.update(targets_fp.to_le_bytes());
    h.update((depth as u64).to_le_bytes());
    h.update((beam_width as u64).to_le_bytes());
    h.update((candidates.len() as u64).to_le_bytes());
    for candidate in candidates {
        h.update((candidate.nodes as u64).to_le_bytes());
        h.update(candidate.loss.to_le_bytes());
        h.update(outputs_fingerprint(&candidate.outputs).to_le_bytes());
        expr_structural_fingerprint(&candidate.expr, &mut h);
    }
    let digest: [u8; 32] = h.finalize().into();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

#[inline]
fn depth_frontier_persist_memo_key(depth_key: &str) -> String {
    format!("depth-frontier-persist-v1:{depth_key}")
}

fn encode_expr(expr: &Expr, out: &mut Vec<u8>) {
    match expr {
        Expr::Input => out.push(0),
        Expr::Const(v) => {
            out.push(1);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Expr::Add(a, b) => {
            out.push(2);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::Sub(a, b) => {
            out.push(3);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::Mul(a, b) => {
            out.push(4);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::BitXor(a, b) => {
            out.push(5);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::BitAnd(a, b) => {
            out.push(6);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::BitOr(a, b) => {
            out.push(7);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::CmpGt(a, b) => {
            out.push(8);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::CmpLt(a, b) => {
            out.push(9);
            encode_expr(a, out);
            encode_expr(b, out);
        }
        Expr::Select(a, b) => {
            out.push(10);
            encode_expr(a, out);
            encode_expr(b, out);
        }
    }
}

fn decode_expr(bytes: &[u8], cursor: &mut usize) -> Option<Expr> {
    let tag = *bytes.get(*cursor)?;
    *cursor += 1;
    match tag {
        0 => Some(Expr::Input),
        1 => {
            let mut imm = [0u8; 2];
            imm.copy_from_slice(bytes.get(*cursor..*cursor + 2)?);
            *cursor += 2;
            Some(Expr::Const(i16::from_le_bytes(imm)))
        }
        2 => Some(Expr::Add(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        3 => Some(Expr::Sub(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        4 => Some(Expr::Mul(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        5 => Some(Expr::BitXor(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        6 => Some(Expr::BitAnd(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        7 => Some(Expr::BitOr(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        8 => Some(Expr::CmpGt(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        9 => Some(Expr::CmpLt(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        10 => Some(Expr::Select(
            Box::new(decode_expr(bytes, cursor)?),
            Box::new(decode_expr(bytes, cursor)?),
        )),
        _ => None,
    }
}

#[inline]
fn encode_depth_frontier_candidates(candidates: &[Candidate]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(DEPTH_FRONTIER_CACHE_MAGIC);
    out.extend_from_slice(&(candidates.len() as u32).to_le_bytes());
    for candidate in candidates {
        out.extend_from_slice(&(candidate.nodes as u32).to_le_bytes());
        out.extend_from_slice(&candidate.loss.to_le_bytes());
        let mut expr_bytes = Vec::new();
        encode_expr(&candidate.expr, &mut expr_bytes);
        out.extend_from_slice(&(expr_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&expr_bytes);
    }
    out
}

#[inline]
fn decode_depth_frontier_candidates(bytes: &[u8], inputs: &[i64]) -> Option<Vec<Candidate>> {
    if bytes.len() < DEPTH_FRONTIER_CACHE_MAGIC.len() + 4
        || !bytes.starts_with(DEPTH_FRONTIER_CACHE_MAGIC)
    {
        return None;
    }
    let mut cursor = DEPTH_FRONTIER_CACHE_MAGIC.len();
    let mut count_bytes = [0u8; 4];
    count_bytes.copy_from_slice(bytes.get(cursor..cursor + 4)?);
    cursor += 4;
    let count = u32::from_le_bytes(count_bytes) as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut nodes_bytes = [0u8; 4];
        let mut loss_bytes = [0u8; 16];
        let mut expr_len_bytes = [0u8; 4];
        nodes_bytes.copy_from_slice(bytes.get(cursor..cursor + 4)?);
        cursor += 4;
        loss_bytes.copy_from_slice(bytes.get(cursor..cursor + 16)?);
        cursor += 16;
        expr_len_bytes.copy_from_slice(bytes.get(cursor..cursor + 4)?);
        cursor += 4;
        let expr_len = u32::from_le_bytes(expr_len_bytes) as usize;
        let expr_slice = bytes.get(cursor..cursor + expr_len)?;
        cursor += expr_len;
        let mut expr_cursor = 0usize;
        let expr = decode_expr(expr_slice, &mut expr_cursor)?;
        if expr_cursor != expr_slice.len() {
            return None;
        }
        let outputs = eval_expr_outputs(&expr, inputs);
        out.push(Candidate {
            expr,
            outputs,
            loss: u128::from_le_bytes(loss_bytes),
            nodes: u32::from_le_bytes(nodes_bytes) as usize,
        });
    }
    Some(out)
}

fn eval_expr_outputs(expr: &Expr, inputs: &[i64]) -> Vec<i64> {
    match expr {
        Expr::Input => inputs.to_vec(),
        Expr::Const(v) => vec![*v as i64; inputs.len()],
        Expr::Add(a, b) => eval_binary_expr_outputs_with(a, b, inputs, i64::wrapping_add),
        Expr::Sub(a, b) => eval_binary_expr_outputs_with(a, b, inputs, i64::wrapping_sub),
        Expr::Mul(a, b) => eval_binary_expr_outputs_with(a, b, inputs, i64::wrapping_mul),
        Expr::BitXor(a, b) => eval_binary_expr_outputs_with(a, b, inputs, |l, r| l ^ r),
        Expr::BitAnd(a, b) => eval_binary_expr_outputs_with(a, b, inputs, |l, r| l & r),
        Expr::BitOr(a, b) => eval_binary_expr_outputs_with(a, b, inputs, |l, r| l | r),
        Expr::CmpGt(a, b) => eval_binary_expr_outputs_with(a, b, inputs, |l, r| if l > r { 1 } else { 0 }),
        Expr::CmpLt(a, b) => eval_binary_expr_outputs_with(a, b, inputs, |l, r| if l < r { 1 } else { 0 }),
        Expr::Select(a, b) => eval_binary_expr_outputs_with(a, b, inputs, |l, r| if l != 0 { r } else { 0 }),
    }
}

fn eval_binary_expr_outputs_with(
    left: &Expr,
    right: &Expr,
    inputs: &[i64],
    op: fn(i64, i64) -> i64,
) -> Vec<i64> {
    let left_outputs = eval_expr_outputs(left, inputs);
    let right_outputs = eval_expr_outputs(right, inputs);
    left_outputs
        .into_iter()
        .zip(right_outputs)
        .map(|(l, r)| op(l, r))
        .collect()
}


const PAIR_SCORE_BUNDLE_ENTRY_BYTES: usize = 24;
const PAIR_SCORE_BUNDLE_BYTES: usize = NUM_OPS * PAIR_SCORE_BUNDLE_ENTRY_BYTES;
const PAIR_SCORE_BUNDLE_V2_MAGIC: &[u8; 8] = b"PSB2OUTS";
const PAIR_SCORE_BUNDLE_OUTPUTS_MAX_EXAMPLES: usize = 512;
type PairScoreEntry = (u128, u64);

struct DecodedPairScoreBundle {
    entries: [PairScoreEntry; NUM_OPS],
    outputs: Option<Vec<Vec<i64>>>,
}

#[inline]
fn encode_pair_score_entry(loss: u128, fingerprint: u64) -> [u8; PAIR_SCORE_BUNDLE_ENTRY_BYTES] {
    let mut out = [0u8; PAIR_SCORE_BUNDLE_ENTRY_BYTES];
    out[..16].copy_from_slice(&loss.to_le_bytes());
    out[16..24].copy_from_slice(&fingerprint.to_le_bytes());
    out
}

#[inline]
fn decode_pair_score_entry(bytes: &[u8]) -> Option<PairScoreEntry> {
    if bytes.len() != PAIR_SCORE_BUNDLE_ENTRY_BYTES {
        return None;
    }
    let mut loss_bytes = [0u8; 16];
    let mut fp_bytes = [0u8; 8];
    loss_bytes.copy_from_slice(&bytes[..16]);
    fp_bytes.copy_from_slice(&bytes[16..24]);
    Some((u128::from_le_bytes(loss_bytes), u64::from_le_bytes(fp_bytes)))
}

#[inline]
#[allow(dead_code)] // legacy V1 encoding kept for round-trip test
fn encode_pair_score_bundle(results: &[super::gpu_synth::SynthGpuResult]) -> Vec<u8> {
    let mut out = vec![0u8; PAIR_SCORE_BUNDLE_BYTES];
    for result in results {
        let base = result.op as usize * PAIR_SCORE_BUNDLE_ENTRY_BYTES;
        out[base..base + PAIR_SCORE_BUNDLE_ENTRY_BYTES]
            .copy_from_slice(&encode_pair_score_entry(result.loss, result.fingerprint));
    }
    out
}

#[inline]
fn encode_pair_score_bundle_with_outputs(
    entries: &[PairScoreEntry; NUM_OPS],
    outputs: &[Vec<i64>],
) -> Vec<u8> {
    let n_examples = outputs.first().map(|row| row.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(
        PAIR_SCORE_BUNDLE_V2_MAGIC.len()
            + 4
            + PAIR_SCORE_BUNDLE_BYTES
            + NUM_OPS * n_examples * std::mem::size_of::<i64>(),
    );
    out.extend_from_slice(PAIR_SCORE_BUNDLE_V2_MAGIC);
    out.extend_from_slice(&(n_examples as u32).to_le_bytes());
    for (op_idx, entry) in entries.iter().enumerate() {
        out.extend_from_slice(&encode_pair_score_entry(entry.0, entry.1));
        for &value in &outputs[op_idx] {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

#[inline]
fn decode_pair_score_bundle(bytes: &[u8], n_examples: usize) -> Option<DecodedPairScoreBundle> {
    if bytes.len() != PAIR_SCORE_BUNDLE_BYTES {
        if !bytes.starts_with(PAIR_SCORE_BUNDLE_V2_MAGIC) || bytes.len() < 12 {
            return None;
        }
        let mut count_bytes = [0u8; 4];
        count_bytes.copy_from_slice(&bytes[8..12]);
        let encoded_examples = u32::from_le_bytes(count_bytes) as usize;
        if encoded_examples != n_examples {
            return None;
        }
        let expected_len = 12
            + NUM_OPS * (PAIR_SCORE_BUNDLE_ENTRY_BYTES + n_examples * std::mem::size_of::<i64>());
        if bytes.len() != expected_len {
            return None;
        }
        let mut entries = [(0u128, 0u64); NUM_OPS];
        let mut outputs = Vec::with_capacity(NUM_OPS);
        let mut offset = 12usize;
        for (op_idx, slot) in entries.iter_mut().enumerate() {
            *slot = decode_pair_score_entry(&bytes[offset..offset + PAIR_SCORE_BUNDLE_ENTRY_BYTES])?;
            offset += PAIR_SCORE_BUNDLE_ENTRY_BYTES;
            let mut row = Vec::with_capacity(n_examples);
            for _ in 0..n_examples {
                let mut value_bytes = [0u8; 8];
                value_bytes.copy_from_slice(&bytes[offset..offset + 8]);
                row.push(i64::from_le_bytes(value_bytes));
                offset += 8;
            }
            debug_assert_eq!(outputs.len(), op_idx);
            outputs.push(row);
        }
        return Some(DecodedPairScoreBundle {
            entries,
            outputs: Some(outputs),
        });
    }
    let mut entries = [(0u128, 0u64); NUM_OPS];
    for (op_idx, slot) in entries.iter_mut().enumerate() {
        let base = op_idx * PAIR_SCORE_BUNDLE_ENTRY_BYTES;
        *slot = decode_pair_score_entry(&bytes[base..base + PAIR_SCORE_BUNDLE_ENTRY_BYTES])?;
    }
    Some(DecodedPairScoreBundle {
        entries,
        outputs: None,
    })
}

/// Returns `true` if the score was served from the atlas RESULT cache
/// (i.e. no evaluation against training examples was needed).
fn push_binary(
    out: &mut Vec<Candidate>,
    seen: &mut HashMap<u64, u128>,
    targets: &[i64],
    left: &Candidate,
    right: &Candidate,
    make_expr: fn(Box<Expr>, Box<Expr>) -> Expr,
    op: fn(i64, i64) -> i64,
    targets_fp: u64,
    atlas: Option<&crate::atlas::Atlas>,
    scratch: &mut Vec<i64>,
) -> bool {
    scratch.clear();
    scratch.extend(
        left.outputs
            .iter()
            .copied()
            .zip(right.outputs.iter().copied())
            .map(|(a, b)| op(a, b)),
    );

    if scratch.len() > 1 && scratch.windows(2).all(|w| w[0] == w[1]) {
        return false;
    }

    let fp = outputs_fingerprint(scratch);

    let (candidate_loss, atlas_hit) = if let Some(atlas) = atlas {
        let key = score_key(fp, targets_fp);
        if let Some(packed) = atlas.lookup_with_value(crate::atlas::kind::RESULT, &key) {
            (unpack_score_loss(&packed), true)
        } else {
            let l = loss(scratch, targets);
            let _ = atlas.record_with_value(crate::atlas::kind::RESULT, &key, &pack_score_loss(l));
            (l, false)
        }
    } else {
        (loss(scratch, targets), false)
    };

    if seen
        .get(&fp)
        .is_some_and(|known_loss| *known_loss <= candidate_loss)
    {
        return atlas_hit;
    }
    seen.insert(fp, candidate_loss);
    let expr = make_expr(Box::new(left.expr.clone()), Box::new(right.expr.clone()));
    let nodes = expr_nodes(&expr);
    out.push(Candidate {
        expr,
        outputs: scratch.clone(),
        loss: candidate_loss,
        nodes,
    });
    atlas_hit
}

fn loss(outputs: &[i64], targets: &[i64]) -> u128 {
    crate::cpu_simd::loss_i64_abs_sum(outputs, targets)
}

fn expr_nodes(expr: &Expr) -> usize {
    match expr {
        Expr::Input | Expr::Const(_) => 1,
        Expr::Add(left, right)
        | Expr::Sub(left, right)
        | Expr::Mul(left, right)
        | Expr::BitXor(left, right)
        | Expr::BitAnd(left, right)
        | Expr::BitOr(left, right)
        | Expr::CmpGt(left, right)
        | Expr::CmpLt(left, right)
        | Expr::Select(left, right) => 1 + expr_nodes(left) + expr_nodes(right),
    }
}

// ─── GPU scoring integration ─────────────────────────────────────────────────

const OP_TO_EXPR: [fn(Box<Expr>, Box<Expr>) -> Expr; 9] = [
    |l, r| Expr::Add(l, r),
    |l, r| Expr::Sub(l, r),
    |l, r| Expr::Mul(l, r),
    |l, r| Expr::BitXor(l, r),
    |l, r| Expr::BitAnd(l, r),
    |l, r| Expr::BitOr(l, r),
    |l, r| Expr::CmpGt(l, r),
    |l, r| Expr::CmpLt(l, r),
    |l, r| Expr::Select(l, r),
];

const OP_TO_FN: [fn(i64, i64) -> i64; 9] = [
    i64::wrapping_add,
    i64::wrapping_sub,
    i64::wrapping_mul,
    |a, b| a ^ b,
    |a, b| a & b,
    |a, b| a | b,
    |a, b| if a > b { 1 } else { 0 },
    |a, b| if a < b { 1 } else { 0 },
    |a, b| if a != 0 { b } else { 0 },
];

const NUM_OPS: usize = 9;

#[inline]
fn expr_nodes_count(left_nodes: usize, right_nodes: usize) -> usize {
    1 + left_nodes + right_nodes
}

#[inline]
fn compute_pair_outputs(left: &Candidate, right: &Candidate, op_idx: usize) -> Vec<i64> {
    let op_fn = OP_TO_FN[op_idx];
    left.outputs
        .iter()
        .copied()
        .zip(right.outputs.iter().copied())
        .map(|(a, b)| op_fn(a, b))
        .collect()
}

#[inline]
fn compute_pair_bundle_outputs(left: &Candidate, right: &Candidate) -> Vec<Vec<i64>> {
    (0..NUM_OPS)
        .map(|op_idx| compute_pair_outputs(left, right, op_idx))
        .collect()
}

#[inline]
fn encode_compact_pair_score_bundle(entries: &[PairScoreEntry; NUM_OPS]) -> Vec<u8> {
    let mut out = vec![0u8; PAIR_SCORE_BUNDLE_BYTES];
    for (op_idx, entry) in entries.iter().enumerate() {
        let base = op_idx * PAIR_SCORE_BUNDLE_ENTRY_BYTES;
        out[base..base + PAIR_SCORE_BUNDLE_ENTRY_BYTES]
            .copy_from_slice(&encode_pair_score_entry(entry.0, entry.1));
    }
    out
}

#[inline]
fn encode_preferred_pair_score_bundle(
    entries: &[PairScoreEntry; NUM_OPS],
    left: &Candidate,
    right: &Candidate,
    n_examples: usize,
) -> Vec<u8> {
    if n_examples <= PAIR_SCORE_BUNDLE_OUTPUTS_MAX_EXAMPLES {
        let outputs = compute_pair_bundle_outputs(left, right);
        encode_pair_score_bundle_with_outputs(entries, &outputs)
    } else {
        encode_compact_pair_score_bundle(entries)
    }
}

#[inline]
fn consider_pending_candidate(
    pending: &mut Vec<PendingCandidate>,
    seen: &mut HashMap<u64, u128>,
    pending_cap: usize,
    left_idx: usize,
    right_idx: usize,
    op_idx: usize,
    fingerprint: u64,
    loss: u128,
    nodes: usize,
    outputs: Option<Vec<i64>>,
) {
    if seen
        .get(&fingerprint)
        .is_some_and(|known_loss| *known_loss <= loss)
    {
        return;
    }
    seen.insert(fingerprint, loss);
    pending.push(PendingCandidate {
        left_idx,
        right_idx,
        op_idx,
        loss,
        nodes,
        outputs,
    });
    if pending_cap > 0 && pending.len() > pending_cap.saturating_mul(2) {
        let nth = pending_cap.min(pending.len().saturating_sub(1));
        pending.select_nth_unstable_by_key(nth, |candidate| (candidate.loss, candidate.nodes));
        pending.truncate(pending_cap);
    }
}

/// Try to score all (pair × 9 ops) on GPU. Returns None if GPU unavailable.
fn try_gpu_score_depth(
    smaller: &[Candidate],
    targets: &[i64],
    valid_pairs: &[(usize, usize)],
    beam_width: usize,
    targets_fp: u64,
    store: Option<&crate::Store>,
    atlas: Option<&crate::atlas::Atlas>,
) -> Option<GpuDepthOutcome> {
    use super::gpu_synth::{SynthGpuBatch, pack_job, score_batch_gpu};

    let m = targets.len();
    let n_candidates = smaller.len();
    let candidate_fps = smaller
        .iter()
        .map(|candidate| outputs_fingerprint(&candidate.outputs))
        .collect::<Vec<_>>();
    let mut seen: HashMap<u64, u128> = HashMap::with_capacity(valid_pairs.len() * 6);
    let mut pending = Vec::new();
    let mut atlas_score_hits = 0usize;
    let mut atlas_full_pair_hits = 0usize;
    let mut atlas_opcode_hits = 0usize;
    let mut novel_pairs: Vec<(usize, usize)> = Vec::new();
    let mut novel_pair_keys: Vec<[u8; 32]> = Vec::new();
    let mut novel_pair_entries: Vec<[Option<PairScoreEntry>; NUM_OPS]> = Vec::new();
    let mut novel_jobs: Vec<u32> = Vec::new();
    let mut queued_pair_keys: HashMap<[u8; 32], usize> = HashMap::new();

    if let (Some(store), Some(atlas)) = (store, atlas) {
        for &(li, ri) in valid_pairs {
            let left = &smaller[li];
            let right = &smaller[ri];
            let nodes = expr_nodes_count(left.nodes, right.nodes);
            let hot_key = pair_hot_key(candidate_fps[li], candidate_fps[ri], targets_fp, m as u32);
            let pair_key = crate::atlas::Atlas::pair_score_key(
                hot_key.left_fp,
                hot_key.right_fp,
                hot_key.targets_fp,
                hot_key.n_examples,
            );
            if ENABLE_HOT_PAIR_CACHES {
                if let Some(bundle) = hot_pair_bundle_get(hot_key) {
                    atlas_score_hits += NUM_OPS;
                    atlas_full_pair_hits += 1;
                    for (op_idx, (loss, fingerprint)) in bundle.entries.into_iter().enumerate() {
                        consider_pending_candidate(
                            &mut pending,
                            &mut seen,
                            beam_width,
                            li,
                            ri,
                            op_idx,
                            fingerprint,
                            loss,
                            nodes,
                            bundle
                                .outputs
                                .as_ref()
                                .and_then(|rows| rows.get(op_idx).cloned()),
                        );
                    }
                    continue;
                }
            }
            if let Some(result_hash) = atlas.lookup_result(&pair_key) {
                if let Some(bundle_bytes) = store.load(&Hash::from_bytes(result_hash)) {
                    if let Some(bundle) = decode_pair_score_bundle(&bundle_bytes, m) {
                        atlas_score_hits += NUM_OPS;
                        atlas_full_pair_hits += 1;
                        let DecodedPairScoreBundle { entries, outputs } = bundle;
                        if ENABLE_HOT_PAIR_CACHES {
                            hot_pair_bundle_insert(
                                hot_key,
                                HotPairBundle {
                                    entries,
                                    outputs: outputs.clone().map(Arc::new),
                                },
                            );
                            for (op_idx, entry) in entries.iter().copied().enumerate() {
                                hot_pair_op_insert(
                                    PairOpHotKey {
                                        pair: hot_key,
                                        op: op_idx as u8,
                                    },
                                    entry,
                                );
                            }
                        }
                        for (op_idx, (loss, fingerprint)) in entries.into_iter().enumerate() {
                            consider_pending_candidate(
                                &mut pending,
                                &mut seen,
                                beam_width,
                                li,
                                ri,
                                op_idx,
                                fingerprint,
                                loss,
                                nodes,
                                outputs.as_ref().and_then(|rows| rows.get(op_idx).cloned()),
                            );
                        }
                        continue;
                    }
                }
            }

            if queued_pair_keys.contains_key(&pair_key) {
                continue;
            }

            let mut known_entries = [None; NUM_OPS];
            let mut missing_ops = Vec::new();
            for op_idx in 0..NUM_OPS {
                let op_hot_key = PairOpHotKey {
                    pair: hot_key,
                    op: op_idx as u8,
                };
                let op_key = crate::atlas::Atlas::pair_op_score_key(
                    hot_key.left_fp,
                    hot_key.right_fp,
                    hot_key.targets_fp,
                    hot_key.n_examples,
                    op_idx as u8,
                );
                if ENABLE_HOT_PAIR_CACHES {
                    if let Some((loss, fingerprint)) = hot_pair_op_get(op_hot_key) {
                        atlas_score_hits += 1;
                        atlas_opcode_hits += 1;
                        known_entries[op_idx] = Some((loss, fingerprint));
                        consider_pending_candidate(
                            &mut pending,
                            &mut seen,
                            beam_width,
                            li,
                            ri,
                            op_idx,
                            fingerprint,
                            loss,
                            nodes,
                            None,
                        );
                        continue;
                    }
                }
                if let Some(result_hash) = atlas.lookup_result(&op_key) {
                    if let Some(entry_bytes) = store.load(&Hash::from_bytes(result_hash)) {
                        if let Some((loss, fingerprint)) = decode_pair_score_entry(&entry_bytes) {
                            atlas_score_hits += 1;
                            atlas_opcode_hits += 1;
                            if ENABLE_HOT_PAIR_CACHES {
                                hot_pair_op_insert(op_hot_key, (loss, fingerprint));
                            }
                            known_entries[op_idx] = Some((loss, fingerprint));
                            consider_pending_candidate(
                                &mut pending,
                                &mut seen,
                                beam_width,
                                li,
                                ri,
                                op_idx,
                                fingerprint,
                                loss,
                                nodes,
                                None,
                            );
                            continue;
                        }
                    }
                }
                missing_ops.push(op_idx);
            }

            if missing_ops.is_empty() {
                let entries = known_entries.map(|entry| entry.expect("fully known opcode set"));
                if ENABLE_HOT_PAIR_CACHES {
                    hot_pair_bundle_insert(
                        hot_key,
                        HotPairBundle {
                            entries,
                            outputs: if m <= PAIR_SCORE_BUNDLE_OUTPUTS_MAX_EXAMPLES {
                                Some(Arc::new(compute_pair_bundle_outputs(left, right)))
                            } else {
                                None
                            },
                        },
                    );
                }
                if let Ok(blob_hash) = store.store(&encode_preferred_pair_score_bundle(
                    &entries,
                    left,
                    right,
                    m,
                )) {
                    let _ = atlas.record_result(&pair_key, blob_hash.as_bytes());
                }
                continue;
            }

            let local_pair_idx = novel_pairs.len();
            queued_pair_keys.insert(pair_key, local_pair_idx);
            novel_pairs.push((li, ri));
            novel_pair_keys.push(pair_key);
            novel_pair_entries.push(known_entries);
            for op_idx in missing_ops {
                novel_jobs.push(pack_job(local_pair_idx as u32, op_idx as u8));
            }
        }
    } else {
        novel_pairs.extend(valid_pairs.iter().copied());
        novel_pair_entries.resize(novel_pairs.len(), [None; NUM_OPS]);
        for pair_idx in 0..novel_pairs.len() {
            for op_idx in 0..NUM_OPS {
                novel_jobs.push(pack_job(pair_idx as u32, op_idx as u8));
            }
        }
    }

    let total_jobs = valid_pairs.len() * NUM_OPS;
    let jobs_dispatched = novel_jobs.len();
    let jobs_skipped = total_jobs.saturating_sub(jobs_dispatched);
    if pending.len() > beam_width {
        pending.select_nth_unstable_by_key(beam_width, |candidate| (candidate.loss, candidate.nodes));
        pending.truncate(beam_width);
    }
    pending.sort_by_key(|candidate| (candidate.loss, candidate.nodes));

    if novel_pairs.is_empty() {
        return Some(GpuDepthOutcome {
            pending,
            atlas_score_hits,
            atlas_full_pair_hits,
            atlas_opcode_hits,
            jobs_dispatched,
            jobs_skipped,
            gpu_used: false,
        });
    }

    let mut candidate_outputs = Vec::with_capacity(n_candidates * m);
    for c in smaller {
        candidate_outputs.extend_from_slice(&c.outputs);
    }

    let pairs: Vec<(u16, u16)> = novel_pairs
        .iter()
        .map(|&(l, r)| (l as u16, r as u16))
        .collect();

    let batch = SynthGpuBatch {
        candidate_outputs,
        targets: targets.to_vec(),
        n_examples: m,
        n_candidates,
        pairs,
        jobs: novel_jobs,
    };

    match score_batch_gpu(&batch) {
        Ok(results) => {
            if results.len() != jobs_dispatched {
                return None;
            }

            if let (Some(store), Some(atlas)) = (store, atlas) {
                for result in &results {
                    let local_pair_idx = result.pair_idx as usize;
                    let (li, ri) = novel_pairs[local_pair_idx];
                    let hot_key = pair_hot_key(candidate_fps[li], candidate_fps[ri], targets_fp, m as u32);
                    let op_key = crate::atlas::Atlas::pair_op_score_key(
                        hot_key.left_fp,
                        hot_key.right_fp,
                        hot_key.targets_fp,
                        hot_key.n_examples,
                        result.op,
                    );
                    novel_pair_entries[local_pair_idx][result.op as usize] =
                        Some((result.loss, result.fingerprint));
                    hot_pair_op_insert(
                        PairOpHotKey {
                            pair: hot_key,
                            op: result.op,
                        },
                        (result.loss, result.fingerprint),
                    );
                    if let Ok(blob_hash) =
                        store.store(&encode_pair_score_entry(result.loss, result.fingerprint))
                    {
                        let _ = atlas.record_result(&op_key, blob_hash.as_bytes());
                    }
                }

                for (pair_idx, entries) in novel_pair_entries.iter().enumerate() {
                    if entries.iter().any(Option::is_none) {
                        continue;
                    }
                    let (li, ri) = novel_pairs[pair_idx];
                    let left = &smaller[li];
                    let right = &smaller[ri];
                    let pair_entries = entries.map(|entry| entry.expect("fully populated pair"));
                    hot_pair_bundle_insert(
                        pair_hot_key(candidate_fps[li], candidate_fps[ri], targets_fp, m as u32),
                        HotPairBundle {
                            entries: pair_entries,
                            outputs: if m <= PAIR_SCORE_BUNDLE_OUTPUTS_MAX_EXAMPLES {
                                Some(Arc::new(compute_pair_bundle_outputs(left, right)))
                            } else {
                                None
                            },
                        },
                    );
                    if let Ok(blob_hash) = store.store(&encode_preferred_pair_score_bundle(
                        &pair_entries,
                        left,
                        right,
                        m,
                    )) {
                        let _ = atlas.record_result(&novel_pair_keys[pair_idx], blob_hash.as_bytes());
                    }
                }
            }

            for result in results {
                let (li, ri) = novel_pairs[result.pair_idx as usize];
                let left = &smaller[li];
                let right = &smaller[ri];
                consider_pending_candidate(
                    &mut pending,
                    &mut seen,
                    beam_width,
                    li,
                    ri,
                    result.op as usize,
                    result.fingerprint,
                    result.loss,
                    expr_nodes_count(left.nodes, right.nodes),
                    None,
                );
            }
            if pending.len() > beam_width {
                pending.select_nth_unstable_by_key(beam_width, |candidate| (candidate.loss, candidate.nodes));
                pending.truncate(beam_width);
            }
            pending.sort_by_key(|candidate| (candidate.loss, candidate.nodes));

            Some(GpuDepthOutcome {
                pending,
                atlas_score_hits,
                atlas_full_pair_hits,
                atlas_opcode_hits,
                jobs_dispatched,
                jobs_skipped,
                gpu_used: true,
            })
        }
        Err(_) => None,
    }
}

// ─── End GPU scoring ─────────────────────────────────────────────────────────

fn finish(
    candidate: Candidate,
    candidates_evaluated: usize,
    combinations_tried: usize,
    atlas_score_hits: usize,
    atlas_full_pair_hits: usize,
    atlas_opcode_hits: usize,
    gpu_jobs_dispatched: usize,
    gpu_jobs_skipped: usize,
) -> Option<SynthesisOutcome> {
    let program = expr_to_program(&candidate.expr)?;
    Some(SynthesisOutcome {
        program,
        loss: candidate.loss,
        candidates_evaluated,
        combinations_tried,
        atlas_score_hits,
        atlas_full_pair_hits,
        atlas_opcode_hits,
        gpu_jobs_dispatched,
        gpu_jobs_skipped,
    })
}

fn expr_to_program(expr: &Expr) -> Option<Program> {
    let mut nodes = Vec::new();
    let output = emit_expr(expr, &mut nodes)?;
    nodes.push(Node::output(output, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).ok()
}

fn emit_expr(expr: &Expr, nodes: &mut Vec<Node>) -> Option<u16> {
    let idx = match expr {
        Expr::Input => {
            nodes.push(Node::input(0));
            nodes.len() - 1
        }
        Expr::Const(value) => {
            nodes.push(Node::const_i64(*value));
            nodes.len() - 1
        }
        Expr::Add(left, right) => emit_binary(nodes, left, right, Node::add)?,
        Expr::Sub(left, right) => emit_binary(nodes, left, right, Node::sub)?,
        Expr::Mul(left, right) => emit_binary(nodes, left, right, Node::mul)?,
        Expr::BitXor(left, right) => emit_binary(nodes, left, right, Node::bit_xor)?,
        Expr::BitAnd(left, right) => emit_binary(nodes, left, right, Node::bit_and)?,
        Expr::BitOr(left, right) => emit_binary(nodes, left, right, Node::bit_or)?,
        Expr::CmpGt(left, right) => {
            let l = emit_expr(left, nodes)?;
            let r = emit_expr(right, nodes)?;
            emit_bool_as_i64(nodes, Node::lt(r, l))?
        }
        Expr::CmpLt(left, right) => {
            let l = emit_expr(left, nodes)?;
            let r = emit_expr(right, nodes)?;
            emit_bool_as_i64(nodes, Node::lt(l, r))?
        }
        Expr::Select(cond, val) => {
            let cond_i64 = emit_expr(cond, nodes)?;
            let val_i64 = emit_expr(val, nodes)?;
            let zero = emit_const_zero(nodes);
            let is_zero = push_bool_node(nodes, Node::eq(cond_i64, zero))?;
            let is_nonzero = push_bool_node(nodes, Node::not(is_zero))?;
            nodes.push(Node::select_i64(is_nonzero, val_i64, zero));
            nodes.len() - 1
        }
    };
    u16::try_from(idx).ok()
}

fn emit_const_zero(nodes: &mut Vec<Node>) -> u16 {
    nodes.push(Node::const_i64(0));
    (nodes.len() - 1) as u16
}

fn push_bool_node(nodes: &mut Vec<Node>, node: Node) -> Option<u16> {
    nodes.push(node);
    u16::try_from(nodes.len() - 1).ok()
}

fn emit_bool_as_i64(nodes: &mut Vec<Node>, bool_node: Node) -> Option<usize> {
    let pred = push_bool_node(nodes, bool_node)?;
    let one = {
        nodes.push(Node::const_i64(1));
        u16::try_from(nodes.len() - 1).ok()?
    };
    let zero = emit_const_zero(nodes);
    nodes.push(Node::select_i64(pred, one, zero));
    Some(nodes.len() - 1)
}

fn emit_binary(
    nodes: &mut Vec<Node>,
    left: &Expr,
    right: &Expr,
    make: fn(u16, u16) -> Node,
) -> Option<usize> {
    let left = emit_expr(left, nodes)?;
    let right = emit_expr(right, nodes)?;
    nodes.push(make(left, right));
    Some(nodes.len() - 1)
}

#[cfg(test)]
mod bench_only {
    use super::*;

    /// Microbench for `synthesize_i64` exercising the per-call scratch
    /// reuse path. Tuned for ~5-30 s of wall-time : 500 examples, beam
    /// 32, max_nodes 3 → ~32² × 6 × 2 = ~12k push_binary calls, each
    /// writing/reading a 500-element scratch buffer.
    ///
    /// Marked `#[ignore]` ; invoke via :
    ///   cargo test --lib --release --features cuda,wgpu \
    ///     synth_scratch_buffer_bench -- --ignored --nocapture
    #[test]
    #[ignore]
    fn synth_scratch_buffer_bench() {
        let n = 500usize;
        let examples: Vec<(i64, i64)> = (0..n as i64)
            .map(|i| (i, 3i64.wrapping_mul(i).wrapping_add(7)))
            .collect();
        let cfg = MonsterTrainingConfig {
            max_nodes: 3,
            beam_width: 32,
            progress: None,
        };
        // Run twice : first call warms allocator caches, second call
        // measures steady-state throughput.
        let _warmup = synthesize_i64(&examples, &cfg, None, None).expect("warmup");
        let t0 = std::time::Instant::now();
        let outcome = synthesize_i64(&examples, &cfg, None, None).expect("synth ok");
        let elapsed = t0.elapsed();
        println!(
            "synth_scratch_buffer_bench: n={} beam={} max_nodes={} -> cands={} in {:.2} ms (loss={}, exact={})",
            n,
            cfg.beam_width,
            cfg.max_nodes,
            outcome.candidates_evaluated,
            elapsed.as_secs_f64() * 1000.0,
            outcome.loss,
            outcome.loss == 0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryGovernor, Store};
    

    #[test]
    fn monster_trains_affine_i64_program_from_examples() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("train-affine")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-8..=8)
            .map(|x| (x, x * 9 + 1))
            .collect::<Vec<_>>();

        let trained = monster
            .train_i64_program(&examples, MonsterTrainingConfig::default())
            .unwrap();

        assert!(trained.exact);
        assert_eq!(trained.loss, 0);
        assert!(trained.candidates_evaluated > 0);
        let values = (-128..128).collect::<Vec<i64>>();
        let out = monster.call_many_values_i64(&trained.program_hash, &values).unwrap();
        for (got, input) in out.iter().zip(values.iter()) {
            assert_eq!(*got, input * 9 + 1);
        }
    }

    #[test]
    fn monster_trains_non_affine_i64_program_from_examples() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("train-square")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-8..=8)
            .map(|x| (x, x * x + 3))
            .collect::<Vec<_>>();

        let trained = monster
            .train_i64_program(
                &examples,
                MonsterTrainingConfig {
                    max_nodes: 6,
                    beam_width: 512,
                    progress: None,
                },
            )
            .unwrap();

        assert!(trained.exact);
        let values = (-128..128).collect::<Vec<i64>>();
        let out = monster.call_many_values_i64(&trained.program_hash, &values).unwrap();
        for (got, input) in out.iter().zip(values.iter()) {
            assert_eq!(*got, input * input + 3);
        }
    }

    #[test]
    fn monster_trains_bitwise_i64_program_from_examples() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("train-bitwise")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-8..=8)
            .map(|x| (x, (x ^ 7) | 3))
            .collect::<Vec<_>>();

        let trained = monster
            .train_i64_program(
                &examples,
                MonsterTrainingConfig {
                    max_nodes: 6,
                    beam_width: 768,
                    progress: None,
                },
            )
            .unwrap();

        assert!(trained.exact);
        let values = (-128..128).collect::<Vec<i64>>();
        let out = monster.call_many_values_i64(&trained.program_hash, &values).unwrap();
        for (got, input) in out.iter().zip(values.iter()) {
            assert_eq!(*got, (*input ^ 7) | 3);
        }
    }

    #[test]
    fn monster_training_reuses_persisted_winner_memo() {
        if !ENABLE_TRAIN_WINNER_MEMO {
            return;
        }
        let monster = MonsterNode::new(
            Store::open(fresh_path("train-memo-winner")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let examples = (-8..=8)
            .map(|x| (x, x * 9 + 1))
            .collect::<Vec<_>>();
        let config = MonsterTrainingConfig {
            max_nodes: 6,
            beam_width: 256,
            progress: None,
        };

        let cold = monster.train_i64_program(&examples, config.clone()).unwrap();
        assert!(cold.candidates_evaluated > 0);

        let warm = monster.train_i64_program(&examples, config).unwrap();
        assert_eq!(warm.program_hash, cold.program_hash);
        assert_eq!(warm.loss, cold.loss);
        assert_eq!(warm.candidates_evaluated, 0);
        assert_eq!(warm.combinations_tried, 0);
    }

    #[test]
    fn synth_depth_frontier_cache_hits_before_pair_enumeration() {
        if !ENABLE_DEPTH_FRONTIER_RAM_CACHE {
            return;
        }
        let examples = (-8..=8)
            .map(|x| (x, x * 9 + 1))
            .collect::<Vec<_>>();
        let cfg = MonsterTrainingConfig {
            max_nodes: 6,
            beam_width: 256,
            progress: None,
        };

        let _cold = synthesize_i64(&examples, &cfg, None, None).expect("cold synth");

        let seen = Arc::new(Mutex::new(Vec::<SynthProgress>::new()));
        let seen_cb = Arc::clone(&seen);
        let warm_cfg = MonsterTrainingConfig {
            max_nodes: 6,
            beam_width: 256,
            progress: Some(Arc::new(move |p: SynthProgress| {
                seen_cb.lock().expect("progress mutex poisoned").push(p);
            })),
        };
        let _warm = synthesize_i64(&examples, &warm_cfg, None, None).expect("warm synth");

        let progress = seen.lock().expect("progress mutex poisoned");
        assert!(progress.iter().any(|p| p.phase == "done" && p.gpu_backend == "CACHE"));
    }

    #[test]
    fn hot_pair_bundle_cache_roundtrips_entries() {
        let key = pair_hot_key(11, 22, 33, 1440);
        let entries = std::array::from_fn(|op_idx| ((op_idx as u128) * 7 + 1, (op_idx as u64) * 13 + 5));
        hot_pair_bundle_insert(
            key,
            HotPairBundle {
                entries,
                outputs: None,
            },
        );
        let bundle = hot_pair_bundle_get(key).expect("bundle present");
        assert_eq!(bundle.entries, entries);
        assert!(bundle.outputs.is_none());
    }

    #[test]
    fn depth_frontier_candidates_roundtrip() {
        let inputs = vec![1, 2, 3];
        let candidates = vec![
            Candidate {
                expr: Expr::Input,
                outputs: inputs.clone(),
                loss: 5,
                nodes: 1,
            },
            Candidate {
                expr: Expr::CmpLt(Box::new(Expr::Input), Box::new(Expr::Const(7))),
                outputs: vec![1, 1, 1],
                loss: 9,
                nodes: 3,
            },
        ];
        let encoded = encode_depth_frontier_candidates(&candidates);
        let decoded =
            decode_depth_frontier_candidates(&encoded, &inputs).expect("decode frontier candidates");
        assert_eq!(decoded.len(), candidates.len());
        assert!(decoded[0].expr == candidates[0].expr);
        assert_eq!(decoded[0].outputs, candidates[0].outputs);
        assert_eq!(decoded[0].loss, candidates[0].loss);
        assert_eq!(decoded[0].nodes, candidates[0].nodes);
        assert!(decoded[1].expr == candidates[1].expr);
        assert_eq!(decoded[1].outputs, candidates[1].outputs);
        assert_eq!(decoded[1].loss, candidates[1].loss);
        assert_eq!(decoded[1].nodes, candidates[1].nodes);
    }

    #[test]
    fn pair_score_bundle_roundtrips() {
        let results = (0..NUM_OPS)
            .map(|op| crate::monster::gpu_synth::SynthGpuResult {
                pair_idx: 0,
                op: op as u8,
                loss: (op as u128) * 17 + 3,
                fingerprint: (op as u64) * 31 + 9,
            })
            .collect::<Vec<_>>();
        let encoded = encode_pair_score_bundle(&results);
        let decoded = decode_pair_score_bundle(&encoded, 0).expect("decode bundle");
        for (op_idx, (loss, fingerprint)) in decoded.entries.into_iter().enumerate() {
            assert_eq!(loss, results[op_idx].loss);
            assert_eq!(fingerprint, results[op_idx].fingerprint);
        }
        assert!(decoded.outputs.is_none());
    }

    #[test]
    fn pair_score_bundle_with_outputs_roundtrips() {
        let entries = std::array::from_fn(|op_idx| ((op_idx as u128) * 19 + 5, (op_idx as u64) * 37 + 11));
        let outputs = (0..NUM_OPS)
            .map(|op_idx| vec![op_idx as i64, (op_idx as i64) * 2, -((op_idx as i64) + 1)])
            .collect::<Vec<_>>();
        let encoded = encode_pair_score_bundle_with_outputs(&entries, &outputs);
        let decoded = decode_pair_score_bundle(&encoded, 3).expect("decode bundle with outputs");
        let DecodedPairScoreBundle {
            entries: decoded_entries,
            outputs: decoded_outputs,
        } = decoded;
        assert_eq!(decoded_entries, entries);
        assert_eq!(decoded_outputs.expect("outputs present"), outputs);
    }

    #[test]
    fn expr_to_program_supports_comparisons_and_select_as_i64() {
        let monster = MonsterNode::new(
            Store::open(fresh_path("train-cmp-select")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );

        let cmp_program = expr_to_program(&Expr::CmpLt(
            Box::new(Expr::Input),
            Box::new(Expr::Const(5)),
        ))
        .expect("cmp program");
        let cmp_hash = monster.store().store(cmp_program.bytes()).unwrap();
        let cmp_out = monster
            .call_many_values_i64(&cmp_hash, &[-2, 4, 5, 9])
            .unwrap();
        assert_eq!(cmp_out, vec![1, 1, 0, 0]);

        let sel_program = expr_to_program(&Expr::Select(
            Box::new(Expr::Input),
            Box::new(Expr::Const(7)),
        ))
        .expect("select program");
        let sel_hash = monster.store().store(sel_program.bytes()).unwrap();
        let sel_out = monster
            .call_many_values_i64(&sel_hash, &[0, 1, -3, 8])
            .unwrap();
        assert_eq!(sel_out, vec![0, 7, 7, 7]);
    }

    fn fresh_path(tag: &str) -> std::path::PathBuf {
        crate::fresh_tmp_path("scan-monster", tag)
    }
}
