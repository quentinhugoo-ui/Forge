//! Π.21 (Wave 12, 2026-05-02) — Tick → Bar resampler streaming.
//!
//! **Origine** : TimescaleDB `time_bucket`, Pandas `resample()`,
//! KX kdb+ `xbar`. Idée centrale : un stream de ticks (ts, price, size)
//! est aggrégé en bars OHLCV de période fixe (1s, 1min, 1h). Le
//! resampler maintient l'état `current_bar` et émet un bar fermé dès
//! que le tick suivant tombe dans un nouveau bucket.
//!
//! ## Pourquoi pour Forge ?
//!
//! Backtests sur tick data (NASDAQ ITCH ~100M ticks/jour) nécessitent
//! un downsampling déterministe vers OHLCV. Sans resampler streaming,
//! il faut buffer tous les ticks puis grouper en mémoire — OOM
//! garanti à 100M ticks.
//!
//! Avec streaming :
//!   - Mémoire constante (1 bar en cours + buffer ticks du bucket actuel)
//!   - Cohérence cross-resolution : 60 bars 1-sec → 1 bar 1-min via
//!     ré-aggregation déterministe (chaining resamplers Π.21)
//!   - Hash content-addressed du bar fermé → cache hit auto sur replay
//!
//! ## Architecture Wave 12 minimal viable
//!
//! - `BarResampler { period_ns, current: Option<PendingBar> }`.
//! - `PendingBar { bucket_ts, open, high, low, close, volume }`.
//! - `add_tick(ts, price, size) -> Option<OhlcvBar>` : feed tick,
//!   retourne Some(bar) si le bar precedent est ferme par ce tick.
//! - `flush() -> Option<OhlcvBar>` : finalize current bar (fin de
//!   stream).
//! - State machine pure, no I/O, no alloc dans le steady state.
//!
//! ## Limitations Wave 12 minimal
//!
//! - Period fixe (pas de calendar buckets style "monthly").
//! - Single-symbol per resampler.
//! - Pas de "warm-up" — le premier tick définit le bucket initial.
//! - Pas de "fill missing buckets" pour empty intervals — Wave 13+
//!   peut ajouter via tick virtuel.

use crate::kasm::fixed::Q3132;
use crate::kasm::ohlcv::OhlcvBar;
use crate::kasm::timestamp::Timestamp;

/// Bar en cours de construction, aggrégé tick par tick.
#[derive(Debug, Clone, Copy)]
struct PendingBar {
    /// Bucket-aligned start timestamp (ts.bucket(period_ns)).
    bucket_ts: i64,
    open: i64,    // Q31.32 raw
    high: i64,
    low: i64,
    close: i64,
    volume: i64,  // sum of tick sizes
}

impl PendingBar {
    fn from_first_tick(bucket_ts: i64, price: i64, size: i64) -> Self {
        Self {
            bucket_ts,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: size,
        }
    }

    fn add_tick(&mut self, price: i64, size: i64) {
        if price > self.high {
            self.high = price;
        }
        if price < self.low {
            self.low = price;
        }
        self.close = price;
        self.volume = self.volume.saturating_add(size);
    }

    fn into_bar(self) -> OhlcvBar {
        OhlcvBar {
            ts: Timestamp::from_nanos(self.bucket_ts),
            open: Q3132::from_raw(self.open),
            high: Q3132::from_raw(self.high),
            low: Q3132::from_raw(self.low),
            close: Q3132::from_raw(self.close),
            volume: self.volume,
        }
    }
}

/// Resampler streaming. Stocke le bar en cours, émet un bar finalisé
/// dès qu'un tick du bucket suivant arrive.
#[derive(Debug, Clone)]
pub struct BarResampler {
    period_ns: i64,
    current: Option<PendingBar>,
    /// Stats : ticks reçus, bars émis (observabilité).
    ticks_seen: u64,
    bars_emitted: u64,
}

impl BarResampler {
    /// Construit avec la période en nanos. period_ns > 0 requis ;
    /// sinon le resampler dégénère (chaque tick = un bar).
    pub fn new(period_ns: i64) -> Self {
        Self {
            period_ns: period_ns.max(1),
            current: None,
            ticks_seen: 0,
            bars_emitted: 0,
        }
    }

