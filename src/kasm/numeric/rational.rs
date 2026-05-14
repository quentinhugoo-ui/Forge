//! `Rational` — arithmétique rationnelle exacte sur i128.
//!
//! Forme canonique : `(num, denom)` avec `denom > 0` et `gcd(|num|, denom) == 1`.
//! Tout résultat d'opération est **immédiatement** réduit, garantissant que
//! deux rationnels égaux ont la même représentation, donc le même hash.
//!
//! Bornes : i128 numerator / denominator. Pour les calculs où les produits
//! cumulés dépassent i128 (matmul de très grandes matrices), il faut Ω-3.2
//! (big-int).
//! Sur les tailles M ≤ 64 et coefficients ≤ 16 bits, i128 tient large.
//!
//! **Propriété centrale Ω-3** : `Rational::add` et `Rational::mul` sont
//! **bit-exactement associatives**. Le mur f32 disparaît.

use std::cmp::Ordering;

use super::{Associative, BitStable, Numeric};

#[derive(Clone, Copy, Debug)]
pub struct Rational {
    num: i128,
    denom: i128, // toujours > 0, en forme canonique
}

impl Rational {
    /// Construit un rationnel `num / denom` et le réduit en forme canonique.
    /// Renvoie `None` si `denom == 0`.
    pub fn new(num: i128, denom: i128) -> Option<Self> {
        if denom == 0 {
            return None;
        }
        let (n, d) = if denom < 0 { (-num, -denom) } else { (num, denom) };
        let g = gcd(n.unsigned_abs(), d.unsigned_abs()) as i128;
        Some(Self { num: n / g, denom: d / g })
    }

    pub fn from_int(n: i128) -> Self {
        Self { num: n, denom: 1 }
    }

    pub fn num(&self) -> i128 {
        self.num
    }
    pub fn denom(&self) -> i128 {
        self.denom
    }

    /// Approximation f64 (utile pour debug uniquement, jamais pour le hash).
    pub fn to_f64_lossy(&self) -> f64 {
        self.num as f64 / self.denom as f64
    }

    /// `true` si la valeur est représentable exactement comme `i64`.
    pub fn is_integer(&self) -> bool {
        self.denom == 1
    }
}

// ---------------------------------------------------------------------------
// Arithmétique
// ---------------------------------------------------------------------------

impl Rational {
    /// `a/b + c/d = (a*d + c*b) / (b*d)`, immédiatement réduite.
    /// Panic en cas d'overflow i128 (plutôt que de produire un résultat
    /// erroné sans le savoir — la doctrine SCAN refuse le silent corruption).
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let n = self.num.checked_mul(other.denom)?
            .checked_add(other.num.checked_mul(self.denom)?)?;
        let d = self.denom.checked_mul(other.denom)?;
        Self::new(n, d)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_add(other.neg())
    }

    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let n = self.num.checked_mul(other.num)?;
        let d = self.denom.checked_mul(other.denom)?;
        Self::new(n, d)
    }

    pub fn checked_div(self, other: Self) -> Option<Self> {
        if other.num == 0 {
            return None;
        }
        Self::new(self.num.checked_mul(other.denom)?, self.denom.checked_mul(other.num)?)
    }

    pub fn neg(self) -> Self {
        Self { num: -self.num, denom: self.denom }
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Self) -> bool {
        // Forme canonique ⇒ comparaison byte-pour-byte des deux entiers.
        self.num == other.num && self.denom == other.denom
    }
}

impl Eq for Rational {}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        // a/b vs c/d : comparer a*d vs c*b (b, d > 0). On utilise i128 ⇒
        // peut overflow sur des valeurs limites. Pour l'instant, la spec
        // est : pas de comparaison sur des rationnels au-delà de la zone
        // confortable (~i64 num/denom).
        let lhs = self.num.saturating_mul(other.denom);
        let rhs = other.num.saturating_mul(self.denom);
        lhs.cmp(&rhs)
    }
}

// ---------------------------------------------------------------------------
// Sérialisation byte-stable (16 bytes : i128 num + i128 denom)
// ---------------------------------------------------------------------------

