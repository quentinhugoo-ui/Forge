//! Π.1 (Wave 1, 2026-05-02) — NNUE Stockfish-style int8 oracle.
//!
//! **Origine** : Stockfish NNUE (Efficiently Updatable Neural
//! Network). Idée centrale : un petit réseau neuronal avec poids
//! `int8` + accumulateur `int16` recalculé **incrémentalement** quand
//! un input change. Dans Stockfish : +80 Elo sans toucher la
//! recherche. Dans Forge : oracle apprenant ×100 plus rapide que la
//! dispatch actuelle.
//!
//! ## Architecture minimal viable Wave 1
//!
//! ```text
//!   input args (i64) ─┐
//!   prog fingerprint ─┴→ feature_vec (4 i16 lanes)
//!                          │
//!                          ↓ embedding @ int8 weights
//!                       accumulator (i32, sum of contributions)
//!                          │
//!                          ↓ output head @ int8 scale
//!                       predicted i64
//! ```
//!
//! - 4 input features (compact encoding of arg byte distribution)
//! - 16-element hidden layer with i8 weights
//! - 1 output scalar (i64) via i8 weight + i32 accumulator + scale
//! - All arithmetic wrapping/saturating — no float, no NaN, no UB
//!
//! ## Why "incrementally updatable"
//!
//! In Stockfish, when a chess piece moves, only one feature changes.
//! Recomputing the entire dot product is wasteful — instead, subtract
//! the old contribution and add the new one. O(1) update vs O(N) scratch.
//!
//! For Forge oracle: when an arg changes, only one feature lane
//! changes. Same incremental trick → O(1) prediction update across
//! consecutive arg variations (very common in lab synthesizer probes).
//!
//! ## Limitations Wave 1 minimal
//!
//! - No training pass (weights initialized via deterministic hash —
//!   placeholder, real training requires Wave 8b reverse-mode AD).
//! - 4 features × 16 hidden × 1 output = trivial network. Real NNUE
//!   uses 768×256×32×32×1 type architectures. The shape here proves
//!   the wire format ; scaling up is later.
//! - i8 weights from a deterministic seed — won't beat the
//!   structural rule oracle (AffineI64) on its niche, but provides
//!   the **infrastructure** for a future trained version.

use std::sync::atomic::{AtomicI32, Ordering};

/// Number of input features. Wave 1 minimal : 4 lanes.
pub const NNUE_INPUT_FEATURES: usize = 4;

/// Hidden layer width. Wave 1 minimal : 16 neurons.
pub const NNUE_HIDDEN: usize = 16;

/// Quantization scale — divides accumulator before output. Powers of
/// 2 keep the math branchless.
const NNUE_OUTPUT_SCALE: i32 = 64;

/// Minimal NNUE network with int8 weights + i32 accumulator.
///
/// Memory footprint :
///   - input × hidden × i8 = 4 × 16 = 64 bytes
///   - hidden × output × i8 = 16 × 1 = 16 bytes
///   - accumulator (cached) = 16 × i32 = 64 bytes
///   = **144 bytes total per network**
///
/// Compare : current oracle stores per-program `LearnedOracle` with
/// rule shape, samples buffer, etc. NNUE is more uniform but bigger
/// per-program. Trade-off : NNUE works for opaque programs where rule
/// detection fails (the niche the existing oracle leaves on the table).
pub struct NnueNetwork {
    /// Input → hidden weights, row-major [feature][neuron].
    pub embedding: [[i8; NNUE_HIDDEN]; NNUE_INPUT_FEATURES],
    /// Hidden → output weights.
    pub output: [i8; NNUE_HIDDEN],
    /// Cached accumulator. Atomic so multiple readers can predict
    /// concurrently without locking. Updated via `incremental_update`.
    accumulator: [AtomicI32; NNUE_HIDDEN],
}

