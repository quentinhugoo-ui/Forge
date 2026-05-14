//! Π.17 (Wave 11, 2026-05-02) — Time-series timestamp arithmetic.
//!
//! **Origine** : Q/Kdb+ `nanos`, Pandas `Timedelta`, TimescaleDB
//! `time_bucket`. Idée centrale : les timestamps sont des `i64` en
//! nanoseconds depuis epoch (UTC). Subtraction = `Duration` en nanos.
//! Tout déterministe, total, et content-addressable (un hash de
//! window est le hash de [ts_start, ts_end] bytes).
//!
//! ## Pourquoi pour Forge ?
//!
//! Backtest = chaîne d'événements horodatés. Pour un replay
//! reproductible, l'arithmétique sur timestamps doit être :
//! 1. Déterministe (pas de timezone-dependent shenanigans)
//! 2. Total (pas d'UB sur overflow — saturating tout du long)
//! 3. Content-addressable (le hash d'une fenêtre est stable)
//!
//! Pandas/Q/Kdb+ encodent en i64 nanos UTC depuis 1970. Range :
//! ±292 ans autour de 1970 — ample pour 30 ans de backtest historique.
//!
//! ## Architecture Wave 11 minimal viable
//!
//! - `Timestamp(i64)` : nanos UTC depuis epoch (1970-01-01 00:00:00).
//! - `Duration(i64)` : delta en nanos (signé pour negative durations).
//! - Constants : `NANOS_PER_SEC`, `NANOS_PER_MILLI`, etc.
//! - Operations : `ts.diff(other)`, `ts.add(duration)`, `ts.bucket(period)`.
//! - Ordering : Timestamps ordonnés naturellement par i64 (PartialOrd).
//!
//! ## Limitations Wave 11 minimal
//!
//! - Pas de timezone awareness (UTC only — convention Q/Kdb+).
//! - Pas de leap seconds (les marchés ne s'en préoccupent pas).
//! - Pas de calendar arithmetic (e.g. "next business day") — Wave 12+
//!   peut ajouter via tableaux jours fériés content-addressed.

use std::fmt;

/// Constantes de conversion.
pub const NANOS_PER_MICRO: i64 = 1_000;
pub const NANOS_PER_MILLI: i64 = 1_000_000;
pub const NANOS_PER_SEC: i64 = 1_000_000_000;
pub const NANOS_PER_MIN: i64 = 60 * NANOS_PER_SEC;
pub const NANOS_PER_HOUR: i64 = 3600 * NANOS_PER_SEC;
pub const NANOS_PER_DAY: i64 = 86_400 * NANOS_PER_SEC;

/// Timestamp en nanos UTC depuis epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub i64);

/// Duration entre deux timestamps en nanos signés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration(pub i64);

impl Timestamp {
    /// Epoch (1970-01-01 00:00:00 UTC).
    pub const EPOCH: Timestamp = Timestamp(0);
    /// Min representable.
    pub const MIN: Timestamp = Timestamp(i64::MIN);
    /// Max representable.
    pub const MAX: Timestamp = Timestamp(i64::MAX);

    /// Construit depuis un i64 nanos UTC.
    pub fn from_nanos(n: i64) -> Self {
        Timestamp(n)
    }

    /// Construit depuis un i64 secondes UTC.
    pub fn from_seconds(s: i64) -> Self {
        Timestamp(s.saturating_mul(NANOS_PER_SEC))
    }

    /// Construit depuis un i64 millis UTC (compatibility avec
    /// JavaScript `Date.getTime()` et `System.currentTimeMillis()`).
    pub fn from_millis(ms: i64) -> Self {
        Timestamp(ms.saturating_mul(NANOS_PER_MILLI))
    }

    /// Nanos UTC raw.
    pub fn nanos(self) -> i64 {
        self.0
    }

    /// Diff entre self et `other`. Saturating si overflow.
    pub fn diff(self, other: Timestamp) -> Duration {
        Duration(self.0.saturating_sub(other.0))
    }

    /// Ajoute une duration. Saturating.
    pub fn add(self, d: Duration) -> Timestamp {
        Timestamp(self.0.saturating_add(d.0))
    }

    /// Soustrait une duration. Saturating.
    pub fn sub(self, d: Duration) -> Timestamp {
        Timestamp(self.0.saturating_sub(d.0))
    }