impl Numeric for Rational {
    fn zero() -> Self {
        Self { num: 0, denom: 1 }
    }
    fn one() -> Self {
        Self { num: 1, denom: 1 }
    }

    fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32);
        out.extend_from_slice(&self.num.to_le_bytes());
        out.extend_from_slice(&self.denom.to_le_bytes());
        out
    }
}

impl BitStable for Rational {
    fn from_canonical_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let num = i128::from_le_bytes(bytes[..16].try_into().ok()?);
        let denom = i128::from_le_bytes(bytes[16..].try_into().ok()?);
        // Re-vérifier la forme canonique (pas de bypass de l'invariant).
        Self::new(num, denom).filter(|r| r.num == num && r.denom == denom)
    }
}

impl Associative for Rational {
    fn add_is_exact() -> bool {
        true
    }
    fn mul_is_exact() -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn gcd(mut a: u128, mut b: u128) -> u128 {
    if a == 0 {
        return b.max(1);
    }
    if b == 0 {
        return a;
    }
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i128, d: i128) -> Rational {
        Rational::new(n, d).unwrap()
    }

    #[test]
    fn canonical_form_reduces_fractions() {
        assert_eq!(r(2, 4), r(1, 2));
        assert_eq!(r(-6, 9), r(-2, 3));
        assert_eq!(r(0, 17), r(0, 1));
        assert_eq!(r(7, 1), Rational::from_int(7));
    }

    #[test]
    fn canonical_form_normalises_sign() {
        // Le dénominateur doit toujours être positif après new().
        let a = r(1, -2);
        assert_eq!(a.num(), -1);
        assert_eq!(a.denom(), 2);
    }

    #[test]
    fn rational_addition_is_bit_exactly_associative() {
        // Le mur f32 ferme : (a+b)+c et a+(b+c) produisent les MÊMES bytes.
        let a = r(1, 3);
        let b = r(1, 7);
        let c = r(1, 5);
        let lhs = a.checked_add(b).unwrap().checked_add(c).unwrap();
        let rhs = a.checked_add(b.checked_add(c).unwrap()).unwrap();
        assert_eq!(lhs, rhs);
        assert_eq!(lhs.to_canonical_bytes(), rhs.to_canonical_bytes());
    }

    #[test]
    fn rational_multiplication_is_bit_exactly_associative() {
        let a = r(2, 3);
        let b = r(5, 7);
        let c = r(11, 13);
        let lhs = a.checked_mul(b).unwrap().checked_mul(c).unwrap();
        let rhs = a.checked_mul(b.checked_mul(c).unwrap()).unwrap();
        assert_eq!(lhs, rhs);
        assert_eq!(lhs.to_canonical_bytes(), rhs.to_canonical_bytes());
    }

    #[test]
    fn rational_addition_is_commutative_byte_exact() {
        let a = r(7, 11);
        let b = r(13, 17);
        let lhs = a.checked_add(b).unwrap();
        let rhs = b.checked_add(a).unwrap();
        assert_eq!(lhs.to_canonical_bytes(), rhs.to_canonical_bytes());
    }

    fn pairwise(mut buf: Vec<Rational>) -> Rational {
        while buf.len() > 1 {
            let mut next = Vec::with_capacity((buf.len() + 1) / 2);
            let mut i = 0;
            while i + 1 < buf.len() {
                next.push(buf[i].checked_add(buf[i + 1]).unwrap());
                i += 2;
            }
            if i < buf.len() {
                next.push(buf[i]);
            }
            buf = next;
        }
        *buf.first().unwrap()
    }

    fn ltr_sum(xs: &[Rational]) -> Rational {
        let mut acc = Rational::zero();
        for v in xs {
            acc = acc.checked_add(*v).unwrap();
        }
        acc
    }

    #[test]
    fn pairwise_vs_ltr_integers_same_bit_pattern() {
        // 64 entiers : [1, 2, ..., 64] → somme = 64*65/2 = 2080.
        // Quel que soit l'ordre de réduction, le rationnel exact est 2080/1.
        let xs: Vec<Rational> = (1..=64).map(|i| Rational::from_int(i)).collect();
        let ltr = ltr_sum(&xs);
        let pair = pairwise(xs);
        assert_eq!(ltr, pair);
        assert_eq!(ltr.num(), 2080);
        assert_eq!(ltr.denom(), 1);
        assert_eq!(ltr.to_canonical_bytes(), pair.to_canonical_bytes());
    }

