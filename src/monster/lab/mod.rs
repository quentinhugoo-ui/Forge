//! Lab ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â passive types & generators absorbÃƒÆ’Ã‚Â©s depuis `examples/lab_runner.rs`.
//!
//! # ÃƒÅ½Ã‚Â¦.ÃƒÅ½Ã‚Â½.2.a ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â premiÃƒÆ’Ã‚Â¨re vague de migration (Track B / vision VÃƒÂ¢Ã‹â€ Ã…Â¾)
//!
//! Ce module est le **cÃƒâ€¦Ã¢â‚¬Å“ur cognitif autonome** du nÃƒâ€¦Ã¢â‚¬Å“ud ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â la matiÃƒÆ’Ã‚Â¨re premiÃƒÆ’Ã‚Â¨re
//! que `MonsterNode::lab_probe()` et `MonsterNode::lab_run()` (ÃƒÆ’Ã‚Â  venir en
//! ÃƒÅ½Ã‚Â¦.ÃƒÅ½Ã‚Â½.2.c) utiliseront pour expÃƒÆ’Ã‚Â©rimenter sur lui-mÃƒÆ’Ã‚Âªme sans driver externe.
//!
//! ## Vocabulaire conservÃƒÆ’Ã‚Â© (ÃƒÅ½Ã‚Â¦.ÃƒÅ½Ã‚Â½.1)
//!
//! Le terme `lab_runner` reste le nom du **binaire d'entrÃƒÆ’Ã‚Â©e historique**
//! (`cargo run --release --example lab_runner -- 10000`). Le module ici
//! s'appelle `lab` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â c'est le **rÃƒÆ’Ã‚Â´le cognitif** d'expÃƒÆ’Ã‚Â©rimentation autonome
//! qu'un nÃƒâ€¦Ã¢â‚¬Å“ud joue *sur lui-mÃƒÆ’Ã‚Âªme*.
//!
//! ## Ce qui vit ici (ÃƒÅ½Ã‚Â¦.ÃƒÅ½Ã‚Â½.2.a)
//!
//!   * `XorShift64` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â PRNG dÃƒÆ’Ã‚Â©terministe (zÃƒÆ’Ã‚Â©ro RNG ambiant, doctrine V7)
//!   * `TargetTemplate` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â 37 variantes de targets (V7 + V8 diversification +
//!     ultra_glyph + wall probes + Tier 1 aging + literature-backed)
//!   * `random_target`, `generate_random_kasm_program` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â gÃƒÆ’Ã‚Â©nÃƒÆ’Ã‚Â©rateurs
//!   * `random_evolve_config`, `build_diverse_inputs` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â config d'expÃƒÆ’Ã‚Â©rience
//!   * `contract_probe_inputs`, `tier1_contract_targets` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â Tier 1 audit
//!   * `execute_i64`, `audit_loss` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â scoring KASM
//!
//! ## Ce qui suivra (ÃƒÅ½Ã‚Â¦.ÃƒÅ½Ã‚Â½.2.b/c)
//!
//!   * `ExperimentResult`, `LabCounters`, `LabReport` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â agrÃƒÆ’Ã‚Â©gation
//!   * `MonsterNode::lab_probe(template, seed)` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â un essai
//!   * `MonsterNode::self_improve(budget)` ÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Â boucle autonome lab + oracle + atlas

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::kasm::{compose, execute as kasm_execute, F64SubOp, Node as KNode, Op, Program, Target};
use crate::{MemoryGovernor, MonsterEvolutionConfig, Store};

use super::atlas::{
    canonical_outputs, fnv64_outputs, output_fingerprint, AtlasIngest, LiveAtlas,
    ATLAS_CANONICAL_INPUTS, LIVE_ATLAS_PATH,
};
use super::oracle::DistillConfig;
use super::{MonsterEvolutionOutcome, MonsterNode};

/// Path par dÃƒÆ’Ã‚Â©faut du log JSONL append-only.
pub const LOG_PATH: &str = "lab_findings.jsonl";

/// Nombre d'itÃƒÆ’Ã‚Â©rations par dÃƒÆ’Ã‚Â©faut quand `cargo run --example lab_runner`
/// est invoquÃƒÆ’Ã‚Â© sans argument.
pub const DEFAULT_ITERATIONS: usize = 100;

/// Convertit `(candidates_evaluated, elapsed_us)` en candidats/seconde.
pub fn candidates_per_sec(candidates_evaluated: u64, elapsed_us: u64) -> u64 {
    let denom = elapsed_us.max(1) as u128;
    ((candidates_evaluated as u128) * 1_000_000 / denom) as u64
}

/// Formate un dÃƒÆ’Ã‚Â©bit en kilo-candidats/seconde, 1 dÃƒÆ’Ã‚Â©cimale.
pub fn format_kcps(rate: u64) -> String {
    format!("{:.1}", rate as f64 / 1000.0)
}


pub struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
    fn i64_in(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(hi > lo);
        let span = (hi - lo) as u64;
        lo + (self.next() % span) as i64
    }
}

// ============================================================
// Target templates (functions DreamForge has to recover)
// ============================================================

/// V8 â€” domaines diversifiÃ©s. Chaque variante teste un sous-espace
/// diffÃ©rent de l'expressivitÃ© du synthÃ©tiseur (`dream_structured_candidates`
/// supporte Add/Sub/Mul/BitXor/BitAnd/BitOr/Shl/Shr/Min/Max). Le but
/// n'est pas que tout rÃ©ussisse â€” c'est que le `lab_findings.jsonl`
/// trace **lesquels domaines sont accessibles** au synthÃ©tiseur actuel,
/// guidant les futures extensions (Op::Hash64, Op::SelectI64, etc.).
#[derive(Clone, Debug)]
pub enum TargetTemplate {
    // ----- Targets V7 (existants) -----
    Affine { mul: i64, add: i64 },
    Poly2 { a: i64, b: i64, c: i64 },
    Poly3 { a: i64, b: i64, c: i64, d: i64 },
    BitMixer { shl: u32, shr: u32 },
    Piecewise { threshold: i64, lo: (i64, i64), hi: (i64, i64) },
    NoisyAffine { mul: i64, add: i64, noise_period: i64 },

    // ----- Targets V8 (diversification) -----
    /// `x ^ (x << shl)` â€” pattern simple de bit-mix, distinct du
    /// BitMixer original (1 shift au lieu de 2). Solvable en 2 atomes
    /// via Shl + BitXor.
    ShiftXor { shl: u32 },
    /// `x & mask` â€” modular truncation. Equivalent Ã  `x mod 2^k`
    /// quand mask = 2^k - 1. Solvable trivialement via BitAnd.
    AndMask { mask: i64 },
    /// `x | mask` â€” set forcÃ© de bits. Solvable via BitOr.
    OrMask { mask: i64 },
    /// `max(lo, min(x, hi))` â€” clamping classique. Solvable en 2
    /// atomes via Min + Max (dÃ©jÃ  dans le catalogue structurÃ©).
    Clamp { lo: i64, hi: i64 },
    /// `(x << shift) + add` â€” pattern d'addressage avec slot, ou
    /// premiÃ¨re Ã©tape d'un LCG. Solvable en 2 atomes via Shl + Add.
    AddShifted { shift: u32, add: i64 },
    /// `(x * mul) & mask` â€” multiplication tronquÃ©e, cÅ“ur d'un
    /// linear congruential generator. Solvable en 2 atomes via
    /// Mul + BitAnd.
    MulMask { mul: i64, mask: i64 },

    // ----- Wall probes / ultra-glyph swarm -----
    /// `clamp(x * mul + add, lo, hi)` â€” composition affine+clamp :
    /// le mur Ã  tester est "retrouver la famille composÃ©e sans
    /// explorer le produit cartÃ©sien des deux familles".
    UltraClampAffine { mul: i64, add: i64, lo: i64, hi: i64 },
    /// `abs(x * mul + add)` â€” V-shape cachÃ©e. Ce probe force Forge Ã 
    /// infÃ©rer une symÃ©trie plutÃ´t qu'une simple formule affine.
    UltraAbsAffine { mul: i64, add: i64 },

    /// Î¦.1 â€” `trunc(sqrt(|mul*x + add|))`. **Premier wall probe F64**:
    /// le synthÃ©tiseur ne peut hit ce target que via la chaÃ®ne
    /// `i64â†’f64 / fabs / fsqrt / f64â†’i64` introduite en Î¦.0. Sans
    /// recognizer dÃ©diÃ©, beam-search seul ne le trouve jamais (l'ops
    /// F64 n'est pas dans `DreamExpr`). C'est prÃ©cisÃ©ment ce qui en
    /// fait une preuve de connexion Î¦.0 â†” synthÃ©tiseur.
    UltraFSqrtAffine { mul: i64, add: i64 },

    /// Î¦.2 â€” `trunc(c / (x + b))`. Premier target qui force **fdivc**
    /// (division IEEE 754). b > 0 garantit denom != 0 sur lab inputs.
    UltraFDivAffine { b: i64, c: i64 },

    /// Î¦.2 â€” `trunc(c / sqrt(|mul*x + add|))`. Active **fdivc + fsqrt
    /// + fabs** simultanÃ©ment. Chain depth = 5.
    UltraInvSqrtAffine { mul: i64, add: i64, c: i64 },

    /// Î¦.2 â€” `trunc(min(hi, sqrt(|mul*x + add|)))`. Active **fmin**
    /// avec un seuil f64. Saturation physique.
    UltraClampFSqrt { mul: i64, add: i64, hi: i64 },

    /// Î¦.2 â€” `trunc(-sqrt(|mul*x + add|))`. Active **fneg** dans le
    /// domaine f64 (nÃ©gation avant troncation, distincte de neg i64).
    UltraFNegFSqrt { mul: i64, add: i64 },

    // ----- Î¦.3.1 â€” Wall probes: deliberately delusional targets -----
    //
    // Each `Wall*` template encodes a composition that NO Î¦.0..Î¦.3
    // recognizer can currently hit. Their job is to fail and tell us
    // where the next wall is. The cache (Î¦.2.1) absorbs the cost of
    // already-known domains so the lab spends all its cycles probing
    // these new walls instead of redoing the easy stuff.

    /// Î¦.3.1 â€” `trunc(c / (sqrt(|aÂ·x + b|) + d))`. **Activates fadd**
    /// (currently 0 invocations across 10k iter). Chain depth 6.
    /// No single existing recognizer can compose this; it would need
    /// a future "compound-inverse-sqrt" glyph or the beam search
    /// growing F64 ops in DreamExpr.
    WallCompoundInvSqrt { mul: i64, add: i64, c: i64, d: i64 },

    /// Î¦.3.1 â€” `trunc(sqrt(bÂ·b + 4Â·aÂ·x))` â€” quadratic discriminant.
    /// Recognising this requires inferring three coefficients (a, b, c)
    /// from outputs that pass through a sqrt. Currently impossible.
    WallQuadraticDisc { a: i64, b: i64 },

    /// Î¦.3.1 â€” `trunc(min(hi, c/(x+b)))`. Composition of fmin + fdivc.
    /// Each ingredient has a recognizer; the COMPOSITION does not.
    WallComposeClampDiv { b: i64, c: i64, hi: i64 },

    // ----- Î¦.8 â€” Ultra wall probes (3 levers) -----
    //
    // Lever 2: real-world domain formulas. Each probe encodes a
    // canonical scientific equation with i16-fitting parameters. They
    // FAIL with high probability on the current Forge â€” that's the
    // point. Their hit rate over time is the visible measure of how
    // much real-world reach Forge has accumulated.

    /// Î¦.8 / Lever 2 â€” Michaelisâ€“Menten saturation kinetics
    /// (biochemistry): `v = trunc((vmax Â· |x|) / (km + |x|))`.
    /// Asymptotes to vmax as |x| grows. Linear near 0. Failing this
    /// means Forge can't help any enzyme-binding study.
    DomainMichaelisMenten { vmax: i64, km: i64 },

    /// Î¦.8 / Lever 2 â€” Hill equation, n=2 (pharmacology dose-
    /// response): `y = trunc((100 Â· xÂ²) / (kÂ² + xÂ²))`. The 100 is
    /// fixed so the saturation amplitude is non-trivial on lab inputs.
    /// Failing this means Forge can't fit dose-response curves.
    DomainHillN2 { k: i64 },

    /// Î¦.8 / Lever 2 â€” Arrhenius rate (chemistry):
    /// `k = trunc(a Â· exp(-c / |x|))`. Activates Î¦.7a's `fexp`. Failing
    /// this even though `fexp` exists means the synthesizer hasn't
    /// learned to compose `fexp` with `fdiv` yet â€” direct intel.
    DomainArrhenius { a: i64, c: i64 },

    /// Î¦.8 / Lever 2 â€” Gravitational / Coulomb 1/rÂ² law (physics):
    /// `F = trunc(g Â· 1000 / (|x|Â·|x| + 1))`. The +1 keeps the
    /// denominator non-zero when x = 0. The 1000 rescaling lifts the
    /// output above the noise floor for moderate |x|.
    DomainInverseSquare { g: i64 },

    /// Î¦.8 / Lever 2 â€” Logistic growth (biology population):
    /// `N = trunc(k / (1 + a Â· exp(-|x| / 1000)))`. The /1000 keeps
    /// `exp` arguments in a finite regime for lab inputs spanning
    /// Â±50_000 (otherwise exp(50_000) overflows). Composition of
    /// `fexp` + `fmul` + `fadd` + `fdiv` â€” currently impossible to
    /// synthesise with any single existing recognizer.
    DomainLogistic { k: i64, a: i64 },

    /// Φ.12 — Beer–Lambert absorbance (spectroscopy / colorimetry):
    /// `A = trunc(c · ln(|x|))`. Activates **fln**, the last F64
    /// sub-op that was idle pre-Φ.12. Models drug-concentration
    /// readings, optical density, pH titration tail.
    DomainBeerLambert { c: i64 },
    /// Source-backed Beer-Lambert absorbance in its linear form:
    /// `A = trunc(epsilon_l * concentration)`, with `abs(x)` acting
    /// as concentration proxy and `epsilon_l = epsilon * pathlength`.
    DomainBeerLambertLinear { epsilon_l: i64 },
    /// Source-backed Arrhenius proxy with absolute-temperature guard:
    /// `k = trunc(a * exp(-ea_over_r / (abs(x) + 273)))`.
    DomainArrheniusKelvin { a: i64, ea_over_r: i64 },

    // ----- Tier 1 — aging / cellular decision primitives -----
    /// Cooperative Michaelis-Menten / Hill saturation:
    /// `v = trunc((vmax * |x|^n) / (k^n + |x|^n))`, n in {2, 3}.
    DomainMichaelisMentenCooperative { vmax: i64, k: i64, hill: i64 },
    /// NAD+-dependent sirtuin activity. Kept intentionally as the
    /// direct Michaelis-Menten primitive so downstream domains can
    /// share a byte-identical sub-computation.
    DomainSirtuinNadDependentActivity { vmax: i64, km: i64 },
    /// mTOR/AMPK nutrient switch: signed logistic balance.
    /// `m = trunc(amp / (1 + exp(-x / slope)))`.
    DomainMtorSignalingBalance { amp: i64, slope: i64 },
    /// NAD+ depletion/recovery curve:
    /// `nad = trunc(baseline - drop * exp(-|x| / tau))`.
    DomainNadDepletionRecovery { baseline: i64, drop: i64, tau: i64 },
    /// p53 activation gate over accumulated damage:
    /// `p = trunc(amp / (1 + exp(-(|x| - threshold) / slope)))`.
    DomainP53ActivationThreshold { amp: i64, threshold: i64, slope: i64 },

    // ----- Lever 3: adversarial noise on a known target shape -----

    /// Î¦.8 / Lever 3 â€” Sparse-outlier sqrt-affine: a normal
    /// fsqrt_affine target with a single deterministic Â±1 bump on
    /// one canonical lab input. This preserves a strong exact
    /// majority while keeping an exact holdout criterion meaningful.
    /// The wall measures whether Forge can recover the clean law in
    /// the presence of bounded, rare adversarial outliers.
    WallNoisyFSqrtAffine { mul: i64, add: i64, noise_seed: u64 },

    // ----- Lever 1: auto-generated KASM programs -----

    /// Î¦.8 / Lever 1 â€” A randomly-generated KASM program of 4..=8
    /// nodes. The lab pre-runs it on the input vector to obtain
    /// (x, y) examples and asks the synthesizer to recover the
    /// program. The hit rate over the random KASM grammar
    /// constitutes the **reachability map** â€” every commit moves
    /// this number, exposing exactly how much of the program space
    /// Forge spans today.
    WallRandomKasm { program_bytes: Vec<u8> },
}