    pub fn period_ns(&self) -> i64 {
        self.period_ns
    }
    pub fn ticks_seen(&self) -> u64 {
        self.ticks_seen
    }
    pub fn bars_emitted(&self) -> u64 {
        self.bars_emitted
    }
    /// Vrai si un bar est en cours d'aggregation.
    pub fn has_pending(&self) -> bool {
        self.current.is_some()
    }

    /// Ajoute un tick. Si le tick tombe dans le même bucket que le
    /// bar courant, l'aggrège. Sinon, ferme le bar courant (retourné
    /// Some) et démarre un nouveau bar pour ce tick.
    ///
    /// Convention : ts en nanos UTC, price en Q31.32 raw i64, size en i64.
    pub fn add_tick(
        &mut self,
        ts: Timestamp,
        price: Q3132,
        size: i64,
    ) -> Option<OhlcvBar> {
        self.ticks_seen += 1;
        let bucket_ts = ts.bucket(self.period_ns).nanos();

        match self.current {
            None => {
                self.current = Some(PendingBar::from_first_tick(
                    bucket_ts, price.raw(), size,
                ));
                None
            }
            Some(ref mut pending) if pending.bucket_ts == bucket_ts => {
                pending.add_tick(price.raw(), size);
                None
            }
            Some(pending) => {
                let emitted = pending.into_bar();
                self.bars_emitted += 1;
                self.current = Some(PendingBar::from_first_tick(
                    bucket_ts, price.raw(), size,
                ));
                Some(emitted)
            }
        }
    }

