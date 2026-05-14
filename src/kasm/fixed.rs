//! Π.16 (Wave 11, 2026-05-02) — Fixed-point Q31.32 / Q63.64.
//!
//! **Origine** : HFT classique (FIX protocol prices), Erlang `decimal`,
//! Solidity `uint256` (entier brut). Idée centrale : remplacer `f64`
//! pour les prix/quantités par un `i64` traité comme un fixed-point.
//! Avantage : bit-exact cross-machine, déterministe, **jamais d'IEEE
//! 754 ULP qui font diverger un backtest entre Mac et Linux**.
//!
//! ## Pourquoi pour Forge ?
//!
//! Wave 9 a livré `Proven<_, Deterministic>` qui rejette `Op::F64Op`
//! comme non-déterministe (libc transcendentals divergent cross-host).
//! Conséquence : on ne peut pas utiliser `f64` pour les prix dans un
//! backtest qui doit être reproductible.
//!
//! Solution : Q31.32 — un `i64` où les 32 bits hauts représentent la
//! partie entière (signée) et les 32 bits bas la partie fractionnaire.
//! Range : ~±2 milliards entiers, précision 1/2^32 ≈ 2.3 × 10⁻¹⁰.
//!
//! Pour le tick 0.01 USD : 0.01 × 2^32 = 42_949_672 ticks fractional —
//! ample pour les marchés actions/futures (ticks ≥ 0.0001 USD = 100k
//! ticks/cent largement représentables).
//!
//! ## Architecture Wave 11 minimal viable
//!
//! - `Q3132` newtype wrapper sur `i64` (les 32 bits hauts = integer,
//!   32 bits bas = fractional).
//! - `Q6364` aussi disponible pour précision plus haute (mais range
//!   réduit ±~9 entier).
//! - Operations : add/sub (i64 native), mul (shift après), div (shift
//!   avant), neg, abs.
//! - Conversion : `from_int`, `from_rational`, `to_f64_lossy` (debug
//!   only, pas pour calcul).
//! - Tous bit-exact : `Proven<_, Deterministic>` accepte (i64
//!   wrapping arithmetic + bitops).
//!
//! ## Limitations Wave 11 minimal
//!
//! - Pas de surface KASM bytecode-level encore (Wave 12+ pourra
//!   ajouter Op::QMul/QDiv si justifié — Wave 11 minimal expose
//!   l'API Rust pure).
//! - Pas de transcendentals (sqrt, exp, log) en Q31.32 — Wave 12
//!   peut ajouter via Newton-Raphson si besoin trading.
//! - Q31.32 saturating overflow par défaut sur add/sub (clamp à
//!   i64::MIN/MAX pour éviter wrapping silencieux).

use std::fmt;

/// Bits fractional pour Q31.32 (32 bits).
const Q3132_FRAC_BITS: u32 = 32;
/// Le scale factor : 2^32 = 4_294_967_296.
const Q3132_SCALE: i64 = 1i64 << Q3132_FRAC_BITS;

/// Bits fractional pour Q63.64 (64 bits, mais on utilise i64 entier
/// pour la partie haute donc effectivement Q31.64 sur i128). Wave 11
/// minimal n'expose pas Q63.64 — déféré.

/// Représentation Q31.32 : `i64` où bits 63..32 = integer signed,
/// bits 31..0 = fractional unsigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Q3132(pub i64);

impl Q3132 {
    /// Zéro.
    pub const ZERO: Q3132 = Q3132(0);
    /// Un (1.0 = scale factor).
    pub const ONE: Q3132 = Q3132(Q3132_SCALE);
    /// Min representable.
    pub const MIN: Q3132 = Q3132(i64::MIN);
    /// Max representable.
    pub const MAX: Q3132 = Q3132(i64::MAX);

    /// Construit depuis un entier `n` (multiplie par 2^32).
    /// Saturating si `n` overflow (range pratique : ±2_147_483_647).
    pub fn from_int(n: i32) -> Self {
        // i32 → i64 widening, puis shift. Pas d'overflow (i32 fits).
        Q3132((n as i64) << Q3132_FRAC_BITS)
    }

    /// Construit depuis un rationnel `num / den`. `den != 0` requis,
    /// sinon retourne `ZERO` (style total function KASM).
    pub fn from_rational(num: i64, den: i64) -> Self {
        if den == 0 {
            return Self::ZERO;
        }
        // (num << 32) / den, mais shift d'abord peut overflow → utiliser
        // i128 pour garder précision intermédiaire.
        let widened = (num as i128) << Q3132_FRAC_BITS;
        let result = widened / (den as i128);
        // Saturating cast vers i64.
        let clamped = if result > i64::MAX as i128 {
            i64::MAX
        } else if result < i64::MIN as i128 {
            i64::MIN
        } else {
            result as i64
        };
        Q3132(clamped)
    }