impl TargetTemplate {
    fn name(&self) -> &'static str {
        match self {
            TargetTemplate::Affine { .. } => "affine",
            TargetTemplate::Poly2 { .. } => "poly2",
            TargetTemplate::Poly3 { .. } => "poly3",
            TargetTemplate::BitMixer { .. } => "bit_mixer",
            TargetTemplate::Piecewise { .. } => "piecewise",
            TargetTemplate::NoisyAffine { .. } => "noisy_affine",
            // V8
            TargetTemplate::ShiftXor { .. } => "shift_xor",
            TargetTemplate::AndMask { .. } => "and_mask",
            TargetTemplate::OrMask { .. } => "or_mask",
            TargetTemplate::Clamp { .. } => "clamp",
            TargetTemplate::AddShifted { .. } => "add_shifted",
            TargetTemplate::MulMask { .. } => "mul_mask",
            TargetTemplate::UltraClampAffine { .. } => "ultra_clamp_affine",
            TargetTemplate::UltraAbsAffine { .. } => "ultra_abs_affine",
            TargetTemplate::UltraFSqrtAffine { .. } => "ultra_fsqrt_affine",
            TargetTemplate::UltraFDivAffine { .. } => "ultra_fdiv_affine",
            TargetTemplate::UltraInvSqrtAffine { .. } => "ultra_invsqrt_affine",
            TargetTemplate::UltraClampFSqrt { .. } => "ultra_clamp_fsqrt",
            TargetTemplate::UltraFNegFSqrt { .. } => "ultra_fneg_fsqrt",
            TargetTemplate::WallCompoundInvSqrt { .. } => "wall_compound_invsqrt",
            TargetTemplate::WallQuadraticDisc { .. } => "wall_quadratic_disc",
            TargetTemplate::WallComposeClampDiv { .. } => "wall_compose_clamp_div",
            TargetTemplate::DomainMichaelisMenten { .. } => "domain_michaelis_menten",
            TargetTemplate::DomainHillN2 { .. } => "domain_hill_n2",
            TargetTemplate::DomainArrhenius { .. } => "domain_arrhenius",
            TargetTemplate::DomainInverseSquare { .. } => "domain_inverse_square",
            TargetTemplate::DomainLogistic { .. } => "domain_logistic",
            TargetTemplate::DomainBeerLambert { .. } => "domain_beer_lambert",
            TargetTemplate::DomainBeerLambertLinear { .. } => "domain_beer_lambert_linear",
            TargetTemplate::DomainArrheniusKelvin { .. } => "domain_arrhenius_kelvin",
            TargetTemplate::DomainMichaelisMentenCooperative { .. } => {
                "domain_michaelis_menten_cooperative"
            }
            TargetTemplate::DomainSirtuinNadDependentActivity { .. } => {
                "domain_sirtuin_nad_dependent_activity"
            }
            TargetTemplate::DomainMtorSignalingBalance { .. } => "domain_mtor_signaling_balance",
            TargetTemplate::DomainNadDepletionRecovery { .. } => "domain_nad_depletion_recovery",
            TargetTemplate::DomainP53ActivationThreshold { .. } => "domain_p53_activation_threshold",
            TargetTemplate::WallNoisyFSqrtAffine { .. } => "wall_noisy_fsqrt_affine",
            TargetTemplate::WallRandomKasm { .. } => "wall_random_kasm",
        }
    }

    fn eval(&self, x: i64) -> i64 {
        match self {
            TargetTemplate::Affine { mul, add } => x.wrapping_mul(*mul).wrapping_add(*add),
            TargetTemplate::Poly2 { a, b, c } => a
                .wrapping_mul(x)
                .wrapping_mul(x)
                .wrapping_add(b.wrapping_mul(x))
                .wrapping_add(*c),
            TargetTemplate::Poly3 { a, b, c, d } => {
                let x2 = x.wrapping_mul(x);
                let x3 = x2.wrapping_mul(x);
                a.wrapping_mul(x3)
                    .wrapping_add(b.wrapping_mul(x2))
                    .wrapping_add(c.wrapping_mul(x))
                    .wrapping_add(*d)
            }
            TargetTemplate::BitMixer { shl, shr } => {
                let v = x as u64;
                (v.wrapping_shl(*shl) ^ v.wrapping_shr(*shr)) as i64
            }
            TargetTemplate::Piecewise { threshold, lo, hi } => {
                if x < *threshold {
                    x.wrapping_mul(lo.0).wrapping_add(lo.1)
                } else {
                    x.wrapping_mul(hi.0).wrapping_add(hi.1)
                }
            }
            TargetTemplate::NoisyAffine { mul, add, noise_period } => {
                let base = x.wrapping_mul(*mul).wrapping_add(*add);
                if noise_period.abs() > 0 && x % noise_period == 0 {
                    base.wrapping_add(0xCAFE)
                } else {
                    base
                }
            }
            // V8
            TargetTemplate::ShiftXor { shl } => {
                let v = x as u64;
                (v ^ v.wrapping_shl(*shl)) as i64
            }
            TargetTemplate::AndMask { mask } => x & mask,
            TargetTemplate::OrMask { mask } => x | mask,
            TargetTemplate::Clamp { lo, hi } => x.max(*lo).min(*hi),
            TargetTemplate::AddShifted { shift, add } => {
                let v = x as u64;
                (v.wrapping_shl(*shift) as i64).wrapping_add(*add)
            }
            TargetTemplate::MulMask { mul, mask } => x.wrapping_mul(*mul) & mask,
            TargetTemplate::UltraClampAffine { mul, add, lo, hi } => {
                x.wrapping_mul(*mul).wrapping_add(*add).max(*lo).min(*hi)
            }
            TargetTemplate::UltraAbsAffine { mul, add } => {
                x.wrapping_mul(*mul).wrapping_add(*add).wrapping_abs()
            }
            TargetTemplate::UltraFSqrtAffine { mul, add } => {
                // Î¦.1 â€” match F64ToI64 truncation semantics exactly:
                // (f64 abs) â†’ sqrt â†’ cast-to-i64 (truncates toward 0).
                // The recognizer in evolve.rs uses the same chain, so
                // this target is hit iff the F64 surface round-trips.
                let inner = (x as f64).mul_add(*mul as f64, *add as f64).abs();
                let r = inner.sqrt();
                if r.is_finite() {
                    r as i64
                } else {
                    0
                }
            }
            // Î¦.2 â€” F64 surface saturation glyphs. All four mirror the
            // F64Op kill-switch semantics (NaN/Inf â†’ 0) used by the
            // KASM interpreter, so the lab and the synthesizer agree
            // bit-for-bit on every example.
            TargetTemplate::UltraFDivAffine { b, c } => {
                let denom = x as f64 + *b as f64;
                let r = (*c as f64) / denom;
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::UltraInvSqrtAffine { mul, add, c } => {
                let inner = (x as f64).mul_add(*mul as f64, *add as f64).abs();
                let denom = inner.sqrt();
                let r = (*c as f64) / denom;
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::UltraClampFSqrt { mul, add, hi } => {
                let inner = (x as f64).mul_add(*mul as f64, *add as f64).abs();
                let r = inner.sqrt().min(*hi as f64);
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::UltraFNegFSqrt { mul, add } => {
                let inner = (x as f64).mul_add(*mul as f64, *add as f64).abs();
                let r = -(inner.sqrt());
                if r.is_finite() { r as i64 } else { 0 }
            }
            // Î¦.3.1 â€” wall probes. Each one mirrors KASM F64Op kill-
            // switch semantics so the synthesizer COULD in principle
            // hit them; what it lacks is a recognizer that composes
            // the right F64 sub-ops in the right order.
            TargetTemplate::WallCompoundInvSqrt { mul, add, c, d } => {
                let inner = (x as f64).mul_add(*mul as f64, *add as f64).abs();
                let denom = inner.sqrt() + (*d as f64);
                let r = (*c as f64) / denom;
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::WallQuadraticDisc { a, b } => {
                let bb = (*b as f64) * (*b as f64);
                let four_ax = 4.0 * (*a as f64) * (x as f64);
                let r = (bb + four_ax).abs().sqrt();
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::WallComposeClampDiv { b, c, hi } => {
                let denom = (x as f64) + (*b as f64);
                let div = (*c as f64) / denom;
                let r = div.min(*hi as f64);
                if r.is_finite() { r as i64 } else { 0 }
            }
            // ----- Î¦.8 / Lever 2 : real-world domain probes -----
            TargetTemplate::DomainMichaelisMenten { vmax, km } => {
                let xa = (x.unsigned_abs() as f64).max(0.0);
                let r = ((*vmax as f64) * xa) / ((*km as f64) + xa);
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainHillN2 { k } => {
                let xa = x as f64;
                let xa2 = xa * xa;
                let k2 = (*k as f64) * (*k as f64);
                let r = (100.0 * xa2) / (k2 + xa2);
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainArrhenius { a, c } => {
                let xa = (x.unsigned_abs() as f64).max(1.0);
                let r = (*a as f64) * (-(*c as f64) / xa).exp();
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainInverseSquare { g } => {
                let xa = x as f64;
                let r = (*g as f64) * 1000.0 / (xa * xa + 1.0);
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainLogistic { k, a } => {
                let xa = (x.unsigned_abs() as f64) / 1000.0;
                let r = (*k as f64) / (1.0 + (*a as f64) * (-xa).exp());
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainBeerLambert { c } => {
                let xa = x.unsigned_abs() as f64;
                let ln_x = xa.ln();
                let r = if ln_x.is_finite() { (*c as f64) * ln_x } else { 0.0 };
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainBeerLambertLinear { epsilon_l } => {
                let concentration = x.unsigned_abs() as f64;
                let r = (*epsilon_l as f64) * concentration;
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainArrheniusKelvin { a, ea_over_r } => {
                let temp_k = (x.unsigned_abs() as f64) + 273.0;
                let r = (*a as f64) * (-(*ea_over_r as f64) / temp_k).exp();
                if r.is_finite() { r as i64 } else { 0 }
            }
            // ----- Tier 1 : cellular aging primitives -----
            TargetTemplate::DomainMichaelisMentenCooperative { vmax, k, hill } => {
                let xa = x.unsigned_abs() as f64;
                let xn = if *hill == 3 { xa * xa * xa } else { xa * xa };
                let kn = if *hill == 3 {
                    (*k as f64) * (*k as f64) * (*k as f64)
                } else {
                    (*k as f64) * (*k as f64)
                };
                let r = (*vmax as f64) * xn / (kn + xn);
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainSirtuinNadDependentActivity { vmax, km } => {
                let nad = x.unsigned_abs() as f64;
                let r = (*vmax as f64) * nad / ((*km as f64) + nad);
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainMtorSignalingBalance { amp, slope } => {
                let r = (*amp as f64) / (1.0 + (-(x as f64) / (*slope as f64)).exp());
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainNadDepletionRecovery { baseline, drop, tau } => {
                let xa = x.unsigned_abs() as f64;
                let r = (*baseline as f64) - (*drop as f64) * (-(xa) / (*tau as f64)).exp();
                if r.is_finite() { r as i64 } else { 0 }
            }
            TargetTemplate::DomainP53ActivationThreshold { amp, threshold, slope } => {
                let damage = x.unsigned_abs() as f64;
                let arg = -((damage - (*threshold as f64)) / (*slope as f64));
                let r = (*amp as f64) / (1.0 + arg.exp());
                if r.is_finite() { r as i64 } else { 0 }
            }
            // ----- Î¦.8 / Lever 3 : sparse-outlier sqrt_affine -----
            TargetTemplate::WallNoisyFSqrtAffine { mul, add, noise_seed } => {
                let inner = (x as f64).mul_add(*mul as f64, *add as f64).abs();
                let base = inner.sqrt();
                let base_i = if base.is_finite() { base as i64 } else { 0 };
                // One deterministic outlier on the fixed lab input
                // cube. This keeps the target reproducible across
                // runs while aligning with the "majority exact +
                // bounded outliers" doctrine of Î¦.9-noise.
                let anchors = [-7, -1, 1, 11, -100, 100, -987, 987, -12345, -50000, 12345, 50000];
                let index = (*noise_seed % anchors.len() as u64) as usize;
                let sign = if ((*noise_seed >> 8) & 1) == 0 { -1 } else { 1 };
                if x == anchors[index] {
                    base_i + sign
                } else {
                    base_i
                }
            }
            // ----- Î¦.8 / Lever 1 : auto-generated KASM programs -----
            TargetTemplate::WallRandomKasm { program_bytes } => {
                // Parse the persisted program bytes and execute it on
                // the input. Same KASM execution path the synthesizer
                // would produce on hit, so the recognition is fair.
                let Ok(program) = Program::from_bytes(program_bytes) else {
                    return 0;
                };
                let bytes = x.to_le_bytes();
                let Ok(out) = kasm_execute(&program, &bytes) else {
                    return 0;
                };
                if out.len() == 8 {
                    i64::from_le_bytes(out.try_into().unwrap())
                } else {
                    0
                }
            }
        }
    }
}

pub fn random_target(rng: &mut XorShift64) -> TargetTemplate {
    // 37 domaines equiprobables. 6 V7 + 6 V8 diversification + 2
    // ultra-glyph i64 + 1 F64 wall (Î¦.1) + 4 F64 (Î¦.2) + 3
    // dÃ©lusionnels (Î¦.3.1) + **7 ultra wall probes (Î¦.8)** + **5
    // Tier 1 aging primitives** (mTOR, p53, NAD recovery, sirtuin,
    // MM coopératif) + **2 literature-backed** (Arrhenius Kelvin,
    // Beer-Lambert linéaire). Les Tier 1 et literature-backed sont
    // en rotation random pour que le nano_probe / atom mining
    // découvre leurs atomes universels (la nano_atomique passe par
    // toutes les familles présentes).
    // Les Î¦.8 probes sont lÃ  pour Ã‰CHOUER â€”
    // c'est leur job. Le miss rate trace la carte de ce que Forge
    // ne sait PAS encore faire ; chaque commit dÃ©place cette
    // frontiÃ¨re.
    match rng.range(37) {
        // ----- V7 -----
        0 => TargetTemplate::Affine {
            mul: rng.i64_in(-99, 100),
            add: rng.i64_in(-999, 1000),
        },
        1 => TargetTemplate::Poly2 {
            a: rng.i64_in(-19, 20),
            b: rng.i64_in(-99, 100),
            c: rng.i64_in(-999, 1000),
        },
        2 => TargetTemplate::Poly3 {
            a: rng.i64_in(-9, 10),
            b: rng.i64_in(-19, 20),
            c: rng.i64_in(-99, 100),
            d: rng.i64_in(-999, 1000),
        },
        3 => TargetTemplate::BitMixer {
            shl: 1 + (rng.next() as u32 % 10),
            shr: 1 + (rng.next() as u32 % 10),
        },
        4 => TargetTemplate::Piecewise {
            threshold: rng.i64_in(-50, 50),
            lo: (rng.i64_in(-9, 10), rng.i64_in(-99, 100)),
            hi: (rng.i64_in(-9, 10), rng.i64_in(-99, 100)),
        },
        5 => TargetTemplate::NoisyAffine {
            mul: rng.i64_in(-99, 100),
            add: rng.i64_in(-999, 1000),
            noise_period: rng.i64_in(3, 15),
        },
        // ----- V8 (diversification) -----
        6 => TargetTemplate::ShiftXor {
            shl: 1 + (rng.next() as u32 % 12),
        },
        7 => {
            // V8 fix lab-driven : KASM Const::imm est i16, donc tout
            // masque > 32767 demanderait une synthÃ¨se de constante en
            // plusieurs nÅ“uds (hors portÃ©e 2-atom). On reste dans le
            // range i16-fittable pour que le synthÃ©tiseur ait une
            // chance rÃ©elle de trouver le programme.
            let masks: [i64; 7] = [0x1F, 0x3F, 0x7F, 0xFF, 0xFFF, 0x3FFF, 0x7FFF];
            TargetTemplate::AndMask {
                mask: masks[rng.range(masks.len())],
            }
        }
        8 => {
            // V8 fix lab-driven : idem, masques i16-fittables.
            let masks: [i64; 6] = [0x1, 0x10, 0x80, 0xFF, 0x0F0F, 0x1000];
            TargetTemplate::OrMask {
                mask: masks[rng.range(masks.len())],
            }
        }
        9 => {
            let lo = rng.i64_in(-1000, 0);
            let hi = rng.i64_in(1, 1001);
            TargetTemplate::Clamp { lo, hi }
        }
        10 => TargetTemplate::AddShifted {
            shift: 1 + (rng.next() as u32 % 8),
            add: rng.i64_in(-999, 1000),
        },
        11 => TargetTemplate::MulMask {
            mul: rng.i64_in(-99, 100),
            // V8 fix lab-driven : masques i16-fittables uniquement.
            mask: [0x1F, 0x7F, 0xFF, 0x3FF, 0xFFF, 0x7FFF][rng.range(6)],
        },
        12 => {
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            let lo = rng.i64_in(-600, -50);
            let hi = rng.i64_in(50, 601);
            TargetTemplate::UltraClampAffine {
                mul,
                add: rng.i64_in(-30, 31),
                lo,
                hi,
            }
        }
        13 => {
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            TargetTemplate::UltraAbsAffine {
                mul,
                add: rng.i64_in(-50, 51),
            }
        }
        14 => {
            // Î¦.1 â€” UltraFSqrtAffine. mul âˆˆ [-9, 9]\{0}, add âˆˆ [-50, 50].
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            TargetTemplate::UltraFSqrtAffine {
                mul,
                add: rng.i64_in(-50, 51),
            }
        }
        // ----- Î¦.2 â€” Saturate the F64 sub-op surface -----
        15 => {
            // UltraFDivAffine. b > 0 to keep denom positive.
            let b = rng.i64_in(1, 51);
            let mut c = rng.i64_in(-200, 201);
            if c == 0 {
                c = 1;
            }
            TargetTemplate::UltraFDivAffine { b, c }
        }
        16 => {
            // UltraInvSqrtAffine. mul âˆˆ [-9, 9]\{0}, add âˆˆ [-50, 50],
            // c âˆˆ [-300, 300]\{0}. Recognizer brute-forces the same
            // cube â€” picking from the same range guarantees the lab
            // hits a representable target every time.
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            let add = rng.i64_in(-50, 51);
            let mut c = rng.i64_in(-300, 301);
            if c == 0 {
                c = 1;
            }
            TargetTemplate::UltraInvSqrtAffine { mul, add, c }
        }
        17 => {
            // UltraClampFSqrt. The clamp ceiling `hi` must be
            // achievable by `sqrt(|mul*x+add|)` for at least one
            // sample, otherwise the clamp never fires and the glyph
            // collapses to fsqrt_affine. Pick hi small enough.
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            let add = rng.i64_in(-50, 51);
            let hi = rng.i64_in(1, 100);
            TargetTemplate::UltraClampFSqrt { mul, add, hi }
        }
        18 => {
            // UltraFNegFSqrt. Same params as fsqrt_affine; outputs are
            // â‰¤ 0 by construction.
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            TargetTemplate::UltraFNegFSqrt {
                mul,
                add: rng.i64_in(-50, 51),
            }
        }
        // ----- Î¦.3.1 â€” Wall probes (deliberately unreachable) -----
        19 => {
            // WallCompoundInvSqrt. Forces fadd in the F64 surface.
            let mut mul = rng.i64_in(-5, 6);
            if mul == 0 {
                mul = 1;
            }
            let mut c = rng.i64_in(-200, 201);
            if c == 0 {
                c = 1;
            }
            let mut d = rng.i64_in(1, 30);
            if d == 0 {
                d = 1;
            }
            TargetTemplate::WallCompoundInvSqrt {
                mul,
                add: rng.i64_in(-30, 31),
                c,
                d,
            }
        }
        20 => {
            // WallQuadraticDisc. b*b + 4*a*x â€” quadratic discriminant.
            let mut a = rng.i64_in(-9, 10);
            if a == 0 {
                a = 1;
            }
            TargetTemplate::WallQuadraticDisc { a, b: rng.i64_in(-30, 31) }
        }
        21 => {
            // WallComposeClampDiv. Composition of fmin + fdivc.
            let b = rng.i64_in(1, 51);
            let mut c = rng.i64_in(-200, 201);
            if c == 0 {
                c = 1;
            }
            let hi = rng.i64_in(1, 50);
            TargetTemplate::WallComposeClampDiv { b, c, hi }
        }
        // ----- Î¦.8 â€” Ultra wall probes (Lever 2: domain probes) -----
        22 => TargetTemplate::DomainMichaelisMenten {
            vmax: rng.i64_in(50, 200),
            km: rng.i64_in(5, 50),
        },
        23 => TargetTemplate::DomainHillN2 {
            k: rng.i64_in(2, 30),
        },
        24 => TargetTemplate::DomainArrhenius {
            a: rng.i64_in(50, 200),
            c: rng.i64_in(1, 30),
        },
        25 => TargetTemplate::DomainInverseSquare {
            g: rng.i64_in(1, 30),
        },
        26 => TargetTemplate::DomainLogistic {
            k: rng.i64_in(50, 200),
            a: rng.i64_in(1, 20),
        },
        // Φ.12 — Beer-Lambert (spectroscopy): activates fln.
        29 => TargetTemplate::DomainBeerLambert {
            c: rng.i64_in(1, 30),
        },
        // ----- Î¦.8 â€” Lever 3: adversarial noise -----
        27 => {
            let mut mul = rng.i64_in(-9, 10);
            if mul == 0 {
                mul = 1;
            }
            TargetTemplate::WallNoisyFSqrtAffine {
                mul,
                add: rng.i64_in(-50, 51),
                noise_seed: rng.next(),
            }
        }
        // ----- Tier 1 — aging primitives (rotation random pour atom mining) -----
        30 => TargetTemplate::DomainMichaelisMentenCooperative {
            vmax: rng.i64_in(50, 200),
            k: rng.i64_in(5, 30),
            hill: 2 + (rng.range(2) as i64),
        },
        31 => TargetTemplate::DomainSirtuinNadDependentActivity {
            vmax: rng.i64_in(40, 160),
            km: rng.i64_in(8, 80),
        },
        32 => TargetTemplate::DomainMtorSignalingBalance {
            amp: rng.i64_in(50, 200),
            slope: rng.i64_in(80, 800),
        },
        33 => TargetTemplate::DomainNadDepletionRecovery {
            baseline: rng.i64_in(80, 220),
            drop: rng.i64_in(10, 80),
            tau: rng.i64_in(50, 900),
        },
        34 => TargetTemplate::DomainP53ActivationThreshold {
            amp: rng.i64_in(50, 200),
            threshold: rng.i64_in(50, 5000),
            slope: rng.i64_in(20, 500),
        },
        // ----- Literature-backed primitives (Codex bibliography) -----
        // Φ.μ.7.10 — fix range mismatch : recognizer accepte
        // a∈[50,200] ea_over_r∈[100,1000] step 10. Le générateur
        // produisait a∈[50,250] ea∈[500,5000] (free) → ~93% des seeds
        // étaient structurellement non-recognizable, d'où les 7.4%
        // exact mesurés en lab_runner -- 10000.
        35 => TargetTemplate::DomainArrheniusKelvin {
            a: rng.i64_in(50, 200),
            ea_over_r: rng.i64_in(10, 100) * 10,
        },
        36 => TargetTemplate::DomainBeerLambertLinear {
            epsilon_l: rng.i64_in(1, 50),
        },
        // ----- Î¦.8 â€” Lever 1: auto-generated KASM program -----
        _ => TargetTemplate::WallRandomKasm {
            program_bytes: generate_random_kasm_program(rng),
        },
    }
}

/// Î¦.8 / Lever 1 â€” Generate a random valid KASM program of 5â€“8
/// nodes. Used as a wall probe target: the lab evaluates this
/// program on the input vector to obtain (x, y) examples and asks
/// the synthesizer to recover an equivalent program.
///
/// The hit rate over many such random programs is the **reachability
/// map** of Forge's current synthesizer. Every commit that improves
/// the recognizer chain or the beam search shifts this number;
/// regressions drop it. Single most-actionable scalar in the lab.
pub fn generate_random_kasm_program(rng: &mut XorShift64) -> Vec<u8> {
    use crate::kasm::{Node, Ty};

    let const_a = (rng.i64_in(-30, 31)) as i16;
    let const_b = (rng.i64_in(-30, 31)) as i16;
    // Always start with input + 2 small constants â€” gives the random
    // ops a non-trivial base to compose from.
    let mut nodes: Vec<Node> = vec![
        Node::input(0),
        Node::const_i64(const_a),
        Node::const_i64(const_b),
    ];

    // 3..=6 random ops on top.
    let num_ops = 3 + rng.range(4);
    for _ in 0..num_ops {
        let kind = rng.range(8);
        let a_idx = rng.range(nodes.len()) as u16;
        let b_idx = rng.range(nodes.len()) as u16;
        let new_node = match kind {
            0 => Node::add(a_idx, b_idx),
            1 => Node::sub(a_idx, b_idx),
            2 => Node::mul(a_idx, b_idx),
            3 => Node::bit_xor(a_idx, b_idx),
            4 => Node::bit_and(a_idx, b_idx),
            5 => Node::bit_or(a_idx, b_idx),
            6 => Node::shl(a_idx, b_idx),
            _ => Node::shr(a_idx, b_idx),
        };
        nodes.push(new_node);
    }

    let last_idx = (nodes.len() - 1) as u16;
    nodes.push(Node::output(last_idx, Ty::I64));

    let total = nodes.len() as u32;
    Program::new(Target::Cpu, 1, 1, total, nodes)
        .expect("generated random KASM should validate by construction")
        .bytes()
        .to_vec()
}

pub fn random_evolve_config(rng: &mut XorShift64) -> MonsterEvolutionConfig {
    // V8 c â€” cible 500 iter/s. Capped HARD :
    //   generations: 1 â†’ catalogue structurÃ© uniquement (74% des succÃ¨s
    //                    tombent Ã  gens 0-1 historiquement, donc on
    //                    perd ~26% des succÃ¨s marginaux)
    //   beam_width:  32 â†’ minimum viable pour le catalogue
    //   max_nodes:   4..7 â†’ programmes courts seulement
    // C'est le compromis honnÃªte : perdre les succÃ¨s rares pour
    // gagner 10Ã— sur le hot lab loop.
    MonsterEvolutionConfig {
        generations: 1,
        max_nodes: 4 + rng.range(3),
        beam_width: 32,
        holdout_stride: 2 + rng.range(3),
        progress: None,
        skip_prepass: false,
    }
}

/// V8 c (Niveau 3) â€” inputs **dÃ©terministes** partagÃ©s entre toutes
/// les iters. Permet de memoizer les atomes du catalogue structurÃ©
/// au-delÃ  d'une seule iter : sur 500 iters, le catalogue est
/// calculÃ© UNE FOIS (sur le 1er thread qui passe), puis tous les
/// autres l'aspirent du cache.
///
/// DiversitÃ© prÃ©servÃ©e :
///   - 4 small (-7, -1, 1, 11)
///   - 2 mid (-100, 100)
///   - 2 large (-987, 987)
///   - 2 negative-edge (-12345, -50000)
///   - 2 positive-edge (12345, 50000)
///
/// Le `_rng` est gardÃ© pour compatibilitÃ© de signature mais ignorÃ©.
pub fn build_diverse_inputs(_rng: &mut XorShift64) -> Vec<i64> {
    vec![-7, -1, 1, 11, -100, 100, -987, 987, -12345, -50000, 12345, 50000]
}

pub fn contract_probe_inputs(target: &TargetTemplate) -> Vec<i64> {
    let mut inputs = BTreeSet::new();
    for x in [
        -50000, -25000, -12345, -987, -500, -250, -100, -50, -11, -7, -1, 0, 1, 7, 11, 50,
        100, 250, 500, 987, 12345, 25000, 50000,
    ] {
        inputs.insert(x);
    }

    let mut add_symmetric = |v: i64| {
        for delta in [-2i64, -1, 0, 1, 2] {
            let p = v.saturating_add(delta);
            inputs.insert(p);
            inputs.insert(p.saturating_neg());
        }
        inputs.insert(v.saturating_mul(2));
        inputs.insert(v.saturating_mul(2).saturating_neg());
    };

    match target {
        TargetTemplate::DomainMichaelisMentenCooperative { k, .. } => add_symmetric(*k),
        TargetTemplate::DomainSirtuinNadDependentActivity { km, .. } => add_symmetric(*km),
        TargetTemplate::DomainMtorSignalingBalance { slope, .. } => add_symmetric(*slope),
        TargetTemplate::DomainNadDepletionRecovery { tau, .. } => add_symmetric(*tau),
        TargetTemplate::DomainP53ActivationThreshold { threshold, slope, .. } => {
            add_symmetric(*threshold);
            for offset in [*slope, slope.saturating_mul(2), slope.saturating_mul(4)] {
                let lo = threshold.saturating_sub(offset);
                let hi = threshold.saturating_add(offset);
                inputs.insert(lo);
                inputs.insert(lo.saturating_neg());
                inputs.insert(hi);
                inputs.insert(hi.saturating_neg());
            }
        }
        _ => {}
    }

    inputs.into_iter().collect()
}

pub fn tier1_contract_targets() -> Vec<TargetTemplate> {
    vec![
        TargetTemplate::DomainMichaelisMentenCooperative { vmax: 80, k: 7, hill: 2 },
        TargetTemplate::DomainMichaelisMentenCooperative { vmax: 150, k: 23, hill: 3 },
        TargetTemplate::DomainSirtuinNadDependentActivity { vmax: 40, km: 8 },
        TargetTemplate::DomainSirtuinNadDependentActivity { vmax: 155, km: 77 },
        TargetTemplate::DomainMtorSignalingBalance { amp: 75, slope: 80 },
        TargetTemplate::DomainMtorSignalingBalance { amp: 180, slope: 750 },
        TargetTemplate::DomainNadDepletionRecovery { baseline: 100, drop: 15, tau: 75 },
        TargetTemplate::DomainNadDepletionRecovery { baseline: 215, drop: 75, tau: 900 },
        TargetTemplate::DomainP53ActivationThreshold { amp: 65, threshold: 100, slope: 20 },
        TargetTemplate::DomainP53ActivationThreshold { amp: 120, threshold: 500, slope: 80 },
        TargetTemplate::DomainP53ActivationThreshold { amp: 190, threshold: 4500, slope: 450 },
    ]
}

pub fn execute_i64(program: &Program, x: i64) -> Option<i64> {
    let out = kasm_execute(program, &x.to_le_bytes()).ok()?;
    if out.len() < 8 {
        return None;
    }
    Some(i64::from_le_bytes(out[..8].try_into().ok()?))
}

pub fn audit_loss(program: &Program, target: &TargetTemplate, inputs: &[i64]) -> u128 {
    let mut loss = 0u128;
    for &x in inputs {
        let expected = target.eval(x);
        let Some(actual) = execute_i64(program, x) else {
            return u128::MAX;
        };
        loss = loss.saturating_add(actual.abs_diff(expected) as u128);
    }
    loss
}

/// Î¦.1.5 â€” Post-mortem analysis of a synthesized program. Walks the
/// node DAG once and extracts:
///
///   * `f64_op_count`        : total `F64Op` nodes
///   * `f64_const_count`     : total `ConstF64` nodes
///   * `f64_chain_max_depth` : max number of consecutive F64-typed
///                             nodes along any data-flow chain
///   * `subop_counts`        : breakdown by `F64SubOp` selector

/// Compact O(N) analysis extracted from the historical `lab_runner`.
/// It is used after synthesis only, never in the dispatch hot path.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProgramAnalysis {
    pub f64_op_count: u32,
    pub f64_const_count: u32,
    pub f64_chain_max_depth: u32,
    pub subop_counts: [u32; 13],
}

#[derive(Debug, Clone)]
pub struct GlyphIntel {
    pub candidate: bool,
    pub candidate_kind: &'static str,
    pub canonical_hash: [u8; 8],
    pub semantic_fp: [u8; 8],
    pub canonical_nodes: usize,
    pub compression_saved: usize,
    pub atlas_region: &'static str,
}

#[derive(Debug)]
pub struct ExperimentResult {
    pub iter: usize,
    pub target_name: &'static str,
    pub config: MonsterEvolutionConfig,
    pub elapsed_ms: u64,
    pub elapsed_us: u64,
    pub outcome: ExperimentOutcome,
    pub frontier_score: f64,
    pub monitoring_sample: bool,
    pub wall_family: &'static str,
}

#[derive(Debug)]
pub enum ExperimentOutcome {
    Completed {
        source: &'static str,
        exact_train: bool,
        exact_holdout: bool,
        generations_used: usize,
        candidates_evaluated: usize,
        program_nodes: usize,
        train_loss: u128,
        holdout_loss: u128,
        analysis: ProgramAnalysis,
        glyph_intel: GlyphIntel,
    },
    Errored { message: String },
}

pub fn format_jsonl(result: &ExperimentResult) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let body = match &result.outcome {
        ExperimentOutcome::Completed {
            source,
            exact_train,
            exact_holdout,
            generations_used,
            candidates_evaluated,
            program_nodes,
            train_loss,
            holdout_loss,
            analysis,
            glyph_intel,
        } => {
            let candidate_rate =
                candidates_per_sec(*candidates_evaluated as u64, result.elapsed_us);
            let subops_json = analysis
                .subop_counts
                .iter()
                .enumerate()
                .filter(|(_, c)| **c > 0)
                .map(|(i, c)| format!(r#""{i}":{c}"#))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                r#""outcome":"completed","source":"{source}","exact_train":{exact_train},"exact_holdout":{exact_holdout},"generations_used":{generations_used},"candidates_evaluated":{candidates_evaluated},"candidates_per_sec":{candidate_rate},"program_nodes":{program_nodes},"train_loss":{train_loss},"holdout_loss":{holdout_loss},"glyph_candidate":{},"glyph_kind":"{}","canonical_hash":"{}","semantic_fp":"{}","canonical_nodes":{},"compression_saved":{},"atlas_region":"{}","f64":{{"ops":{},"consts":{},"chain_max":{},"subops":{{{}}}}}"#,
                glyph_intel.candidate,
                glyph_intel.candidate_kind,
                fmt_hash8(glyph_intel.canonical_hash),
                fmt_hash8(glyph_intel.semantic_fp),
                glyph_intel.canonical_nodes,
                glyph_intel.compression_saved,
                glyph_intel.atlas_region,
                analysis.f64_op_count,
                analysis.f64_const_count,
                analysis.f64_chain_max_depth,
                subops_json,
            )
        }
        ExperimentOutcome::Errored { message } => format!(
            r#""outcome":"errored","message":"{}""#,
            message.replace('"', "'").replace('\n', " ")
        ),
    };
    format!(
        r#"{{"ts":{now},"iter":{},"target":"{}","cfg":{{"generations":{},"max_nodes":{},"beam_width":{},"holdout_stride":{}}},"elapsed_ms":{},"elapsed_us":{},"frontier_score":{:.4},"monitoring_sample":{},"wall_family":"{}",{body}}}{NL}"#,
        result.iter,
        result.target_name,
        result.config.generations,
        result.config.max_nodes,
        result.config.beam_width,
        result.config.holdout_stride,
        result.elapsed_ms,
        result.elapsed_us,
        result.frontier_score,
        result.monitoring_sample,
        result.wall_family,
        NL = "\n",
    )
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub iter: u64,
    pub target: &'static str,
    pub max_nodes: u32,
    pub elapsed_us: u64,
    pub outcome: LogOutcome,
}

#[derive(Debug, Clone)]
pub enum LogOutcome {
    Completed {
        source: &'static str,
        exact_train: bool,
        exact_holdout: bool,
        generations_used: u32,
        program_nodes: u32,
        candidates_evaluated: u64,
        candidates_per_sec: u64,
        f64_ops: u32,
        f64_chain_max: u32,
        glyph_candidate: bool,
        glyph_kind: &'static str,
        canonical_hash: [u8; 8],
        semantic_fp: [u8; 8],
        canonical_nodes: u32,
        compression_saved: u32,
        atlas_region: &'static str,
    },
    Errored(String),
}

fn intern_target_name(s: &str) -> &'static str {
    match s {
        "affine" => "affine", "poly2" => "poly2", "poly3" => "poly3",
        "bit_mixer" => "bit_mixer", "piecewise" => "piecewise", "noisy_affine" => "noisy_affine",
        "shift_xor" => "shift_xor", "and_mask" => "and_mask", "or_mask" => "or_mask",
        "clamp" => "clamp", "add_shifted" => "add_shifted", "mul_mask" => "mul_mask",
        "ultra_clamp_affine" => "ultra_clamp_affine", "ultra_abs_affine" => "ultra_abs_affine",
        "ultra_fsqrt_affine" => "ultra_fsqrt_affine", "ultra_fdiv_affine" => "ultra_fdiv_affine",
        "ultra_invsqrt_affine" => "ultra_invsqrt_affine", "ultra_clamp_fsqrt" => "ultra_clamp_fsqrt",
        "ultra_fneg_fsqrt" => "ultra_fneg_fsqrt",
        "wall_compound_invsqrt" => "wall_compound_invsqrt",
        "wall_quadratic_disc" => "wall_quadratic_disc",
        "wall_compose_clamp_div" => "wall_compose_clamp_div",
        "wall_noisy_fsqrt_affine" => "wall_noisy_fsqrt_affine",
        "wall_random_kasm" => "wall_random_kasm",
        "domain_michaelis_menten" => "domain_michaelis_menten",
        "domain_hill_n2" => "domain_hill_n2", "domain_arrhenius" => "domain_arrhenius",
        "domain_inverse_square" => "domain_inverse_square", "domain_logistic" => "domain_logistic",
        "domain_beer_lambert" => "domain_beer_lambert",
        "domain_beer_lambert_linear" => "domain_beer_lambert_linear",
        "domain_arrhenius_kelvin" => "domain_arrhenius_kelvin",
        "domain_michaelis_menten_cooperative" => "domain_michaelis_menten_cooperative",
        "domain_sirtuin_nad_dependent_activity" => "domain_sirtuin_nad_dependent_activity",
        "domain_mtor_signaling_balance" => "domain_mtor_signaling_balance",
        "domain_nad_depletion_recovery" => "domain_nad_depletion_recovery",
        "domain_p53_activation_threshold" => "domain_p53_activation_threshold",
        _ => "unknown",
    }
}

fn intern_source(s: &str) -> &'static str {
    match s {
        "glyph" => "glyph",
        "retrieval" => "retrieval",
        "ultra_glyph" => "ultra_glyph",
        "structured" => "structured",
        _ => "beam",
    }
}

fn intern_glyph_kind(s: &str) -> &'static str {
    match s {
        "beam_promote" => "beam_promote",
        "structured_seed" => "structured_seed",
        _ => "none",
    }
}

// 6 sources × 2 types (i64/f64) × 10 depths = 120 static strings.
// Column layout: [i64:d0..d9, f64:d0..d9].
const ATLAS_REGIONS: [[&str; 20]; 6] = [
    ["beam:i64:d0","beam:i64:d1","beam:i64:d2","beam:i64:d3","beam:i64:d4",
     "beam:i64:d5","beam:i64:d6","beam:i64:d7","beam:i64:d8","beam:i64:d9",
     "beam:f64:d0","beam:f64:d1","beam:f64:d2","beam:f64:d3","beam:f64:d4",
     "beam:f64:d5","beam:f64:d6","beam:f64:d7","beam:f64:d8","beam:f64:d9"],
    ["glyph:i64:d0","glyph:i64:d1","glyph:i64:d2","glyph:i64:d3","glyph:i64:d4",
     "glyph:i64:d5","glyph:i64:d6","glyph:i64:d7","glyph:i64:d8","glyph:i64:d9",
     "glyph:f64:d0","glyph:f64:d1","glyph:f64:d2","glyph:f64:d3","glyph:f64:d4",
     "glyph:f64:d5","glyph:f64:d6","glyph:f64:d7","glyph:f64:d8","glyph:f64:d9"],
    ["retrieval:i64:d0","retrieval:i64:d1","retrieval:i64:d2","retrieval:i64:d3","retrieval:i64:d4",
     "retrieval:i64:d5","retrieval:i64:d6","retrieval:i64:d7","retrieval:i64:d8","retrieval:i64:d9",
     "retrieval:f64:d0","retrieval:f64:d1","retrieval:f64:d2","retrieval:f64:d3","retrieval:f64:d4",
     "retrieval:f64:d5","retrieval:f64:d6","retrieval:f64:d7","retrieval:f64:d8","retrieval:f64:d9"],
    ["ultra_glyph:i64:d0","ultra_glyph:i64:d1","ultra_glyph:i64:d2","ultra_glyph:i64:d3","ultra_glyph:i64:d4",
     "ultra_glyph:i64:d5","ultra_glyph:i64:d6","ultra_glyph:i64:d7","ultra_glyph:i64:d8","ultra_glyph:i64:d9",
     "ultra_glyph:f64:d0","ultra_glyph:f64:d1","ultra_glyph:f64:d2","ultra_glyph:f64:d3","ultra_glyph:f64:d4",
     "ultra_glyph:f64:d5","ultra_glyph:f64:d6","ultra_glyph:f64:d7","ultra_glyph:f64:d8","ultra_glyph:f64:d9"],
    ["structured:i64:d0","structured:i64:d1","structured:i64:d2","structured:i64:d3","structured:i64:d4",
     "structured:i64:d5","structured:i64:d6","structured:i64:d7","structured:i64:d8","structured:i64:d9",
     "structured:f64:d0","structured:f64:d1","structured:f64:d2","structured:f64:d3","structured:f64:d4",
     "structured:f64:d5","structured:f64:d6","structured:f64:d7","structured:f64:d8","structured:f64:d9"],
    ["memo:i64:d0","memo:i64:d1","memo:i64:d2","memo:i64:d3","memo:i64:d4",
     "memo:i64:d5","memo:i64:d6","memo:i64:d7","memo:i64:d8","memo:i64:d9",
     "memo:f64:d0","memo:f64:d1","memo:f64:d2","memo:f64:d3","memo:f64:d4",
     "memo:f64:d5","memo:f64:d6","memo:f64:d7","memo:f64:d8","memo:f64:d9"],
];

fn intern_atlas_region(source: &'static str, uses_f64: bool, depth: u8) -> &'static str {
    let src = match source {
        "glyph" => 1, "retrieval" => 2, "ultra_glyph" => 3, "structured" => 4, "memo" => 5, _ => 0,
    };
    ATLAS_REGIONS[src][if uses_f64 { 10 } else { 0 } + depth.min(9) as usize]
}

fn intern_atlas_region_str(s: &str) -> &'static str {
    for row in &ATLAS_REGIONS {
        for &v in row {
            if v == s { return v; }
        }
    }
    "beam:i64:d0"
}

pub fn parse_jsonl_line(line: &str) -> Option<LogEntry> {
    let iter = grab_u64(line, "\"iter\":")?;
    let target = intern_target_name(&grab_string(line, "\"target\":\"")?[..]);
    let max_nodes = grab_u64(line, "\"max_nodes\":")? as u32;
    let _beam_width = grab_u64(line, "\"beam_width\":")? as u32;
    let _holdout_stride = grab_u64(line, "\"holdout_stride\":")? as u32;
    let generations = grab_u64(line, "\"generations\":")? as u32;
    let elapsed_ms = grab_u64(line, "\"elapsed_ms\":")?;
    let elapsed_us =
        grab_optional_u64(line, "\"elapsed_us\":").unwrap_or(elapsed_ms.saturating_mul(1000));
    let outcome_kind = grab_string(line, "\"outcome\":\"")?;
    let outcome = match outcome_kind.as_str() {
        "completed" => LogOutcome::Completed {
            source: grab_string(line, "\"source\":\"")
                .as_deref()
                .map(intern_source)
                .unwrap_or_else(|| {
                    if grab_optional_u64(line, "\"generations_used\":")
                        .unwrap_or(generations as u64)
                        == 0
                    {
                        "structured"
                    } else {
                        "beam"
                    }
                }),
            exact_train: grab_bool(line, "\"exact_train\":")?,
            exact_holdout: grab_bool(line, "\"exact_holdout\":")?,
            generations_used: grab_optional_u64(line, "\"generations_used\":")
                .unwrap_or(generations as u64) as u32,
            program_nodes: grab_u64(line, "\"program_nodes\":")? as u32,
            candidates_evaluated: grab_u64(line, "\"candidates_evaluated\":")?,
            candidates_per_sec: grab_optional_u64(line, "\"candidates_per_sec\":")
                .unwrap_or_else(|| {
                    candidates_per_sec(
                        grab_u64(line, "\"candidates_evaluated\":").unwrap_or(0),
                        elapsed_us,
                    )
                }),
            f64_ops: grab_optional_u64(line, "\"f64\":{\"ops\":").unwrap_or(0) as u32,
            f64_chain_max: grab_optional_u64(line, "\"chain_max\":").unwrap_or(0) as u32,
            glyph_candidate: grab_bool(line, "\"glyph_candidate\":").unwrap_or(false),
            glyph_kind: grab_string(line, "\"glyph_kind\":\"")
                .as_deref()
                .map(intern_glyph_kind)
                .unwrap_or("none"),
            canonical_hash: grab_string(line, "\"canonical_hash\":\"")
                .map(|s| short_hash(&s))
                .unwrap_or(LEGACY_FP),
            semantic_fp: grab_string(line, "\"semantic_fp\":\"")
                .map(|s| short_hash(&s))
                .unwrap_or(LEGACY_FP),
            canonical_nodes: grab_optional_u64(line, "\"canonical_nodes\":").unwrap_or(0) as u32,
            compression_saved: grab_optional_u64(line, "\"compression_saved\":").unwrap_or(0)
                as u32,
            atlas_region: grab_string(line, "\"atlas_region\":\"")
                .as_deref()
                .map(intern_atlas_region_str)
                .unwrap_or("beam:i64:d0"),
        },
        "errored" => LogOutcome::Errored(grab_string(line, "\"message\":\"")?),
        _ => return None,
    };
    Some(LogEntry {
        iter,
        target,
        max_nodes,
        elapsed_us,
        outcome,
    })
}

pub fn read_lab_entries(limit: Option<usize>) -> io::Result<Vec<LogEntry>> {
    let file = File::open(LOG_PATH)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Some(entry) = parse_jsonl_line(&line) {
            entries.push(entry);
        }
    }
    if let Some(limit) = limit {
        let start = entries.len().saturating_sub(limit);
        entries.drain(0..start);
    }
    Ok(entries)
}

pub fn recent_frontier_scores(limit: usize) -> io::Result<HashMap<&'static str, f64>> {
    let mut totals: HashMap<&'static str, (usize, usize, u64)> = HashMap::new();
    for entry in read_lab_entries(Some(limit))? {
        let slot = totals.entry(entry.target).or_default();
        slot.0 += 1;
        if let LogOutcome::Completed { exact_holdout, .. } = entry.outcome {
            if exact_holdout {
                slot.1 += 1;
            }
        }
        slot.2 += entry.elapsed_us;
    }
    let mut scores = HashMap::new();
    for (target, (total, hits, elapsed_us)) in totals {
        if total == 0 {
            continue;
        }
        let miss_rate = 1.0 - (hits as f64 / total as f64);
        let avg_ms = (elapsed_us as f64 / total as f64) / 1000.0;
        scores.insert(target, miss_rate * avg_ms);
    }
    Ok(scores)
}

#[derive(Default)]
pub struct FrontierWeights {
    scores: HashMap<&'static str, f64>,
}

impl FrontierWeights {
    pub fn from_recent_log(limit: usize) -> Self {
        let scores = recent_frontier_scores(limit).unwrap_or_default();
        Self { scores }
    }

    pub fn score(&self, name: &str) -> f64 {
        self.scores.get(name).copied().unwrap_or(1.0)
    }

    pub fn wall_family(name: &str) -> &'static str {
        if name.starts_with("bit_mixer")
            || name.starts_with("shift_xor")
            || name.starts_with("and_mask")
            || name.starts_with("or_mask")
            || name.starts_with("mul_mask")
        {
            "bitwise"
        } else if name.starts_with("piecewise") || name.starts_with("noisy") {
            "piecewise"
        } else if name.starts_with("domain_") {
            "real_world"
        } else if name.starts_with("wall_") || name.starts_with("ultra_") {
            "wall_probe"
        } else {
            "known"
        }
    }
}

/// 20% pure random (monitoring), 80% tournament k=3 (hardest of 3).
pub fn frontier_target_sample(
    rng: &mut XorShift64,
    weights: &FrontierWeights,
) -> (TargetTemplate, bool, f64, &'static str) {
    let monitoring = rng.range(10) < 2;
    if monitoring {
        let target = random_target(rng);
        let score = weights.score(target.name());
        let family = FrontierWeights::wall_family(target.name());
        return (target, true, score, family);
    }
    let mut best = random_target(rng);
    let mut best_score = weights.score(best.name());
    for _ in 1..3 {
        let target = random_target(rng);
        let score = weights.score(target.name());
        if score > best_score {
            best_score = score;
            best = target;
        }
    }
    let family = FrontierWeights::wall_family(best.name());
    (best, false, best_score, family)
}

pub type AtomCatalogueSummary = HashMap<String, (HashSet<&'static str>, usize)>;
pub type PerTargetSummary = HashMap<&'static str, (usize, usize)>;

pub fn read_lab_catalogue_summaries() -> io::Result<(AtomCatalogueSummary, PerTargetSummary)> {
    let file = File::open(LOG_PATH)?;
    let reader = BufReader::new(file);
    let mut per_atom: AtomCatalogueSummary = HashMap::new();
    let mut per_target: PerTargetSummary = HashMap::new();

    for line in reader.lines().map_while(Result::ok) {
        let get = |key: &str| -> Option<String> {
            let needle = format!("\"{}\":", key);
            let p = line.find(&needle)?;
            let rest = &line[p + needle.len()..];
            let rest = rest.trim_start();
            if rest.starts_with('"') {
                let after = &rest[1..];
                let end = after.find('"')?;
                Some(after[..end].to_string())
            } else {
                let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
                Some(rest[..end].trim().to_string())
            }
        };

        let Some(source) = get("source") else {
            continue;
        };
        match source.as_str() {
            "atom_catalogue" => {
                let Some(atom) = get("atom") else { continue };
                let occ: usize = get("occurrences").and_then(|s| s.parse().ok()).unwrap_or(0);
                let fams_marker = "\"families\":[";
                if let Some(p) = line.find(fams_marker) {
                    let after = &line[p + fams_marker.len()..];
                    let end = after.find(']').unwrap_or(after.len());
                    let fams: HashSet<&'static str> = after[..end]
                        .split(',')
                        .filter_map(|s| {
                            let t = s.trim().trim_matches('"');
                            if t.is_empty() { None } else { Some(intern_target_name(t)) }
                        })
                        .collect();
                    let entry = per_atom.entry(atom).or_insert_with(|| (HashSet::new(), 0));
                    entry.0.extend(fams);
                    entry.1 = entry.1.max(occ);
                }
            }
            "per_target_summary" => {
                let Some(target) = get("target") else {
                    continue;
                };
                let hits: usize = get("hits").and_then(|s| s.parse().ok()).unwrap_or(0);
                let misses: usize = get("misses").and_then(|s| s.parse().ok()).unwrap_or(0);
                let entry = per_target.entry(intern_target_name(&target)).or_insert((0, 0));
                entry.0 += hits;
                entry.1 += misses;
            }
            _ => {}
        }
    }

    Ok((per_atom, per_target))
}

/// Φ.ν.7 — `target_fingerprint` is now `fnv64(target.eval(canonical_inputs))`,
/// reusing the canonical `ATLAS_CANONICAL_INPUTS` and `atlas::fnv64`. The
/// resulting key is bit-stable with the legacy `lab::fnv64` it replaces, so
/// `LiveAtlas::lookup_hot(target_fingerprint(t))` matches the same RAM cache
/// entry as the old `HotAtlas::lookup(fp)`.
pub fn target_fingerprint(target: &TargetTemplate) -> u64 {
    let mut outs = [0i64; 12];
    for (i, &x) in ATLAS_CANONICAL_INPUTS.iter().enumerate() {
        outs[i] = target.eval(x);
    }
    fnv64_outputs(&outs)
}

pub fn meta_glyph_phase(
    pool: Vec<ProgramEntry>,
    rng: &mut XorShift64,
) -> (MetaGlyphCounters, Vec<String>) {
    let mut counters = MetaGlyphCounters::default();
    let mut jsonl_lines: Vec<String> = Vec::new();

    if pool.len() < 2 {
        return (counters, jsonl_lines);
    }

    let mut seen_fps: HashSet<Vec<u8>> = HashSet::new();
    let deduped: Vec<&ProgramEntry> = pool
        .iter()
        .filter(|entry| {
            if let Ok(fp) = entry.program.semantic_fingerprint() {
                seen_fps.insert(fp.to_vec())
            } else {
                true
            }
        })
        .collect();
    counters.dedup_hits = pool.len() - deduped.len();

    if deduped.len() < 2 {
        return (counters, jsonl_lines);
    }

    let mut seen_composed: HashSet<Vec<u8>> = HashSet::new();

    let max_depth2 = 500usize;
    let mut depth2_tried = 0usize;
    'depth2: for i in 0..deduped.len() {
        for j in 0..deduped.len() {
            if i == j {
                continue;
            }
            if depth2_tried >= max_depth2 {
                break 'depth2;
            }
            let left = &deduped[i].program;
            let right = &deduped[j].program;
            if left.outputs() as usize != right.inputs() as usize {
                continue;
            }

            counters.attempts += 1;
            depth2_tried += 1;

            let composed = match compose(left, right, Target::Cpu) {
                Ok(program) => program,
                Err(_) => {
                    counters.rejects += 1;
                    continue;
                }
            };

            if let Ok(fp) = composed.semantic_fingerprint() {
                if !seen_composed.insert(fp.to_vec()) {
                    counters.dedup_hits += 1;
                    continue;
                }
            }

            counters.hits += 1;
            counters.depth2_hits += 1;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            jsonl_lines.push(format!(
                r#"{{"ts":{now},"source":"meta_glyph","composition_depth":2,"parent_left":"{}","parent_right":"{}"}}{nl}"#,
                deduped[i].target,
                deduped[j].target,
                nl = "\n"
            ));
        }
    }

    let n = deduped.len();
    for _ in 0..200usize.min(n * n) {
        let i = rng.range(n);
        let j = rng.range(n);
        let k = rng.range(n);
        if i == j || j == k || i == k {
            continue;
        }
        let left = &deduped[i].program;
        let mid = &deduped[j].program;
        let right = &deduped[k].program;
        if left.outputs() as usize != mid.inputs() as usize {
            continue;
        }

        counters.attempts += 1;
        let Ok(composed_lm) = compose(left, mid, Target::Cpu) else {
            counters.rejects += 1;
            continue;
        };
        if composed_lm.outputs() as usize != right.inputs() as usize {
            continue;
        }
        let Ok(composed) = compose(&composed_lm, right, Target::Cpu) else {
            counters.rejects += 1;
            continue;
        };

        if let Ok(fp) = composed.semantic_fingerprint() {
            if !seen_composed.insert(fp.to_vec()) {
                counters.dedup_hits += 1;
                continue;
            }
        }
        counters.hits += 1;
        counters.depth3_hits += 1;
    }

    (counters, jsonl_lines)
}

fn op_atom_label(op: Op, imm: i16) -> &'static str {
    match op {
        Op::Input => "input",
        Op::ConstI64 => "cst",
        Op::ConstF64 => "cst_f",
        Op::AddI64 => "add",
        Op::MulI64 => "mul",
        Op::SubI64 => "sub",
        Op::DivI64Checked => "div",
        Op::MinI64 => "min",
        Op::MaxI64 => "max",
        Op::BitAndI64 => "band",
        Op::BitOrI64 => "bor",
        Op::BitXorI64 => "bxor",
        Op::ShlI64 => "shl",
        Op::ShrI64 => "shr",
        Op::NegI64 => "neg",
        Op::BitFlipI64 => "bnot",
        Op::Hash64 => "hash",
        Op::EqI64 => "eq",
        Op::LtI64 => "lt",
        Op::LeI64 => "le",
        Op::AndBool => "and",
        Op::OrBool => "or",
        Op::NotBool => "not",
        Op::SatAddI64 => "sadd",
        Op::SatSubI64 => "ssub",
        Op::ModI64Checked => "mod",
        Op::ClampI64 => "clamp",
        Op::SelectI64 => "sel",
        Op::ReverseBitsI64 => "rev",
        Op::ByteswapI64 => "bswap",
        Op::PopcntI64 => "popcnt",
        Op::LzcntI64 => "lzcnt",
        Op::TzcntI64 => "tzcnt",
        Op::PextI64 => "pext",
        Op::PdepI64 => "pdep",
        Op::Lazy => "lazy",
        Op::Force => "force",
        Op::ReduceAddI64 => "radd",
        Op::ReduceMulI64 => "rmul",
        Op::Output => "out",
        Op::F64Op => match F64SubOp::from_imm(imm) {
            Ok(F64SubOp::Add) => "fadd",
            Ok(F64SubOp::Sub) => "fsub",
            Ok(F64SubOp::Mul) => "fmul",
            Ok(F64SubOp::DivChecked) => "fdiv",
            Ok(F64SubOp::Min) => "fmin",
            Ok(F64SubOp::Max) => "fmax",
            Ok(F64SubOp::Sqrt) => "fsqrt",
            Ok(F64SubOp::Abs) => "fabs",
            Ok(F64SubOp::Neg) => "fneg",
            Ok(F64SubOp::FromI64) => "i2f",
            Ok(F64SubOp::ToI64) => "f2i",
            Ok(F64SubOp::Exp) => "fexp",
            Ok(F64SubOp::Ln) => "fln",
            _ => "f64?",
        },
        // KASM v1.0 atom labels
        Op::Adaptive => "adapt",
        Op::Comptime => "ctime",
        Op::Grad => "grad",
        Op::Cond => "cond",
        Op::Memoize => "memo",
        Op::Pipeline => "pipe",
        Op::Vmap => "vmap",
        Op::Pmap => "pmap",
        Op::Fori => "fori",
        Op::WhileLoop => "while",
        Op::Reduce => "reduce",
        Op::Scan => "scan",
        Op::VLenI64 => "vlen",
        Op::VSumI64 => "vsum",
        Op::VAddI64 => "vadd",
        Op::VMulI64 => "vmul",
        Op::VSubI64 => "vsub",
        Op::VMaxI64 => "vmax",
        Op::VMinI64 => "vmin",
        Op::VRangeI64 => "vrange",
        Op::VConcatI64 => "vconcat",
        Op::VReverseI64 => "vreverse",
        Op::VBroadcastI64 => "vbroadcast",
        Op::VEqI64 => "veq",
        Op::VAndI64 => "vand",
        Op::VOrI64 => "vor",
        Op::VXorI64 => "vxor",
        Op::VAbsI64 => "vabs",
        Op::VNegI64 => "vneg",
        Op::VBitFlipI64 => "vbitflip",
        Op::VGetI64 => "vget",     // Wave 7i — Vec random-access
        Op::Fractal => "fractal",  // Wave 8 self-hosting
        Op::Eval => "eval",        // Wave 8 self-hosting
    }
}

fn subtree_label(nodes: &[KNode], idx: usize, depth: usize) -> String {
    let Some(node) = nodes.get(idx) else {
        return "?".to_string();
    };
    match node.op {
        Op::Input => "input".to_string(),
        Op::ConstI64 | Op::ConstF64 => "cst".to_string(),
        Op::Output => subtree_label(nodes, node.a as usize, depth),
        _ if depth == 1 => op_atom_label(node.op, node.imm).to_string(),
        _ => {
            let label = op_atom_label(node.op, node.imm);
            let ca = subtree_label(nodes, node.a as usize, depth - 1);
            if node.b != 0 {
                let cb = subtree_label(nodes, node.b as usize, depth - 1);
                format!("{}({},{})", label, ca, cb)
            } else {
                format!("{}({})", label, ca)
            }
        }
    }
}

pub fn extract_atoms_v2(program: &Program) -> Vec<String> {
    let nodes = program.nodes();
    let mut atoms: HashSet<String> = HashSet::new();
    for (i, node) in nodes.iter().enumerate() {
        if matches!(node.op, Op::Output | Op::Input | Op::ConstI64 | Op::ConstF64) {
            continue;
        }
        let _ = node;
        for depth in 1..=4usize {
            let label = subtree_label(nodes, i, depth);
            if !label.contains('?') {
                atoms.insert(label);
            }
        }
    }
    atoms.into_iter().collect()
}

pub fn format_atom_catalogue_lines(
    atoms_map: &AtomCatalogueSummary,
    now_ts: u64,
) -> Vec<String> {
    let mut universal_atoms: Vec<_> = atoms_map
        .iter()
        .filter(|(_, (fams, _))| fams.len() >= 2)
        .collect();
    universal_atoms.sort_by(|a, b| b.1.0.len().cmp(&a.1.0.len()).then(b.1.1.cmp(&a.1.1)));
    universal_atoms
        .into_iter()
        .map(|(atom, (fams, cnt))| {
            let fam_json = fams
                .iter()
                .map(|family| format!("\"{}\"", family))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                r#"{{"ts":{now_ts},"source":"atom_catalogue","atom":"{}","family_count":{},"occurrences":{},"families":[{}]}}{nl}"#,
                atom,
                fams.len(),
                cnt,
                fam_json,
                nl = "\n"
            )
        })
        .collect()
}

pub fn format_target_summary_lines(target_misses: &PerTargetSummary, now_ts: u64) -> Vec<String> {
    target_misses
        .iter()
        .map(|(target, (hits, misses))| {
            format!(
                r#"{{"ts":{now_ts},"source":"per_target_summary","target":"{}","hits":{},"misses":{}}}{nl}"#,
                target,
                hits,
                misses,
                nl = "\n"
            )
        })
        .collect()
}

pub fn append_lab_log_slices(slices: &[&[String]]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;
    use std::io::Write as _;
    for slice in slices {
        for line in *slice {
            file.write_all(line.as_bytes())?;
        }
    }
    Ok(())
}

pub fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn collapse_audit_sample(
    program: &Program,
    examples: &[(i64, i64)],
    rng: &mut XorShift64,
) -> bool {
    if examples.is_empty() {
        return true;
    }
    let idx = rng.range(examples.len());
    let (x, expected_y) = examples[idx];
    let input_bytes = x.to_le_bytes();
    match kasm_execute(program, &input_bytes) {
        Ok(out) if out.len() >= 8 => {
            let y = i64::from_le_bytes(out[..8].try_into().unwrap());
            y == expected_y
        }
        _ => false,
    }
}

#[derive(Default)]
pub struct TargetCounters {
    pub total: usize,
    pub holdout_exact: usize,
    pub train_only: usize,
    pub errored: usize,
    pub total_candidates_evaluated: u128,
    pub total_elapsed_us: u128,
    pub total_program_nodes: u128,
    pub f64_program_count: usize,
    pub f64_chain_depth_sum: u128,
}

#[derive(Default, Clone)]
pub struct MarketCounters {
    pub total: usize,
    pub holdout_exact: usize,
    pub train_only: usize,
    pub candidates: usize,
    pub structured_seeds: usize,
    pub total_elapsed_us: u128,
    pub total_nodes: u128,
    pub total_canonical_nodes: u128,
    pub canonical_hashes: HashSet<[u8; 8]>,
}

#[derive(Default, Clone)]
pub struct AtlasCounters {
    pub total: usize,
    pub holdout_exact: usize,
    pub train_only: usize,
    pub candidates: usize,
    pub semantic_fps: HashSet<[u8; 8]>,
    pub canonical_hashes: HashSet<[u8; 8]>,
    pub total_elapsed_us: u128,
}

pub fn glyph_market_score(stats: &MarketCounters) -> f64 {
    if stats.total == 0 {
        return 0.0;
    }
    let hit = stats.holdout_exact as f64 / stats.total as f64;
    let train_only = stats.train_only as f64 / stats.total as f64;
    let avg_us = stats.total_elapsed_us as f64 / stats.total as f64;
    let redundancy_penalty = stats.canonical_hashes.len().saturating_sub(1) as f64 * 12.5;
    let compression_bonus = if stats.total_nodes > stats.total_canonical_nodes {
        ((stats.total_nodes - stats.total_canonical_nodes) as f64 / stats.total as f64) * 4.0
    } else {
        0.0
    };
    (hit * 1000.0) + (train_only * 100.0) + compression_bonus
        - (avg_us / 1000.0)
        - redundancy_penalty
}

#[derive(Default, Clone, Copy)]
pub struct ChainDepthHistogram {
    pub buckets: [u32; 6],
}

impl ChainDepthHistogram {
    pub fn record(&mut self, depth: u32) {
        let idx = depth.min(5) as usize;
        self.buckets[idx] = self.buckets[idx].saturating_add(1);
    }

    pub fn merge(&mut self, other: ChainDepthHistogram) {
        for (a, b) in self.buckets.iter_mut().zip(other.buckets.iter()) {
            *a = a.saturating_add(*b);
        }
    }
}

#[derive(Default)]
pub struct LabCounters {
    pub completed_exact: usize,
    pub completed_exact_retrieval: usize,
    pub completed_exact_glyph: usize,
    pub completed_exact_ultra_glyph: usize,
    pub completed_exact_structured: usize,
    pub completed_exact_evolved: usize,
    pub completed_partial: usize,
    pub errored: usize,
    pub total_candidates_evaluated: u128,
    pub total_elapsed_us: u128,
    pub by_target: HashMap<&'static str, TargetCounters>,
    pub f64_programs_total: usize,
    pub f64_ops_total: u64,
    pub subop_totals: [u64; 13],
    pub chain_depth_hist: ChainDepthHistogram,
    pub f64_by_source: [u64; 4],
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub cache_hit_elapsed_us: u128,
    pub glyph_candidates: usize,
    pub glyph_structured_seeds: usize,
    pub glyph_compression_saved: usize,
    pub glyph_market: HashMap<[u8; 8], MarketCounters>,
    pub semantic_atlas: HashMap<&'static str, AtlasCounters>,
    pub semantic_covered_retrieval: u128,
    pub semantic_covered_glyph: u128,
    pub semantic_covered_beam: u128,
    pub collapse_audit_pass: usize,
    pub collapse_audit_fail: usize,
    pub frontier_experiments: usize,
    pub monitoring_experiments: usize,
    pub atlas_hits: usize,
}

impl LabCounters {
    pub fn merge(&mut self, other: LabCounters) {
        self.completed_exact += other.completed_exact;
        self.completed_exact_retrieval += other.completed_exact_retrieval;
        self.completed_exact_glyph += other.completed_exact_glyph;
        self.completed_exact_ultra_glyph += other.completed_exact_ultra_glyph;
        self.completed_exact_structured += other.completed_exact_structured;
        self.completed_exact_evolved += other.completed_exact_evolved;
        self.completed_partial += other.completed_partial;
        self.errored += other.errored;
        self.total_candidates_evaluated += other.total_candidates_evaluated;
        self.total_elapsed_us += other.total_elapsed_us;
        for (k, other_entry) in other.by_target {
            let entry = self.by_target.entry(k).or_default();
            entry.total += other_entry.total;
            entry.holdout_exact += other_entry.holdout_exact;
            entry.train_only += other_entry.train_only;
            entry.errored += other_entry.errored;
            entry.total_candidates_evaluated += other_entry.total_candidates_evaluated;
            entry.total_elapsed_us += other_entry.total_elapsed_us;
            entry.total_program_nodes += other_entry.total_program_nodes;
            entry.f64_program_count += other_entry.f64_program_count;
            entry.f64_chain_depth_sum += other_entry.f64_chain_depth_sum;
        }
        self.f64_programs_total += other.f64_programs_total;
        self.f64_ops_total += other.f64_ops_total;
        for (a, b) in self.subop_totals.iter_mut().zip(other.subop_totals.iter()) {
            *a = a.saturating_add(*b);
        }
        self.chain_depth_hist.merge(other.chain_depth_hist);
        for (a, b) in self.f64_by_source.iter_mut().zip(other.f64_by_source.iter()) {
            *a = a.saturating_add(*b);
        }
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        self.cache_hit_elapsed_us += other.cache_hit_elapsed_us;
        self.glyph_candidates += other.glyph_candidates;
        self.glyph_structured_seeds += other.glyph_structured_seeds;
        self.glyph_compression_saved += other.glyph_compression_saved;
        for (k, other_entry) in other.glyph_market {
            let entry = self.glyph_market.entry(k).or_default();
            entry.total += other_entry.total;
            entry.holdout_exact += other_entry.holdout_exact;
            entry.train_only += other_entry.train_only;
            entry.candidates += other_entry.candidates;
            entry.structured_seeds += other_entry.structured_seeds;
            entry.total_elapsed_us += other_entry.total_elapsed_us;
            entry.total_nodes += other_entry.total_nodes;
            entry.total_canonical_nodes += other_entry.total_canonical_nodes;
            entry.canonical_hashes.extend(other_entry.canonical_hashes);
        }
        for (k, other_entry) in other.semantic_atlas {
            let entry = self.semantic_atlas.entry(k).or_default();
            entry.total += other_entry.total;
            entry.holdout_exact += other_entry.holdout_exact;
            entry.train_only += other_entry.train_only;
            entry.candidates += other_entry.candidates;
            entry.semantic_fps.extend(other_entry.semantic_fps);
            entry.canonical_hashes.extend(other_entry.canonical_hashes);
            entry.total_elapsed_us += other_entry.total_elapsed_us;
        }
        self.semantic_covered_retrieval += other.semantic_covered_retrieval;
        self.semantic_covered_glyph += other.semantic_covered_glyph;
        self.semantic_covered_beam += other.semantic_covered_beam;
        self.collapse_audit_pass += other.collapse_audit_pass;
        self.collapse_audit_fail += other.collapse_audit_fail;
        self.frontier_experiments += other.frontier_experiments;
        self.monitoring_experiments += other.monitoring_experiments;
        self.atlas_hits += other.atlas_hits;
    }

    pub fn absorb(&mut self, result: &ExperimentResult) {
        match &result.outcome {
            ExperimentOutcome::Completed {
                source,
                exact_train,
                exact_holdout,
                generations_used,
                candidates_evaluated,
                program_nodes,
                analysis,
                glyph_intel,
                ..
            } => {
                if *exact_train && *exact_holdout {
                    self.completed_exact += 1;
                    if *source == "retrieval" {
                        self.completed_exact_retrieval += 1;
                    } else if *source == "glyph" {
                        self.completed_exact_glyph += 1;
                    } else if *source == "ultra_glyph" {
                        self.completed_exact_ultra_glyph += 1;
                    } else if *source == "atlas" {
                        self.atlas_hits += 1;
                    } else if *source == "memo" {
                        self.cache_hits += 1;
                        self.cache_hit_elapsed_us += result.elapsed_us as u128;
                    } else if *generations_used == 0 {
                        self.completed_exact_structured += 1;
                    } else {
                        self.completed_exact_evolved += 1;
                    }
                } else {
                    self.completed_partial += 1;
                }
                if *source != "memo" && *exact_train && *exact_holdout {
                    self.cache_misses += 1;
                }
                self.total_candidates_evaluated += *candidates_evaluated as u128;
                self.total_elapsed_us += result.elapsed_us as u128;
                let entry = self.by_target.entry(result.target_name).or_default();
                entry.total += 1;
                if *exact_holdout {
                    entry.holdout_exact += 1;
                } else if *exact_train {
                    entry.train_only += 1;
                }
                entry.total_candidates_evaluated += *candidates_evaluated as u128;
                entry.total_elapsed_us += result.elapsed_us as u128;
                entry.total_program_nodes += *program_nodes as u128;
                if analysis.uses_f64() {
                    entry.f64_program_count += 1;
                    entry.f64_chain_depth_sum += analysis.f64_chain_max_depth as u128;
                    self.f64_programs_total += 1;
                    self.f64_ops_total += analysis.f64_op_count as u64;
                    for (slot, count) in analysis.subop_counts.iter().enumerate() {
                        self.subop_totals[slot] =
                            self.subop_totals[slot].saturating_add(*count as u64);
                    }
                    self.chain_depth_hist.record(analysis.f64_chain_max_depth);
                    let bucket = match *source {
                        "retrieval" => 0usize,
                        "glyph" => 1,
                        "ultra_glyph" => 2,
                        _ => 3,
                    };
                    self.f64_by_source[bucket] = self.f64_by_source[bucket].saturating_add(1);
                }
                if glyph_intel.candidate {
                    self.glyph_candidates += 1;
                    self.glyph_compression_saved += glyph_intel.compression_saved;
                } else if glyph_intel.candidate_kind == "structured_seed" {
                    self.glyph_structured_seeds += 1;
                }
                let market = self
                    .glyph_market
                    .entry(glyph_intel.semantic_fp)
                    .or_default();
                market.total += 1;
                if *exact_holdout {
                    market.holdout_exact += 1;
                } else if *exact_train {
                    market.train_only += 1;
                }
                if glyph_intel.candidate {
                    market.candidates += 1;
                } else if glyph_intel.candidate_kind == "structured_seed" {
                    market.structured_seeds += 1;
                }
                market.total_elapsed_us += result.elapsed_us as u128;
                market.total_nodes += *program_nodes as u128;
                market.total_canonical_nodes += glyph_intel.canonical_nodes as u128;
                market
                    .canonical_hashes
                    .insert(glyph_intel.canonical_hash);

                let atlas = self
                    .semantic_atlas
                    .entry(glyph_intel.atlas_region)
                    .or_default();
                atlas.total += 1;
                if *exact_holdout {
                    atlas.holdout_exact += 1;
                } else if *exact_train {
                    atlas.train_only += 1;
                }
                if glyph_intel.candidate {
                    atlas.candidates += 1;
                }
                atlas.semantic_fps.insert(glyph_intel.semantic_fp);
                atlas
                    .canonical_hashes
                    .insert(glyph_intel.canonical_hash);
                atlas.total_elapsed_us += result.elapsed_us as u128;
            }
            ExperimentOutcome::Errored { .. } => {
                self.errored += 1;
                let entry = self.by_target.entry(result.target_name).or_default();
                entry.total += 1;
                entry.errored += 1;
                entry.total_elapsed_us += result.elapsed_us as u128;
            }
        }
        if result.monitoring_sample {
            self.monitoring_experiments += 1;
        } else {
            self.frontier_experiments += 1;
        }
    }

    pub fn absorb_collapse(
        &mut self,
        result: &ExperimentResult,
        examples: &[(i64, i64)],
        program: Option<&Program>,
        rng: &mut XorShift64,
        audit_rate: u64,
    ) {
        if let ExperimentOutcome::Completed {
            source,
            exact_holdout,
            candidates_evaluated,
            ..
        } = &result.outcome
        {
            if *exact_holdout {
                match *source {
                    "retrieval" => {
                        self.semantic_covered_retrieval += (*candidates_evaluated).max(1) as u128;
                        if let Some(prog) = program {
                            if rng.next() % audit_rate == 0 {
                                if collapse_audit_sample(prog, examples, rng) {
                                    self.collapse_audit_pass += 1;
                                } else {
                                    self.collapse_audit_fail += 1;
                                }
                            }
                        }
                    }
                    "glyph" | "ultra_glyph" => {
                        self.semantic_covered_glyph += 1;
                    }
                    _ => {
                        self.semantic_covered_beam += 1;
                    }
                }
            }
        }
    }
}

fn grab_u64(line: &str, key: &str) -> Option<u64> {
    let i = line.find(key)? + key.len();
    let rest = &line[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-')?;
    rest[..end].parse().ok()
}

fn grab_optional_u64(line: &str, key: &str) -> Option<u64> {
    let i = line.find(key)? + key.len();
    let rest = &line[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-')?;
    rest[..end].parse().ok()
}

fn grab_bool(line: &str, key: &str) -> Option<bool> {
    let i = line.find(key)? + key.len();
    let rest = &line[i..];
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn grab_string(line: &str, key: &str) -> Option<String> {
    let i = line.find(key)? + key.len();
    let rest = &line[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

impl ProgramAnalysis {
    pub fn analyze(program: &Program) -> Self {
        let nodes = program.nodes();
        let mut analysis = ProgramAnalysis::default();
        let mut depth: Vec<u32> = vec![0; nodes.len()];

        for (idx, node) in nodes.iter().enumerate() {
            match node.op {
                Op::ConstF64 => {
                    analysis.f64_const_count += 1;
                    depth[idx] = 1;
                }
                Op::F64Op => {
                    analysis.f64_op_count += 1;
                    if let Ok(sub) = F64SubOp::from_imm(node.imm) {
                        let sel = sub.imm() as usize;
                        if sel < analysis.subop_counts.len() {
                            analysis.subop_counts[sel] += 1;
                        }
                    }
                    let parent_a_depth = depth.get(node.a as usize).copied().unwrap_or(0);
                    let parent_b_depth = if F64SubOp::from_imm(node.imm)
                        .map(|s| s.is_binary())
                        .unwrap_or(false)
                    {
                        depth.get(node.b as usize).copied().unwrap_or(0)
                    } else {
                        parent_a_depth
                    };
                    let bridge = matches!(
                        F64SubOp::from_imm(node.imm),
                        Ok(F64SubOp::FromI64) | Ok(F64SubOp::ToI64)
                    );
                    depth[idx] = if bridge {
                        1
                    } else {
                        parent_a_depth.max(parent_b_depth).saturating_add(1)
                    };
                    analysis.f64_chain_max_depth =
                        analysis.f64_chain_max_depth.max(depth[idx]);
                }
                _ => {
                    depth[idx] = 0;
                }
            }
        }
        analysis
    }

    pub fn uses_f64(&self) -> bool {
        self.f64_op_count > 0 || self.f64_const_count > 0
    }
}

impl GlyphIntel {
    pub fn from_program(
        program: &Program,
        source: &'static str,
        exact_train: bool,
        exact_holdout: bool,
        analysis: &ProgramAnalysis,
    ) -> Self {
        let canonical = program.canonical().ok();
        let canonical_nodes = canonical
            .as_ref()
            .map(|p| p.nodes().len())
            .unwrap_or_else(|| program.nodes().len());
        let canonical_hash = program
            .canonical_hash_hex()
            .map(|h| short_hash(&h))
            .unwrap_or(LEGACY_FP);
        let semantic_fp = program
            .semantic_fingerprint_hex()
            .map(|h| short_hash(&h))
            .unwrap_or(canonical_hash);
        let candidate = source == "beam" && exact_train && exact_holdout;
        let candidate_kind = if candidate {
            "beam_promote"
        } else if source == "structured" && exact_train && exact_holdout {
            "structured_seed"
        } else {
            "none"
        };
        let atlas_region = intern_atlas_region(source, analysis.uses_f64(), analysis.f64_chain_max_depth as u8);
        Self {
            candidate,
            candidate_kind,
            canonical_hash,
            semantic_fp,
            canonical_nodes,
            compression_saved: program.nodes().len().saturating_sub(canonical_nodes),
            atlas_region,
        }
    }
}

fn parse_hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

fn short_hash(hex: &str) -> [u8; 8] {
    let b = hex.as_bytes();
    let mut out = [0u8; 8];
    for i in 0..8 {
        let hi = parse_hex_nibble(*b.get(i * 2).unwrap_or(&b'0'));
        let lo = parse_hex_nibble(*b.get(i * 2 + 1).unwrap_or(&b'0'));
        out[i] = (hi << 4) | lo;
    }
    out
}

fn fmt_hash8(b: [u8; 8]) -> String {
    b.iter().fold(String::with_capacity(16), |mut s, byte| {
        s.push_str(&format!("{byte:02x}"));
        s
    })
}

const LEGACY_FP: [u8; 8] = [0u8; 8];

/// Resultat d'un probe lab autonome porte par `MonsterNode`.
#[derive(Debug, Clone)]
pub struct LabProbeResult {
    pub target_name: &'static str,
    pub config: MonsterEvolutionConfig,
    pub inputs: Vec<i64>,
    pub examples: Vec<(i64, i64)>,
    pub elapsed_ms: u64,
    pub elapsed_us: u64,
    pub outcome: LabProbeStatus,
}

#[derive(Debug)]
pub struct LabExperimentReport {
    pub result: ExperimentResult,
    pub exact_program: Option<Program>,
    pub examples: Vec<(i64, i64)>,
}

#[derive(Debug, Default, Clone)]
pub struct MetaGlyphCounters {
    pub attempts: usize,
    pub hits: usize,
    pub rejects: usize,
    pub dedup_hits: usize,
    pub depth2_hits: usize,
    pub depth3_hits: usize,
}

pub struct ProgramEntry {
    pub target: &'static str,
    pub program: Program,
    pub examples: Vec<(i64, i64)>,
}

/// Statut d'un probe lab. L'erreur reste une chaine pour que le binaire
/// historique conserve son format JSONL exact sans exposer `io::Error`.
#[derive(Debug, Clone)]
pub enum LabProbeStatus {
    Completed(MonsterEvolutionOutcome),
    Errored(String),
}

/// Budget de la boucle autonome `MonsterNode::self_improve`.
#[derive(Debug, Clone)]
pub struct SelfImproveBudget {
    pub mode: SelfImproveMode,
    pub seed: u64,
    pub distill_every: usize,
}

#[derive(Debug, Clone)]
pub enum SelfImproveMode {
    Iterations(usize),
    WallTime(std::time::Duration),
    UntilFingerprint { fingerprint: [u8; 32], max_iterations: usize },
}

impl SelfImproveBudget {
    pub fn iterations(count: usize) -> Self {
        Self {
            mode: SelfImproveMode::Iterations(count),
            seed: 0xF0E6_51E1_F1AB_2026,
            distill_every: 8,
        }
    }

    pub fn wall_time(duration: std::time::Duration) -> Self {
        Self {
            mode: SelfImproveMode::WallTime(duration),
            seed: 0xF0E6_51E1_F1AB_2026,
            distill_every: 8,
        }
    }

    pub fn until_fingerprint(fingerprint: [u8; 32], max_iterations: usize) -> Self {
        Self {
            mode: SelfImproveMode::UntilFingerprint { fingerprint, max_iterations },
            seed: 0xF0E6_51E1_F1AB_2026,
            distill_every: 8,
        }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_distill_every(mut self, distill_every: usize) -> Self {
        self.distill_every = distill_every;
        self
    }
}

/// Rapport compact de l'auto-amÃƒÆ’Ã‚Â©lioration d'un nÃƒâ€¦Ã¢â‚¬Å“ud.
#[derive(Debug, Clone, Default)]
pub struct SelfImproveReport {
    pub probes_run: usize,
    pub exact_train: usize,
    pub exact_holdout: usize,
    pub errored: usize,
    pub candidates_evaluated: usize,
    pub atlas_submitted: usize,
    pub atlas_accepted: usize,
    pub oracle_ticks: usize,
    pub oracles_learned: usize,
    pub elapsed_ms: u64,
    pub discoveries: Vec<SelfImproveDiscovery>,
}

#[derive(Debug, Clone)]
pub struct SelfImproveDiscovery {
    pub target_name: &'static str,
    pub source: &'static str,
    pub program_hash: crate::Hash,
    pub output_fingerprint: [u8; 32],
    pub atlas_accepted: bool,
}

impl MonsterNode {
    /// Execute une experience lab complete depuis une seed.
    pub fn lab_probe(&self, target: TargetTemplate, seed: u64) -> LabProbeResult {
        let mut rng = XorShift64::new(seed);
        let config = random_evolve_config(&mut rng);
        let inputs = build_diverse_inputs(&mut rng);
        self.lab_probe_with(target, config, inputs)
    }

    /// Execute une experience lab avec config/inputs explicites.
    pub fn lab_probe_with(
        &self,
        target: TargetTemplate,
        config: MonsterEvolutionConfig,
        inputs: Vec<i64>,
    ) -> LabProbeResult {
        let examples: Vec<(i64, i64)> = inputs.iter().map(|&x| (x, target.eval(x))).collect();
        let t_start = Instant::now();
        let outcome = self.evolve_i64_program(&examples, config.clone());
        let elapsed = t_start.elapsed();
        LabProbeResult {
            target_name: target.name(),
            config,
            inputs,
            examples,
            elapsed_ms: elapsed.as_millis() as u64,
            elapsed_us: elapsed.as_micros() as u64,
            outcome: match outcome {
                Ok(outcome) => LabProbeStatus::Completed(outcome),
                Err(err) => LabProbeStatus::Errored(err.to_string()),
            },
        }
    }

    /// Execute une iteration complete du lab historique avec shortcut atlas,
    /// probe synthese et promotion locale du programme exact dans l'Atlas L1.
    pub fn lab_run_experiment(
        &self,
        rng: &mut XorShift64,
        iter: usize,
        weights: &FrontierWeights,
        live_atlas: &LiveAtlas,
    ) -> LabExperimentReport {
        let (target, monitoring_sample, frontier_score, wall_family) =
            frontier_target_sample(rng, weights);
        let config = random_evolve_config(rng);
        let inputs = build_diverse_inputs(rng);
        let examples: Vec<(i64, i64)> = inputs.iter().map(|&x| (x, target.eval(x))).collect();

        // Φ.ν.7 — capture target canonical outputs once. Used for the hot
        // path lookup key AND for the atlas insert key after exact_holdout.
        // Indexing by target outputs (not synthesized program outputs)
        // ensures lookup hits even when the program diverges from target
        // outside the probe's holdout inputs.
        let target_canonical_outputs: Vec<i64> = ATLAS_CANONICAL_INPUTS
            .iter()
            .map(|&x| target.eval(x))
            .collect();
        let fp = fnv64_outputs(&target_canonical_outputs);
        if let Some(prog_arc) = live_atlas.lookup_hot(fp) {
            let prog = (*prog_arc).clone();
            let analysis = ProgramAnalysis::analyze(&prog);
            let glyph_intel = GlyphIntel::from_program(&prog, "atlas", true, true, &analysis);
            return LabExperimentReport {
                result: ExperimentResult {
                    iter,
                    target_name: target.name(),
                    config,
                    elapsed_ms: 0,
                    elapsed_us: 0,
                    outcome: ExperimentOutcome::Completed {
                        source: "atlas",
                        exact_train: true,
                        exact_holdout: true,
                        generations_used: 0,
                        candidates_evaluated: 0,
                        program_nodes: prog.nodes().len(),
                        train_loss: 0,
                        holdout_loss: 0,
                        analysis,
                        glyph_intel,
                    },
                    frontier_score,
                    monitoring_sample,
                    wall_family,
                },
                exact_program: Some(prog),
                examples,
            };
        }

        let probe = self.lab_probe_with(target, config, inputs);
        match probe.outcome {
            LabProbeStatus::Completed(outcome) => {
                let analysis = ProgramAnalysis::analyze(&outcome.program);
                let glyph_intel = GlyphIntel::from_program(
                    &outcome.program,
                    outcome.source,
                    outcome.exact_train,
                    outcome.exact_holdout,
                    &analysis,
                );
                let exact_program = if outcome.exact_holdout {
                    // Submit using target canonical outputs (captured before
                    // target was consumed by lab_probe_with). This makes
                    // by_fnv64 / cache keys match the lookup_hot key derived
                    // from `target_fingerprint(target)`.
                    let fp32 = output_fingerprint(&target_canonical_outputs);
                    live_atlas.submit(fp32, &target_canonical_outputs, &outcome.program);
                    Some(outcome.program.clone())
                } else {
                    None
                };
                LabExperimentReport {
                    result: ExperimentResult {
                        iter,
                        target_name: probe.target_name,
                        config: probe.config,
                        elapsed_ms: probe.elapsed_ms,
                        elapsed_us: probe.elapsed_us,
                        outcome: ExperimentOutcome::Completed {
                            source: outcome.source,
                            exact_train: outcome.exact_train,
                            exact_holdout: outcome.exact_holdout,
                            generations_used: outcome.generations,
                            candidates_evaluated: outcome.candidates_evaluated,
                            program_nodes: outcome.program.nodes().len(),
                            train_loss: outcome.train_loss,
                            holdout_loss: outcome.holdout_loss,
                            analysis,
                            glyph_intel,
                        },
                        frontier_score,
                        monitoring_sample,
                        wall_family,
                    },
                    exact_program,
                    examples: probe.examples,
                }
            }
            LabProbeStatus::Errored(message) => LabExperimentReport {
                result: ExperimentResult {
                    iter,
                    target_name: probe.target_name,
                    config: probe.config,
                    elapsed_ms: probe.elapsed_ms,
                    elapsed_us: probe.elapsed_us,
                    outcome: ExperimentOutcome::Errored { message },
                    frontier_score,
                    monitoring_sample,
                    wall_family,
                },
                exact_program: None,
                examples: probe.examples,
            },
        }
    }

    /// Boucle cognitive autonome : le nÃƒâ€¦Ã¢â‚¬Å“ud gÃƒÆ’Ã‚Â©nÃƒÆ’Ã‚Â¨re ses probes, synthÃƒÆ’Ã‚Â©tise,
    /// promeut les programmes exacts, dÃƒÆ’Ã‚Â©clenche l'oracle learning et publie
    /// les fingerprints exacts vers l'Atlas quand une implÃƒÆ’Ã‚Â©mentation existe.
    pub fn self_improve(&self, budget: SelfImproveBudget) -> SelfImproveReport {
        // Φ.ν.7.b — default channel is the LiveAtlas (ATLASV2 inline,
        // RAM-resident programs, no forge.cas dependency). Transient
        // fallback keeps the loop running without persistence.
        let live = LiveAtlas::open(LIVE_ATLAS_PATH).unwrap_or_else(|_| LiveAtlas::transient());
        self.self_improve_with_atlas(budget, &live)
    }

    /// Variante connectÃƒÆ’Ã‚Â©e ÃƒÆ’Ã‚Â  Track A : un Atlas live peut ingÃƒÆ’Ã‚Â©rer les formes
    /// dÃƒÆ’Ã‚Â©couvertes sans crÃƒÆ’Ã‚Â©er de dÃƒÆ’Ã‚Â©pendance circulaire.
    pub fn self_improve_with_atlas(
        &self,
        budget: SelfImproveBudget,
        atlas: &dyn AtlasIngest,
    ) -> SelfImproveReport {
        let started = Instant::now();
        let mut rng = XorShift64::new(budget.seed);
        let mut report = SelfImproveReport::default();
        let distill_config = DistillConfig::default();

        loop {
            if self_improve_budget_exhausted(&budget.mode, report.probes_run, started) {
                break;
            }

            let target = random_target(&mut rng);
            let probe_seed = rng.next();
            let probe = self.lab_probe(target, probe_seed);
            report.probes_run += 1;

            match probe.outcome {
                LabProbeStatus::Completed(ref outcome) => {
                    report.candidates_evaluated = report
                        .candidates_evaluated
                        .saturating_add(outcome.candidates_evaluated);
                    if outcome.exact_train {
                        report.exact_train += 1;
                    }
                    if outcome.exact_holdout {
                        report.exact_holdout += 1;
                        if let Some(discovery) =
                            self.promote_self_improve_outcome(&probe, &outcome, atlas)
                        {
                            if discovery.atlas_accepted {
                                report.atlas_accepted += 1;
                            }
                            report.atlas_submitted += 1;
                            let found = matches!(
                                &budget.mode,
                                SelfImproveMode::UntilFingerprint { fingerprint, .. }
                                    if *fingerprint == discovery.output_fingerprint
                            );
                            report.discoveries.push(discovery);
                            if found {
                                break;
                            }
                        }
                    }
                }
                LabProbeStatus::Errored(_) => {
                    report.errored += 1;
                }
            }

            if budget.distill_every > 0 && report.probes_run % budget.distill_every == 0 {
                report.oracle_ticks += 1;
                report.oracles_learned = report
                    .oracles_learned
                    .saturating_add(self.distill_tick(&distill_config));
            }
        }

        if budget.distill_every > 0 {
            report.oracle_ticks += 1;
            report.oracles_learned = report
                .oracles_learned
                .saturating_add(self.distill_tick(&distill_config));
        }

        report.elapsed_ms = started.elapsed().as_millis() as u64;
        report
    }

    fn promote_self_improve_outcome(
        &self,
        probe: &LabProbeResult,
        outcome: &MonsterEvolutionOutcome,
        atlas: &dyn AtlasIngest,
    ) -> Option<SelfImproveDiscovery> {
        let canonical_outputs = canonical_outputs(&outcome.program)?;
        let output_fingerprint = output_fingerprint(&canonical_outputs);
        for &(x, _) in probe.examples.iter().take(8) {
            let _ = self.call_one_i64(&outcome.program_hash, x);
        }
        let atlas_accepted =
            atlas.submit(output_fingerprint, &canonical_outputs, &outcome.program);
        Some(SelfImproveDiscovery {
            target_name: probe.target_name,
            source: outcome.source,
            program_hash: outcome.program_hash,
            output_fingerprint,
            atlas_accepted,
        })
    }
}

pub fn default_lab_threads() -> usize {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    std::env::var("FORGE_LAB_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| if logical >= 4 { logical / 2 } else { logical })
}

pub fn open_shared_lab_store() -> Arc<Store> {
    let shared_store_path = std::env::current_dir()
        .unwrap()
        .join(".codex-tmp")
        .join("lab-shared");
    Arc::new(Store::open(&shared_store_path).expect("lab shared store open failed"))
}

pub fn spawn_lab_worker(shared_store: Arc<Store>) -> MonsterNode {
    MonsterNode::shared(shared_store, MemoryGovernor::new(16 * 1024 * 1024))
}

fn run_lab_batch_impl(iterations: usize) -> io::Result<()> {
    // V8 c â€” env var FORGE_LAB_THREADS pour expÃ©rimenter ; sinon
    // heuristique = physical cores (logical / 2 sur SMT).
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let num_threads = std::env::var("FORGE_LAB_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| if logical >= 4 { logical / 2 } else { logical });

    println!("=== lab_runner: autonomous experiment loop ===");
    println!("  iterations    : {iterations}");
    println!("  threads       : {num_threads} (parallel iterations)");
    println!("  log path      : {LOG_PATH}");
    println!();

    // Î¦.2.1 â€” Shared persistent Store across threads AND across
    // lab runs. Forge's first promise: known computation = no
    // recomputation. The previous lab created a fresh store per
    // thread under `.codex-tmp/lab-{nanos}-t{thread_id}` and threw
    // it away every run, so 10 000 iterations re-synthesised the
    // same programs from scratch even when they had been computed
    // moments earlier in another thread (or in any prior run).
    //
    // Now: one Arc<Store> at `.codex-tmp/lab-shared`. Threads share
    // it. Successive `lab_runner -- N` invocations reuse the same
    // CAS file, so memos survive process boundaries.
    let shared_store_path = std::env::current_dir()
        .unwrap()
        .join(".codex-tmp")
        .join("lab-shared");
    let shared_store = std::sync::Arc::new(
        Store::open(&shared_store_path).expect("lab shared store open failed"),
    );

    // Obj 2 — load wall scores from last 10 000 JSONL entries.
    let frontier_weights = FrontierWeights::from_recent_log(10_000);

    // Φ.ν.7.b — LiveAtlas: unified ATLASV2 inline atlas. Replaces HotAtlas
    // (custom format) + NoopAtlasIngest stub + the previous Φ.ν.7 forge.cas
    // externalization (D9 amputated for perf — see CARNET). Programs are
    // RAM-resident via `Arc<Program>`; one file (atlas-live.bin) holds the
    // serialized snapshot. Migration from legacy `hot-atlas.bin` runs
    // inside `LiveAtlas::open` if V2 file doesn't exist.
    let live_atlas = std::sync::Arc::new(
        LiveAtlas::open(LIVE_ATLAS_PATH).expect("live atlas open failed"),
    );
    let loaded_atlas_size = live_atlas.len();
    let migrated_count = live_atlas
        .counters
        .migrated
        .load(std::sync::atomic::Ordering::Relaxed);

    let counters_mutex = Mutex::new(LabCounters::default());
    let log_buffers_mutex = Mutex::new(Vec::<String>::new());
    let pool_mutex = Mutex::new(Vec::<ProgramEntry>::new());
    // Φ.μ.4 — atom mining + per-target hit/miss integrated into the official run.
    let atom_registry: Mutex<AtomCatalogueSummary> = Mutex::new(HashMap::new());
    let per_target_miss: Mutex<PerTargetSummary> = Mutex::new(HashMap::new());
    let global_iter_idx = AtomicUsize::new(0);
    let lab_start = Instant::now();

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(num_threads);
        for thread_id in 0..num_threads {
            let counters_mutex = &counters_mutex;
            let atom_registry = &atom_registry;
            let per_target_miss = &per_target_miss;
            let log_buffers_mutex = &log_buffers_mutex;
            let pool_mutex = &pool_mutex;
            let global_iter_idx = &global_iter_idx;
            let frontier_weights = &frontier_weights;
            let live_atlas = live_atlas.clone();
            let shared_store = shared_store.clone();
            handles.push(scope.spawn(move || {
                // Each thread shares the same Arc<Store> — memos
                // written by any thread are visible to all the
                // others (and to subsequent lab runs).
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let monster = MonsterNode::shared(
                    shared_store,
                    MemoryGovernor::new(16 * 1024 * 1024),
                );
                let mut rng = XorShift64::new(
                    nanos as u64
                        ^ ((thread_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
                );

                let mut local_log: Vec<String> = Vec::with_capacity(64);
                let mut local_counters = LabCounters::default();
                let mut local_pool: Vec<ProgramEntry> = Vec::new();
                let mut local_atoms: AtomCatalogueSummary = HashMap::new();
                let mut local_target_miss: PerTargetSummary = HashMap::new();

                loop {
                    let iter = global_iter_idx.fetch_add(1, Ordering::Relaxed);
                    if iter >= iterations {
                        break;
                    }
                    let experiment = monster.lab_run_experiment(&mut rng, iter, frontier_weights, &live_atlas);
                    let result = experiment.result;
                    let opt_prog = experiment.exact_program;
                    let examples = experiment.examples;
                    local_log.push(format_jsonl(&result));
                    local_counters.absorb(&result);
                    local_counters.absorb_collapse(
                        &result, &examples, opt_prog.as_ref(), &mut rng, 32,
                    );
                    // Φ.μ.4 — per-target hit/miss + atom mining (integrated nano_probe)
                    if let ExperimentOutcome::Completed { exact_train, exact_holdout, .. } =
                        &result.outcome
                    {
                        let entry = local_target_miss
                            .entry(result.target_name)
                            .or_insert((0, 0));
                        if *exact_holdout { entry.0 += 1; }
                        else if *exact_train { entry.1 += 1; }
                    }
                    if let Some(prog) = opt_prog {
                        // Mine atoms from exact_holdout programs (verified-correct).
                        let atoms = extract_atoms_v2(&prog);
                        for atom in &atoms {
                            let e = local_atoms
                                .entry(atom.clone())
                                .or_insert_with(|| (HashSet::new(), 0));
                            e.0.insert(result.target_name);
                            e.1 += 1;
                        }
                        // Obj 1 — collect for meta-glyph phase
                        local_pool.push(ProgramEntry {
                            target: result.target_name,
                            program: prog,
                            examples,
                        });
                    }
                }

                // Single drain of this thread's local buffers into the
                // shared sinks (Mutex contention happens once per
                // thread, not once per iter).
                {
                    let mut shared = log_buffers_mutex.lock().unwrap();
                    shared.append(&mut local_log);
                }
                {
                    let mut shared = counters_mutex.lock().unwrap();
                    shared.merge(local_counters);
                }
                {
                    let mut shared = pool_mutex.lock().unwrap();
                    shared.append(&mut local_pool);
                }
                {
                    let mut shared = atom_registry.lock().unwrap();
                    for (atom, (fams, cnt)) in local_atoms {
                        let e = shared.entry(atom).or_insert_with(|| (HashSet::new(), 0));
                        e.0.extend(fams);
                        e.1 += cnt;
                    }
                }
                {
                    let mut shared = per_target_miss.lock().unwrap();
                    for (t, (h, m)) in local_target_miss {
                        let e = shared.entry(t).or_insert((0, 0));
                        e.0 += h; e.1 += m;
                    }
                }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
    });

    // Obj 1 — meta-glyph phase: compose winning programs from the run
    let pool = pool_mutex.into_inner().unwrap();
    let mut meta_rng = XorShift64::new(0xDEADBEEF_CAFE1234);
    let (meta_counters, meta_jsonl) = meta_glyph_phase(pool, &mut meta_rng);

    // Φ.μ.4 — drain atom + miss registries before flushing JSONL.
    let atoms_map = atom_registry.into_inner().unwrap();
    let target_misses = per_target_miss.into_inner().unwrap();
    let now_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Atom catalogue lines (universal atoms ≥ 2 families).
    let mut universal_atoms: Vec<(&String, &(HashSet<&'static str>, usize))> = atoms_map
        .iter()
        .filter(|(_, (fams, _))| fams.len() >= 2)
        .collect();
    universal_atoms.sort_by(|a, b| {
        b.1.0.len().cmp(&a.1.0.len()).then(b.1.1.cmp(&a.1.1))
    });
    let atom_lines: Vec<String> = universal_atoms.iter().map(|(atom, (fams, cnt))| {
        let fam_json = fams.iter()
            .map(|f| format!("\"{}\"", f))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"ts":{now_ts},"source":"atom_catalogue","atom":"{}","family_count":{},"occurrences":{},"families":[{}]}}{NL}"#,
            atom, fams.len(), cnt, fam_json, NL = "\n"
        )
    }).collect();

    // Per-target hit/miss summary lines.
    let target_lines: Vec<String> = target_misses.iter().map(|(target, (h, m))| {
        format!(
            r#"{{"ts":{now_ts},"source":"per_target_summary","target":"{}","hits":{},"misses":{}}}{NL}"#,
            target, h, m, NL = "\n"
        )
    }).collect();

    // Flush the log buffers + meta_glyph + atom catalogue + per-target JSONL in one shot.
    {
        let buffers = log_buffers_mutex.into_inner().unwrap();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOG_PATH)
        {
            for line in &buffers {
                let _ = file.write_all(line.as_bytes());
            }
            for line in &meta_jsonl {
                let _ = file.write_all(line.as_bytes());
            }
            for line in &atom_lines {
                let _ = file.write_all(line.as_bytes());
            }
            for line in &target_lines {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }

    // Φ.ν.7 — flush LiveAtlas (ATLASV2 + forge.cas) explicitly. The Drop
    // impl would also do it, but flushing here lets the summary print the
    // post-flush size and surface any I/O error verbosely. E6 let-it-crash:
    // a flush failure is logged but doesn't poison the summary.
    if let Err(e) = live_atlas.flush() {
        eprintln!("  [warn] live atlas flush failed: {e}");
    }
    let atlas_size = live_atlas.len();
    let atlas_new_forms = atlas_size.saturating_sub(loaded_atlas_size);
    let counters = counters_mutex.into_inner().unwrap();
    println!();
    println!("=== summary ===");
    println!(
        "  total iterations     : {}",
        counters.completed_exact + counters.completed_partial + counters.errored,
    );
    println!("  completed exact      : {}", counters.completed_exact);
    println!(
        "  completed partial    : {}  (train fit, holdout failed â†’ overfitting)",
        counters.completed_partial,
    );
    println!("  errored              : {}", counters.errored);
    println!(
        "  total candidates     : {}",
        counters.total_candidates_evaluated,
    );
    println!("  wall elapsed         : {:.2?}", lab_start.elapsed());
    println!(
        "  iter/sec             : {:.1}",
        iterations as f64 / lab_start.elapsed().as_secs_f64(),
    );
    println!(
        "  candidates/sec       : {:.1}",
        counters.total_candidates_evaluated as f64 / lab_start.elapsed().as_secs_f64(),
    );
    println!(
        "  effective cand/sec   : {:.1}",
        counters.total_candidates_evaluated as f64
            / ((counters.total_elapsed_us.max(1) as f64) / 1_000_000.0),
    );
    println!(
        "  exact retrieval      : {}",
        counters.completed_exact_retrieval,
    );
    println!("  exact glyph          : {}", counters.completed_exact_glyph);
    println!(
        "  exact ultra glyph    : {}",
        counters.completed_exact_ultra_glyph,
    );
    println!(
        "  exact structured     : {}",
        counters.completed_exact_structured,
    );
    println!(
        "  exact evolved        : {}",
        counters.completed_exact_evolved,
    );
    println!("  exact atlas (hot L1) : {}", counters.atlas_hits);
    // V∞ — Beam Fossil metric: % of exact solutions that needed real beam evolution.
    // Target trend: → 0% as atlas/glyph/retrieval absorb everything.
    let beam_fossil_pct = if counters.completed_exact > 0 {
        100.0 * counters.completed_exact_evolved as f64 / counters.completed_exact as f64
    } else {
        0.0
    };
    println!("  beam fossil %%        : {beam_fossil_pct:.1}%  (↓ = Forge absorbs more)");
    println!();
    println!("  per-target intel (holdout / total, avg ms, avg kc/s, avg cand) :");
    let mut entries: Vec<_> = counters.by_target.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (target, stats) in entries {
        let pct = if stats.total > 0 {
            100.0 * (stats.holdout_exact as f64) / (stats.total as f64)
        } else {
            0.0
        };
        let avg_ms = if stats.total > 0 {
            (stats.total_elapsed_us as f64 / stats.total as f64) / 1000.0
        } else {
            0.0
        };
        let avg_candidates = if stats.total > 0 {
            stats.total_candidates_evaluated as f64 / stats.total as f64
        } else {
            0.0
        };
        let avg_rate = if stats.total_elapsed_us > 0 {
            (stats.total_candidates_evaluated as f64 * 1000.0) / stats.total_elapsed_us as f64
        } else {
            0.0
        };
        println!(
            "    {target:<14}  {holdout:>3}/{total:<3} ({pct:>5.1}%)  avg_ms={avg_ms:>7.2}  avg_kc/s={avg_rate:>8.1}  avg_cand={avg_candidates:>8.1}",
            holdout = stats.holdout_exact,
            total = stats.total,
        );
    }
    println!();

    // Î¦.1.5 â€” F64 surface telemetry. Reveals whether the F64 ops
    // introduced in Î¦.0 + the fsqrt-affine recognizer added in Î¦.1
    // are actually being exercised, and at what depth.
    let f64_ratio = if counters.completed_exact + counters.completed_partial > 0 {
        100.0 * (counters.f64_programs_total as f64)
            / ((counters.completed_exact + counters.completed_partial) as f64)
    } else {
        0.0
    };
    println!("  --- F64 surface (Î¦.0 + Î¦.1) ---");
    println!(
        "    f64 programs        : {} / {} ({:.1}% of completed)",
        counters.f64_programs_total,
        counters.completed_exact + counters.completed_partial,
        f64_ratio,
    );
    println!(
        "    total F64Op nodes   : {}  (avg per F64 program: {:.2})",
        counters.f64_ops_total,
        if counters.f64_programs_total > 0 {
            counters.f64_ops_total as f64 / counters.f64_programs_total as f64
        } else {
            0.0
        },
    );
    println!("    sub-op breakdown    :");
    let subop_names = [
        "fadd", "fsub", "fmul", "fdivc", "fmin", "fmax", "fsqrt", "fabs", "fneg",
        "i64_to_f64", "f64_to_i64", "fexp", "fln",
    ];
    for (idx, name) in subop_names.iter().enumerate() {
        let count = counters.subop_totals[idx];
        if count > 0 {
            println!("      {name:<12} : {count}");
        }
    }
    println!("    chain-depth histo   :");
    for (idx, count) in counters.chain_depth_hist.buckets.iter().enumerate() {
        if *count > 0 {
            let label = if idx == 5 { "5+".to_string() } else { idx.to_string() };
            println!("      depth = {label:<3} : {count}");
        }
    }
    println!(
        "    F64 by source       : retrieval={}  glyph={}  ultra_glyph={}  other={}",
        counters.f64_by_source[0],
        counters.f64_by_source[1],
        counters.f64_by_source[2],
        counters.f64_by_source[3],
    );
    println!();

    // Î¦.2.1 â€” Memoization telemetry. The cache hit ratio is the
    // single most important number this lab produces: it tells us
    // how much of Forge's "known computation = no recomputation"
    // promise is actually being delivered.
    let total_finalised = counters.cache_hits + counters.cache_misses;
    println!("  --- Memoization (Î¦.2.1) ---");
    println!(
        "    cache hits       : {}  cache misses : {}  total : {}",
        counters.cache_hits, counters.cache_misses, total_finalised,
    );
    if total_finalised > 0 {
        let hit_pct = 100.0 * (counters.cache_hits as f64) / (total_finalised as f64);
        println!("    cache hit ratio  : {hit_pct:.1}%");
    }
    if counters.cache_hits > 0 {
        let avg_hit_us =
            counters.cache_hit_elapsed_us as f64 / counters.cache_hits as f64;
        println!("    avg hit elapsed  : {avg_hit_us:.1} Âµs (memo lookup + verify)");
    }

    println!();
    println!("  --- Glyph OS telemetry ---");
    println!(
        "    compiler queue    : beam_candidates={}  structured_seeds={}  nodes_saved={}",
        counters.glyph_candidates,
        counters.glyph_structured_seeds,
        counters.glyph_compression_saved,
    );
    let mut market_rows: Vec<_> = counters.glyph_market.iter().collect();
    market_rows.sort_by(|a, b| {
        glyph_market_score(b.1)
            .partial_cmp(&glyph_market_score(a.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!("    market top        : semantic_fp       score   hit%  total cand redun avg_us");
    for (fp, stats) in market_rows.into_iter().take(5) {
        let fp_s = fmt_hash8(*fp);
        let hit_pct = if stats.total > 0 {
            100.0 * stats.holdout_exact as f64 / stats.total as f64
        } else {
            0.0
        };
        let avg_us = if stats.total > 0 {
            stats.total_elapsed_us as f64 / stats.total as f64
        } else {
            0.0
        };
        println!(
            "      {fp_s:<16} {score:>7.1} {hit_pct:>5.1} {total:>6} {cand:>4} {redun:>5} {avg_us:>6.1}",
            score = glyph_market_score(stats),
            total = stats.total,
            cand = stats.candidates + stats.structured_seeds,
            redun = stats.canonical_hashes.len(),
        );
    }
    let mut atlas_rows: Vec<_> = counters.semantic_atlas.iter().collect();
    atlas_rows.sort_by(|a, b| b.1.total.cmp(&a.1.total));
    println!("    atlas regions     : region              total hit% unique_sem redun cand avg_us");
    for (region, stats) in atlas_rows.into_iter().take(8) {
        let hit_pct = if stats.total > 0 {
            100.0 * stats.holdout_exact as f64 / stats.total as f64
        } else {
            0.0
        };
        let avg_us = if stats.total > 0 {
            stats.total_elapsed_us as f64 / stats.total as f64
        } else {
            0.0
        };
        println!(
            "      {region:<19} {total:>5} {hit_pct:>5.1} {sem:>10} {redun:>5} {cand:>4} {avg_us:>6.1}",
            total = stats.total,
            sem = stats.semantic_fps.len(),
            redun = stats.canonical_hashes.len(),
            cand = stats.candidates,
        );
    }
    // Obj 2 — Frontier Lab summary
    let total_exps = counters.frontier_experiments + counters.monitoring_experiments;
    if total_exps > 0 {
        println!();
        println!("  --- Obj 2: Frontier Lab ---");
        let frontier_pct = 100.0 * counters.frontier_experiments as f64 / total_exps as f64;
        let monitor_pct = 100.0 * counters.monitoring_experiments as f64 / total_exps as f64;
        println!(
            "    frontier (hard walls) : {}  ({:.1}%)",
            counters.frontier_experiments, frontier_pct,
        );
        println!(
            "    monitoring (random)   : {}  ({:.1}%)",
            counters.monitoring_experiments, monitor_pct,
        );
    }

    // Obj 3 — Iteration Collapse summary
    let real_iter = counters.completed_exact
        + counters.completed_partial
        + counters.errored;
    let semantic_covered = counters.semantic_covered_retrieval
        + counters.semantic_covered_glyph
        + counters.semantic_covered_beam;
    if semantic_covered > 0 || counters.collapse_audit_pass + counters.collapse_audit_fail > 0 {
        println!();
        println!("  --- Obj 3: Iteration Collapse (V∞) ---");
        println!("    real iterations          : {real_iter}");
        println!("    semantic covered total   : {semantic_covered}");
        println!(
            "      retrieval (candidates): {}",
            counters.semantic_covered_retrieval,
        );
        println!(
            "      glyph/ultra_glyph     : {}",
            counters.semantic_covered_glyph,
        );
        println!(
            "      beam/other            : {}",
            counters.semantic_covered_beam,
        );
        if real_iter > 0 {
            let collapse_factor =
                semantic_covered as f64 / real_iter as f64;
            println!("    collapse_factor          : {collapse_factor:.2}×");
        }
        let audit_total = counters.collapse_audit_pass + counters.collapse_audit_fail;
        if audit_total > 0 {
            let pass_pct =
                100.0 * counters.collapse_audit_pass as f64 / audit_total as f64;
            println!(
                "    audit (spot-check 1/32)  : {}/{} pass ({:.1}%)",
                counters.collapse_audit_pass, audit_total, pass_pct,
            );
        }
    }

    // Obj 1 — MetaGlyph summary
    if meta_counters.attempts > 0 {
        println!();
        println!("  --- Obj 1: MetaGlyph ---");
        println!("    attempts     : {}", meta_counters.attempts);
        println!("    hits         : {}", meta_counters.hits);
        println!("    rejects      : {}", meta_counters.rejects);
        println!("    dedup_hits   : {}", meta_counters.dedup_hits);
        println!("    depth-2 hits : {}", meta_counters.depth2_hits);
        println!("    depth-3 hits : {}", meta_counters.depth3_hits);
        if meta_counters.attempts > 0 {
            let hit_rate =
                100.0 * meta_counters.hits as f64 / meta_counters.attempts as f64;
            println!("    hit rate     : {hit_rate:.1}%");
        }
    }

    // Φ.ν.7.b — LiveAtlas summary (ATLASV2 inline, RAM-resident programs)
    println!();
    println!("  --- Φ.ν.7 LiveAtlas (ATLASV2 inline, RAM-resident, fp32 dedup) ---");
    if migrated_count > 0 {
        println!("    migrated from hot-atlas.bin: {migrated_count}  (one-shot Φ.ν.7 migration)");
    }
    println!("    atlas loaded (prev runs)   : {loaded_atlas_size}");
    println!("    atlas new forms this run   : {atlas_new_forms}");
    println!("    atlas size total           : {atlas_size}");
    println!("    atlas hits (skipped synth) : {}", counters.atlas_hits);
    let live_submitted = live_atlas
        .counters
        .submitted
        .load(std::sync::atomic::Ordering::Relaxed);
    let live_accepted = live_atlas
        .counters
        .accepted
        .load(std::sync::atomic::Ordering::Relaxed);
    let live_dedup = live_atlas
        .counters
        .dedup_rejects
        .load(std::sync::atomic::Ordering::Relaxed);
    let live_hits = live_atlas
        .counters
        .hits_hot
        .load(std::sync::atomic::Ordering::Relaxed);
    println!("    AtlasIngest submitted      : {live_submitted}");
    println!("    AtlasIngest accepted (new) : {live_accepted}");
    println!("    AtlasIngest dedup rejects  : {live_dedup}");
    println!("    hot-path lookup hits       : {live_hits}");
    println!("    flush path                 : {LIVE_ATLAS_PATH}");
    let total_exact = counters.completed_exact;
    if total_exact > 0 {
        let atlas_coverage =
            100.0 * counters.atlas_hits as f64 / total_exact as f64;
        let shortcut_pct = 100.0
            * (counters.atlas_hits
                + counters.completed_exact_retrieval
                + counters.completed_exact_glyph
                + counters.completed_exact_ultra_glyph
                + counters.completed_exact_structured) as f64
            / total_exact as f64;
        println!("    atlas coverage / exact     : {atlas_coverage:.1}%");
        println!("    shortcut coverage / exact  : {shortcut_pct:.1}%  (no beam needed)");
        println!("    beam fossil / exact        : {beam_fossil_pct:.1}%  (↓ → beam extinction)");
    }

    // Φ.μ.4 — per-target hit/miss breakdown
    println!();
    println!("  --- Per-target hit/miss (Φ.μ.4 integrated nano_miss) ---");
    println!("  {:<32} {:>7} {:>7}  miss%", "target", "hits", "misses");
    let mut miss_rows: Vec<_> = target_misses.iter().collect();
    miss_rows.sort_by_key(|(_, (_h, m))| std::cmp::Reverse(*m));
    for (target, (h, m)) in &miss_rows {
        let total = h + m;
        if total == 0 { continue; }
        let miss_pct = (*m as f64 / total as f64) * 100.0;
        println!("  {:<32} {:>7} {:>7}  {:.1}%", target, h, m, miss_pct);
    }

    // Φ.μ.4 — atom catalogue (top universal sub-computations this run)
    println!();
    println!("  --- Atom catalogue (≥ 2 families, top 30 by family-breadth) ---");
    println!("  {:<44} {:>8}  {:>5}", "atom", "occ", "fams");
    for (atom, (fams, cnt)) in universal_atoms.iter().take(30) {
        println!("  {:<44} {:>8}  {:>5}", atom, cnt, fams.len());
    }
    println!("  total atoms discovered          : {}", atoms_map.len());
    println!("  universal atoms (≥ 2 families)  : {}", universal_atoms.len());

    println!();
    println!("  full log : {LOG_PATH}");
    Ok(())
}

// ============================================================
// Mode: analyze JSONL
// ============================================================

fn analyze_lab_log_impl(limit: Option<usize>) -> io::Result<()> {
    let file = File::open(LOG_PATH)?;
    let reader = BufReader::new(file);
    let mut entries: Vec<LogEntry> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Some(e) = parse_jsonl_line(&line) { entries.push(e); }
    }
    if let Some(limit) = limit {
        let start = entries.len().saturating_sub(limit);
        entries.drain(0..start);
    }
    println!("=== lab_runner analyze : {} entries from {} ===", entries.len(), LOG_PATH);
    println!();

    let mut by_target: HashMap<&str, TargetCounters> = HashMap::new();
    let mut by_max_nodes: HashMap<u32, TargetCounters> = HashMap::new();
    let mut completed = 0usize;
    let mut exact_structured = 0usize;
    let mut exact_evolved = 0usize;
    let mut exact_retrieval = 0usize;
    let mut exact_glyph = 0usize;
    let mut exact_ultra_glyph = 0usize;
    let mut exact_memo = 0usize;
    let mut total_candidates = 0u128;
    let mut total_elapsed_us = 0u128;
    let mut all_lat_us: Vec<u64> = Vec::new();
    let mut all_candidate_rates: Vec<u64> = Vec::new();
    let mut by_source: HashMap<&'static str, usize> = HashMap::new();
    let mut glyph_candidates = 0usize;
    let mut glyph_structured_seeds = 0usize;
    let mut glyph_compression_saved = 0usize;
    let mut glyph_market: HashMap<[u8; 8], MarketCounters> = HashMap::new();
    let mut semantic_atlas: HashMap<&'static str, AtlasCounters> = HashMap::new();
    for e in &entries {
        let slot = by_target.entry(e.target).or_default();
        slot.total += 1;
        slot.total_elapsed_us += e.elapsed_us as u128;
        let by_nodes = by_max_nodes.entry(e.max_nodes).or_default();
        by_nodes.total += 1;
        by_nodes.total_elapsed_us += e.elapsed_us as u128;
        all_lat_us.push(e.elapsed_us);
        match &e.outcome {
            LogOutcome::Completed {
                source,
                exact_train,
                exact_holdout,
                generations_used,
                program_nodes,
                candidates_evaluated,
                candidates_per_sec,
                glyph_candidate,
                glyph_kind,
                canonical_hash,
                semantic_fp,
                canonical_nodes,
                compression_saved,
                atlas_region,
                ..
            } => {
                completed += 1;
                *by_source.entry(*source).or_insert(0) += 1;
                total_candidates += *candidates_evaluated as u128;
                total_elapsed_us += e.elapsed_us as u128;
                all_candidate_rates.push(*candidates_per_sec);
                slot.total_candidates_evaluated += *candidates_evaluated as u128;
                slot.total_program_nodes += *program_nodes as u128;
                by_nodes.total_candidates_evaluated += *candidates_evaluated as u128;
                by_nodes.total_program_nodes += *program_nodes as u128;
                if *exact_holdout {
                    slot.holdout_exact += 1;
                    by_nodes.holdout_exact += 1;
                    if *source == "retrieval" {
                        exact_retrieval += 1;
                    } else if *source == "glyph" {
                        exact_glyph += 1;
                    } else if *source == "ultra_glyph" {
                        exact_ultra_glyph += 1;
                    } else if *source == "memo" {
                        exact_memo += 1;
                    } else if *generations_used == 0 {
                        exact_structured += 1;
                    } else {
                        exact_evolved += 1;
                    }
                } else if *exact_train {
                    slot.train_only += 1;
                    by_nodes.train_only += 1;
                }
                if *semantic_fp != LEGACY_FP {
                    if *glyph_candidate {
                        glyph_candidates += 1;
                        glyph_compression_saved += *compression_saved as usize;
                    } else if *glyph_kind == "structured_seed" {
                        glyph_structured_seeds += 1;
                    }
                    let market = glyph_market.entry(*semantic_fp).or_default();
                    market.total += 1;
                    if *exact_holdout {
                        market.holdout_exact += 1;
                    } else if *exact_train {
                        market.train_only += 1;
                    }
                    if *glyph_candidate {
                        market.candidates += 1;
                    } else if *glyph_kind == "structured_seed" {
                        market.structured_seeds += 1;
                    }
                    market.total_elapsed_us += e.elapsed_us as u128;
                    market.total_nodes += *program_nodes as u128;
                    market.total_canonical_nodes += *canonical_nodes as u128;
                    market.canonical_hashes.insert(*canonical_hash);

                    let atlas = semantic_atlas.entry(*atlas_region).or_default();
                    atlas.total += 1;
                    if *exact_holdout {
                        atlas.holdout_exact += 1;
                    } else if *exact_train {
                        atlas.train_only += 1;
                    }
                    if *glyph_candidate {
                        atlas.candidates += 1;
                    }
                    atlas.semantic_fps.insert(*semantic_fp);
                    atlas.canonical_hashes.insert(*canonical_hash);
                    atlas.total_elapsed_us += e.elapsed_us as u128;
                }
            }
            LogOutcome::Errored(_) => {
                slot.errored += 1;
                by_nodes.errored += 1;
            }
        }
    }
    all_lat_us.sort_unstable();
    all_candidate_rates.sort_unstable();
    println!("--- global throughput ---");
    println!("  completed            : {completed}");
    if !by_source.is_empty() {
        let mut sources: Vec<_> = by_source.iter().collect();
        sources.sort_by(|a, b| a.0.cmp(b.0));
        for (source, count) in sources {
            println!("  source[{source:<10}] : {count}");
        }
    }
    println!("  exact retrieval      : {exact_retrieval}");
    println!("  exact glyph          : {exact_glyph}");
    println!("  exact ultra glyph    : {exact_ultra_glyph}");
    println!("  exact memo           : {exact_memo}");
    println!("  exact structured     : {exact_structured}");
    println!("  exact evolved        : {exact_evolved}");
    println!("  total candidates     : {total_candidates}");
    if total_elapsed_us > 0 {
        println!(
            "  effective cand/sec   : {:.1}",
            total_candidates as f64 / (total_elapsed_us as f64 / 1_000_000.0),
        );
    }
    println!();
    println!("--- hit rate per target ---");
    println!("  target          | total | holdout | train_only | errored | %holdout | avg_ms | avg_kc/s | avg_cand");
    let mut ts: Vec<_> = by_target.iter().collect();
    ts.sort_by(|a, b| a.0.cmp(b.0));
    for (target, stats) in ts {
        let pct = if stats.total > 0 {
            100.0 * (stats.holdout_exact as f64) / (stats.total as f64)
        } else {
            0.0
        };
        let avg_ms = if stats.total > 0 {
            (stats.total_elapsed_us as f64 / stats.total as f64) / 1000.0
        } else {
            0.0
        };
        let avg_kcps = if stats.total_elapsed_us > 0 {
            (stats.total_candidates_evaluated as f64 * 1000.0) / stats.total_elapsed_us as f64
        } else {
            0.0
        };
        let avg_cand = if stats.total > 0 {
            stats.total_candidates_evaluated as f64 / stats.total as f64
        } else {
            0.0
        };
        println!(
            "  {target:<15} | {total:>5} | {holdout:>7} | {train_only:>10} | {errored:>7} | {pct:>6.1}% | {avg_ms:>6.1} | {avg_kcps:>8.1} | {avg_cand:>8.1}",
            total = stats.total,
            holdout = stats.holdout_exact,
            train_only = stats.train_only,
            errored = stats.errored,
        );
    }
    println!();

    println!("--- elapsed distribution ---");
    println!(
        "  p50={:.2}ms  p95={:.2}ms  p99={:.2}ms  max={:.2}ms",
        percentile(&all_lat_us, 0.50) as f64 / 1000.0,
        percentile(&all_lat_us, 0.95) as f64 / 1000.0,
        percentile(&all_lat_us, 0.99) as f64 / 1000.0,
        all_lat_us.last().copied().unwrap_or(0) as f64 / 1000.0,
    );
    println!();

    println!("--- candidate throughput distribution ---");
    println!(
        "  p50={} kc/s  p95={} kc/s  p99={} kc/s  max={} kc/s",
        format_kcps(percentile(&all_candidate_rates, 0.50)),
        format_kcps(percentile(&all_candidate_rates, 0.95)),
        format_kcps(percentile(&all_candidate_rates, 0.99)),
        format_kcps(all_candidate_rates.last().copied().unwrap_or(0)),
    );
    println!();

    println!("--- by max_nodes ---");
    println!("  max_nodes | total | holdout | train_only | errored | avg_ms | avg_kc/s | avg_cand");
    let mut max_nodes_rows: Vec<_> = by_max_nodes.iter().collect();
    max_nodes_rows.sort_by_key(|(max_nodes, _)| **max_nodes);
    for (max_nodes, stats) in max_nodes_rows {
        let avg_ms = if stats.total > 0 {
            (stats.total_elapsed_us as f64 / stats.total as f64) / 1000.0
        } else {
            0.0
        };
        let avg_kcps = if stats.total_elapsed_us > 0 {
            (stats.total_candidates_evaluated as f64 * 1000.0) / stats.total_elapsed_us as f64
        } else {
            0.0
        };
        let avg_cand = if stats.total > 0 {
            stats.total_candidates_evaluated as f64 / stats.total as f64
        } else {
            0.0
        };
        println!(
            "  {max_nodes:>9} | {total:>5} | {holdout:>7} | {train_only:>10} | {errored:>7} | {avg_ms:>6.1} | {avg_kcps:>8.1} | {avg_cand:>8.1}",
            total = stats.total,
            holdout = stats.holdout_exact,
            train_only = stats.train_only,
            errored = stats.errored,
        );
    }
    println!();

    println!("--- Glyph OS telemetry ---");
    println!(
        "  compiler queue : beam_candidates={}  structured_seeds={}  nodes_saved={}",
        glyph_candidates, glyph_structured_seeds, glyph_compression_saved,
    );
    let mut market_rows: Vec<_> = glyph_market.iter().collect();
    market_rows.sort_by(|a, b| {
        glyph_market_score(b.1)
            .partial_cmp(&glyph_market_score(a.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!("  market top     | semantic_fp       | score  | hit%  | total | cand | redun | avg_us");
    for (fp, stats) in market_rows.into_iter().take(10) {
        let fp_s = fmt_hash8(*fp);
        let hit_pct = if stats.total > 0 {
            100.0 * stats.holdout_exact as f64 / stats.total as f64
        } else {
            0.0
        };
        let avg_us = if stats.total > 0 {
            stats.total_elapsed_us as f64 / stats.total as f64
        } else {
            0.0
        };
        println!(
            "  glyph          | {fp_s:<16} | {score:>6.1} | {hit_pct:>5.1} | {total:>5} | {cand:>4} | {redun:>5} | {avg_us:>6.1}",
            score = glyph_market_score(stats),
            total = stats.total,
            cand = stats.candidates + stats.structured_seeds,
            redun = stats.canonical_hashes.len(),
        );
    }
    let mut atlas_rows: Vec<_> = semantic_atlas.iter().collect();
    atlas_rows.sort_by(|a, b| b.1.total.cmp(&a.1.total));
    println!("  atlas          | region              | total | hit%  | unique_sem | redun | cand | avg_us");
    for (region, stats) in atlas_rows.into_iter().take(12) {
        let hit_pct = if stats.total > 0 {
            100.0 * stats.holdout_exact as f64 / stats.total as f64
        } else {
            0.0
        };
        let avg_us = if stats.total > 0 {
            stats.total_elapsed_us as f64 / stats.total as f64
        } else {
            0.0
        };
        println!(
            "  region         | {region:<19} | {total:>5} | {hit_pct:>5.1} | {sem:>10} | {redun:>5} | {cand:>4} | {avg_us:>6.1}",
            total = stats.total,
            sem = stats.semantic_fps.len(),
            redun = stats.canonical_hashes.len(),
            cand = stats.candidates,
        );
    }
    println!();

    println!("--- slowest experiments ---");
    let mut slowest: Vec<&LogEntry> = entries.iter().collect();
    slowest.sort_by(|a, b| b.elapsed_us.cmp(&a.elapsed_us));
    for entry in slowest.into_iter().take(8) {
        match &entry.outcome {
            LogOutcome::Completed {
                source,
                exact_train,
                exact_holdout,
                generations_used,
                candidates_evaluated,
                candidates_per_sec,
                ..
            } => {
                println!(
                    "  iter={:<4} target={:<12} source={:<10} max_nodes={} holdout={} train={} gens_used={} elapsed_ms={:>7.2} cand={} kc/s={}",
                    entry.iter,
                    entry.target,
                    source,
                    entry.max_nodes,
                    exact_holdout,
                    exact_train,
                    generations_used,
                    entry.elapsed_us as f64 / 1000.0,
                    candidates_evaluated,
                    format_kcps(*candidates_per_sec),
                );
            }
            LogOutcome::Errored(msg) => {
                println!(
                    "  iter={:<4} target={:<12} max_nodes={} errored elapsed_ms={:>7.2} msg={}",
                    entry.iter,
                    entry.target,
                    entry.max_nodes,
                    entry.elapsed_us as f64 / 1000.0,
                    msg,
                );
            }
        }
    }
    println!();

    let mut errors: HashMap<String, usize> = HashMap::new();
    for e in &entries {
        if let LogOutcome::Errored(msg) = &e.outcome {
            *errors.entry(msg.clone()).or_insert(0) += 1;
        }
    }
    if !errors.is_empty() {
        println!("--- unique error messages ---");
        let mut es: Vec<_> = errors.iter().collect();
        es.sort_by(|a, b| b.1.cmp(a.1));
        for (msg, count) in es {
            println!("  {count:>4}Ã—  {msg}");
        }
        println!();
    }

    // Î¦.1.5b â€” F64 surface dimensions extracted from JSONL. Reveals
    // where the F64 footprint actually lives across a run AND lets us
    // compare F64-vs-I64-only program latencies â€” the comparison the
    // run-time summary alone can't expose.
    let mut f64_lat_us: Vec<u64> = Vec::new();
    let mut i64_lat_us: Vec<u64> = Vec::new();
    let mut f64_holdout_pass = 0usize;
    let mut f64_holdout_fail = 0usize;
    let mut f64_chain_hist: [u32; 8] = [0; 8]; // 0..=6, 7+
    let mut f64_per_target: HashMap<&'static str, (usize, usize)> = HashMap::new(); // (programs, holdout_exact)
    for e in &entries {
        if let LogOutcome::Completed { exact_holdout, f64_ops, f64_chain_max, .. } = &e.outcome {
            if *f64_ops > 0 {
                f64_lat_us.push(e.elapsed_us);
                if *exact_holdout {
                    f64_holdout_pass += 1;
                } else {
                    f64_holdout_fail += 1;
                }
                let bucket = (*f64_chain_max).min(7) as usize;
                f64_chain_hist[bucket] = f64_chain_hist[bucket].saturating_add(1);
                let entry = f64_per_target.entry(e.target).or_default();
                entry.0 += 1;
                if *exact_holdout {
                    entry.1 += 1;
                }
            } else {
                i64_lat_us.push(e.elapsed_us);
            }
        }
    }
    f64_lat_us.sort_unstable();
    i64_lat_us.sort_unstable();

    if !f64_lat_us.is_empty() || !i64_lat_us.is_empty() {
        println!("--- F64 vs I64 latency distribution ---");
        if !f64_lat_us.is_empty() {
            println!(
                "  F64-using ({:>4} progs)  p50={:.2}ms  p95={:.2}ms  p99={:.2}ms  max={:.2}ms",
                f64_lat_us.len(),
                percentile(&f64_lat_us, 0.50) as f64 / 1000.0,
                percentile(&f64_lat_us, 0.95) as f64 / 1000.0,
                percentile(&f64_lat_us, 0.99) as f64 / 1000.0,
                f64_lat_us.last().copied().unwrap_or(0) as f64 / 1000.0,
            );
        }
        if !i64_lat_us.is_empty() {
            println!(
                "  I64-only  ({:>4} progs)  p50={:.2}ms  p95={:.2}ms  p99={:.2}ms  max={:.2}ms",
                i64_lat_us.len(),
                percentile(&i64_lat_us, 0.50) as f64 / 1000.0,
                percentile(&i64_lat_us, 0.95) as f64 / 1000.0,
                percentile(&i64_lat_us, 0.99) as f64 / 1000.0,
                i64_lat_us.last().copied().unwrap_or(0) as f64 / 1000.0,
            );
        }
        println!();

        if f64_holdout_pass + f64_holdout_fail > 0 {
            let pct = 100.0 * (f64_holdout_pass as f64)
                / ((f64_holdout_pass + f64_holdout_fail) as f64);
            println!("--- F64 holdout outcome ---");
            println!(
                "  pass = {f64_holdout_pass}  fail = {f64_holdout_fail}  ratio = {pct:.1}%",
            );
            println!();
        }

        let f64_targets_present = !f64_per_target.is_empty();
        if f64_targets_present {
            println!("--- F64 programs per target ---");
            let mut rows: Vec<_> = f64_per_target.iter().collect();
            rows.sort_by(|a, b| a.0.cmp(b.0));
            for (target, (count, holdout)) in rows {
                let pct = if *count > 0 {
                    100.0 * (*holdout as f64) / (*count as f64)
                } else {
                    0.0
                };
                println!(
                    "  {target:<22} programs={count:>4}  holdout_exact={holdout:>4} ({pct:>5.1}%)",
                );
            }
            println!();

            println!("--- F64 chain depth distribution ---");
            for (depth, count) in f64_chain_hist.iter().enumerate() {
                if *count > 0 {
                    let label = if depth == 7 { "7+".to_string() } else { depth.to_string() };
                    println!("  depth = {label:<3} : {count}");
                }
            }
            println!();
        }
    }

    // Î¦.8 â€” Ultra wall probes : 3 sections dÃ©diÃ©es qui exposent
    // exactement ce que la frontiÃ¨re de Forge **ne** sait pas faire.
    // Chaque section liste les targets concernÃ©s, leur miss rate,
    // et la moyenne de candidats brÃ»lÃ©s. Les chiffres suivent les
    // commits : un Î¦.X qui couvre un nouveau pic les fait chuter.
    let domain_targets = [
        "domain_michaelis_menten",
        "domain_michaelis_menten_cooperative",
        "domain_sirtuin_nad_dependent_activity",
        "domain_mtor_signaling_balance",
        "domain_nad_depletion_recovery",
        "domain_p53_activation_threshold",
        "domain_hill_n2",
        "domain_arrhenius",
        "domain_arrhenius_kelvin",
        "domain_inverse_square",
        "domain_logistic",
        "domain_beer_lambert_linear",
    ];
    let domain_present = domain_targets
        .iter()
        .any(|name| by_target.contains_key(*name));
    if domain_present {
        println!("--- Î¦.8 Lever 2 : real-world domain probes ---");
        println!("  formula                     | total | holdout | %hit  | avg_ms");
        for name in domain_targets {
            if let Some(stats) = by_target.get(name) {
                let pct = if stats.total > 0 {
                    100.0 * (stats.holdout_exact as f64) / (stats.total as f64)
                } else {
                    0.0
                };
                let avg_ms = if stats.total > 0 {
                    (stats.total_elapsed_us as f64 / stats.total as f64) / 1000.0
                } else {
                    0.0
                };
                println!(
                    "  {name:<27} | {total:>5} | {holdout:>7} | {pct:>4.1}% | {avg_ms:>6.1}",
                    total = stats.total,
                    holdout = stats.holdout_exact,
                );
            }
        }
        println!();
    }

    if let Some(noise_stats) = by_target.get("wall_noisy_fsqrt_affine") {
        let pct = if noise_stats.total > 0 {
            100.0 * (noise_stats.holdout_exact as f64) / (noise_stats.total as f64)
        } else {
            0.0
        };
        println!("--- Φ.8 Lever 3 : adversarial noise ---");
        println!(
            "  wall_noisy_fsqrt_affine      | {total:>5} | holdout={holdout:>4} ({pct:>4.1}%)",
            total = noise_stats.total,
            holdout = noise_stats.holdout_exact,
        );
        if noise_stats.holdout_exact == 0 {
            println!("  → 0% = no noise-tolerant recognizer exists yet");
        } else if pct < 50.0 {
            println!("  → recognizer emerging, but wall still below the 50% gate");
        } else {
            println!("  → recognizer active: sparse-outlier wall is now cleared above the 50% gate");
        }
        println!();
    }

    if let Some(rk_stats) = by_target.get("wall_random_kasm") {
        let pct = if rk_stats.total > 0 {
            100.0 * (rk_stats.holdout_exact as f64) / (rk_stats.total as f64)
        } else {
            0.0
        };
        let avg_ms = if rk_stats.total > 0 {
            (rk_stats.total_elapsed_us as f64 / rk_stats.total as f64) / 1000.0
        } else {
            0.0
        };
        println!("--- Î¦.8 Lever 1 : reachability map (random KASM) ---");
        println!(
            "  wall_random_kasm             | {total:>5} | holdout={holdout:>4} ({pct:>4.1}%) | avg_ms={avg_ms:>5.1}",
            total = rk_stats.total,
            holdout = rk_stats.holdout_exact,
        );
        println!("  â†’ fraction of random KASM programs Forge can recover");
        println!();
    }

    // Φ.μ.4 — atom catalogue + per-target summary aggregated across all runs.
    // Re-scan the file with simple string parsing for atom_catalogue and
    // per_target_summary entries (parse_jsonl_line skips them).
    let mut per_atom: HashMap<String, (HashSet<&'static str>, usize)> = HashMap::new();
    let mut per_target: PerTargetSummary = HashMap::new();
    if let Ok(file) = File::open(LOG_PATH) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let get = |key: &str| -> Option<String> {
                let needle = format!("\"{}\":", key);
                let p = line.find(&needle)?;
                let rest = &line[p + needle.len()..];
                let rest = rest.trim_start();
                if rest.starts_with('"') {
                    let after = &rest[1..];
                    let end = after.find('"')?;
                    Some(after[..end].to_string())
                } else {
                    let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
                    Some(rest[..end].trim().to_string())
                }
            };
            let Some(source) = get("source") else { continue; };
            match source.as_str() {
                "atom_catalogue" => {
                    let Some(atom) = get("atom") else { continue; };
                    let occ: usize = get("occurrences").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let fams_marker = "\"families\":[";
                    if let Some(p) = line.find(fams_marker) {
                        let after = &line[p + fams_marker.len()..];
                        let end = after.find(']').unwrap_or(after.len());
                        let fams: HashSet<&'static str> = after[..end]
                            .split(',')
                            .filter_map(|s| {
                                let t = s.trim().trim_matches('"');
                                if t.is_empty() { None } else { Some(intern_target_name(t)) }
                            })
                            .collect();
                        let e = per_atom.entry(atom).or_insert_with(|| (HashSet::new(), 0));
                        e.0.extend(fams);
                        e.1 = e.1.max(occ);
                    }
                }
                "per_target_summary" => {
                    let Some(target) = get("target") else { continue; };
                    let h: usize = get("hits").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let m: usize = get("misses").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let e = per_target.entry(intern_target_name(&target)).or_insert((0, 0));
                    e.0 += h; e.1 += m;
                }
                _ => {}
            }
        }
    }

    if !per_target.is_empty() {
        println!("--- Φ.μ.4 per-target hit/miss (cumulative across runs) ---");
        println!("  {:<32} {:>8} {:>8}  miss%", "target", "hits", "misses");
        let mut rows: Vec<_> = per_target.iter().collect();
        rows.sort_by_key(|(_, (_, m))| std::cmp::Reverse(*m));
        for (target, (h, m)) in &rows {
            let total = h + m;
            if total == 0 { continue; }
            let miss_pct = (*m as f64 / total as f64) * 100.0;
            println!("  {:<32} {:>8} {:>8}  {:.1}%", target, h, m, miss_pct);
        }
        println!();
    }

    if !per_atom.is_empty() {
        println!("--- Φ.μ.4 atom catalogue (universal ≥2 families, top 60 cumulative) ---");
        let mut atoms_sorted: Vec<_> = per_atom.iter()
            .filter(|(_, (fams, _))| fams.len() >= 2)
            .collect();
        atoms_sorted.sort_by(|a, b| b.1.0.len().cmp(&a.1.0.len()).then(b.1.1.cmp(&a.1.1)));
        println!("  {:<48} {:>5}  {:>10}", "atom", "fams", "max_occ");
        for (atom, (fams, occ)) in atoms_sorted.iter().take(60) {
            println!("  {:<48} {:>5}  {:>10}", atom, fams.len(), occ);
        }
        // breadth histogram
        let mut hist: HashMap<usize, usize> = HashMap::new();
        for (_, (fams, _)) in per_atom.iter() {
            *hist.entry(fams.len()).or_insert(0) += 1;
        }
        let mut hist_rows: Vec<_> = hist.iter().collect();
        hist_rows.sort_by_key(|(k, _)| std::cmp::Reverse(**k));
        println!();
        println!("--- atom family-breadth histogram ---");
        for (k, v) in hist_rows.iter() {
            let bar_len = (**v).min(60);
            let bar = "█".repeat(bar_len);
            println!("  {:>2} families : {:>5}  {}", k, v, bar);
        }
        println!();
    }

    Ok(())
}

// ============================================================
// Mode: parasite hunt â€” inspect SCAN's own programs
// ============================================================

#[derive(Debug)]
pub struct ParasiteReport {
    pub total_nodes: usize,
    pub dead: Vec<usize>,
    pub duplicate_clusters: Vec<Vec<usize>>,
    pub trivial_identities: Vec<(usize, &'static str)>,
}

impl ParasiteReport {
    pub fn parasite_count(&self) -> usize {
        self.dead.len()
            + self.duplicate_clusters.iter().map(|c| c.len() - 1).sum::<usize>()
            + self.trivial_identities.len()
    }
}

fn node_references(node: KNode) -> Vec<u32> {
    match node.op {
        Op::Input | Op::ConstI64 | Op::ConstF64 => vec![],
        // Unary i64 â†’ i64 ops
        Op::Hash64 | Op::NotBool | Op::Output | Op::BitFlipI64 | Op::NegI64
        | Op::ReverseBitsI64 | Op::ByteswapI64
        | Op::PopcntI64 | Op::LzcntI64 | Op::TzcntI64 => vec![node.a as u32],
        // Binary i64,i64 â†’ i64/bool ops
        Op::AddI64 | Op::SubI64 | Op::MulI64 | Op::DivI64Checked | Op::ModI64Checked
        | Op::MinI64 | Op::MaxI64 | Op::EqI64 | Op::LtI64 | Op::LeI64
        | Op::BitAndI64 | Op::BitOrI64 | Op::BitXorI64 | Op::ShlI64 | Op::ShrI64
        | Op::SatAddI64 | Op::SatSubI64 | Op::AndBool | Op::OrBool
        | Op::PextI64 | Op::PdepI64 => vec![node.a as u32, node.b as u32],
        Op::SelectI64 | Op::ClampI64 => vec![node.a as u32, node.b as u32, node.imm as u32],
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            let base = node.a as u32;
            let count = node.imm.max(0) as u32;
            (0..count).map(|i| base + i).collect()
        }
        // Î¦.0 â€” F64Op references `a` always; `b` only when binary.
        // Decoding the sub-op selector here keeps reference tracking
        // accurate for any synthesizer pass that walks node graphs.
        Op::F64Op => match F64SubOp::from_imm(node.imm) {
            Ok(sub) if sub.is_binary() => vec![node.a as u32, node.b as u32],
            _ => vec![node.a as u32],
        },
        // KASM v1.0 — wrap-style ops reference `a` only ; binary forms
        // reference `a, b` ; Cond also references `imm` as a slot.
        Op::Adaptive | Op::Comptime | Op::Memoize | Op::Grad
        | Op::Lazy | Op::Force
        | Op::Vmap | Op::Pmap | Op::VLenI64 | Op::VSumI64 | Op::VRangeI64
        | Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64
            => vec![node.a as u32],
        Op::Cond => vec![node.a as u32, node.b as u32, node.imm as u32],
        Op::Pipeline | Op::Fori | Op::WhileLoop | Op::Reduce | Op::Scan
        | Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
        | Op::VConcatI64 | Op::VBroadcastI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
        | Op::VGetI64  // Wave 7i — refs vec_slot + idx_slot
        | Op::Fractal | Op::Eval  // Wave 8 — refs a/b, no imm slot
        => {
            vec![node.a as u32, node.b as u32]
        }
    }
}

fn subtree_hash(nodes: &[KNode], idx: usize, cache: &mut HashMap<usize, u64>) -> u64 {
    if let Some(h) = cache.get(&idx) { return *h; }
    let node = nodes[idx];
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    h ^= node.op as u64; h = h.wrapping_mul(0x100000001b3); h ^= h >> 32;
    h ^= node.imm as u64; h = h.wrapping_mul(0x100000001b3); h ^= h >> 32;
    for r in node_references(node) {
        let sub = if (r as usize) < nodes.len() { subtree_hash(nodes, r as usize, cache) } else { 0xDEAD_BEEF };
        h ^= sub; h = h.wrapping_mul(0x100000001b3); h ^= h >> 32;
    }
    cache.insert(idx, h);
    h
}

pub fn find_parasites(program: &Program) -> ParasiteReport {
    let nodes = program.nodes();
    let total_nodes = nodes.len();

    // Dead nodes
    let mut referenced = vec![false; nodes.len()];
    for (_i, node) in nodes.iter().enumerate() {
        for r in node_references(*node) {
            if let Some(slot) = referenced.get_mut(r as usize) { *slot = true; }
        }
    }
    let mut dead = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.op == Op::Output { continue; }
        if !referenced[i] { dead.push(i); }
    }

    // Duplicate sub-graphs
    let mut cache = HashMap::with_capacity(nodes.len());
    let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.op == Op::Output { continue; }
        let h = subtree_hash(nodes, i, &mut cache);
        by_hash.entry(h).or_default().push(i);
    }
    let mut duplicate_clusters: Vec<Vec<usize>> = by_hash.into_iter()
        .filter_map(|(_, v)| if v.len() >= 2 { Some(v) } else { None })
        .collect();
    duplicate_clusters.sort_by(|a, b| b.len().cmp(&a.len()));

    // Trivial identities
    let mut trivial_identities: Vec<(usize, &'static str)> = Vec::new();
    let const_at = |idx: u16| -> Option<i16> {
        nodes.get(idx as usize).and_then(|n| if n.op == Op::ConstI64 { Some(n.imm) } else { None })
    };
    for (i, n) in nodes.iter().enumerate() {
        match n.op {
            Op::AddI64 if const_at(n.a) == Some(0) || const_at(n.b) == Some(0) =>
                trivial_identities.push((i, "add_zero")),
            Op::SubI64 if const_at(n.b) == Some(0) =>
                trivial_identities.push((i, "sub_zero")),
            Op::MulI64 if const_at(n.a) == Some(1) || const_at(n.b) == Some(1) =>
                trivial_identities.push((i, "mul_one")),
            Op::MulI64 if const_at(n.a) == Some(0) || const_at(n.b) == Some(0) =>
                trivial_identities.push((i, "mul_zero")),
            Op::DivI64Checked if const_at(n.b) == Some(1) =>
                trivial_identities.push((i, "div_one")),
            Op::BitOrI64 if const_at(n.a) == Some(0) || const_at(n.b) == Some(0) =>
                trivial_identities.push((i, "or_zero")),
            Op::BitXorI64 if const_at(n.a) == Some(0) || const_at(n.b) == Some(0) =>
                trivial_identities.push((i, "xor_zero")),
            Op::SelectI64 if n.b as i32 == n.imm as i32 =>
                trivial_identities.push((i, "select_same_branches")),
            _ => {}
        }
    }

    ParasiteReport { total_nodes, dead, duplicate_clusters, trivial_identities }
}


fn parasite_lab_impl(samples_count: usize) -> io::Result<()> {
    println!("=== lab_runner parasites : DreamForge programs introspection ===");
    println!("  samples : {samples_count}");
    println!();

    let store_path = std::env::current_dir().unwrap().join(".codex-tmp").join(format!(
        "lab-parasites-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let store = Store::open(&store_path)?;
    let monster = MonsterNode::new(store, MemoryGovernor::new(32 * 1024 * 1024));
    let mut rng = XorShift64::new(
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64,
    );

    // Synthesize a known affine target and inspect what DreamForge produces.
    let mut programs_inspected = 0usize;
    let mut total_parasites = 0usize;
    let mut total_nodes_seen = 0usize;
    let mut by_kind: HashMap<&'static str, usize> = HashMap::new();

    for _ in 0..samples_count {
        let target = random_target(&mut rng);
        let inputs = build_diverse_inputs(&mut rng);
        let examples: Vec<(i64, i64)> = inputs.iter().map(|&x| (x, target.eval(x))).collect();
        let cfg = MonsterEvolutionConfig {
            generations: 4,
            max_nodes: 9,
            beam_width: 256,
            holdout_stride: 3,
            progress: None,
            skip_prepass: false,
        };
        let outcome = match monster.evolve_i64_program(&examples, cfg) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let report = find_parasites(&outcome.program);
        programs_inspected += 1;
        total_parasites += report.parasite_count();
        total_nodes_seen += report.total_nodes;
        for (_, kind) in &report.trivial_identities {
            *by_kind.entry(*kind).or_insert(0) += 1;
        }
        println!(
            "  {:<14} nodes={:<2} dead={:<2} dup={:<2} trivial={:<2} {}",
            target.name(),
            report.total_nodes,
            report.dead.len(),
            report.duplicate_clusters.len(),
            report.trivial_identities.len(),
            if report.parasite_count() > 0 { "â˜…" } else { "" },
        );
    }

    println!();
    println!("=== aggregate ===");
    println!("  programs inspected : {programs_inspected}");
    println!("  total nodes seen   : {total_nodes_seen}");
    println!("  total parasites    : {total_parasites}");
    if total_nodes_seen > 0 {
        println!(
            "  parasite ratio     : {:.2}% (parasites / total nodes)",
            100.0 * total_parasites as f64 / total_nodes_seen as f64
        );
    }
    if !by_kind.is_empty() {
        println!("  trivial identity breakdown:");
        let mut ks: Vec<_> = by_kind.into_iter().collect();
        ks.sort_by(|a, b| b.1.cmp(&a.1));
        for (kind, count) in ks {
            println!("    {kind:<22} Ã—{count}");
        }
    }
    Ok(())
}

// ============================================================
// Mode: scientific contract audits
// ============================================================


fn audit_tier1_lab_impl() -> io::Result<()> {
    let shared_store_path = std::env::current_dir()
        .unwrap()
        .join(".codex-tmp")
        .join("lab-contract-audit");
    let shared_store = std::sync::Arc::new(
        Store::open(&shared_store_path).expect("lab contract audit store open failed"),
    );
    let monster = MonsterNode::shared(
        shared_store,
        MemoryGovernor::new(16 * 1024 * 1024),
    );
    let cfg = MonsterEvolutionConfig {
        generations: 1,
        max_nodes: 16,
        beam_width: 64,
        holdout_stride: 3,
        progress: None,
        skip_prepass: false,
    };
    let targets = tier1_contract_targets();
    let start = Instant::now();
    let mut exact = 0usize;
    let mut dense_exact = 0usize;
    let mut errored = 0usize;
    let mut total_candidates = 0usize;

    println!("=== lab_runner audit_tier1 : active scientific contracts ===");
    println!("  contracts     : {}", targets.len());
    println!("  store         : {}", shared_store_path.display());
    println!(
        "  cfg           : generations={} max_nodes={} beam_width={} holdout_stride={}",
        cfg.generations, cfg.max_nodes, cfg.beam_width, cfg.holdout_stride,
    );
    println!();
    println!(
        "  {:<43} {:<11} {:>5} {:>5} {:>5} {:>8} {:>8}",
        "target", "source", "train", "hold", "dense", "nodes", "cand",
    );

    for target in &targets {
        let inputs = contract_probe_inputs(target);
        let examples: Vec<(i64, i64)> = inputs.iter().map(|&x| (x, target.eval(x))).collect();
        let t0 = Instant::now();
        match monster.evolve_i64_program(&examples, cfg.clone()) {
            Ok(outcome) => {
                let dense_loss = audit_loss(&outcome.program, target, &inputs);
                let dense_ok = dense_loss == 0;
                if outcome.exact_train && outcome.exact_holdout {
                    exact += 1;
                }
                if dense_ok {
                    dense_exact += 1;
                }
                total_candidates += outcome.candidates_evaluated;
                println!(
                    "  {:<43} {:<11} {:>5} {:>5} {:>5} {:>8} {:>8}",
                    target.name(),
                    outcome.source,
                    if outcome.exact_train { "ok" } else { "fail" },
                    if outcome.exact_holdout { "ok" } else { "fail" },
                    if dense_ok { "ok" } else { "fail" },
                    outcome.program.nodes().len(),
                    outcome.candidates_evaluated,
                );
                if !dense_ok {
                    println!(
                        "    dense_loss={} elapsed_ms={:.2}",
                        dense_loss,
                        t0.elapsed().as_secs_f64() * 1000.0,
                    );
                }
            }
            Err(err) => {
                errored += 1;
                println!(
                    "  {:<43} {:<11} {:>5} {:>5} {:>5} {:>8} {:>8}",
                    target.name(),
                    "error",
                    "fail",
                    "fail",
                    "fail",
                    0,
                    0,
                );
                println!("    error={}", err);
            }
        }
    }

    println!();
    println!("=== audit_tier1 summary ===");
    println!("  exact train+holdout : {}/{}", exact, targets.len());
    println!("  dense contract pass : {}/{}", dense_exact, targets.len());
    println!("  errored             : {errored}");
    println!("  candidates          : {total_candidates}");
    println!("  elapsed             : {:.2?}", start.elapsed());
    Ok(())
}


// ============================================================
// main: dispatch on argv
// ============================================================

fn self_improve_budget_exhausted(
    mode: &SelfImproveMode,
    probes_run: usize,
    started: Instant,
) -> bool {
    match mode {
        SelfImproveMode::Iterations(count) => probes_run >= *count,
        SelfImproveMode::WallTime(duration) => started.elapsed() >= *duration,
        SelfImproveMode::UntilFingerprint {
            max_iterations, ..
        } => probes_run >= *max_iterations,
    }
}

fn hex_prefix(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(16);
    for b in bytes.iter().take(8) {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}


fn self_improve_lab_impl(iterations: usize) -> io::Result<()> {
    println!("=== MonsterNode lab self_improve : autonomous loop ===");
    println!("  iterations    : {iterations}");

    let store_path = std::env::current_dir()
        .unwrap()
        .join(".codex-tmp")
        .join("lab-self-improve");
    let store = Store::open(&store_path)?;
    let monster = MonsterNode::new(store, MemoryGovernor::new(32 * 1024 * 1024));
    let report = monster.self_improve(SelfImproveBudget::iterations(iterations));

    println!();
    println!("=== self_improve summary ===");
    println!("  probes_run          : {}", report.probes_run);
    println!("  exact_train         : {}", report.exact_train);
    println!("  exact_holdout       : {}", report.exact_holdout);
    println!("  errored             : {}", report.errored);
    println!("  candidates          : {}", report.candidates_evaluated);
    println!("  atlas_submitted     : {}", report.atlas_submitted);
    println!("  atlas_accepted      : {}", report.atlas_accepted);
    println!("  oracle_ticks        : {}", report.oracle_ticks);
    println!("  oracles_learned     : {}", report.oracles_learned);
    println!("  elapsed_ms          : {}", report.elapsed_ms);
    if report.elapsed_ms > 0 {
        let ips = report.probes_run as f64 / (report.elapsed_ms as f64 / 1000.0);
        println!("  probes/sec          : {:.1}", ips);
    }

    if !report.discoveries.is_empty() {
        println!();
        println!("  discoveries:");
        for discovery in report.discoveries.iter().take(12) {
            println!(
                "    {:<34} source={:<12} program={} fp={} atlas={}",
                discovery.target_name,
                discovery.source,
                discovery.program_hash,
                hex_prefix(&discovery.output_fingerprint),
                if discovery.atlas_accepted { "accepted" } else { "noop" },
            );
        }
        if report.discoveries.len() > 12 {
            println!("    ... {} more", report.discoveries.len() - 12);
        }
    }

    Ok(())
}

fn ephemeral_lab_impl(iterations: usize) -> io::Result<()> {
    println!("=== MonsterNode lab ephemeral : pirate probe ===");
    println!("  iterations    : {iterations}");
    println!("  protocol_picks: C2 C8 D5 B3 E7");
    println!();
    let mut rng = XorShift64::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
    );
    let store_path = std::env::current_dir()
        .unwrap()
        .join(".codex-tmp")
        .join(format!(
            "lab-ephemeral-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    let store = Store::open(&store_path)?;
    let monster = MonsterNode::new(store, MemoryGovernor::new(32 * 1024 * 1024));
    let axioms = [
        "execution_is_sequential",
        "code_and_data_are_distinct",
        "branching_is_required_for_choice",
        "compiler_precedes_runtime",
    ];
    let domains = [
        "mycology",
        "crystallography",
        "echolocation",
        "tectonics",
    ];
    let taboo = [
        "loop|compute|function|return",
        "cache|pointer|buffer|index",
        "caller|callee|compile|runtime",
        "condition|iterate|memory|type",
    ];
    let cards = ["beam_choke", "nodes_shrink", "holdout_scramble", "generation_clamp"];
    let mut failures: HashSet<String> = HashSet::new();
    let mut surprise_count = 0usize;
    let mut executed = 0usize;
    let mut goulot_counts: HashMap<&'static str, usize> = HashMap::new();
    let mut prep_samples_us: Vec<u64> = Vec::new();
    let mut synth_samples_us: Vec<u64> = Vec::new();
    let mut cand_rate_samples: Vec<u64> = Vec::new();
    for iter in 0..iterations {
        let target = random_target(&mut rng);
        let ax = axioms[rng.range(axioms.len())];
        let dm = domains[rng.range(domains.len())];
        let vb = taboo[rng.range(taboo.len())];
        let card = cards[rng.range(cards.len())];
        let key = format!("{}|{}|{}|{}", target.name(), ax, dm, vb);
        if failures.contains(&key) && rng.range(100) < 70 {
            println!(
                r#"{{"source":"ephemeral_lab","iter":{},"event":"skip_repeat_failure","failure_cache_key":"{}"}}"#,
                iter, key
            );
            continue;
        }
        let mut cfg = random_evolve_config(&mut rng);
        match card {
            "beam_choke" => cfg.beam_width = cfg.beam_width.min(96),
            "nodes_shrink" => cfg.max_nodes = cfg.max_nodes.min(7),
            "holdout_scramble" => cfg.holdout_stride = cfg.holdout_stride.clamp(2, 7),
            _ => cfg.generations = cfg.generations.min(2),
        }
        let prep_t0 = Instant::now();
        let inputs = build_diverse_inputs(&mut rng);
        let examples: Vec<(i64, i64)> = inputs
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let mut y = target.eval(x);
                y = y.wrapping_add(((i as i64) ^ (x & 31)) - 7);
                if (i & 1) == 1 {
                    y ^= y.rotate_left((i % 13) as u32);
                }
                (x, y)
            })
            .collect();
        let prep_us = prep_t0.elapsed().as_micros() as u64;
        let prediction = !target.name().starts_with("wall_");
        let cfg_beam_width = cfg.beam_width;
        let cfg_generations = cfg.generations;
        let t0 = Instant::now();
        let outcome = monster.evolve_i64_program(&examples, cfg);
        let synth_us = t0.elapsed().as_micros() as u64;
        executed += 1;
        match outcome {
            Ok(out) => {
                let observed = out.exact_holdout;
                let surprise = prediction != observed;
                let cand_per_sec = if synth_us == 0 {
                    0
                } else {
                    ((out.candidates_evaluated as u128) * 1_000_000u128 / synth_us as u128) as u64
                };
                let goulot = if out.source == "memo" {
                    "memo_replay"
                } else if prep_us > synth_us / 2 && prep_us > 400 {
                    "input_fabrication"
                } else if cfg_beam_width >= 512 && out.candidates_evaluated > 1800 {
                    "beam_explosion"
                } else if cfg_generations >= 4 && synth_us > 12_000 {
                    "generation_depth"
                } else if cand_per_sec > 0 && cand_per_sec < 160_000 {
                    "throughput_collapse"
                } else if out.candidates_evaluated <= 250 && !observed {
                    "search_starvation"
                } else {
                    "none"
                };
                let goulot_score = prep_us.saturating_add(synth_us);
                *goulot_counts.entry(goulot).or_insert(0) += 1;
                prep_samples_us.push(prep_us);
                synth_samples_us.push(synth_us);
                if cand_per_sec > 0 {
                    cand_rate_samples.push(cand_per_sec);
                }
                if surprise {
                    surprise_count += 1;
                } else if !observed {
                    failures.insert(key.clone());
                }
                println!(
                    r#"{{"source":"ephemeral_lab","iter":{},"axiome_supprime":"{}","domaine_analogique":"{}","vocabulaire_mort":"{}","carte_absurde":"{}","prediction_avant_run":"{}","observe_apres_run":"{}","surprise":{},"candidates":{},"prep_us":{},"synth_us":{},"candidates_per_sec":{},"goulot":"{}","goulot_score":{},"failure_cache_key":"{}"}}"#,
                    iter,
                    ax,
                    dm,
                    vb,
                    card,
                    if prediction { "hit" } else { "miss" },
                    if observed { "hit" } else { "miss" },
                    surprise,
                    out.candidates_evaluated,
                    prep_us,
                    synth_us,
                    cand_per_sec,
                    goulot,
                    goulot_score,
                    key
                );
            }
            Err(err) => {
                surprise_count += 1;
                failures.insert(key.clone());
                *goulot_counts.entry("error_path").or_insert(0) += 1;
                prep_samples_us.push(prep_us);
                synth_samples_us.push(synth_us);
                println!(
                    r#"{{"source":"ephemeral_lab","iter":{},"axiome_supprime":"{}","domaine_analogique":"{}","vocabulaire_mort":"{}","carte_absurde":"{}","prediction_avant_run":"{}","observe_apres_run":"error","surprise":true,"prep_us":{},"synth_us":{},"goulot":"error_path","goulot_score":{},"error":"{}","failure_cache_key":"{}"}}"#,
                    iter,
                    ax,
                    dm,
                    vb,
                    card,
                    if prediction { "hit" } else { "miss" },
                    prep_us,
                    synth_us,
                    prep_us.saturating_add(synth_us),
                    err,
                    key
                );
            }
        }
    }
    println!();
    println!("=== ephemeral summary ===");
    println!("  executed       : {executed}");
    println!("  failure_cached : {}", failures.len());
    println!("  surprises      : {surprise_count}");
    if executed > 0 {
        println!(
            "  surprise_ratio : {:.1}%",
            100.0 * surprise_count as f64 / executed as f64
        );
    }
    prep_samples_us.sort_unstable();
    synth_samples_us.sort_unstable();
    cand_rate_samples.sort_unstable();
    println!("  prep_p95_us    : {}", percentile(&prep_samples_us, 0.95));
    println!("  synth_p95_us   : {}", percentile(&synth_samples_us, 0.95));
    println!("  synth_p99_us   : {}", percentile(&synth_samples_us, 0.99));
    println!("  cand_p50_sec   : {}", percentile(&cand_rate_samples, 0.50));
    println!("  cand_p95_sec   : {}", percentile(&cand_rate_samples, 0.95));
    println!("  goulots        :");
    let mut g_rows: Vec<_> = goulot_counts.into_iter().collect();
    g_rows.sort_by(|a, b| b.1.cmp(&a.1));
    for (name, count) in g_rows {
        println!("    {:<20} {}", name, count);
    }
    let _ = std::fs::remove_dir_all(&store_path);
    Ok(())
}

fn ram_probe_once(exp: &str, seed: u64) -> (usize, usize, usize, Vec<u64>, f64, usize, bool) {
    let mut rng = XorShift64::new(seed);
    let mut lat = Vec::new();
    match exp {
        "stride_resonance" => {
            let strides = [1usize, 4, 16, 64, 256, 1024, 4096, 2 * 1024 * 1024];
            let stride = strides[rng.range(strides.len())];
            let ws = 8 * 1024 * 1024usize;
            let mut data = vec![0u8; ws];
            for _ in 0..5 {
                let t0 = Instant::now();
                let mut idx = 0usize;
                let mut touches = 0usize;
                while idx < ws {
                    data[idx] = data[idx].wrapping_add(1);
                    idx += stride;
                    touches += 1;
                }
                lat.push((t0.elapsed().as_nanos() as u64) / touches.max(1) as u64);
            }
            (ws, stride, 1, lat, if stride <= 64 { 0.8 } else { 0.2 }, stride / 64, stride >= 4096)
        }
        "working_set_cliff" => {
            let sizes = [32 * 1024usize, 256 * 1024, 2 * 1024 * 1024, 16 * 1024 * 1024, 64 * 1024 * 1024];
            let ws = sizes[rng.range(sizes.len())];
            let stride = 64usize;
            let mut data = vec![0u8; ws];
            for _ in 0..5 {
                let t0 = Instant::now();
                let mut idx = 0usize;
                let mut touches = 0usize;
                while idx < ws {
                    data[idx] = data[idx].wrapping_add(1);
                    idx += stride;
                    touches += 1;
                }
                lat.push((t0.elapsed().as_nanos() as u64) / touches.max(1) as u64);
            }
            (ws, stride, 1, lat, if ws < 2 * 1024 * 1024 { 0.85 } else { 0.35 }, ws / (1024 * 1024), ws >= 16 * 1024 * 1024)
        }
        "pointer_maze" => {
            let n = 1usize << (16 + rng.range(2));
            let ws = n * std::mem::size_of::<u32>();
            let mut next: Vec<u32> = (0..n as u32).collect();
            for i in 0..n {
                let j = rng.range(n);
                next.swap(i, j);
            }
            let mut p = next[0] as usize;
            for _ in 0..4 {
                let t0 = Instant::now();
                for _ in 0..250_000usize {
                    p = next[p] as usize;
                }
                std::hint::black_box(p);
                lat.push((t0.elapsed().as_nanos() as u64) / 250_000);
            }
            (ws, 0, 1, lat, 0.08, 13, true)
        }
        "false_sharing_storm" => {
            let threads = 2 + rng.range(4);
            let cells = Arc::new(
                (0..threads)
                    .map(|_| std::sync::atomic::AtomicU64::new(0))
                    .collect::<Vec<_>>(),
            );
            for _ in 0..3 {
                let t0 = Instant::now();
                thread::scope(|scope| {
                    for tid in 0..threads {
                        let cells = Arc::clone(&cells);
                        scope.spawn(move || {
                            for k in 0..100_000usize {
                                cells[tid].fetch_add(k as u64, Ordering::Relaxed);
                            }
                        });
                    }
                });
                lat.push((t0.elapsed().as_nanos() as u64) / (threads * 100_000) as u64);
            }
            (threads * 8, 8, threads, lat, 0.3, threads, false)
        }
        "eviction_roulette" => {
            let ways = 64 + rng.range(64);
            let stride = 4096usize;
            let ws = ways * stride;
            let mut data = vec![0u8; ws];
            for _ in 0..5 {
                let t0 = Instant::now();
                for i in 0..ways {
                    let idx = i * stride;
                    data[idx] = data[idx].wrapping_add(1);
                }
                lat.push((t0.elapsed().as_nanos() as u64) / ways as u64);
            }
            (ws, stride, 1, lat, 0.18, ways / 8, true)
        }
        _ => {
            let ws = (8 + rng.range(24)) * 1024 * 1024;
            let stride = 64usize;
            let mut data = vec![0u8; ws];
            for _ in 0..4 {
                let t0 = Instant::now();
                let mut idx = 0usize;
                let mut touches = 0usize;
                while idx < ws {
                    data[idx] = data[idx].wrapping_add(1);
                    idx += stride;
                    touches += 1;
                }
                lat.push((t0.elapsed().as_nanos() as u64) / touches.max(1) as u64);
            }
            (ws, stride, 1, lat, 0.35, 5, ws >= 16 * 1024 * 1024)
        }
    }
}

#[derive(Default, Clone, Copy)]
struct DnaGroupStats {
    total: usize,
    exact_hits: usize,
    approx_hits: usize,
}

fn dna_parse_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes && matches!(chars.peek(), Some('"')) {
                    cur.push('"');
                    let _ = chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn dna_normalize_seq(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let up = ch.to_ascii_uppercase();
        match up {
            'A' | 'C' | 'G' | 'T' | 'N' => out.push(up),
            'U' => out.push('T'),
            _ => {}
        }
    }
    out
}

fn dna_hamming_leq(a: &[u8], b: &[u8], k: usize) -> bool {
    let mut d = 0usize;
    for i in 0..a.len() {
        if a[i] != b[i] {
            d += 1;
            if d > k {
                return false;
            }
        }
    }
    true
}

fn dna_contains_motif(seq: &str, motif: &str, max_mismatch: usize) -> (bool, bool) {
    if motif.is_empty() {
        return (false, false);
    }
    if seq.len() < motif.len() {
        return (false, false);
    }
    let sb = seq.as_bytes();
    let mb = motif.as_bytes();
    let mut exact = false;
    let mut approx = false;
    for i in 0..=(sb.len() - mb.len()) {
        let w = &sb[i..(i + mb.len())];
        if w == mb {
            exact = true;
            approx = true;
            break;
        }
        if max_mismatch > 0 && dna_hamming_leq(w, mb, max_mismatch) {
            approx = true;
        }
    }
    (exact, approx)
}

fn dna_group_bucket(raw: &str) -> &'static str {
    let g = raw.trim().to_ascii_lowercase();
    if g == "responder" || g == "r" || g == "1" || g == "yes" {
        "responder"
    } else if g == "non_responder"
        || g == "non-responder"
        || g == "nonresponder"
        || g == "nr"
        || g == "0"
        || g == "no"
    {
        "non_responder"
    } else {
        "other"
    }
}

fn safe_pct(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        100.0 * num as f64 / den as f64
    }
}

fn odds_ratio_exact(
    resp_hit: usize,
    resp_total: usize,
    non_hit: usize,
    non_total: usize,
) -> f64 {
    let a = resp_hit as f64 + 0.5;
    let b = (resp_total.saturating_sub(resp_hit)) as f64 + 0.5;
    let c = non_hit as f64 + 0.5;
    let d = (non_total.saturating_sub(non_hit)) as f64 + 0.5;
    (a / b) / (c / d)
}

fn dna_motif_lab_impl(csv_path: &str, motif: &str, max_mismatch: usize) -> io::Result<()> {
    let motif = dna_normalize_seq(motif);
    if motif.is_empty() {
        return Err(io::Error::other("motif must contain at least one DNA base"));
    }

    let file = File::open(csv_path)?;
    let mut lines = BufReader::new(file).lines();
    let header_line = match lines.next() {
        Some(line) => line?,
        None => return Err(io::Error::other("csv is empty")),
    };
    let headers = dna_parse_csv_line(&header_line);
    if headers.is_empty() {
        return Err(io::Error::other("csv header is empty"));
    }

    let mut group_idx = None;
    let mut seq_idx = None;
    for (i, h) in headers.iter().enumerate() {
        let hl = h.trim().to_ascii_lowercase();
        if group_idx.is_none()
            && (hl == "group" || hl == "label" || hl == "class" || hl == "cohort")
        {
            group_idx = Some(i);
        }
        if seq_idx.is_none()
            && (hl == "sequence" || hl == "seq" || hl == "dna" || hl == "window")
        {
            seq_idx = Some(i);
        }
    }
    let group_idx = group_idx.unwrap_or(0);
    let seq_idx = seq_idx.unwrap_or(if headers.len() > 1 { 1 } else { 0 });

    let mut stats: HashMap<&'static str, DnaGroupStats> = HashMap::new();
    stats.insert("responder", DnaGroupStats::default());
    stats.insert("non_responder", DnaGroupStats::default());
    stats.insert("other", DnaGroupStats::default());

    let mut malformed_rows = 0usize;
    let mut short_rows = 0usize;
    let mut empty_rows = 0usize;
    let mut processed_rows = 0usize;

    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            empty_rows += 1;
            continue;
        }
        let cols = dna_parse_csv_line(&line);
        if cols.len() <= group_idx || cols.len() <= seq_idx {
            malformed_rows += 1;
            continue;
        }
        let bucket = dna_group_bucket(&cols[group_idx]);
        let seq = dna_normalize_seq(&cols[seq_idx]);
        if seq.len() < motif.len() {
            short_rows += 1;
            continue;
        }

        let (exact, approx) = dna_contains_motif(&seq, &motif, max_mismatch);
        let entry = stats.entry(bucket).or_default();
        entry.total += 1;
        if exact {
            entry.exact_hits += 1;
        }
        if approx {
            entry.approx_hits += 1;
        }
        processed_rows += 1;
    }

    let resp = stats.get("responder").copied().unwrap_or_default();
    let non = stats.get("non_responder").copied().unwrap_or_default();
    let other = stats.get("other").copied().unwrap_or_default();

    let odds_exact = odds_ratio_exact(resp.exact_hits, resp.total, non.exact_hits, non.total);
    let enrich_exact = if non.exact_hits == 0 || non.total == 0 {
        f64::INFINITY
    } else {
        (resp.exact_hits as f64 / resp.total.max(1) as f64)
            / (non.exact_hits as f64 / non.total as f64)
    };

    println!("=== Forge DNA motif experiment ===");
    println!("  file                 : {csv_path}");
    println!("  motif                : {}", motif);
    println!("  max_mismatch         : {max_mismatch}");
    println!("  rows_processed       : {processed_rows}");
    println!("  rows_empty           : {empty_rows}");
    println!("  rows_malformed       : {malformed_rows}");
    println!("  rows_too_short       : {short_rows}");
    println!();
    println!(
        "  responder            : total={} exact={} ({:.2}%) approx={} ({:.2}%)",
        resp.total,
        resp.exact_hits,
        safe_pct(resp.exact_hits, resp.total),
        resp.approx_hits,
        safe_pct(resp.approx_hits, resp.total),
    );
    println!(
        "  non_responder        : total={} exact={} ({:.2}%) approx={} ({:.2}%)",
        non.total,
        non.exact_hits,
        safe_pct(non.exact_hits, non.total),
        non.approx_hits,
        safe_pct(non.approx_hits, non.total),
    );
    if other.total > 0 {
        println!(
            "  other                : total={} exact={} ({:.2}%) approx={} ({:.2}%)",
            other.total,
            other.exact_hits,
            safe_pct(other.exact_hits, other.total),
            other.approx_hits,
            safe_pct(other.approx_hits, other.total),
        );
    }
    println!();
    println!(
        "  enrichment_exact     : {}",
        if enrich_exact.is_infinite() {
            "inf".to_string()
        } else {
            format!("{:.3}x", enrich_exact)
        }
    );
    println!("  odds_ratio_exact     : {:.3}", odds_exact);
    println!(
        "  interpretation       : {}",
        if odds_exact > 1.2 {
            "motif enriched in responders"
        } else if odds_exact < 0.8 {
            "motif depleted in responders"
        } else {
            "no strong enrichment signal"
        }
    );
    Ok(())
}

fn ephemeral_ram_lab_impl(iterations: usize) -> io::Result<()> {
    println!("=== MonsterNode lab ephemeral_ram : RAM detour ===");
    println!("  iterations    : {iterations}");
    println!("  protocol_picks: C2 C8 D5 B3 E7");
    println!();
    let exps = [
        "stride_resonance",
        "working_set_cliff",
        "pointer_maze",
        "false_sharing_storm",
        "eviction_roulette",
        "writeback_pressure",
    ];
    let axioms = ["execution_is_sequential", "code_and_data_are_distinct", "compiler_precedes_runtime"];
    let domains = ["mycology", "crystallography", "echolocation", "tectonics"];
    let taboo = ["loop|compute|function|return", "cache|pointer|buffer|index", "caller|callee|compile|runtime"];
    let mut rng = XorShift64::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
            ^ 0xBADC0FFE,
    );
    let store_path = std::env::current_dir()
        .unwrap()
        .join(".codex-tmp")
        .join(format!(
            "lab-ephemeral-ram-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    let store = Store::open(&store_path)?;
    let monster = MonsterNode::new(store, MemoryGovernor::new(32 * 1024 * 1024));
    let mut failures: HashSet<String> = HashSet::new();
    let mut executed = 0usize;
    let mut surprise_count = 0usize;
    for iter in 0..iterations {
        let exp = exps[rng.range(exps.len())];
        let ax = axioms[rng.range(axioms.len())];
        let dm = domains[rng.range(domains.len())];
        let vb = taboo[rng.range(taboo.len())];
        let seed = rng.next();
        let key = format!("{}|{}|{}|{}", exp, ax, dm, vb);
        if failures.contains(&key) && rng.range(100) < 75 {
            println!(
                r#"{{"source":"ephemeral_ram","iter":{},"event":"skip_repeat_failure","failure_cache_key":"{}"}}"#,
                iter, key
            );
            continue;
        }
        let (working_set_bytes, stride_bytes, threads, mut lat, hit_ratio_est, miss_burst_len, tlb_cliff_hint) =
            ram_probe_once(exp, seed);
        lat.sort_unstable();
        let p50 = percentile(&lat, 0.50);
        let p95 = percentile(&lat, 0.95);
        let p99 = percentile(&lat, 0.99);
        let max = lat.last().copied().unwrap_or(0);
        let predicted_wall = if stride_bytes >= 4096 { "TLB" } else if working_set_bytes >= 16 * 1024 * 1024 { "DRAM" } else { "L3" };
        let observed_wall = if p95 <= 8 { "L1_or_L2" } else if p95 <= 25 { "L3" } else if p95 <= 120 { "DRAM" } else { "TLB_or_sync" };
        let surprise = predicted_wall != observed_wall;
        if surprise {
            surprise_count += 1;
        } else {
            failures.insert(key.clone());
        }
        let invariant = if surprise { "topology_over_arithmetic" } else { "model_matches_wall" };
        let telem_examples = vec![
            (0, p50 as i64),
            (1, p95 as i64),
            (2, p99 as i64),
            (3, working_set_bytes as i64),
            (4, stride_bytes as i64),
        ];
        let cfg = MonsterEvolutionConfig {
            generations: 1,
            max_nodes: 7,
            beam_width: 64,
            holdout_stride: 2,
            progress: None,
            skip_prepass: false,
        };
        let detour = match monster.evolve_i64_program(&telem_examples, cfg) {
            Ok(out) if out.exact_holdout => "model_hit",
            Ok(_) => "model_partial",
            Err(_) => "model_error",
        };
        println!(
            r#"{{"source":"ephemeral_ram","iter":{},"exp":"{}","seed":{},"axiom_cut":"{}","domain":"{}","taboo_vocab":"{}","working_set_bytes":{},"stride_bytes":{},"threads":{},"ops":{},"cycles_p50":{},"cycles_p95":{},"cycles_p99":{},"latency_ns_p50":{},"latency_ns_p95":{},"latency_ns_p99":{},"latency_ns_max":{},"hit_ratio_est":{:.3},"miss_burst_len":{},"tlb_cliff_hint":{},"predicted_wall":"{}","observed_wall":"{}","surprise":{},"surprise_kind":"{}","invariant_candidate":"{}","failure_cache_key":"{}","detournement_model":"{}"}}"#,
            iter,
            exp,
            seed,
            ax,
            dm,
            vb,
            working_set_bytes,
            stride_bytes,
            threads,
            lat.len() * 1000,
            (p50 as u128 * 32 / 10) as u64,
            (p95 as u128 * 32 / 10) as u64,
            (p99 as u128 * 32 / 10) as u64,
            p50,
            p95,
            p99,
            max,
            hit_ratio_est,
            miss_burst_len,
            tlb_cliff_hint,
            predicted_wall,
            observed_wall,
            surprise,
            if surprise { "wall_mismatch" } else { "expected" },
            invariant,
            key,
            detour
        );
        executed += 1;
    }
    println!();
    println!("=== ephemeral_ram summary ===");
    println!("  executed       : {executed}");
    println!("  failure_cached : {}", failures.len());
    println!("  surprises      : {surprise_count}");
    if executed > 0 {
        println!(
            "  surprise_ratio : {:.1}%",
            100.0 * surprise_count as f64 / executed as f64
        );
    }
    let _ = std::fs::remove_dir_all(&store_path);
    Ok(())
}


// ───────────────────────────────────────────────────────────────────
// Feature validation suite — extracted to validate_features.rs in
// audit 2026-05-02 to honor CLAUDE.md 800-line rule (lab.rs 7003 LoC).
// ───────────────────────────────────────────────────────────────────

mod validate_features;
pub(self) use validate_features::validate_features_impl;


impl MonsterNode {
    pub fn run_lab_batch(iterations: usize) -> io::Result<()> {
        run_lab_batch_impl(iterations)
    }

    /// Φ.μ.feature-validate — exécute la suite de validation par feature
    /// (1 ligne JSONL par feature dans `lab_findings.jsonl`,
    /// source="feature_validation"). Couvre les 13 features v1.0 KASM
    /// + Wave 9 NotFound + suppressions Σ.1 / Σ.7. Retourne `Err` si au
    /// moins 1 feature FAIL.
    pub fn validate_features() -> io::Result<()> {
        validate_features_impl()
    }

    pub fn analyze_lab_log(limit: Option<usize>) -> io::Result<()> {
        analyze_lab_log_impl(limit)
    }

    pub fn audit_tier1_lab() -> io::Result<()> {
        audit_tier1_lab_impl()
    }

    pub fn parasite_lab(samples_count: usize) -> io::Result<()> {
        parasite_lab_impl(samples_count)
    }

    pub fn self_improve_lab(iterations: usize) -> io::Result<()> {
        self_improve_lab_impl(iterations)
    }

    pub fn ephemeral_lab(iterations: usize) -> io::Result<()> {
        ephemeral_lab_impl(iterations)
    }

    pub fn ephemeral_ram_lab(iterations: usize) -> io::Result<()> {
        ephemeral_ram_lab_impl(iterations)
    }

    pub fn dna_motif_lab(csv_path: &str, motif: &str, max_mismatch: usize) -> io::Result<()> {
        dna_motif_lab_impl(csv_path, motif, max_mismatch)
    }

    /// Φ.ν.7c — sonde dendritique (offline) sur atlas-live.bin : mesure
    /// la distribution des ramées (topologies KASM avec constantes gelées).
    pub fn dendritic_probe() -> io::Result<()> {
        dendritic_probe_lab_impl()
    }
}

/// Φ.ν.7c — empreinte de la **ramée** d'un programme : hash structurel
/// avec les `Const` gelés (placeholder ⌬). Deux programmes de la même
/// classe paramétrique partagent leur ramée. FNV-1a 64-bit.
pub fn ramée(prog: &Program) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for n in prog.nodes() {
        h ^= n.op as u8 as u64;
        h = h.wrapping_mul(0x100000001b3);
        h ^= n.ty as u8 as u64;
        h = h.wrapping_mul(0x100000001b3);
        h ^= n.a as u64;
        h = h.wrapping_mul(0x100000001b3);
        h ^= n.b as u64;
        h = h.wrapping_mul(0x100000001b3);
        let frozen = match n.op {
            Op::ConstI64 | Op::ConstF64 => 0i16,
            _ => n.imm,
        };
        h ^= frozen as u16 as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn dendritic_probe_lab_impl() -> io::Result<()> {
    let live = LiveAtlas::open(LIVE_ATLAS_PATH)?;
    let mut buckets: HashMap<u64, usize> = HashMap::new();
    let mut total = 0usize;
    let mut node_count_total = 0usize;
    let started = Instant::now();
    live.for_each_entry(|prog, _seve| {
        let r = ramée(prog);
        *buckets.entry(r).or_insert(0) += 1;
        total += 1;
        node_count_total += prog.nodes().len();
    });
    let elapsed_us = started.elapsed().as_micros() as u64;
    let unique_ramees = buckets.len();
    let avg_reuse = total as f64 / unique_ramees.max(1) as f64;
    let avg_nodes = node_count_total as f64 / total.max(1) as f64;
    let mut top: Vec<(u64, usize)> = buckets.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));

    println!("=== Dendritic probe (atlas-live.bin) — ramée distribution ===");
    println!("  total programmes      : {total}");
    println!("  ramées uniques        : {unique_ramees}");
    println!("  réutilisation moyenne : {avg_reuse:.1} prog / ramée");
    println!("  nœuds avg / programme : {avg_nodes:.1}");
    println!("  walk elapsed          : {elapsed_us} µs");
    println!("  Top 12 ramées par occurrence :");
    for (i, (r, c)) in top.iter().take(12).enumerate() {
        let pct = 100.0 * *c as f64 / total.max(1) as f64;
        println!("    #{:2} ramée={r:016x} occurrences={c:6} ({pct:.1} %)", i + 1);
    }

    let now_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let log = format!(
        r#"{{"ts":{now_ts},"source":"dendritic_probe","total":{total},"unique_ramees":{unique_ramees},"avg_reuse":{avg_reuse:.3},"avg_nodes":{avg_nodes:.2},"walk_us":{elapsed_us}}}"#
    );
    let mut file = OpenOptions::new().create(true).append(true).open(LOG_PATH)?;
    writeln!(file, "{log}")?;
    Ok(())
}
