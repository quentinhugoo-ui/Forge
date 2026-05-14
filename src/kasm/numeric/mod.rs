//! Ω-3 — La Mort du Float : arithmétique déterministe exacte / bornée.
//!
//! Ce module remplace l'usage de `f32`/`f64` dans le contrat SCAN par trois
//! types numériques :
//!
//!  * [`Rational`] — **exact**, associatif, content-addressable. Le mur
//!    d'associativité IEEE 754 ne s'applique pas. Coût : taille variable
//!    (i128 num / denom dans cette version, big-int prévu Ω-3.2).
//!  * `Posit<N>` — **dense**, déterministe, prévu pour les tenseurs.
//!    Implémentation complète reportée Ω-3.1 (cf. `posit.rs` pour l'API
//!    et le mur d'émulation GPU).
//!
//! Doctrine : les trois types implémentent [`Numeric`] et exposent une
//! sérialisation **byte-stable** ; toute valeur sémantiquement égale
//! produit le même hash content-addressable. C'est le critère de victoire
//! Ω-3 : *"le mur d'associativité disparaît"*.
//!
//! Φ.μ.7 : `traits.rs` (35 lignes) replié ici.

mod posit;
mod rational;

pub use posit::{Posit16, Posit32};
pub use rational::Rational;

// ----- Traits Ω-3 -----
//
// Caractérise les types numériques qui respectent la doctrine SCAN
// (déterminisme + content-addressing + associativité prouvable).

/// Type numérique compatible avec l'arithmétique SCAN.
///
/// Les implémentations doivent :
///  * être déterministes (pas de NaN non-canonique, pas de signed zero)
///  * supporter une forme **canonique** (équivalence par valeur ⇒ même bytes)
///  * exposer `to_canonical_bytes` pour le hashing content-addressable
pub trait Numeric: Sized + Clone + PartialEq {
    fn zero() -> Self;
    fn one() -> Self;

    /// Sérialisation byte-stable. Deux valeurs sémantiquement égales
    /// **doivent** produire la même séquence d'octets.
    fn to_canonical_bytes(&self) -> Vec<u8>;
}

/// Promesse explicite : le type est associatif sous addition et
/// multiplication, à la précision près du type. Pour les types **exacts**
/// (Rational), associativité = bit-exacte. Pour les types **approchés**
/// (Posit), associativité = à epsilon-près *par construction* du type.
pub trait Associative: Numeric {
    /// `true` ⇔ `(a+b)+c == a+(b+c)` pour toutes valeurs représentables.
    fn add_is_exact() -> bool;
    /// `true` ⇔ `(a*b)*c == a*(b*c)` pour toutes valeurs représentables.
    fn mul_is_exact() -> bool;
}

/// Promesse de stabilité bit-pour-bit sous (de)sérialisation.
/// `from_canonical_bytes(to_canonical_bytes(x)) == x` byte-exact.
pub trait BitStable: Numeric {
    fn from_canonical_bytes(bytes: &[u8]) -> Option<Self>;
}
