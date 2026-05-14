//! Π.18 (Wave 11, 2026-05-02) — OHLCV columnar layout pour bars.
//!
//! **Origine** : KX kdb+ (HFT analytics), Polars (Rust DataFrame), Pandas
//! `read_csv` OHLCV. Idée centrale : un "bar" = (Open, High, Low, Close,
//! Volume, Timestamp). Au lieu de stocker `Vec<Bar>` (row-store, 6×8 = 48
//! bytes/bar non aligné cache line), stocker 6 colonnes parallèles
//! (column-store, scan SIMD-friendly).
//!
//! ## Pourquoi pour Forge ?
//!
//! Wave 4 a livré `ColumnStore` générique (Π.9 Q/Kdb+). `OhlcvStore` est
//! une **spécialisation type-safe** pour le pattern OHLCV très commun
//! en backtest :
//!
//!   - Ajouter un bar = 6 push parallèles
//!   - Indicateurs techniques (SMA, ATR, drawdown) scannent UNE colonne
//!     à la fois → cache hit perfect
//!   - Filter par fenêtre de temps via index ts_col
//!
//! Avec Π.16 fixed-point Q31.32, les prix OHLCV sont des `i64` raw —
//! déterministe cross-machine, compatible `Proven<_, Deterministic>`.
//!
//! ## Architecture Wave 11 minimal viable
//!
//! ```text
//!   OhlcvStore {
//!     ts:       Vec<i64>,  // Timestamp en nanos UTC
//!     open:     Vec<i64>,  // raw Q31.32
//!     high:     Vec<i64>,
//!     low:      Vec<i64>,
//!     close:    Vec<i64>,
//!     volume:   Vec<i64>,  // entier (shares/contracts)
//!   }
//! ```
//!
//! Methods Wave 11 minimal :
//!   - `push_bar(ts, o, h, l, c, v)` : append synchronisé
//!   - `len()`, `is_empty()`, `bar(idx) -> OhlcvBar`
//!   - `sma(period)` : Simple Moving Average sur close (Q31.32)
//!   - `atr(period)` : Average True Range (Q31.32)
//!   - `max_drawdown()` : drawdown maximum sur close (Q31.32)
//!   - `slice(start, end)` : sous-range timestamp-bounded
//!
//! ## Limitations Wave 11 minimal
//!
//! - Push-only (pas d'insert/delete random — append-only Forge style)
//! - SMA/ATR par valeur Q31.32 raw — caller doit utiliser Q3132 type
//!   pour interpréter
//! - Pas de pattern detection (engulfing, doji, etc.) — Wave 12 DSL
//! - Pas de tick-stream → bar resampling — Wave 12 Π.21

use std::fmt;

use crate::kasm::fixed::Q3132;
use crate::kasm::timestamp::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OhlcvError {
    EmptyStore,
    BadIndex { idx: usize, len: usize },
    BadPeriod { period: usize },
    /// L'invariant H ≥ max(O, C) ≥ min(O, C) ≥ L est violé.
    InvalidBar { idx: usize, reason: &'static str },
}

impl fmt::Display for OhlcvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OhlcvError::EmptyStore => write!(f, "ohlcv: store is empty"),
            OhlcvError::BadIndex { idx, len } =>
                write!(f, "ohlcv: idx {} >= len {}", idx, len),
            OhlcvError::BadPeriod { period } =>
                write!(f, "ohlcv: period {} invalid (must be > 0 and <= len)", period),
            OhlcvError::InvalidBar { idx, reason } =>
                write!(f, "ohlcv: bar {} invalid: {}", idx, reason),
        }
    }
}

/// Une "bar" complète (snapshot pour API caller).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OhlcvBar {
    pub ts: Timestamp,
    pub open: Q3132,
    pub high: Q3132,
    pub low: Q3132,
    pub close: Q3132,
    pub volume: i64,
}

/// Column-store OHLCV pour backtest. Tous les prix en Q31.32 raw i64.
#[derive(Debug, Clone, Default)]
pub struct OhlcvStore {
    ts: Vec<i64>,
    open: Vec<i64>,
    high: Vec<i64>,
    low: Vec<i64>,
    close: Vec<i64>,
    volume: Vec<i64>,
}