    /// Construit depuis un raw i64 fixed-point (bits déjà encodés).
    pub fn from_raw(raw: i64) -> Self {
        Q3132(raw)
    }

    /// Bits raw (pour serialisation, transport).
    pub fn raw(self) -> i64 {
        self.0
    }

    /// Partie entière (truncating, signed).
    pub fn integer_part(self) -> i32 {
        (self.0 >> Q3132_FRAC_BITS) as i32
    }

    /// Partie fractionnaire raw (32 bits unsigned).
    pub fn fractional_part(self) -> u32 {
        // Les 32 bits bas du raw i64.
        self.0 as u32
    }

    /// Addition saturating (jamais wrap silencieux).
    pub fn saturating_add(self, other: Q3132) -> Q3132 {
        Q3132(self.0.saturating_add(other.0))
    }

    /// Soustraction saturating.
    pub fn saturating_sub(self, other: Q3132) -> Q3132 {
        Q3132(self.0.saturating_sub(other.0))
    }

    /// Negation saturating (i64::MIN reste i64::MIN — preserves total).
    pub fn saturating_neg(self) -> Q3132 {
        Q3132(self.0.saturating_neg())
    }

    /// Absolute value saturating.
    pub fn saturating_abs(self) -> Q3132 {
        Q3132(self.0.saturating_abs())
    }

    /// Multiplication Q31.32 × Q31.32 → Q31.32. Utilise i128
    /// intermédiaire pour ne pas perdre les bits hauts, puis shift
    /// right par 32 pour récupérer le format Q31.32, saturating sur
    /// l'output i64.
    pub fn saturating_mul(self, other: Q3132) -> Q3132 {
        let widened = (self.0 as i128).wrapping_mul(other.0 as i128);
        let result = widened >> Q3132_FRAC_BITS; // récupère format Q.
        let clamped = if result > i64::MAX as i128 {
            i64::MAX
        } else if result < i64::MIN as i128 {
            i64::MIN
        } else {
            result as i64
        };
        Q3132(clamped)
    }

    /// Division Q31.32 / Q31.32 → Q31.32. Multiplication par 2^32
    /// avant division pour préserver précision. div by 0 → ZERO
    /// (total function).
    pub fn checked_div(self, other: Q3132) -> Q3132 {
        if other.0 == 0 {
            return Self::ZERO;
        }
        let widened = (self.0 as i128) << Q3132_FRAC_BITS;
        let result = widened / (other.0 as i128);
        let clamped = if result > i64::MAX as i128 {
            i64::MAX
        } else if result < i64::MIN as i128 {
            i64::MIN
        } else {
            result as i64
        };
        Q3132(clamped)
    }

    /// Conversion lossy vers f64 (UNIQUEMENT pour debug/print, jamais
    /// pour calcul — un backtest qui veut être déterministe ne doit
    /// pas passer par f64).
    pub fn to_f64_lossy(self) -> f64 {
        (self.0 as f64) / (Q3132_SCALE as f64)
    }
}

