//! Σ.4 / Π.6 (Wave 2, 2026-05-02) — NaN-boxing pour `Value` packé.
//!
//! **Origine** : Lua 5.3, V8, SpiderMonkey, JavaScriptCore. Idée
//! centrale : un f64 IEEE 754 a 2^52 NaN différents (toute valeur où
//! l'exposant est 0x7FF et la mantisse non-nulle). On utilise les bits
//! de la mantisse comme « payload » pour encoder d'autres types
//! (i64 tagué, bool, pointer, ...) dans la même width 8 bytes que
//! le f64 lui-même.
//!
//! ## Pourquoi pour Forge ?
//!
//! `Value` actuel = `enum { I64(i64), Bool(bool), VecI64(u32) }` =
//! tag (1 byte) + variant (max 8) = 16 bytes après padding. Pour le
//! cache RAM `MonsterNode` qui stocke 100k+ entries, c'est ×2 plus
//! cher que nécessaire.
//!
//! Avec NaN-boxing : 8 bytes pour TOUT `Value`. Cache 2× plus dense,
//! cache lines 64 B contiennent 8 valeurs au lieu de 4. Le miss rate
//! L1 chute proportionnellement.
//!
//! ## Encoding Wave 2 minimal viable
//!
//! ```text
//!   bits 63..52  | bits 51..48  | bits 47..0
//!   ─────────────┼──────────────┼─────────────
//!   exp = 0x7FF  | tag (4 bits) | payload (48 bits)
//! ```
//!
//! Tags supportés Wave 2 :
//! - 0x0 : I48 — i64 tronqué à 48 bits (couvre [-2^47, 2^47-1])
//! - 0x1 : Bool — payload bit 0 = true/false
//! - 0x2 : VecHandle — u32 handle dans `vec_pool`
//! - 0x3-0xF : réservés (Wave 11+ : Hash, GpuRef, ...)
//!
//! Pour i64 hors [-2^47, 2^47-1] (rare mais possible), fallback à un
//! `Value::I64Boxed(i64)` non-packé via Wave 2 avec un Vec<i64> spill.
//! Wave 2 minimal n'expose que I48 — la doctrine "déjà OK" couvre les
//! workloads observés (tous nos tests fittent dans 47 bits signés).
//!
//! ## Limitations Wave 2 minimal
//!
//! - Pas de pointer-tagging (ABI-dependent) → seuls types value type.
//! - i64 saturated à 47 bits signés (clamp + flag de débordement).
//! - Pas de string interning / hash interning (Wave 5+).

/// Tag bits stockés en bits 51..48 d'un f64 NaN-boxed. 4-bit tags.
/// Pour qu'un f64 ait son exposant = all 1s (NaN ou inf), on need
/// bits 62..52 = 0x7FF. Le sign bit 63 + quiet bit n'importent pas
/// pour la sémantique NaN — tant que mantissa != 0 c'est un NaN.
///
/// Convention Forge : bits 63..52 = 0xFFF (sign=1, exp=0x7FF), bits
/// 51..48 = tag (4 bits libres), bits 47..0 = payload. Tag=0 + payload=0
/// donnerait -∞ (pas NaN) — on évite cette combinaison en utilisant
/// des tags ≥ 1, et on encode i48=0 avec une astuce de set bit.
const TAG_I48: u64       = 0x1; // i48 (≠ 0 pour assurer mantissa!=0)
const TAG_BOOL: u64      = 0x2;
const TAG_VEC_HANDLE: u64 = 0x3;
const TAG_F64_SENTINEL: u64 = 0xF;

/// Masque pour extraire la mantisse 48-bit payload.
const PAYLOAD_MASK: u64 = (1u64 << 48) - 1;
/// Bits 51..48 = tag.
const TAG_SHIFT: u32 = 48;
const TAG_MASK: u64 = 0xF << TAG_SHIFT;
/// Header bits 63..52 = 0xFFF : sign=1, exp=all 1s (qualifie NaN si
/// mantisse non nulle). Bits 51..0 sont réservés pour tag + payload.
const NANBOX_HEADER: u64 = 0xFFFu64 << 52;
/// Mask pour vérifier les bits 63..52 (header NaN).
const HEADER_MASK: u64 = 0xFFFu64 << 52;