    /// Bucket : retourne le timestamp arrondi vers le bas au multiple
    /// de `period_nanos`. Pattern Q/Kdb+ `time_bucket`. period_nanos
    /// doit être > 0 ; sinon retourne self inchangé.
    ///
    /// Exemple : ts = "2024-03-15 14:23:45.789" (nanos depuis epoch),
    /// bucket(NANOS_PER_MIN) → "2024-03-15 14:23:00.000".
    pub fn bucket(self, period_nanos: i64) -> Timestamp {
        if period_nanos <= 0 {
            return self;
        }
        // Floor division : (n / p) * p, mais en signed avec rounding
        // vers -inf pour les nanos négatifs.
        let n = self.0;
        let p = period_nanos;
        let bucketed = if n >= 0 {
            (n / p) * p
        } else {
            // Floor div pour signed negative : ((n - p + 1) / p) * p
            // évite le rounding-toward-zero qui mettrait des values
            // négatives dans le mauvais bucket.
            let q = (n - p + 1) / p;
            q * p
        };
        Timestamp(bucketed)
    }
}

impl Duration {
    pub const ZERO: Duration = Duration(0);

    pub fn from_nanos(n: i64) -> Self {
        Duration(n)
    }
    pub fn from_micros(us: i64) -> Self {
        Duration(us.saturating_mul(NANOS_PER_MICRO))
    }
    pub fn from_millis(ms: i64) -> Self {
        Duration(ms.saturating_mul(NANOS_PER_MILLI))
    }
    pub fn from_seconds(s: i64) -> Self {
        Duration(s.saturating_mul(NANOS_PER_SEC))
    }
    pub fn from_minutes(m: i64) -> Self {
        Duration(m.saturating_mul(NANOS_PER_MIN))
    }
    pub fn from_hours(h: i64) -> Self {
        Duration(h.saturating_mul(NANOS_PER_HOUR))
    }
    pub fn from_days(d: i64) -> Self {
        Duration(d.saturating_mul(NANOS_PER_DAY))
    }

    pub fn nanos(self) -> i64 {
        self.0
    }
    pub fn millis(self) -> i64 {
        self.0 / NANOS_PER_MILLI
    }
    pub fn seconds(self) -> i64 {
        self.0 / NANOS_PER_SEC
    }