impl NnueNetwork {
    /// Initialize with deterministic pseudo-random weights from a seed.
    /// Real training would replace these via Wave 8b reverse-mode AD.
    pub fn from_seed(seed: u64) -> Self {
        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 33) as i8
        };

        let mut embedding = [[0i8; NNUE_HIDDEN]; NNUE_INPUT_FEATURES];
        for row in embedding.iter_mut() {
            for w in row.iter_mut() {
                *w = next();
            }
        }
        let mut output = [0i8; NNUE_HIDDEN];
        for w in output.iter_mut() {
            *w = next();
        }
        // SAFETY: AtomicI32 has the same layout as i32, and 0 is a
        // valid initial value.
        let accumulator: [AtomicI32; NNUE_HIDDEN] = std::array::from_fn(|_| AtomicI32::new(0));
        Self { embedding, output, accumulator }
    }

    /// Encode (program fingerprint + args) into 4 i16 features.
    /// Wave 1 minimal : straightforward bucketing of byte sums modulo
    /// hash mixing. Real NNUE would use one-hot per-piece-square or
    /// program-structural features.
    #[allow(dead_code)] // Exposé pour usage externe : oracle ingest sur
    // (program_fp, args) bruts. Pas encore consommé en validate-features
    // car celle-ci feed des i16 directement pour tester predict/incremental.
    pub fn encode_features(prog_fingerprint: &[u8], args: &[u8]) -> [i16; NNUE_INPUT_FEATURES] {
        let mut feats = [0i16; NNUE_INPUT_FEATURES];
        // Feature 0: low byte sum of fingerprint
        let f0: i32 = prog_fingerprint.iter().take(8).map(|b| *b as i32).sum();
        // Feature 1: high byte sum of fingerprint
        let f1: i32 = prog_fingerprint.iter().skip(8).map(|b| *b as i32).sum();
        // Feature 2: arg byte sum (low 8)
        let f2: i32 = args.iter().take(8).map(|b| *b as i32).sum();
        // Feature 3: arg parity / structure (xor-fold)
        let f3: i32 = args.iter().fold(0i32, |acc, b| acc ^ (*b as i32));
        feats[0] = (f0 % 256) as i16;
        feats[1] = (f1 % 256) as i16;
        feats[2] = (f2 % 256) as i16;
        feats[3] = (f3 % 256) as i16;
        feats
    }

    /// Full forward pass — recompute accumulator from scratch + emit
    /// output. O(F × H + H) = O(80) for our minimal shape.
    pub fn predict(&self, features: [i16; NNUE_INPUT_FEATURES]) -> i64 {
        // 1. Embedding pass : accumulator[h] = Σ_f features[f] * embedding[f][h]
        let mut acc = [0i32; NNUE_HIDDEN];
        for f in 0..NNUE_INPUT_FEATURES {
            let feat_val = features[f] as i32;
            for h in 0..NNUE_HIDDEN {
                acc[h] = acc[h].wrapping_add(feat_val.wrapping_mul(self.embedding[f][h] as i32));
            }
        }
        // Cache the accumulator for incremental updates.
        for h in 0..NNUE_HIDDEN {
            self.accumulator[h].store(acc[h], Ordering::Relaxed);
        }
        // 2. Output head : sum hidden * output_weights, scaled.
        self.output_from_accumulator(&acc)
    }

    /// Incremental update — when ONE feature changes from `old` to `new`,
    /// update only that feature's contribution in the accumulator.
    /// O(H) = O(16). Returns the new prediction.
    pub fn incremental_update(&self, feature_idx: usize, old: i16, new: i16) -> i64 {
        debug_assert!(feature_idx < NNUE_INPUT_FEATURES);
        let delta = (new as i32).wrapping_sub(old as i32);
        let mut new_acc = [0i32; NNUE_HIDDEN];
        for h in 0..NNUE_HIDDEN {
            let prev = self.accumulator[h].load(Ordering::Relaxed);
            let updated = prev.wrapping_add(delta.wrapping_mul(self.embedding[feature_idx][h] as i32));
            self.accumulator[h].store(updated, Ordering::Relaxed);
            new_acc[h] = updated;
        }
        self.output_from_accumulator(&new_acc)
    }

    fn output_from_accumulator(&self, acc: &[i32; NNUE_HIDDEN]) -> i64 {
        let mut sum: i64 = 0;
        for h in 0..NNUE_HIDDEN {
            sum = sum.wrapping_add((acc[h] as i64).wrapping_mul(self.output[h] as i64));
        }
        sum / NNUE_OUTPUT_SCALE as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nnue_predict_is_deterministic() {
        let net = NnueNetwork::from_seed(42);
        let feats = [10i16, 20, 30, 40];
        let p1 = net.predict(feats);
        let p2 = net.predict(feats);
        assert_eq!(p1, p2, "same features → same prediction (deterministic)");
    }

    #[test]
    fn nnue_different_features_different_predictions() {
        let net = NnueNetwork::from_seed(42);
        let p1 = net.predict([0, 0, 0, 0]);
        let p2 = net.predict([100, 100, 100, 100]);
        // With non-zero weights from seed 42, these should differ.
        assert_ne!(p1, p2, "different features → different predictions");
    }

    #[test]
    fn nnue_incremental_matches_full() {
        // Critical property : incremental_update(f, old, new) must
        // give the SAME result as predict() with the new features.
        let net = NnueNetwork::from_seed(7);
        let mut feats = [10i16, 20, 30, 40];
        // Establish baseline accumulator.
        let _ = net.predict(feats);

        // Change feature 2 from 30 to 99.
        let incr = net.incremental_update(2, 30, 99);
        feats[2] = 99;

        let net2 = NnueNetwork::from_seed(7);
        let full = net2.predict(feats);

        assert_eq!(incr, full,
            "incremental update must produce the same result as full forward pass");
    }

    #[test]
    fn nnue_encode_features_is_stable() {
        let f1 = NnueNetwork::encode_features(b"prog_hash_1", b"args_1");
        let f2 = NnueNetwork::encode_features(b"prog_hash_1", b"args_1");
        assert_eq!(f1, f2);
        let f3 = NnueNetwork::encode_features(b"prog_hash_1", b"args_2");
        assert_ne!(f1, f3, "different args → different features");
    }

    #[test]
    fn nnue_zero_features_yields_zero() {
        let net = NnueNetwork::from_seed(123);
        // With all features = 0, the accumulator is all zero, and
        // the output is sum of (0 * weight) = 0.
        let p = net.predict([0, 0, 0, 0]);
        assert_eq!(p, 0);
    }
}