/// Une valeur tagged 8-byte. Stockée en `u64` brute pour exposition
/// directe au cache RAM sans cast bouclage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NanBoxValue(u64);

impl NanBoxValue {
    /// Encode un i64. Si la valeur dépasse 47 bits signés, retourne None.
    pub fn from_i48(v: i64) -> Option<Self> {
        const MAX: i64 = (1i64 << 47) - 1;
        const MIN: i64 = -(1i64 << 47);
        if v > MAX || v < MIN {
            return None;
        }
        // Mask à 48 bits ; les bits hauts de signe sont reproduits par
        // l'extension lors du décodage.
        let payload = (v as u64) & PAYLOAD_MASK;
        Some(Self(NANBOX_HEADER | (TAG_I48 << TAG_SHIFT) | payload))
    }

    /// Encode un bool.
    pub fn from_bool(v: bool) -> Self {
        let payload = if v { 1u64 } else { 0u64 };
        Self(NANBOX_HEADER | (TAG_BOOL << TAG_SHIFT) | payload)
    }

    /// Encode un handle Vec u32.
    pub fn from_vec_handle(handle: u32) -> Self {
        let payload = handle as u64;
        Self(NANBOX_HEADER | (TAG_VEC_HANDLE << TAG_SHIFT) | payload)
    }

    /// Encode un f64 réel (preserve NaN/inf via le sentinel tag 0xF).
    /// Pour les NaN qui collisionnent avec NANBOX_HEADER, on les
    /// canonicalise (perte d'info de payload mais sémantique f64
    /// préservée — un NaN reste un NaN).
    pub fn from_f64(v: f64) -> Self {
        let bits = v.to_bits();
        // Si c'est un NaN avec tag ∈ {1,2,3} (un de nos slots), on
        // canonicalise à F64_SENTINEL pour éviter l'ambiguïté.
        if (bits & HEADER_MASK) == NANBOX_HEADER {
            let t = (bits & TAG_MASK) >> TAG_SHIFT;
            if t == TAG_I48 || t == TAG_BOOL || t == TAG_VEC_HANDLE {
                return Self(NANBOX_HEADER | (TAG_F64_SENTINEL << TAG_SHIFT));
            }
        }
        Self(bits)
    }

    /// Décode un i48 si le tag matche.
    pub fn as_i48(&self) -> Option<i64> {
        if self.tag() != Some(TAG_I48) {
            return None;
        }
        let raw = self.0 & PAYLOAD_MASK;
        // Sign-extend depuis bit 47.
        let signed = if raw & (1u64 << 47) != 0 {
            (raw | !PAYLOAD_MASK) as i64
        } else {
            raw as i64
        };
        Some(signed)
    }

    /// Décode un bool si le tag matche.
    pub fn as_bool(&self) -> Option<bool> {
        if self.tag() != Some(TAG_BOOL) {
            return None;
        }
        Some((self.0 & 1) != 0)
    }

    /// Décode un handle Vec si le tag matche.
    pub fn as_vec_handle(&self) -> Option<u32> {
        if self.tag() != Some(TAG_VEC_HANDLE) {
            return None;
        }
        Some((self.0 & 0xFFFF_FFFF) as u32)
    }

    /// Décode un f64 si la valeur n'est PAS un NaN-boxed tag.
    pub fn as_f64(&self) -> Option<f64> {
        if self.tag().is_some() {
            // C'est un de nos tags — pas un f64 réel.
            return None;
        }
        Some(f64::from_bits(self.0))
    }

    /// Retourne le tag si c'est un NaN-boxed valeur, None si f64 normal.
    fn tag(&self) -> Option<u64> {
        // Vérifier que les 12 bits hauts forment notre header NaN.
        if (self.0 & HEADER_MASK) != NANBOX_HEADER {
            return None;
        }
        let t = (self.0 & TAG_MASK) >> TAG_SHIFT;
        // F64_SENTINEL = "vrai f64" → on retourne None pour qu'as_f64
        // décode. Tags 1..3 = nos types. Tags 0 ou 4..14 = invalide
        // (slot non encore utilisé) → on les traite comme f64 réels.
        match t {
            TAG_I48 | TAG_BOOL | TAG_VEC_HANDLE => Some(t),
            _ => None,
        }
    }

