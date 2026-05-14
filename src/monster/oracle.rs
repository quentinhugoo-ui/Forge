//! Oracle library: when an opaque program is observed enough times, the
//! NOTE post-Φ.ν: `distill.rs` has been absorbed into this file; "distill"
//! now names the trigger role, not a separate module.
//! node fits a closed-form rule (affine, bit-mixer, or quadratic) and
//! serves every later input without re-entering the interpreter.
//!
//! # Relation oracle ⊕ distill (Φ.ν.1 clarification)
//!
//! Forge a **un seul système d'apprentissage de loi fermée**, réparti en
//! un seul fichier depuis Φ.ν :
//!
//!   * **`oracle.rs`** (ce fichier) = **détecteur + librairie de règles**.
//!     `observe_execution` accumule les samples, `infer_oracle` essaie de
//!     fitter Affine/BitMixer/Poly2/Poly3/Piecewise. C'est ici que vivent
//!     les règles apprises (`OracleState`), leur sérialisation, et la
//!     shadow-execution de validation.
//!
//!   * **distill trigger** = **trigger d'apprentissage**. Quand un programme
//!     opaque n'est pas naturellement appelé avec assez de diversité
//!     d'inputs, le distill daemon fire des probes synthétiques via
//!     `call_one_i64`, ce qui force la slow lane à appeler
//!     `observe_execution` ici. **Distill ne duplique aucune logique de
//!     détection** — c'est strictement un trigger.
//!
//! Vocabulaire pédagogique conservé en Φ.ν : `oracle` = la règle apprise,
//! `distill` = forcer le détecteur à voir l'espace d'inputs.
//!
//! # Pipeline d'apprentissage
//!
//!   slow lane execute → observe_execution (ici, oracle.rs:255)
//!         ├─ accumule samples dans OracleState
//!         └─ si window plein → infer_oracle → si match → persist refs/oracle/<fp>
//!
//!   spawn_distill_daemon / self_improve → tick → probes → call_one_i64
//!                                                            └─ trigger slow lane
//!                                                                   └─ observe_execution (ici)
//!
//! # Validation
//!
//! Each detector is validated on the **full sample window** (holdout
//! within the same buffer) — a rule is only adopted if it reproduces
//! every observation bit-exactly.
//!
//! Detection order is by ascending complexity:
//!   1. Affine        f(x) = mul·x + add
//!   2. BitMixer      f(x) = (x << shl) ^ (x >> shr)
//!   3. Poly2         f(x) = a·x² + b·x + c        (rejects a == 0,
//!                                                  already covered by Affine)
//!
//! All arithmetic uses wrapping i64 semantics, matching the KASM
//! interpreter exactly.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::Hash;
use super::hotplan::{execute_hot_plan, oracle_eligible, HotProgram};
use super::MonsterNode;

/// Frequency of shadow-execution validation. Every Nth oracle hit, the
/// node executes the program through the interpreter and bit-compares
/// against the oracle output. Any divergence invalidates the oracle.
///
/// 1024 = ~0.1% overhead on a hot path, but a single false-positive
/// oracle is detected within ~thousand calls of being adopted.
const SHADOW_PERIOD: u64 = 1024;

/// Number of samples retained per program. Kept at 10 for backward
/// compatibility with the original `learned_oracle_takes_over_after_...`
/// invariant: 10 executions before the rule activates.
const ORACLE_WINDOW: usize = 10;

/// Smallest sample count each detector can act on. The window size
/// (10) provides additional holdout headroom beyond these minima.
const MIN_SAMPLES_AFFINE: usize = 4;
const MIN_SAMPLES_BIT_MIXER: usize = 4;
const MIN_SAMPLES_POLY2: usize = 6;
const MIN_SAMPLES_POLY3: usize = 8;
const MIN_SAMPLES_PIECEWISE: usize = 8;

/// Bit-mixer search range. KASM's Shl/Shr mask shifts to `& 63`, so
/// `[1, 63]` is the meaningful range. Empirically observed mixers use
/// shifts ≤ 32 (e.g. `(x << 3) ^ (x >> 2)`); we stop at 32 to keep the
/// inner loop ~1024 iterations.
const BIT_MIXER_MAX_SHIFT: u32 = 32;

/// Run `infer_oracle` only every Nth observation once the sample window
/// is full. 32 observations between inferences amortises the inference
/// cost (~15 µs) to ~470 ns/call. For affine/poly programs the rule
/// is still discovered within the first 1-2 inference rounds (32-64
/// samples) — Forge's existing tests assume rule discovery within a
/// few hundred calls, well above this threshold.
const INFER_INTERVAL: u32 = 32;

/// After this many consecutive None outcomes, stop inferring entirely
/// for this fingerprint. The samples window slides so the buffer keeps
/// being refreshed ; if no rule emerges in `INFER_GIVEUP_STREAK *
/// INFER_INTERVAL` ≈ 256 observations after window-fill, we accept
/// "this program has no closed-form rule" and skip future inference.
/// Reset whenever `samples` is cleared (shadow invalidation).
const INFER_GIVEUP_STREAK: u32 = 8;

#[derive(Default)]
pub(super) struct OracleState {
    pub(super) samples: VecDeque<OracleSample>,
    pub(super) learned: Option<LearnedOracle>,
    /// Coalesce — observations since last `infer_oracle` call. We
    /// rate-limit inference to 1-per-INFER_INTERVAL observations once
    /// the window is full. For workloads with no extractable rule
    /// (random hashing, bitmixers like SplitMix64 chains beyond our
    /// detector), running inference on every call burns 5 algos × O(n²)
    /// work for guaranteed-None outcome — measured ~15-20 µs/call on
    /// kmer slow path. Rate-limit drops it to ~150-200 ns/call (99%
    /// skipped). Same coalescing pattern as syscall batching : 5 noops
    /// → 1 actual call.
    pub(super) since_last_infer: u32,
    /// Coalesce — `infer_oracle` returned None this many consecutive
    /// times. Past `INFER_GIVEUP_STREAK`, we mark this fingerprint as
    /// "no rule extractable" and skip inference entirely. Reset on a
    /// shadow invalidation that clears the samples buffer.
    pub(super) infer_none_streak: u32,
}