    #[test]
    fn pairwise_vs_ltr_bounded_denominators_same_bit_pattern() {
        // Dénominateur commun fixe (= 100). L'addition garde un dénominateur
        // borné, donc i128 tient sans problème pour 64 termes.
        // Numérateurs = i, denom = 100. Somme = (1+2+..+64)/100 = 2080/100 = 104/5.
        let xs: Vec<Rational> = (1..=64).map(|i| r(i as i128, 100)).collect();
        let ltr = ltr_sum(&xs);
        let pair = pairwise(xs);
        assert_eq!(ltr, pair);
        assert_eq!(ltr.num(), 104);
        assert_eq!(ltr.denom(), 5);
        assert_eq!(
            ltr.to_canonical_bytes(),
            pair.to_canonical_bytes(),
            "LTR vs Pairwise MUST produce bit-identical bytes (Ω-3 promesse)"
        );
    }

    #[test]
    fn pairwise_vs_ltr_mixed_small_fractions_same_bit_pattern() {
        // Petit jeu (8 termes) avec dénominateurs coprime — i128 supporte.
        // {1/2, 1/3, 1/4, 1/5, 1/6, 1/7, 1/8, 1/9}
        let xs: Vec<Rational> = (2..=9).map(|d| r(1, d as i128)).collect();
        let ltr = ltr_sum(&xs);
        let pair = pairwise(xs);
        assert_eq!(ltr, pair);
        assert_eq!(ltr.to_canonical_bytes(), pair.to_canonical_bytes());
    }

    #[test]
    fn pairwise_vs_ltr_random_orders_all_agree() {
        // Stress : 16 fractions {i/(i+1)} pour i=1..16. Plusieurs ordres
        // (LTR direct, LTR inverse, pairwise) doivent donner les mêmes bytes.
        let xs: Vec<Rational> = (1..=16).map(|i| r(i as i128, (i + 1) as i128)).collect();
        let ltr = ltr_sum(&xs);
        let mut rev = xs.clone();
        rev.reverse();
        let ltr_rev = ltr_sum(&rev);
        let pair = pairwise(xs);
        assert_eq!(ltr.to_canonical_bytes(), ltr_rev.to_canonical_bytes());
        assert_eq!(ltr.to_canonical_bytes(), pair.to_canonical_bytes());
    }

    #[test]
    fn bitstable_roundtrip_preserves_value() {
        let cases = [r(0, 1), r(1, 1), r(-3, 7), r(42, 1), r(123_456_789, 987_654_321)];
        for v in cases {
            let bytes = v.to_canonical_bytes();
            let v2 = Rational::from_canonical_bytes(&bytes).unwrap();
            assert_eq!(v, v2);
        }
    }

    #[test]
    fn bitstable_rejects_non_canonical_bytes() {
        // Bytes avec gcd(num, denom) != 1 doivent être rejetés (forme non
        // canonique). C'est ce qui garantit l'unicité du hash.
        let mut bytes = vec![0u8; 32];
        bytes[..16].copy_from_slice(&2i128.to_le_bytes());
        bytes[16..].copy_from_slice(&4i128.to_le_bytes());
        assert!(Rational::from_canonical_bytes(&bytes).is_none());
    }

    #[test]
    fn division_by_zero_returns_none() {
        let a = r(1, 1);
        let zero = Rational::zero();
        assert!(a.checked_div(zero).is_none());
        assert!(Rational::new(1, 0).is_none());
    }

    #[test]
    fn ordering_works_on_simple_fractions() {
        assert!(r(1, 3) < r(1, 2));
        assert!(r(-1, 2) < Rational::zero());
        assert!(r(2, 3) > r(1, 2));
    }

    #[test]
    fn associative_trait_says_yes() {
        assert!(Rational::add_is_exact());
        assert!(Rational::mul_is_exact());
    }
}