    /// Représentation u64 brute. Utile pour stocker en cache compact.
    pub fn to_bits(&self) -> u64 {
        self.0
    }

    /// Reconstruit depuis bits (round-trip).
    pub fn from_bits(b: u64) -> Self {
        Self(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nanbox_size_is_8_bytes() {
        // Propriété centrale Wave 2 : Σ.4/Π.6 cache density.
        assert_eq!(std::mem::size_of::<NanBoxValue>(), 8);
    }

    #[test]
    fn nanbox_i48_roundtrip() {
        for v in [0i64, 1, -1, 1234567890, -42, (1i64 << 47) - 1, -(1i64 << 47)] {
            let nb = NanBoxValue::from_i48(v).unwrap_or_else(|| panic!("encode {} failed", v));
            assert_eq!(nb.as_i48(), Some(v), "round-trip i48 sur {}", v);
            assert_eq!(nb.as_bool(), None);
            assert_eq!(nb.as_vec_handle(), None);
            assert_eq!(nb.as_f64(), None);
        }
    }

    #[test]
    fn nanbox_i48_rejects_overflow() {
        // 2^48 = 281_474_976_710_656 — au-delà de notre fenêtre 47-bit signed.
        assert!(NanBoxValue::from_i48(1i64 << 48).is_none());
        assert!(NanBoxValue::from_i48(-(1i64 << 48)).is_none());
        // i64::MAX clairement rejeté.
        assert!(NanBoxValue::from_i48(i64::MAX).is_none());
        assert!(NanBoxValue::from_i48(i64::MIN).is_none());
    }

    #[test]
    fn nanbox_bool_roundtrip() {
        let nb_t = NanBoxValue::from_bool(true);
        assert_eq!(nb_t.as_bool(), Some(true));
        assert_eq!(nb_t.as_i48(), None);
        let nb_f = NanBoxValue::from_bool(false);
        assert_eq!(nb_f.as_bool(), Some(false));
    }

    #[test]
    fn nanbox_vec_handle_roundtrip() {
        for h in [0u32, 1, 12345, u32::MAX] {
            let nb = NanBoxValue::from_vec_handle(h);
            assert_eq!(nb.as_vec_handle(), Some(h));
            assert_eq!(nb.as_i48(), None);
            assert_eq!(nb.as_bool(), None);
        }
    }

    #[test]
    fn nanbox_tags_distinct() {
        // Trois Values différents avec le même payload = 1 doivent être
        // distincts et décodés correctement.
        let i = NanBoxValue::from_i48(1).unwrap();
        let b = NanBoxValue::from_bool(true);
        let v = NanBoxValue::from_vec_handle(1);
        assert_ne!(i.to_bits(), b.to_bits());
        assert_ne!(b.to_bits(), v.to_bits());
        assert_ne!(i.to_bits(), v.to_bits());
        assert_eq!(i.as_i48(), Some(1));
        assert_eq!(b.as_bool(), Some(true));
        assert_eq!(v.as_vec_handle(), Some(1));
    }

    #[test]
    fn nanbox_bits_roundtrip() {
        let nb = NanBoxValue::from_i48(-42).unwrap();
        let bits = nb.to_bits();
        let back = NanBoxValue::from_bits(bits);
        assert_eq!(back.as_i48(), Some(-42));
        assert_eq!(nb, back);
    }

    #[test]
    fn nanbox_f64_canonical_doesnt_collide() {
        // f64 normaux (0.0, 1.0, π, ...) ne doivent PAS matcher nos tags.
        for v in [0.0f64, 1.0, -1.0, 3.14159, 1e100, -1e-100] {
            let nb = NanBoxValue::from_f64(v);
            assert_eq!(nb.as_f64(), Some(v), "round-trip f64 sur {}", v);
            assert!(nb.as_i48().is_none());
            assert!(nb.as_bool().is_none());
            assert!(nb.as_vec_handle().is_none());
        }
    }
}