#[derive(Clone, Copy)]
pub(super) struct OracleSample {
    pub(super) input: i64,
    pub(super) output: i64,
}

/// A closed-form rule extracted from observations. Each variant is a
/// pure function of `x` evaluated in O(1) wrapping i64 arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LearnedOracle {
    /// f(x) = mul·x + add
    Affine { mul: i64, add: i64 },
    /// f(x) = a·x² + b·x + c   (a != 0)
    Poly2 { a: i64, b: i64, c: i64 },
    /// f(x) = a·x³ + b·x² + c·x + d   (a != 0)
    Poly3 { a: i64, b: i64, c: i64, d: i64 },
    /// f(x) = (x << shl) ^ (x >>(u) shr)
    ///
    /// Both shifts are LOGICAL (u64 cast), matching KASM `Op::ShlI64`
    /// and `Op::ShrI64` in `kasm/interpreter.rs` which both do
    /// `(v as u64).wrapping_shl/shr(s) as i64`. Using Rust's signed
    /// `>>` here would silently diverge for negative inputs.
    BitMixer { shl: u32, shr: u32 },
    /// f(x) = if x < threshold { lo_mul·x + lo_add } else { hi_mul·x + hi_add }
    PiecewiseAffine {
        threshold: i64,
        lo_mul: i64,
        lo_add: i64,
        hi_mul: i64,
        hi_add: i64,
    },
}

/// Wire format for persisted oracles. Bumped whenever the encoding of
/// any variant changes — old payloads with a different version are
/// silently rejected at deserialise time so an upgrade can't surface
/// stale learned rules with the wrong layout.
pub(super) const ORACLE_WIRE_VERSION: u8 = 1;

const TAG_AFFINE: u8 = 0;
const TAG_POLY2: u8 = 1;
const TAG_POLY3: u8 = 2;
const TAG_BIT_MIXER: u8 = 3;
const TAG_PIECEWISE: u8 = 4;

impl LearnedOracle {
    /// Serialise to a self-describing byte blob suitable for content
    /// addressing under `refs/oracle/<fingerprint>`. Layout:
    ///   [version=1, tag, ...little-endian payload]
    pub(super) fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(48);
        out.push(ORACLE_WIRE_VERSION);
        match *self {
            LearnedOracle::Affine { mul, add } => {
                out.push(TAG_AFFINE);
                out.extend_from_slice(&mul.to_le_bytes());
                out.extend_from_slice(&add.to_le_bytes());
            }
            LearnedOracle::Poly2 { a, b, c } => {
                out.push(TAG_POLY2);
                out.extend_from_slice(&a.to_le_bytes());
                out.extend_from_slice(&b.to_le_bytes());
                out.extend_from_slice(&c.to_le_bytes());
            }
            LearnedOracle::Poly3 { a, b, c, d } => {
                out.push(TAG_POLY3);
                out.extend_from_slice(&a.to_le_bytes());
                out.extend_from_slice(&b.to_le_bytes());
                out.extend_from_slice(&c.to_le_bytes());
                out.extend_from_slice(&d.to_le_bytes());
            }
            LearnedOracle::BitMixer { shl, shr } => {
                out.push(TAG_BIT_MIXER);
                out.extend_from_slice(&shl.to_le_bytes());
                out.extend_from_slice(&shr.to_le_bytes());
            }
            LearnedOracle::PiecewiseAffine {
                threshold,
                lo_mul,
                lo_add,
                hi_mul,
                hi_add,
            } => {
                out.push(TAG_PIECEWISE);
                out.extend_from_slice(&threshold.to_le_bytes());
                out.extend_from_slice(&lo_mul.to_le_bytes());
                out.extend_from_slice(&lo_add.to_le_bytes());
                out.extend_from_slice(&hi_mul.to_le_bytes());
                out.extend_from_slice(&hi_add.to_le_bytes());
            }
        }
        out
    }

    /// Deserialise from the wire format produced by `serialize`.
    /// Returns `None` for any version mismatch, unknown tag, or
    /// truncated payload — never panics on malformed input.
    pub(super) fn deserialize(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 2 || bytes[0] != ORACLE_WIRE_VERSION {
            return None;
        }
        let tag = bytes[1];
        let payload = &bytes[2..];
        fn rd_i64(payload: &[u8], off: usize) -> Option<i64> {
            payload.get(off..off + 8)?.try_into().ok().map(i64::from_le_bytes)
        }
        fn rd_u32(payload: &[u8], off: usize) -> Option<u32> {
            payload.get(off..off + 4)?.try_into().ok().map(u32::from_le_bytes)
        }
        match tag {
            TAG_AFFINE => Some(LearnedOracle::Affine {
                mul: rd_i64(payload, 0)?,
                add: rd_i64(payload, 8)?,
            }),
            TAG_POLY2 => Some(LearnedOracle::Poly2 {
                a: rd_i64(payload, 0)?,
                b: rd_i64(payload, 8)?,
                c: rd_i64(payload, 16)?,
            }),
            TAG_POLY3 => Some(LearnedOracle::Poly3 {
                a: rd_i64(payload, 0)?,
                b: rd_i64(payload, 8)?,
                c: rd_i64(payload, 16)?,
                d: rd_i64(payload, 24)?,
            }),
            TAG_BIT_MIXER => Some(LearnedOracle::BitMixer {
                shl: rd_u32(payload, 0)?,
                shr: rd_u32(payload, 4)?,
            }),
            TAG_PIECEWISE => Some(LearnedOracle::PiecewiseAffine {
                threshold: rd_i64(payload, 0)?,
                lo_mul: rd_i64(payload, 8)?,
                lo_add: rd_i64(payload, 16)?,
                hi_mul: rd_i64(payload, 24)?,
                hi_add: rd_i64(payload, 32)?,
            }),
            _ => None,
        }
    }

    pub(super) fn apply(&self, x: i64) -> i64 {
        match *self {
            LearnedOracle::Affine { mul, add } => x.wrapping_mul(mul).wrapping_add(add),
            LearnedOracle::Poly2 { a, b, c } => a
                .wrapping_mul(x)
                .wrapping_mul(x)
                .wrapping_add(b.wrapping_mul(x))
                .wrapping_add(c),
            LearnedOracle::Poly3 { a, b, c, d } => {
                let xx = x.wrapping_mul(x);
                let xxx = xx.wrapping_mul(x);
                a.wrapping_mul(xxx)
                    .wrapping_add(b.wrapping_mul(xx))
                    .wrapping_add(c.wrapping_mul(x))
                    .wrapping_add(d)
            }
            LearnedOracle::BitMixer { shl, shr } => {
                // Logical shifts via u64 to match KASM's interpreter.
                let v = x as u64;
                (v.wrapping_shl(shl) ^ v.wrapping_shr(shr)) as i64
            }
            LearnedOracle::PiecewiseAffine {
                threshold,
                lo_mul,
                lo_add,
                hi_mul,
                hi_add,
            } => {
                if x < threshold {
                    x.wrapping_mul(lo_mul).wrapping_add(lo_add)
                } else {
                    x.wrapping_mul(hi_mul).wrapping_add(hi_add)
                }
            }
        }
    }
}