    /// Force la fermeture du bar courant (fin de stream).
    pub fn flush(&mut self) -> Option<OhlcvBar> {
        let emitted = self.current.take()?.into_bar();
        self.bars_emitted += 1;
        Some(emitted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::timestamp::NANOS_PER_MIN;

    fn t_sec(s: i64) -> Timestamp {
        Timestamp::from_seconds(s)
    }
    fn q(int: i32) -> Q3132 {
        Q3132::from_int(int)
    }

    #[test]
    fn resampler_first_tick_no_emit() {
        let mut r = BarResampler::new(NANOS_PER_MIN);
        let emit = r.add_tick(t_sec(100), q(100), 10);
        assert!(emit.is_none(), "first tick must not emit");
        assert!(r.has_pending());
    }

    #[test]
    fn resampler_same_bucket_aggregates() {
        // 3 ticks dans le même bucket 1-min → aggregate, no emit.
        // bucket [60, 120) couvre 100, 110, 119.
        let mut r = BarResampler::new(NANOS_PER_MIN);
        r.add_tick(t_sec(100), q(100), 10);
        r.add_tick(t_sec(110), q(105), 5);
        r.add_tick(t_sec(119), q(98), 20);
        assert_eq!(r.bars_emitted(), 0);
        assert_eq!(r.ticks_seen(), 3);
    }

    #[test]
    fn resampler_new_bucket_emits_previous() {
        let mut r = BarResampler::new(NANOS_PER_MIN);
        r.add_tick(t_sec(100), q(100), 10);   // bucket 60-120
        r.add_tick(t_sec(115), q(105), 5);    // same bucket
        let emit = r.add_tick(t_sec(180), q(102), 8);   // new bucket 120-180? Non, 180 → bucket 180.
        let bar = emit.expect("must emit closed bar");
        // Bar emitted : open=100, high=105, low=100, close=105, volume=15.
        assert_eq!(bar.open, q(100));
        assert_eq!(bar.high, q(105));
        assert_eq!(bar.low, q(100));
        assert_eq!(bar.close, q(105));
        assert_eq!(bar.volume, 15);
    }

    #[test]
    fn resampler_flush_finalizes_pending() {
        let mut r = BarResampler::new(NANOS_PER_MIN);
        r.add_tick(t_sec(100), q(100), 10);
        r.add_tick(t_sec(110), q(102), 5);
        let bar = r.flush().expect("flush must emit pending bar");
        assert_eq!(bar.open, q(100));
        assert_eq!(bar.close, q(102));
        assert_eq!(bar.volume, 15);
        assert!(!r.has_pending());
    }

    #[test]
    fn resampler_flush_empty_returns_none() {
        let mut r = BarResampler::new(NANOS_PER_MIN);
        assert!(r.flush().is_none());
    }

    #[test]
    fn resampler_high_low_track_extremes() {
        // Tous dans bucket [60, 120) : 70, 80, 90, 100, 110, 119.
        let mut r = BarResampler::new(NANOS_PER_MIN);
        r.add_tick(t_sec(70), q(100), 1);
        r.add_tick(t_sec(80), q(150), 1);    // high
        r.add_tick(t_sec(90), q(80), 1);     // low
        r.add_tick(t_sec(100), q(120), 1);
        let bar = r.flush().unwrap();
        assert_eq!(bar.high, q(150));
        assert_eq!(bar.low, q(80));
        assert_eq!(bar.open, q(100));
        assert_eq!(bar.close, q(120));
    }

    #[test]
    fn resampler_multiple_buckets_emit_sequence() {
        let mut r = BarResampler::new(NANOS_PER_MIN);
        // Bucket 0..60 : 1 tick
        r.add_tick(t_sec(30), q(100), 10);
        // Bucket 60..120 : ferme le precedent + démarre nouveau
        let bar1 = r.add_tick(t_sec(90), q(105), 5).unwrap();
        // Bucket 120..180 : ferme le second + démarre nouveau
        let bar2 = r.add_tick(t_sec(150), q(110), 8).unwrap();
        let bar3 = r.flush().unwrap();
        assert_eq!(bar1.open, q(100));
        assert_eq!(bar2.open, q(105));
        assert_eq!(bar3.open, q(110));
        assert_eq!(r.bars_emitted(), 3);
    }

    #[test]
    fn resampler_bucket_aligned_timestamps() {
        // ts dans le bucket [60, 120) → bucket_ts = 60.
        let mut r = BarResampler::new(NANOS_PER_MIN);
        r.add_tick(t_sec(75), q(100), 10);
        let bar = r.flush().unwrap();
        // Le ts du bar = bucket_start = 60s = 60 × 10^9 ns.
        assert_eq!(bar.ts.nanos(), 60 * 1_000_000_000);
    }

    #[test]
    fn resampler_volume_aggregates_correctly() {
        let mut r = BarResampler::new(NANOS_PER_MIN);
        r.add_tick(t_sec(0), q(100), 10);
        r.add_tick(t_sec(10), q(101), 20);
        r.add_tick(t_sec(20), q(102), 30);
        let bar = r.flush().unwrap();
        assert_eq!(bar.volume, 60);
    }

    #[test]
    fn resampler_chained_resolution() {
        // Resample tick → 1-sec → 1-min en chaînant 2 resamplers.
        let mut r1s = BarResampler::new(crate::kasm::timestamp::NANOS_PER_SEC);
        let mut r1m = BarResampler::new(crate::kasm::timestamp::NANOS_PER_MIN);

        // 120 ticks, 2 par seconde, sur 60 secondes → 60 bars 1-sec → 1 bar 1-min.
        for i in 0..120 {
            let ts = Timestamp::from_nanos(i as i64 * 500_000_000);  // 500ms apart
            let price = q(100 + (i % 5) as i32);
            if let Some(bar) = r1s.add_tick(ts, price, 10) {
                // Feed le bar 1-sec dans le resampler 1-min.
                r1m.add_tick(bar.ts, bar.close, bar.volume);
            }
        }
        if let Some(bar) = r1s.flush() {
            r1m.add_tick(bar.ts, bar.close, bar.volume);
        }
        let final_bar = r1m.flush().unwrap();
        assert!(final_bar.volume > 0, "1-min bar agglomerates volume");
    }

    #[test]
    fn resampler_period_zero_clamps_to_one() {
        let r = BarResampler::new(0);
        assert_eq!(r.period_ns(), 1);
    }

    #[test]
    fn resampler_deterministic_replay() {
        // Le resampler est pure state machine — replay des mêmes ticks
        // donne le même output.
        let ticks = vec![
            (t_sec(10), q(100), 5),
            (t_sec(20), q(102), 10),
            (t_sec(70), q(101), 7),  // nouveau bucket
            (t_sec(80), q(103), 3),
        ];

        let mut r1 = BarResampler::new(NANOS_PER_MIN);
        let mut r2 = BarResampler::new(NANOS_PER_MIN);

        let bars1: Vec<Option<OhlcvBar>> = ticks.iter()
            .map(|&(t, p, s)| r1.add_tick(t, p, s)).collect();
        let bars2: Vec<Option<OhlcvBar>> = ticks.iter()
            .map(|&(t, p, s)| r2.add_tick(t, p, s)).collect();
        assert_eq!(bars1, bars2);
        assert_eq!(r1.flush(), r2.flush());
    }
}