    pub fn saturating_add(self, other: Duration) -> Duration {
        Duration(self.0.saturating_add(other.0))
    }
    pub fn saturating_neg(self) -> Duration {
        Duration(self.0.saturating_neg())
    }
    pub fn saturating_abs(self) -> Duration {
        Duration(self.0.saturating_abs())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ts({}ns)", self.0)
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.0;
        if n.abs() >= NANOS_PER_DAY {
            write!(f, "{}d", n / NANOS_PER_DAY)
        } else if n.abs() >= NANOS_PER_HOUR {
            write!(f, "{}h", n / NANOS_PER_HOUR)
        } else if n.abs() >= NANOS_PER_MIN {
            write!(f, "{}m", n / NANOS_PER_MIN)
        } else if n.abs() >= NANOS_PER_SEC {
            write!(f, "{}s", n / NANOS_PER_SEC)
        } else if n.abs() >= NANOS_PER_MILLI {
            write!(f, "{}ms", n / NANOS_PER_MILLI)
        } else if n.abs() >= NANOS_PER_MICRO {
            write!(f, "{}us", n / NANOS_PER_MICRO)
        } else {
            write!(f, "{}ns", n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_constants_correct() {
        assert_eq!(Timestamp::EPOCH.nanos(), 0);
        assert_eq!(Timestamp::MIN.nanos(), i64::MIN);
        assert_eq!(Timestamp::MAX.nanos(), i64::MAX);
    }

    #[test]
    fn duration_unit_conversions() {
        assert_eq!(Duration::from_seconds(1).nanos(), 1_000_000_000);
        assert_eq!(Duration::from_minutes(1).nanos(), 60_000_000_000);
        assert_eq!(Duration::from_hours(1).nanos(), 3_600_000_000_000);
        assert_eq!(Duration::from_days(1).nanos(), 86_400_000_000_000);
        assert_eq!(Duration::from_millis(123).millis(), 123);
        assert_eq!(Duration::from_seconds(99).seconds(), 99);
    }

    #[test]
    fn timestamp_diff_returns_duration() {
        let t1 = Timestamp::from_seconds(1_700_000_000);
        let t2 = Timestamp::from_seconds(1_700_001_000);
        let d = t2.diff(t1);
        assert_eq!(d.seconds(), 1000);
    }

    #[test]
    fn timestamp_add_duration() {
        let t = Timestamp::from_seconds(1_700_000_000);
        let d = Duration::from_minutes(5);
        let t2 = t.add(d);
        assert_eq!(t2.diff(t).seconds(), 300);
    }

    #[test]
    fn timestamp_bucket_minute_floor() {
        // 14:23:45 → bucket(1min) → 14:23:00
        let ts = Timestamp::from_seconds(14 * 3600 + 23 * 60 + 45);
        let bucketed = ts.bucket(NANOS_PER_MIN);
        assert_eq!(bucketed.nanos(), (14 * 3600 + 23 * 60) as i64 * NANOS_PER_SEC);
    }

    #[test]
    fn timestamp_bucket_idempotent() {
        // bucket(bucket(t)) = bucket(t).
        let ts = Timestamp::from_seconds(1_700_000_000 + 543);
        let b1 = ts.bucket(NANOS_PER_MIN);
        let b2 = b1.bucket(NANOS_PER_MIN);
        assert_eq!(b1, b2);
    }

    #[test]
    fn timestamp_bucket_negative_floor_correct() {
        // bucket des nanos négatifs : floor vers -infinity.
        // -45 sec → bucket(60 sec) → -60 (pas 0 ni -120).
        let ts = Timestamp::from_seconds(-45);
        let bucketed = ts.bucket(NANOS_PER_MIN);
        assert_eq!(bucketed.nanos(), -60 * NANOS_PER_SEC);
    }

    #[test]
    fn timestamp_bucket_zero_period_returns_self() {
        let ts = Timestamp::from_seconds(123);
        assert_eq!(ts.bucket(0), ts);
        assert_eq!(ts.bucket(-100), ts);
    }

    #[test]
    fn timestamp_diff_associativity() {
        let t0 = Timestamp::from_seconds(1_000);
        let t1 = Timestamp::from_seconds(1_100);
        let t2 = Timestamp::from_seconds(1_250);
        let total = t2.diff(t0);
        let leg1 = t1.diff(t0);
        let leg2 = t2.diff(t1);
        assert_eq!(leg1.saturating_add(leg2), total);
    }

    #[test]
    fn duration_negation_total_on_min() {
        let d = Duration(i64::MIN);
        let neg = d.saturating_neg();
        assert_eq!(neg.nanos(), i64::MAX);
    }

    #[test]
    fn timestamp_saturating_overflow() {
        let max = Timestamp::MAX;
        let huge = Duration::from_days(i64::MAX / NANOS_PER_DAY);
        let result = max.add(huge);
        assert_eq!(result, Timestamp::MAX, "saturating doit clamp à MAX");
    }

    #[test]
    fn timestamp_display_format() {
        let t = Timestamp::from_seconds(1_700_000_000);
        let s = format!("{}", t);
        assert!(s.starts_with("ts("));

        let d = Duration::from_hours(2);
        assert_eq!(format!("{}", d), "2h");
        let d = Duration::from_millis(5);
        assert_eq!(format!("{}", d), "5ms");
        let d = Duration::from_nanos(500);
        assert_eq!(format!("{}", d), "500ns");
    }

    #[test]
    fn timestamp_deterministic_bit_exact() {
        // Cross-machine determinism : un calcul timestamp ne dépend
        // que de i64 wrapping/saturating + division entière.
        let t1 = Timestamp::from_millis(1_700_000_000_000);
        let t2 = Timestamp::from_millis(1_700_001_234_567);
        let d = t2.diff(t1);
        assert_eq!(d.nanos(), 1_234_567_000_000);
        let bucket = t1.bucket(NANOS_PER_MIN);
        // 1_700_000_000_000 ms = 1_700_000_000 sec = 28_333_333 min + 20 sec
        // 28_333_333 min × 60 = 1_699_999_980 sec → bucket = ce ts en nanos.
        assert_eq!(bucket.nanos(), 1_699_999_980 * NANOS_PER_SEC);
    }
}