impl MonsterNode {
    pub(super) fn apply_learned_oracle(
        &self,
        hot: &HotProgram,
        arg_bytes: &[u8],
    ) -> Option<Vec<u8>> {
        if !oracle_eligible(hot) {
            return None;
        }
        let input = decode_i64_bytes(arg_bytes)?;
        let rule = self
            .oracles
            .read()
            .unwrap()
            .get(&hot.semantic_fingerprint)
            .and_then(|state| state.learned)?;
        Some(rule.apply(input).to_le_bytes().to_vec())
    }

    pub(super) fn observe_execution(
        &self,
        hot: &HotProgram,
        arg_bytes: &[u8],
        result_bytes: &[u8],
    ) {
        if !oracle_eligible(hot) {
            return;
        }
        let Some(input) = decode_i64_bytes(arg_bytes) else {
            return;
        };
        let Some(output) = decode_i64_bytes(result_bytes) else {
            return;
        };
        let fingerprint = hot.semantic_fingerprint;

        let mut oracles = self.oracles.write().unwrap();
        let state = oracles.entry(fingerprint).or_default();
        if state.samples.len() == ORACLE_WINDOW {
            state.samples.pop_front();
        }
        state.samples.push_back(OracleSample { input, output });

        // Don't run detection until the window is full — every detector
        // validates on the full buffer, so a too-small sample set risks
        // false positives.
        if state.samples.len() < ORACLE_WINDOW {
            return;
        }

        // Coalesce — skip inference entirely if we already have a rule.
        // The shadow-check path (`shadow_check_oracle` invalidator) is
        // the only thing that should clear `learned`. If a fancier rule
        // could supersede the current one, we'd need to re-infer ; in
        // practice all 5 detectors are tried in order, so the first
        // match was already the best fit. Skipping when learned is
        // pure win.
        if state.learned.is_some() {
            return;
        }

        // Coalesce — rate-limit inference AFTER the first None, to
        // amortise the ~15 µs cost over many calls when no closed-form
        // rule is extractable (random hashing, kmer mixers, crypto).
        //
        // First inference (streak == 0) runs immediately on
        // window-fill : affine/poly programs are detected at the
        // earliest possible moment, preserving the test contract that
        // a learned oracle is available within ~10-15 observations.
        //
        // Once None has been returned at least once, we know there's
        // probably no rule ; further inferences cost the same ~15 µs
        // for the same None outcome. Rate-limit kicks in.
        if state.infer_none_streak > 0 {
            state.since_last_infer = state.since_last_infer.saturating_add(1);
            if state.since_last_infer < INFER_INTERVAL {
                return;
            }
            state.since_last_infer = 0;
        }

        // Coalesce — past `INFER_GIVEUP_STREAK` consecutive Nones, stop
        // inferring. The fingerprint is durably "no rule extractable"
        // until the sample buffer is reset (shadow invalidation).
        if state.infer_none_streak >= INFER_GIVEUP_STREAK {
            return;
        }

        if let Some(rule) = infer_oracle(&state.samples) {
            // Already learned — no need to re-publish (defensive ;
            // the early-return above means we shouldn't reach here
            // with `learned.is_some()`, but keep the check in case
            // future code relaxes the gating).
            let already = state.learned == Some(rule);
            state.learned = Some(rule);
            state.infer_none_streak = 0;
            // Drop the write lock before doing I/O so other call paths
            // can keep reading the oracle map while we persist.
            drop(oracles);
            if !already {
                self.persist_oracle(&fingerprint, &rule);
            }
        } else {
            state.infer_none_streak = state.infer_none_streak.saturating_add(1);
        }
    }