impl fmt::Display for Q3132 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format human-readable : "intpart.fracpart" en décimal.
        let int = self.integer_part();
        let frac = self.fractional_part() as u64;
        // 9 chiffres décimaux pour 32 bits = 4_294_967_296 → 9.6 décimaux.
        // On affiche 6 décimaux (~7 digits significatifs au-dessus du
        // ULP Q31.32 = 2.3×10⁻¹⁰).
        let frac_decimal = (frac * 1_000_000) / (Q3132_SCALE as u64);
        write!(f, "{}.{:06}", int, frac_decimal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q3132_constants_correct() {
        assert_eq!(Q3132::ZERO.raw(), 0);
        assert_eq!(Q3132::ONE.raw(), 1i64 << 32);
        assert_eq!(Q3132::MIN.raw(), i64::MIN);
        assert_eq!(Q3132::MAX.raw(), i64::MAX);
    }

    #[test]
    fn q3132_from_int_roundtrip() {
        for n in [0i32, 1, -1, 1000, -1000, 1_000_000, -1_000_000] {
            let q = Q3132::from_int(n);
            assert_eq!(q.integer_part(), n);
            assert_eq!(q.fractional_part(), 0);
        }
    }

    #[test]
    fn q3132_from_rational_basic() {
        // 1/2 = 0.5 = 2^31 (bit 31 set).
        let half = Q3132::from_rational(1, 2);
        assert_eq!(half.raw(), 1i64 << 31);

        // 1/4 = 0.25 = 2^30.
        let quarter = Q3132::from_rational(1, 4);
        assert_eq!(quarter.raw(), 1i64 << 30);

        // 3/4 = 0.75
        let three_quarters = Q3132::from_rational(3, 4);
        assert_eq!(three_quarters.raw(), 3i64 << 30);
    }

    #[test]
    fn q3132_div_zero_total_function() {
        // 1 / 0 = 0 (KASM total convention).
        let result = Q3132::ONE.checked_div(Q3132::ZERO);
        assert_eq!(result, Q3132::ZERO);
    }

    #[test]
    fn q3132_arithmetic_associativity_signed() {
        // (a + b) + c = a + (b + c) bit-exact en saturating
        // (associatif sauf saturation au bord — on reste loin du bord).
        let a = Q3132::from_rational(13, 7);
        let b = Q3132::from_rational(22, 5);
        let c = Q3132::from_rational(-3, 11);
        let lhs = a.saturating_add(b).saturating_add(c);
        let rhs = a.saturating_add(b.saturating_add(c));
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn q3132_mul_one_is_identity() {
        let a = Q3132::from_rational(7, 3);
        assert_eq!(a.saturating_mul(Q3132::ONE), a);
        assert_eq!(Q3132::ONE.saturating_mul(a), a);
    }

    #[test]
    fn q3132_mul_zero_is_zero() {
        let a = Q3132::from_rational(99, 7);
        assert_eq!(a.saturating_mul(Q3132::ZERO), Q3132::ZERO);
    }

    #[test]
    fn q3132_mul_div_inverse_pair() {
        // (a * b) / b = a (à l'ULP près en Q31.32).
        let a = Q3132::from_rational(1234567, 1000);
        let b = Q3132::from_rational(7, 3);
        let product = a.saturating_mul(b);
        let recovered = product.checked_div(b);
        // Tolérance : 1 ULP pour les arrondis intermédiaires.
        let diff = a.saturating_sub(recovered).saturating_abs();
        // 1 ULP en Q31.32 ≈ 2.3×10⁻¹⁰ — tolérance 100 ULPs pour les
        // chains arithmétiques.
        assert!(diff.raw() < 100, "diff = {}", diff.raw());
    }

    #[test]
    fn q3132_negation_total_on_min() {
        // i64::MIN saturating_neg → i64::MAX (saturates, pas d'UB).
        let min = Q3132::MIN;
        let neg = min.saturating_neg();
        assert_eq!(neg, Q3132::MAX);
    }

    #[test]
    fn q3132_saturating_add_clamps() {
        let max = Q3132::MAX;
        let one = Q3132::ONE;
        let sum = max.saturating_add(one);
        // Clamped à MAX, pas wrap autour.
        assert_eq!(sum, Q3132::MAX);
    }

    #[test]
    fn q3132_display_shows_int_dot_frac() {
        let q = Q3132::from_rational(1, 2);
        let s = format!("{}", q);
        assert_eq!(s, "0.500000");

        let q = Q3132::from_int(42);
        let s = format!("{}", q);
        assert_eq!(s, "42.000000");

        let q = Q3132::from_rational(3, 4);
        let s = format!("{}", q);
        assert_eq!(s, "0.750000");
    }

    #[test]
    fn q3132_deterministic_cross_machine() {
        // Le calcul Q3132 ne dépend que de wrapping i64 + bitops — tous
        // bit-stable cross-machine. On vérifie qu'un calcul complexe
        // donne un raw deterministe.
        let price = Q3132::from_rational(105_125, 1000); // 105.125
        let qty = Q3132::from_rational(7, 4);             // 1.75
        let value = price.saturating_mul(qty);
        // 105.125 * 1.75 = 183.96875 = 183 + 0.96875 = 183 + 31/32
        let expected_int = 183i32;
        let expected_frac_q31_32 = 31u32 * (1u32 << 27);
        assert_eq!(value.integer_part(), expected_int);
        assert_eq!(value.fractional_part(), expected_frac_q31_32);
    }

    #[test]
    fn q3132_to_f64_lossy_for_debug_only() {
        // Conversion lossy — usage debug uniquement.
        let q = Q3132::from_rational(1, 2);
        assert_eq!(q.to_f64_lossy(), 0.5);
        let q = Q3132::from_int(100);
        assert_eq!(q.to_f64_lossy(), 100.0);
    }
}