impl OhlcvStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            ts: Vec::with_capacity(cap),
            open: Vec::with_capacity(cap),
            high: Vec::with_capacity(cap),
            low: Vec::with_capacity(cap),
            close: Vec::with_capacity(cap),
            volume: Vec::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.ts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ts.is_empty()
    }

    /// Push un nouveau bar. Vérifie l'invariant H ≥ max(O, C) ≥
    /// min(O, C) ≥ L. Retourne `InvalidBar` si violé (anti-fat-finger
    /// data corruption).
    pub fn push_bar(
        &mut self,
        ts: Timestamp,
        open: Q3132,
        high: Q3132,
        low: Q3132,
        close: Q3132,
        volume: i64,
    ) -> Result<(), OhlcvError> {
        let idx = self.ts.len();
        let max_oc = open.max(close);
        let min_oc = open.min(close);
        if high < max_oc {
            return Err(OhlcvError::InvalidBar {
                idx, reason: "high < max(open, close)",
            });
        }
        if low > min_oc {
            return Err(OhlcvError::InvalidBar {
                idx, reason: "low > min(open, close)",
            });
        }
        self.ts.push(ts.nanos());
        self.open.push(open.raw());
        self.high.push(high.raw());
        self.low.push(low.raw());
        self.close.push(close.raw());
        self.volume.push(volume);
        Ok(())
    }

    /// Récupère un bar par index.
    pub fn bar(&self, idx: usize) -> Result<OhlcvBar, OhlcvError> {
        if idx >= self.ts.len() {
            return Err(OhlcvError::BadIndex { idx, len: self.ts.len() });
        }
        Ok(OhlcvBar {
            ts: Timestamp::from_nanos(self.ts[idx]),
            open: Q3132::from_raw(self.open[idx]),
            high: Q3132::from_raw(self.high[idx]),
            low: Q3132::from_raw(self.low[idx]),
            close: Q3132::from_raw(self.close[idx]),
            volume: self.volume[idx],
        })
    }

    /// Slice contigus pour scan SIMD.
    pub fn ts_column(&self) -> &[i64] { &self.ts }
    pub fn open_column(&self) -> &[i64] { &self.open }
    pub fn high_column(&self) -> &[i64] { &self.high }
    pub fn low_column(&self) -> &[i64] { &self.low }
    pub fn close_column(&self) -> &[i64] { &self.close }
    pub fn volume_column(&self) -> &[i64] { &self.volume }

    /// Simple Moving Average sur close, fenêtre `period`. Retourne un
    /// Vec<Q3132> de longueur `len() - period + 1` (les premiers
    /// (period - 1) bars n'ont pas de SMA défini).
    pub fn sma_close(&self, period: usize) -> Result<Vec<Q3132>, OhlcvError> {
        if period == 0 || period > self.close.len() {
            return Err(OhlcvError::BadPeriod { period });
        }
        let n = self.close.len();
        let mut out = Vec::with_capacity(n - period + 1);
        // Sliding window sum (linear time, pas O(N×period) naïf).
        // Les close[i] sont déjà en Q31.32 raw — on somme et divise
        // par period (i64) qui préserve le format Q31.32 raw.
        let mut sum: i64 = 0;
        for &c in &self.close[..period] {
            sum = sum.saturating_add(c);
        }
        let period_i = period as i64;
        // sum est en Q31.32 raw, period_i est un int — la div de raw par
        // un int donne raw / int = Q31.32 raw moyenne. Pas de from_rational
        // qui re-shifterait par 32.
        out.push(Q3132::from_raw(sum / period_i));
        for i in period..n {
            sum = sum.saturating_add(self.close[i]);
            sum = sum.saturating_sub(self.close[i - period]);
            out.push(Q3132::from_raw(sum / period_i));
        }
        Ok(out)
    }

    /// True Range = max(H-L, |H-C_prev|, |L-C_prev|). Pour le premier
    /// bar (pas de C_prev), TR = H-L.
    fn true_range(&self, idx: usize) -> Q3132 {
        let h = Q3132::from_raw(self.high[idx]);
        let l = Q3132::from_raw(self.low[idx]);
        let hl = h.saturating_sub(l);
        if idx == 0 {
            return hl;
        }
        let prev_c = Q3132::from_raw(self.close[idx - 1]);
        let h_prev_c = h.saturating_sub(prev_c).saturating_abs();
        let l_prev_c = l.saturating_sub(prev_c).saturating_abs();
        hl.max(h_prev_c).max(l_prev_c)
    }

    /// Average True Range (volatility indicator) sur `period`.
    pub fn atr(&self, period: usize) -> Result<Vec<Q3132>, OhlcvError> {
        if period == 0 || period > self.close.len() {
            return Err(OhlcvError::BadPeriod { period });
        }
        let n = self.close.len();
        let mut tr_values: Vec<i64> = Vec::with_capacity(n);
        for i in 0..n {
            tr_values.push(self.true_range(i).raw());
        }
        let mut out = Vec::with_capacity(n - period + 1);
        let mut sum: i64 = 0;
        for &v in &tr_values[..period] {
            sum = sum.saturating_add(v);
        }
        let period_i = period as i64;
        // Idem SMA : tr_values[i] sont en Q31.32 raw, on divise par
        // un int → Q31.32 raw moyenne.
        out.push(Q3132::from_raw(sum / period_i));
        for i in period..n {
            sum = sum.saturating_add(tr_values[i]);
            sum = sum.saturating_sub(tr_values[i - period]);
            out.push(Q3132::from_raw(sum / period_i));
        }
        Ok(out)
    }

    /// Max drawdown sur close = (running_max - current) / running_max
    /// max sur tout le store. Retourne (max_dd, peak_idx, trough_idx).
    /// Si store vide → EmptyStore.
    pub fn max_drawdown(&self) -> Result<(Q3132, usize, usize), OhlcvError> {
        if self.close.is_empty() {
            return Err(OhlcvError::EmptyStore);
        }
        let mut peak = self.close[0];
        let mut peak_idx = 0;
        let mut max_dd = Q3132::ZERO;
        let mut dd_peak_idx = 0;
        let mut dd_trough_idx = 0;
        for (i, &c) in self.close.iter().enumerate() {
            if c > peak {
                peak = c;
                peak_idx = i;
            }
            let drawdown = Q3132::from_raw(peak).saturating_sub(Q3132::from_raw(c));
            if drawdown > max_dd {
                max_dd = drawdown;
                dd_peak_idx = peak_idx;
                dd_trough_idx = i;
            }
        }
        Ok((max_dd, dd_peak_idx, dd_trough_idx))
    }

    /// Slice timestamp-bounded : retourne les indices [start_idx, end_idx)
    /// dont les timestamps sont dans [start_ts, end_ts).
    /// Assumes timestamps are sorted ascending.
    pub fn slice_by_time(
        &self,
        start_ts: Timestamp,
        end_ts: Timestamp,
    ) -> (usize, usize) {
        let start = self.ts.partition_point(|&t| t < start_ts.nanos());
        let end = self.ts.partition_point(|&t| t < end_ts.nanos());
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::timestamp::NANOS_PER_MIN;

    fn ts_min(n: i64) -> Timestamp {
        Timestamp::from_nanos(n * NANOS_PER_MIN)
    }
    fn q(int: i32) -> Q3132 {
        Q3132::from_int(int)
    }

    #[test]
    fn ohlcv_basic_push_and_bar() {
        let mut s = OhlcvStore::new();
        s.push_bar(ts_min(0), q(100), q(105), q(98), q(103), 1000).unwrap();
        s.push_bar(ts_min(1), q(103), q(110), q(102), q(108), 1500).unwrap();
        assert_eq!(s.len(), 2);
        let b = s.bar(1).unwrap();
        assert_eq!(b.ts, ts_min(1));
        assert_eq!(b.close, q(108));
        assert_eq!(b.volume, 1500);
    }

    #[test]
    fn ohlcv_invariant_high_below_open_rejected() {
        let mut s = OhlcvStore::new();
        let err = s.push_bar(ts_min(0), q(100), q(98), q(95), q(99), 1000).unwrap_err();
        // High 98 < Open 100 → invalid.
        assert!(matches!(err, OhlcvError::InvalidBar { reason: "high < max(open, close)", .. }));
    }

    #[test]
    fn ohlcv_invariant_low_above_close_rejected() {
        let mut s = OhlcvStore::new();
        let err = s.push_bar(ts_min(0), q(100), q(110), q(105), q(103), 1000).unwrap_err();
        // Low 105 > Close 103 → invalid.
        assert!(matches!(err, OhlcvError::InvalidBar { reason: "low > min(open, close)", .. }));
    }

    #[test]
    fn ohlcv_bar_out_of_range() {
        let s = OhlcvStore::new();
        let err = s.bar(0).unwrap_err();
        assert!(matches!(err, OhlcvError::BadIndex { idx: 0, len: 0 }));
    }

    #[test]
    fn ohlcv_columns_contigu_for_simd() {
        let mut s = OhlcvStore::new();
        for i in 0..5 {
            s.push_bar(ts_min(i as i64), q(100+i), q(105+i), q(98+i), q(102+i), 1000+i as i64).unwrap();
        }
        let close_col = s.close_column();
        assert_eq!(close_col.len(), 5);
        assert_eq!(close_col, &[
            q(102).raw(), q(103).raw(), q(104).raw(), q(105).raw(), q(106).raw(),
        ]);
    }

    #[test]
    fn ohlcv_sma_close_3_period() {
        let mut s = OhlcvStore::new();
        // close = [10, 20, 30, 40, 50]
        for (i, c) in [10, 20, 30, 40, 50].iter().enumerate() {
            s.push_bar(ts_min(i as i64), q(*c), q(*c), q(*c), q(*c), 1000).unwrap();
        }
        let sma = s.sma_close(3).unwrap();
        // SMA(3) = [(10+20+30)/3, (20+30+40)/3, (30+40+50)/3] = [20, 30, 40]
        assert_eq!(sma.len(), 3);
        assert_eq!(sma[0], q(20));
        assert_eq!(sma[1], q(30));
        assert_eq!(sma[2], q(40));
    }

    #[test]
    fn ohlcv_atr_3_period() {
        let mut s = OhlcvStore::new();
        // High-Low constant = 5 partout, pas de gap → TR = 5.
        for i in 0..5 {
            s.push_bar(ts_min(i as i64), q(100), q(105), q(100), q(102), 1000).unwrap();
        }
        let atr = s.atr(3).unwrap();
        assert_eq!(atr.len(), 3);
        // ATR de 3 valeurs constantes 5 → 5.
        assert_eq!(atr[0], q(5));
        assert_eq!(atr[1], q(5));
        assert_eq!(atr[2], q(5));
    }

    #[test]
    fn ohlcv_max_drawdown_simple() {
        let mut s = OhlcvStore::new();
        // close trajectory : 100 → 120 (peak) → 80 (trough) → 110.
        // Drawdown max = 120 - 80 = 40 (entre idx 1 peak et idx 2 trough).
        for (i, c) in [100, 120, 80, 110].iter().enumerate() {
            s.push_bar(ts_min(i as i64), q(*c), q(*c+5), q(*c-5), q(*c), 1000).unwrap();
        }
        let (dd, peak_idx, trough_idx) = s.max_drawdown().unwrap();
        assert_eq!(dd, q(40));
        assert_eq!(peak_idx, 1);
        assert_eq!(trough_idx, 2);
    }

    #[test]
    fn ohlcv_max_drawdown_empty_errors() {
        let s = OhlcvStore::new();
        assert!(matches!(s.max_drawdown(), Err(OhlcvError::EmptyStore)));
    }

    #[test]
    fn ohlcv_slice_by_time_window() {
        let mut s = OhlcvStore::new();
        for i in 0..10 {
            s.push_bar(ts_min(i), q(100), q(105), q(98), q(102), 1000).unwrap();
        }
        // Window [3 min, 7 min) → indices 3, 4, 5, 6.
        let (start, end) = s.slice_by_time(ts_min(3), ts_min(7));
        assert_eq!(start, 3);
        assert_eq!(end, 7);
    }

    #[test]
    fn ohlcv_with_capacity_no_realloc() {
        let mut s = OhlcvStore::with_capacity(1000);
        for i in 0..1000 {
            s.push_bar(
                ts_min(i as i64),
                q(100), q(101), q(99), q(100), 1000
            ).unwrap();
        }
        assert_eq!(s.len(), 1000);
    }

    #[test]
    fn ohlcv_sma_period_too_long_errors() {
        let mut s = OhlcvStore::new();
        s.push_bar(ts_min(0), q(100), q(105), q(98), q(103), 1000).unwrap();
        let err = s.sma_close(10).unwrap_err();
        assert!(matches!(err, OhlcvError::BadPeriod { period: 10 }));
    }
}