    /// Best-effort persistence of a freshly-learned oracle to
    /// `refs/oracle/<fingerprint_hex>`. Failures are swallowed:
    /// the in-RAM oracle is still installed and serves correct
    /// answers; persistence is a sharing optimisation, not a
    /// correctness invariant.
    fn persist_oracle(&self, fingerprint: &[u8; 32], rule: &LearnedOracle) {
        let blob = rule.serialize();
        let hash = match self.store().store(&blob) {
            Ok(h) => h,
            Err(_) => return,
        };
        let key_hex = oracle_ref_key(fingerprint);
        let _ = self
            .store()
            .write_ref(&format!("refs/oracle/{key_hex}"), &hash, "scan oracle");
    }

    /// Hot-load a persisted oracle into the in-RAM map. Called from
    /// `hot_program` right after the semantic fingerprint is known —
    /// any oracle previously published by THIS node, or shared via
    /// `git fetch refs/oracle/*` from a peer, is materialised before
    /// the first call ever reaches the slow lane.
    pub(super) fn load_persisted_oracle(&self, fingerprint: &[u8; 32]) {
        // Skip if we already have something in RAM (typical re-entry
        // through hot_program after eviction).
        {
            let map = self.oracles.read().unwrap();
            if map.get(fingerprint).map_or(false, |s| s.learned.is_some()) {
                return;
            }
        }
        let key_hex = oracle_ref_key(fingerprint);
        let hash = match self.store().lookup_ref(&format!("refs/oracle/{key_hex}")) {
            Some(h) => h,
            None => return,
        };
        let bytes = match self.store().load(&hash) {
            Some(b) => b,
            None => return,
        };
        let rule = match LearnedOracle::deserialize(&bytes) {
            Some(r) => r,
            None => return,
        };
        let mut map = self.oracles.write().unwrap();
        let state = map.entry(*fingerprint).or_default();
        state.learned = Some(rule);
    }

    pub(super) fn bump_oracle_hit(&self) {
        self.stats_atomic.oracle_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Shadow-execution sampler. Called from the fast lane after
    /// `apply_learned_oracle` succeeds. Every `SHADOW_PERIOD`-th call,
    /// runs the interpreter on the same arg and compares bit-exactly
    /// against the oracle's output. On divergence:
    ///   1. clear `state.learned` so subsequent calls fall back to
    ///      the slow lane and re-observe the program;
    ///   2. delete `refs/oracle/<fingerprint>` so peers that pull our
    ///      memos don't keep importing the bad rule;
    ///   3. bump `shadow_invalidations` for observability.
    ///
    /// Errors during the interpreter run are silently ignored — they
    /// can't reliably distinguish between a genuine oracle disagreement
    /// and an unrelated I/O failure, and the cost of mis-invalidating
    /// a healthy oracle (it just gets re-learned) is small.
    pub(super) fn shadow_check_oracle(
        &self,
        hot: &HotProgram,
        arg_bytes: &[u8],
        oracle_output: &[u8],
    ) {
        let n = self
            .stats_atomic
            .shadow_counter
            .fetch_add(1, Ordering::Relaxed);
        if n % SHADOW_PERIOD != 0 {
            return;
        }
        let interp = match execute_hot_plan(hot, arg_bytes) {
            Ok(b) => b,
            Err(_) => return,
        };
        if interp == oracle_output {
            return;
        }
        // Divergence — the oracle is lying. Invalidate.
        self.invalidate_oracle(&hot.semantic_fingerprint);
    }

    fn invalidate_oracle(&self, fingerprint: &[u8; 32]) {
        {
            let mut map = self.oracles.write().unwrap();
            if let Some(state) = map.get_mut(fingerprint) {
                state.learned = None;
                // Drop accumulated samples too — they're what produced
                // the bad rule. The slow lane's `observe_execution` will
                // collect a fresh window with whatever args are now
                // arriving (which should be more diverse than what
                // misled the original detector).
                state.samples.clear();
                // Reset coalesce counters so the rebuilt window gets a
                // fresh chance to infer (otherwise the post-shadow
                // streak might suppress legit re-inference).
                state.since_last_infer = 0;
                state.infer_none_streak = 0;
            }
        }
        let key_hex = oracle_ref_key(fingerprint);
        let _ = self
            .store()
            .delete_ref(&format!("refs/oracle/{key_hex}"));
        self.stats_atomic
            .shadow_invalidations
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Try each detector in ascending order of cost; the first rule that
/// validates against ALL accumulated samples wins.
pub(super) fn infer_oracle(samples: &VecDeque<OracleSample>) -> Option<LearnedOracle> {
    let points: Vec<OracleSample> = samples.iter().copied().collect();

    if points.len() >= MIN_SAMPLES_AFFINE {
        if let Some(rule) = infer_affine(&points) {
            return Some(rule);
        }
    }
    if points.len() >= MIN_SAMPLES_BIT_MIXER {
        if let Some(rule) = infer_bit_mixer(&points) {
            return Some(rule);
        }
    }
    if points.len() >= MIN_SAMPLES_POLY2 {
        if let Some(rule) = infer_poly2(&points) {
            return Some(rule);
        }
    }
    if points.len() >= MIN_SAMPLES_POLY3 {
        if let Some(rule) = infer_poly3(&points) {
            return Some(rule);
        }
    }
    if points.len() >= MIN_SAMPLES_PIECEWISE {
        if let Some(rule) = infer_piecewise_affine(&points) {
            return Some(rule);
        }
    }
    None
}

fn infer_affine(points: &[OracleSample]) -> Option<LearnedOracle> {
    for (i, left) in points.iter().enumerate() {
        for right in points.iter().skip(i + 1) {
            let dx = right.input.wrapping_sub(left.input);
            if dx == 0 {
                continue;
            }
            let dy = right.output.wrapping_sub(left.output);
            if dy % dx != 0 {
                continue;
            }
            let mul = dy / dx;
            let add = left.output.wrapping_sub(left.input.wrapping_mul(mul));
            let candidate = LearnedOracle::Affine { mul, add };
            if validates(&candidate, points) {
                return Some(candidate);
            }
        }
    }
    None
}

fn infer_bit_mixer(points: &[OracleSample]) -> Option<LearnedOracle> {
    for shl in 1..=BIT_MIXER_MAX_SHIFT {
        for shr in 1..=BIT_MIXER_MAX_SHIFT {
            let candidate = LearnedOracle::BitMixer { shl, shr };
            if validates(&candidate, points) {
                return Some(candidate);
            }
        }
    }
    None
}

fn infer_poly2(points: &[OracleSample]) -> Option<LearnedOracle> {
    // Need 3 distinct x values to solve for (a, b, c).
    let mut distinct: Vec<OracleSample> = Vec::with_capacity(3);
    for p in points {
        if distinct.iter().all(|q| q.input != p.input) {
            distinct.push(*p);
            if distinct.len() == 3 {
                break;
            }
        }
    }
    if distinct.len() < 3 {
        return None;
    }
    let (s0, s1, s2) = (distinct[0], distinct[1], distinct[2]);

    let dx01 = s1.input.wrapping_sub(s0.input);
    let dx02 = s2.input.wrapping_sub(s0.input);
    let dx12 = s1.input.wrapping_sub(s2.input);
    if dx01 == 0 || dx02 == 0 || dx12 == 0 {
        return None;
    }
    let dy01 = s1.output.wrapping_sub(s0.output);
    let dy02 = s2.output.wrapping_sub(s0.output);

    // Slopes must be exact integers for an integer poly2 to exist.
    if dy01 % dx01 != 0 || dy02 % dx02 != 0 {
        return None;
    }
    let slope01 = dy01 / dx01;
    let slope02 = dy02 / dx02;
    let slope_diff = slope01.wrapping_sub(slope02);
    if slope_diff % dx12 != 0 {
        return None;
    }
    let a = slope_diff / dx12;
    // Reject affine fits — Affine detector handles a == 0 and is
    // strictly cheaper to apply (3 ops vs 5).
    if a == 0 {
        return None;
    }
    let b = slope01.wrapping_sub(a.wrapping_mul(s1.input.wrapping_add(s0.input)));
    let c = s0
        .output
        .wrapping_sub(a.wrapping_mul(s0.input).wrapping_mul(s0.input))
        .wrapping_sub(b.wrapping_mul(s0.input));

    let candidate = LearnedOracle::Poly2 { a, b, c };
    if validates(&candidate, points) {
        Some(candidate)
    } else {
        None
    }
}

fn validates(rule: &LearnedOracle, points: &[OracleSample]) -> bool {
    points.iter().all(|s| rule.apply(s.input) == s.output)
}

/// Count distinct input values in a sample slice — helper for the
/// PiecewiseAffine diversity guard.
fn distinct_inputs(samples: &[OracleSample]) -> usize {
    let mut seen: Vec<i64> = Vec::with_capacity(samples.len());
    for s in samples {
        if !seen.contains(&s.input) {
            seen.push(s.input);
        }
    }
    seen.len()
}

/// Fit an affine rule to a slice of samples (helper for piecewise).
/// Returns `(mul, add)` such that `mul·x + add == output` for every
/// sample, or `None` if no such affine exists.
fn fit_affine_slice(points: &[OracleSample]) -> Option<(i64, i64)> {
    if points.len() < 2 {
        return None;
    }
    for (i, left) in points.iter().enumerate() {
        for right in points.iter().skip(i + 1) {
            let dx = right.input.wrapping_sub(left.input);
            if dx == 0 {
                continue;
            }
            let dy = right.output.wrapping_sub(left.output);
            if dy % dx != 0 {
                continue;
            }
            let mul = dy / dx;
            let add = left.output.wrapping_sub(left.input.wrapping_mul(mul));
            if points
                .iter()
                .all(|s| s.input.wrapping_mul(mul).wrapping_add(add) == s.output)
            {
                return Some((mul, add));
            }
        }
    }
    None
}

/// Detect `f(x) = a·x³ + b·x² + c·x + d` (with a != 0) via Newton
/// divided differences over 4 distinct sample points. Every division
/// must be exact in i64 — otherwise no integer cubic exists for these
/// observations and the detector bails. Holdout-validates against the
/// full sample buffer.
fn infer_poly3(points: &[OracleSample]) -> Option<LearnedOracle> {
    // Need 4 distinct x values
    let mut distinct: Vec<OracleSample> = Vec::with_capacity(4);
    for p in points {
        if distinct.iter().all(|q: &OracleSample| q.input != p.input) {
            distinct.push(*p);
            if distinct.len() == 4 {
                break;
            }
        }
    }
    if distinct.len() < 4 {
        return None;
    }
    let x = [distinct[0].input, distinct[1].input, distinct[2].input, distinct[3].input];
    let y = [distinct[0].output, distinct[1].output, distinct[2].output, distinct[3].output];

    // First-order divided differences: f[xi, xi+1]
    let dx10 = x[1].wrapping_sub(x[0]);
    let dx21 = x[2].wrapping_sub(x[1]);
    let dx32 = x[3].wrapping_sub(x[2]);
    if dx10 == 0 || dx21 == 0 || dx32 == 0 {
        return None;
    }
    let dy10 = y[1].wrapping_sub(y[0]);
    let dy21 = y[2].wrapping_sub(y[1]);
    let dy32 = y[3].wrapping_sub(y[2]);
    if dy10 % dx10 != 0 || dy21 % dx21 != 0 || dy32 % dx32 != 0 {
        return None;
    }
    let f01 = dy10 / dx10;
    let f12 = dy21 / dx21;
    let f23 = dy32 / dx32;

    // Second-order
    let dx20 = x[2].wrapping_sub(x[0]);
    let dx31 = x[3].wrapping_sub(x[1]);
    if dx20 == 0 || dx31 == 0 {
        return None;
    }
    let df012 = f12.wrapping_sub(f01);
    let df123 = f23.wrapping_sub(f12);
    if df012 % dx20 != 0 || df123 % dx31 != 0 {
        return None;
    }
    let f012 = df012 / dx20;
    let f123 = df123 / dx31;

    // Third-order
    let dx30 = x[3].wrapping_sub(x[0]);
    if dx30 == 0 {
        return None;
    }
    let df0123 = f123.wrapping_sub(f012);
    if df0123 % dx30 != 0 {
        return None;
    }
    let a = df0123 / dx30;
    if a == 0 {
        // Degenerate to a quadratic — Poly2 detector is cheaper.
        return None;
    }

    // Newton form expanded to standard polynomial:
    //   f(x) = a·(x-x0)(x-x1)(x-x2) + f012·(x-x0)(x-x1) + f01·(x-x0) + y0
    // Letting s1 = x0+x1+x2, s2 = x0x1+x0x2+x1x2, s3 = x0x1x2:
    //   x³ coeff = a
    //   x² coeff = -a·s1 + f012
    //   x  coeff = a·s2 - f012·(x0+x1) + f01
    //   const   = -a·s3 + f012·x0·x1 - f01·x0 + y0
    let s1 = x[0].wrapping_add(x[1]).wrapping_add(x[2]);
    let s2 = x[0]
        .wrapping_mul(x[1])
        .wrapping_add(x[0].wrapping_mul(x[2]))
        .wrapping_add(x[1].wrapping_mul(x[2]));
    let s3 = x[0].wrapping_mul(x[1]).wrapping_mul(x[2]);
    let s_x01 = x[0].wrapping_add(x[1]);
    let p_x01 = x[0].wrapping_mul(x[1]);

    let b = a.wrapping_neg().wrapping_mul(s1).wrapping_add(f012);
    let c = a
        .wrapping_mul(s2)
        .wrapping_sub(f012.wrapping_mul(s_x01))
        .wrapping_add(f01);
    let d = a
        .wrapping_neg()
        .wrapping_mul(s3)
        .wrapping_add(f012.wrapping_mul(p_x01))
        .wrapping_sub(f01.wrapping_mul(x[0]))
        .wrapping_add(y[0]);

    let candidate = LearnedOracle::Poly3 { a, b, c, d };
    if validates(&candidate, points) {
        Some(candidate)
    } else {
        None
    }
}

/// Detect 2-segment piecewise affine: `f(x) = if x < t { m1·x + b1 }
/// else { m2·x + b2 }`. Walks every viable split point of the sorted
/// samples; both halves must fit affine independently AND disagree
/// (otherwise it's a single Affine, already handled). Holdout-validates
/// on the full buffer.
fn infer_piecewise_affine(points: &[OracleSample]) -> Option<LearnedOracle> {
    let mut sorted: Vec<OracleSample> = points.to_vec();
    sorted.sort_by_key(|s| s.input);
    // Need at least 2 distinct samples per side.
    if sorted.len() < 4 {
        return None;
    }

    for split in 2..(sorted.len() - 1) {
        let lo = &sorted[..split];
        let hi = &sorted[split..];
        let threshold = sorted[split].input;

        // Diversity guard: each side must have at least 2 distinct
        // x values. Without this, a piecewise rule learned from
        // single-sided observations confidently applies the wrong
        // segment when an out-of-domain arg arrives later.
        let lo_distinct = distinct_inputs(lo);
        let hi_distinct = distinct_inputs(hi);
        if lo_distinct < 2 || hi_distinct < 2 {
            continue;
        }

        let lo_rule = match fit_affine_slice(lo) {
            Some(r) => r,
            None => continue,
        };
        let hi_rule = match fit_affine_slice(hi) {
            Some(r) => r,
            None => continue,
        };
        // Reject the trivial single-affine case — Affine detector
        // already covers it more cheaply.
        if lo_rule == hi_rule {
            continue;
        }
        let candidate = LearnedOracle::PiecewiseAffine {
            threshold,
            lo_mul: lo_rule.0,
            lo_add: lo_rule.1,
            hi_mul: hi_rule.0,
            hi_add: hi_rule.1,
        };
        if validates(&candidate, points) {
            return Some(candidate);
        }
    }
    None
}

/// Hex-encode the 32-byte semantic fingerprint into a `refs/oracle/*`
/// key. We use the raw fingerprint (already SHA-256 entropy) rather
/// than re-hashing — short enough to fit a git ref name, unique by
/// construction, and shareable via `git push refs/oracle/*`.
pub(super) fn oracle_ref_key(fingerprint: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in fingerprint {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub(super) fn decode_i64_bytes(bytes: &[u8]) -> Option<i64> {
    let chunk: [u8; 8] = bytes.try_into().ok()?;
    Some(i64::from_le_bytes(chunk))
}

pub(super) fn decode_i64_value(bytes: Vec<u8>) -> io::Result<i64> {
    bytes
        .try_into()
        .map(i64::from_le_bytes)
        .map_err(|_| io::Error::other("monster value was not an i64"))
}

/// Probe set fired at every distill candidate. The values cover small
/// magnitudes, signs, mid-range values and edges so oracle detectors get
/// enough variance to reject false positives.
pub const DEFAULT_PROBES: &[i64] = &[
    0,
    1,
    -1,
    2,
    -2,
    7,
    -7,
    100,
    -100,
    12_345,
    -12_345,
    1_000_000,
    -1_000_000,
];

#[derive(Clone, Debug)]
pub struct DistillConfig {
    pub period: Duration,
    pub max_programs_per_round: usize,
    pub probes: Vec<i64>,
}

impl Default for DistillConfig {
    fn default() -> Self {
        Self {
            period: Duration::from_millis(200),
            max_programs_per_round: 64,
            probes: DEFAULT_PROBES.to_vec(),
        }
    }
}

/// Legacy handle for opt-in background oracle distillation. The preferred
/// autonomous path is now `MonsterNode::self_improve`, which calls
/// `distill_tick` synchronously inside the node's cognitive loop.
pub struct DistillDaemon {
    shutdown: Arc<AtomicBool>,
}

impl DistillDaemon {
    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

impl Drop for DistillDaemon {
    fn drop(&mut self) {
        self.stop();
    }
}

impl MonsterNode {
    pub fn spawn_distill_daemon(self: Arc<Self>, config: DistillConfig) -> DistillDaemon {
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&shutdown);
        let monster = self;
        thread::Builder::new()
            .name("scan-distill".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    distill_one_round(&monster, &config, &stop);
                    thread::sleep(config.period);
                }
            })
            .expect("spawn distill thread");
        DistillDaemon { shutdown }
    }

    pub fn distill_tick(&self, config: &DistillConfig) -> usize {
        let stop = AtomicBool::new(false);
        distill_one_round(self, config, &stop)
    }
}

fn distill_one_round(node: &MonsterNode, config: &DistillConfig, stop: &AtomicBool) -> usize {
    let candidates = collect_distill_candidates(node, config.max_programs_per_round);
    let mut newly_distilled = 0usize;
    for func in candidates {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if try_distill(node, &func, &config.probes) {
            newly_distilled += 1;
        }
    }
    newly_distilled
}

fn collect_distill_candidates(node: &MonsterNode, max: usize) -> Vec<Hash> {
    let progs = node.programs.read().unwrap();
    let oracles = node.oracles.read().unwrap();
    let mut out = Vec::with_capacity(progs.len().min(max));
    for (hash, hot) in progs.iter() {
        if !oracle_eligible(hot) {
            continue;
        }
        let already = oracles
            .get(&hot.semantic_fingerprint)
            .map_or(false, |s| s.learned.is_some());
        if already {
            continue;
        }
        out.push(*hash);
        if out.len() >= max {
            break;
        }
    }
    out
}

fn try_distill(node: &MonsterNode, func: &Hash, probes: &[i64]) -> bool {
    let hot = match node.lookup_program(func) {
        Some(h) => h,
        None => return false,
    };
    let fp = hot.semantic_fingerprint;
    let was_learned = node
        .oracles
        .read()
        .unwrap()
        .get(&fp)
        .map_or(false, |s| s.learned.is_some());

    for &probe in probes {
        let _ = node.call_one_i64(func, probe);
        node.stats_atomic
            .distillations_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    let now_learned = node
        .oracles
        .read()
        .unwrap()
        .get(&fp)
        .map_or(false, |s| s.learned.is_some());

    if !was_learned && now_learned {
        node.stats_atomic
            .distillations_succeeded
            .fetch_add(1, Ordering::Relaxed);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples_of<F: Fn(i64) -> i64>(inputs: &[i64], f: F) -> VecDeque<OracleSample> {
        inputs
            .iter()
            .map(|&x| OracleSample { input: x, output: f(x) })
            .collect()
    }

    #[test]
    fn affine_detector_recovers_mul_x_plus_add() {
        let samples = samples_of(&[-3, -1, 0, 2, 4, 7, 9, 11, 13, 15], |x| 9 * x + 1);
        match infer_oracle(&samples) {
            Some(LearnedOracle::Affine { mul: 9, add: 1 }) => {}
            other => panic!("expected Affine{{9,1}}, got {other:?}"),
        }
    }

    #[test]
    fn bit_mixer_detector_recovers_shl3_xor_shr2() {
        let f = |x: i64| {
            let v = x as u64;
            (v.wrapping_shl(3) ^ v.wrapping_shr(2)) as i64
        };
        let samples = samples_of(&[1, 2, 3, 5, 7, 11, 13, 17, 19, 23], f);
        match infer_oracle(&samples) {
            Some(LearnedOracle::BitMixer { shl: 3, shr: 2 }) => {}
            other => panic!("expected BitMixer{{3,2}}, got {other:?}"),
        }
    }

    #[test]
    fn poly2_detector_recovers_3xx_plus_2x_plus_1() {
        let f = |x: i64| 3i64.wrapping_mul(x).wrapping_mul(x).wrapping_add(2 * x).wrapping_add(1);
        let samples = samples_of(&[-3, -2, -1, 0, 1, 2, 3, 4, 5, 6], f);
        match infer_oracle(&samples) {
            Some(LearnedOracle::Poly2 { a: 3, b: 2, c: 1 }) => {}
            other => panic!("expected Poly2{{3,2,1}}, got {other:?}"),
        }
    }

    #[test]
    fn detector_holdout_rejects_partial_fit() {
        // 5 affine points + 5 random points — no rule should match.
        let mut samples: VecDeque<OracleSample> = (0..5)
            .map(|x| OracleSample { input: x, output: 9 * x + 1 })
            .collect();
        // Append 5 deliberately non-affine, non-poly2, non-mixer points.
        for (x, y) in [(100, 7), (101, 13), (102, 1234), (103, -5), (104, 999)] {
            samples.push_back(OracleSample { input: x, output: y });
        }
        assert!(infer_oracle(&samples).is_none(), "no rule should fit");
    }

    #[test]
    fn detector_prefers_affine_over_poly2_for_linear_data() {
        // Pure affine: Poly2 detector rejects a==0, Affine wins.
        let f = |x: i64| 5 * x - 2;
        let samples = samples_of(&[-2, -1, 0, 1, 2, 3, 4, 5, 6, 7], f);
        match infer_oracle(&samples) {
            Some(LearnedOracle::Affine { mul: 5, add }) if add == -2 => {}
            other => panic!("expected Affine{{5,-2}}, got {other:?}"),
        }
    }

    #[test]
    fn poly3_detector_recovers_x_cubed_plus_2x() {
        let f = |x: i64| {
            let xx = x.wrapping_mul(x);
            let xxx = xx.wrapping_mul(x);
            xxx.wrapping_add(2i64.wrapping_mul(x))
        };
        let samples = samples_of(&[-3, -2, -1, 0, 1, 2, 3, 4, 5, 6], f);
        match infer_oracle(&samples) {
            Some(LearnedOracle::Poly3 { a: 1, b: 0, c: 2, d: 0 }) => {}
            other => panic!("expected Poly3{{1,0,2,0}}, got {other:?}"),
        }
    }

    #[test]
    fn piecewise_affine_detector_recovers_split_at_zero() {
        // f(x) = if x < 0 then 2x - 1 else 3x + 5
        let f = |x: i64| if x < 0 { 2 * x - 1 } else { 3 * x + 5 };
        let samples = samples_of(&[-5, -4, -3, -2, -1, 0, 1, 2, 3, 4], f);
        match infer_oracle(&samples) {
            Some(LearnedOracle::PiecewiseAffine {
                threshold: 0,
                lo_mul: 2,
                lo_add,
                hi_mul: 3,
                hi_add: 5,
            }) if lo_add == -1 => {}
            other => panic!("expected PiecewiseAffine split@0, got {other:?}"),
        }
    }

    #[test]
    fn poly3_apply_matches_reference() {
        let r = LearnedOracle::Poly3 { a: 2, b: -1, c: 3, d: 7 };
        for x in -8i64..8 {
            let xx = x.wrapping_mul(x);
            let xxx = xx.wrapping_mul(x);
            let expected = 2i64
                .wrapping_mul(xxx)
                .wrapping_add((-1i64).wrapping_mul(xx))
                .wrapping_add(3i64.wrapping_mul(x))
                .wrapping_add(7);
            assert_eq!(r.apply(x), expected);
        }
    }

    #[test]
    fn piecewise_apply_matches_reference() {
        let r = LearnedOracle::PiecewiseAffine {
            threshold: 10,
            lo_mul: 2,
            lo_add: 0,
            hi_mul: -1,
            hi_add: 100,
        };
        assert_eq!(r.apply(5), 10);
        assert_eq!(r.apply(9), 18);
        assert_eq!(r.apply(10), 90);
        assert_eq!(r.apply(20), 80);
    }

    #[test]
    fn piecewise_diversity_guard_rejects_single_sided_samples() {
        // 10 samples ALL on one side — even though they fit a 2-segment
        // template trivially, the diversity guard must reject.
        let f = |x: i64| if x < 0 { 2 * x - 1 } else { 3 * x + 5 };
        let positive_only = samples_of(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9], f);
        match infer_oracle(&positive_only) {
            Some(LearnedOracle::Affine { mul: 3, add: 5 }) => {} // OK: degrade to Affine
            None => {} // Also acceptable
            other => panic!(
                "expected Affine fallback or None on single-sided data, got {other:?}"
            ),
        }
    }

    #[test]
    fn oracle_serialization_roundtrip_all_variants() {
        let cases = [
            LearnedOracle::Affine { mul: 9, add: 1 },
            LearnedOracle::Affine { mul: i64::MIN, add: i64::MAX },
            LearnedOracle::Poly2 { a: 3, b: 2, c: 1 },
            LearnedOracle::Poly3 { a: -2, b: 7, c: -11, d: 13 },
            LearnedOracle::BitMixer { shl: 3, shr: 2 },
            LearnedOracle::BitMixer { shl: 31, shr: 17 },
            LearnedOracle::PiecewiseAffine {
                threshold: -42,
                lo_mul: 2,
                lo_add: -1,
                hi_mul: -3,
                hi_add: 5,
            },
        ];
        for original in cases {
            let bytes = original.serialize();
            let decoded =
                LearnedOracle::deserialize(&bytes).expect("valid blob must decode");
            assert_eq!(decoded, original, "roundtrip failed for {original:?}");
        }
    }

    #[test]
    fn oracle_deserialize_rejects_garbage() {
        // Empty
        assert!(LearnedOracle::deserialize(&[]).is_none());
        // Wrong version
        assert!(LearnedOracle::deserialize(&[0x99, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]).is_none());
        // Unknown tag
        assert!(LearnedOracle::deserialize(&[ORACLE_WIRE_VERSION, 0xff]).is_none());
        // Truncated payload (Affine claims 16 bytes after tag, only 8 here)
        let truncated = [&[ORACLE_WIRE_VERSION, TAG_AFFINE][..], &[0u8; 8]].concat();
        assert!(LearnedOracle::deserialize(&truncated).is_none());
    }

    #[test]
    fn affine_and_bit_mixer_apply_match_reference() {
        // Sanity: apply() reproduces the reference function.
        let r = LearnedOracle::Affine { mul: 7, add: 13 };
        for x in -10..10 {
            assert_eq!(r.apply(x), 7i64.wrapping_mul(x).wrapping_add(13));
        }
        let r = LearnedOracle::BitMixer { shl: 5, shr: 7 };
        for x in -10i64..10 {
            let v = x as u64;
            let expected = (v.wrapping_shl(5) ^ v.wrapping_shr(7)) as i64;
            assert_eq!(r.apply(x), expected);
        }
        let r = LearnedOracle::Poly2 { a: 2, b: -3, c: 1 };
        for x in -5..5 {
            let expected = 2i64
                .wrapping_mul(x)
                .wrapping_mul(x)
                .wrapping_add((-3i64).wrapping_mul(x))
                .wrapping_add(1);
            assert_eq!(r.apply(x), expected);
        }
    }
}
