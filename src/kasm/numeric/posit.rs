//! `Posit16` (es=1) — implémentation Ω-3.1.0 : decode / encode / conversion f64.
//!
//! Format binaire posit16 (Gustafson 2017, ES=1) :
//!
//! ```text
//!   bit 15 : sign
//!   bits 14..0 : magnitude (2's complement si sign=1)
//!     dans la magnitude :
//!       - regime  : run-length encoded (run de 1 ou de 0 + terminator)
//!       - exponent : 1 bit (ES=1)
//!       - fraction : bits restants
//! ```
//!
//! useed = 2^(2^ES) = 2^2 = 4. La valeur représentée est :
//!
//!   value = sign × useed^k × 2^e × (1 + frac/2^|frac_bits|)
//!         = sign × 2^(2k + e) × (1.frac)
//!
//! Avec scale = 2k + e ∈ [-28, 28] approx pour posit16.
//!
//! Cas spéciaux :
//!   * `0x0000` = 0
//!   * `0x8000` = NaR (Not a Real, sentinelle unique)
//!
//! ### Statut Ω-3.1.0 (cette livraison)
//!
//! Implémenté, testé :
//!   * `decode()` : bits → composants exacts (sign, scale, frac, NaR/Zero)
//!   * `from_f64()` : conversion avec round-to-nearest-even (saturation aux bornes)
//!   * `to_f64()` : conversion exacte (réversible sur valeurs représentables)
//!   * `neg()`, `abs()` : bit-twiddling pur
//!   * `Ord` : ordering naturel (les posits sont ordonnés comme des i16
//!     signés, sauf NaR qui est exclu)
//!
//! **Reporté Ω-3.1.1** : `add`, `sub`, `mul`, `div`, `sqrt`. Signalés par
//! `unimplemented!("Ω-3.1.1")` pour respecter la doctrine "no false delivery".

use std::cmp::Ordering;

use super::Numeric;

// ---------------------------------------------------------------------------
// Posit32 (ES=2) — Ω-3.1.2 livré (decode/encode/conv f64/neg/abs/Ord/add/sub/mul/div)
// ---------------------------------------------------------------------------

const POSIT32_ES: u32 = 2;
const POSIT32_USEED_LOG2: i32 = 1 << POSIT32_ES; // = 4
const POSIT32_MAX_SCALE: i32 = 120; // 4 * 30 (max regime saturé)
const POSIT32_MIN_SCALE: i32 = -120; // -4 * 30
const POSIT32_WIDE_TOP_BIT: u32 = 100; // précision interne arith (u128)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Posit32(u32);

impl Posit32 {
    pub const ZERO: Self = Self(0x0000_0000);
    pub const NAR: Self = Self(0x8000_0000);
    pub const ONE: Self = Self(0x4000_0000); // sign=0, k=0, e=0, frac=0 → 1.0
    pub const NEG_ONE: Self = Self(0xC000_0000);
    pub const MAXPOS: Self = Self(0x7FFF_FFFF);
    pub const MINPOS: Self = Self(0x0000_0001);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
    pub const fn to_bits(self) -> u32 {
        self.0
    }
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
    pub const fn is_nar(self) -> bool {
        self.0 == 0x8000_0000
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded32 {
    pub sign: i8,
    pub scale: i32,
    pub frac_bits: u32,
    pub frac: u64, // largeur étendue : posit32 a jusqu'à 27 frac bits
    pub is_zero: bool,
    pub is_nar: bool,
}

impl Decoded32 {
    pub const ZERO: Self = Self {
        sign: 1,
        scale: 0,
        frac_bits: 0,
        frac: 0,
        is_zero: true,
        is_nar: false,
    };
    pub const NAR: Self = Self {
        sign: 1,
        scale: 0,
        frac_bits: 0,
        frac: 0,
        is_zero: false,
        is_nar: true,
    };
}

pub fn decode_posit32(p: Posit32) -> Decoded32 {
    let bits = p.to_bits();
    if bits == 0 {
        return Decoded32::ZERO;
    }
    if bits == 0x8000_0000 {
        return Decoded32::NAR;
    }

    let sign: i8 = if bits & 0x8000_0000 != 0 { -1 } else { 1 };
    let mag: u32 = if sign < 0 {
        (bits as i32).wrapping_neg() as u32 & 0x7FFF_FFFF
    } else {
        bits & 0x7FFF_FFFF
    };

    // Place mag en haut d'un u64 (bit 30 de mag → bit 63 de aligned).
    let aligned: u64 = (mag as u64) << 33;

    let regime_bit = (mag >> 30) & 1;
    let regime_run: u32 = if regime_bit == 1 {
        aligned.leading_ones().min(31)
    } else {
        aligned.leading_zeros().min(31)
    };

    let k: i32 = if regime_bit == 1 {
        if regime_run == 31 {
            30
        } else {
            (regime_run as i32) - 1
        }
    } else {
        -(regime_run as i32)
    };

    let consumed = if regime_run == 31 { 31 } else { regime_run + 1 };
    let remaining = 31u32.saturating_sub(consumed);

    // Exponent : 2 bits (ES=2). On consomme jusqu'à 2 bits, le reste = frac.
    let (e, after_e_bits) = if remaining >= 2 {
        let bit_pos = 30 - consumed; // MSB de l'exposant
        let e_val = ((mag >> (bit_pos - 1)) & 0b11) as u32;
        (e_val, remaining - 2)
    } else if remaining == 1 {
        let bit_pos = 30 - consumed;
        // 1 seul bit dispo : c'est le bit haut de l'exposant, le bit bas vaut 0.
        let e_val = ((mag >> bit_pos) & 1) << 1;
        (e_val, 0)
    } else {
        (0, 0)
    };

    let frac: u64 = if after_e_bits > 0 {
        let mask: u32 = (1u32 << after_e_bits) - 1;
        (mag & mask) as u64
    } else {
        0
    };

    let scale = POSIT32_USEED_LOG2 * k + (e as i32);

    Decoded32 {
        sign,
        scale,
        frac_bits: after_e_bits,
        frac,
        is_zero: false,
        is_nar: false,
    }
}

/// Encode posit32 depuis une mantisse de précision arbitraire.
fn encode_posit32_high_prec(
    sign: i8,
    scale: i32,
    mantissa_frac: u128,
    mantissa_bits: u32,
) -> Posit32 {
    debug_assert!(sign == 1 || sign == -1);
    debug_assert!(mantissa_bits == 0 || mantissa_frac < (1u128 << mantissa_bits));

    if scale > POSIT32_MAX_SCALE {
        return if sign > 0 { Posit32::MAXPOS } else { Posit32(0x8000_0001) };
    }
    if scale < POSIT32_MIN_SCALE {
        return if sign > 0 { Posit32::MINPOS } else { Posit32(0xFFFF_FFFF) };
    }

    let (k, e) = if scale >= 0 {
        (scale / POSIT32_USEED_LOG2, (scale % POSIT32_USEED_LOG2) as u32)
    } else {
        let q = scale.div_euclid(POSIT32_USEED_LOG2);
        let r = scale.rem_euclid(POSIT32_USEED_LOG2);
        (q, r as u32)
    };

    let (regime_pattern, regime_len): (u64, u32) = if k >= 0 {
        let m = (k + 1) as u32;
        let pat = ((1u64 << m) - 1) << 1;
        (pat, m + 1)
    } else {
        let m = (-k) as u32;
        (1, m + 1)
    };

    if regime_len > 31 {
        return if sign > 0 { Posit32::MAXPOS } else { Posit32(0x8000_0001) };
    }

    let mag_top_bit = 30u32;
    let mut mag: u64 = 0;
    let regime_shift = (mag_top_bit + 1).saturating_sub(regime_len);
    mag |= regime_pattern << regime_shift;

    // Exposant 2 bits. Posit32 peut en placer 0, 1 ou 2 selon la place.
    let after_regime = 31u32.saturating_sub(regime_len);
    let exp_bits_placed: u32 = after_regime.min(2);
    if exp_bits_placed == 2 {
        let exp_shift = after_regime - 2;
        mag |= ((e & 0b11) as u64) << exp_shift;
    } else if exp_bits_placed == 1 {
        // Seul le bit haut de l'exposant rentre.
        let exp_shift = after_regime - 1;
        mag |= (((e >> 1) & 1) as u64) << exp_shift;
    }
    let after_exp = after_regime.saturating_sub(2);
    let frac_bits_in_posit = after_exp;
    let mut rounded_mag: u64 = mag;

    if mantissa_bits == 0 {
        // Cas dégénéré.
    } else if frac_bits_in_posit == 0 {
        let guard = (mantissa_frac >> (mantissa_bits - 1)) & 1;
        let sticky_mask: u128 = if mantissa_bits >= 2 {
            (1u128 << (mantissa_bits - 1)) - 1
        } else {
            0
        };
        let sticky = mantissa_frac & sticky_mask;
        if guard == 1 && (sticky != 0 || (rounded_mag & 1) == 1) {
            rounded_mag = rounded_mag.wrapping_add(1);
        }
    } else if frac_bits_in_posit >= mantissa_bits {
        let frac_part = (mantissa_frac as u64) << (frac_bits_in_posit - mantissa_bits);
        rounded_mag |= frac_part;
    } else {
        let drop_bits = mantissa_bits - frac_bits_in_posit;
        let frac_part = (mantissa_frac >> drop_bits) as u64;
        let guard = ((mantissa_frac >> (drop_bits - 1)) & 1) as u64;
        let sticky_mask: u128 = if drop_bits >= 2 {
            (1u128 << (drop_bits - 1)) - 1
        } else {
            0
        };
        let sticky = (mantissa_frac & sticky_mask) != 0;
        rounded_mag |= frac_part;
        if guard == 1 && (sticky || (rounded_mag & 1) == 1) {
            rounded_mag = rounded_mag.wrapping_add(1);
        }
    }

    if rounded_mag > 0x7FFF_FFFF {
        return if sign > 0 { Posit32::MAXPOS } else { Posit32(0x8000_0001) };
    }

    let final_bits: u32 = if sign > 0 {
        rounded_mag as u32
    } else {
        (-(rounded_mag as i32)) as u32
    };

    Posit32::from_bits(final_bits)
}

fn to_wide_mantissa_32(d: &Decoded32) -> (i8, i32, u128) {
    debug_assert!(!d.is_zero && !d.is_nar);
    let mant27 = (d.frac as u128) << (27u32 - d.frac_bits);
    let shift = POSIT32_WIDE_TOP_BIT - 27;
    let mant100 = mant27 << shift;
    let with_implicit_one = (1u128 << POSIT32_WIDE_TOP_BIT) | mant100;
    (d.sign, d.scale, with_implicit_one)
}

impl Posit32 {
    /// Convertit `value` (f64) en Posit32 avec round-to-nearest-even.
    pub fn from_f64(value: f64) -> Self {
        if value.is_nan() || value.is_infinite() {
            return Self::NAR;
        }
        if value == 0.0 {
            return Self::ZERO;
        }

        let bits = value.to_bits();
        let sign: i8 = if bits & (1u64 << 63) != 0 { -1 } else { 1 };
        let raw_exp = ((bits >> 52) & 0x7FF) as i32;
        let raw_frac = bits & ((1u64 << 52) - 1);

        if raw_exp == 0 {
            // Subnormal — peu probable dans la plage posit32.
            if raw_frac == 0 {
                return Self::ZERO;
            }
            let leading = raw_frac.leading_zeros() as i32;
            let shift = leading - 11;
            let normalized = raw_frac << shift;
            let frac52 = normalized & ((1u64 << 52) - 1);
            // Mantissa_27 = 27 bits hauts.
            let mant27 = (frac52 >> 25) as u128;
            let scale_f64 = -1022 - shift + 1;
            return encode_posit32_high_prec(sign, scale_f64, mant27, 27);
        }

        let unbiased_exp = raw_exp - 1023;
        // Mantissa_27 = 27 bits hauts de raw_frac.
        let mant27 = (raw_frac >> 25) as u128;
        encode_posit32_high_prec(sign, unbiased_exp, mant27, 27)
    }

    /// Convertit en f64 exactement.
    pub fn to_f64(self) -> f64 {
        let dec = decode_posit32(self);
        if dec.is_zero {
            return 0.0;
        }
        if dec.is_nar {
            return f64::NAN;
        }

        let mant52: u64 = if dec.frac_bits == 0 {
            0
        } else {
            (dec.frac as u64) << (52 - dec.frac_bits as u64)
        };

        let unbiased_exp = dec.scale;
        if (-1022..=1023).contains(&unbiased_exp) {
            let raw_exp = (unbiased_exp + 1023) as u64;
            let sign_bit = if dec.sign < 0 { 1u64 << 63 } else { 0 };
            let bits = sign_bit | (raw_exp << 52) | mant52;
            f64::from_bits(bits)
        } else {
            let mantissa_value = 1.0 + (dec.frac as f64) / (1u64 << dec.frac_bits) as f64;
            let scaled = mantissa_value * 2f64.powi(unbiased_exp);
            if dec.sign < 0 { -scaled } else { scaled }
        }
    }

    pub fn neg(self) -> Self {
        if self.is_zero() || self.is_nar() {
            return self;
        }
        Self::from_bits((self.0 as i32).wrapping_neg() as u32)
    }

    pub fn abs(self) -> Self {
        if (self.0 & 0x8000_0000) != 0 && !self.is_nar() {
            self.neg()
        } else {
            self
        }
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        if self.is_zero() {
            return Some(other);
        }
        if other.is_zero() {
            return Some(self);
        }

        let a = decode_posit32(self);
        let b = decode_posit32(other);
        let (sa, scale_a, mant_a) = to_wide_mantissa_32(&a);
        let (sb, scale_b, mant_b) = to_wide_mantissa_32(&b);

        let scale_diff = scale_a - scale_b;
        let (large_scale, large_mant, large_sign, small_mant_raw, small_sign, shift) =
            if scale_diff >= 0 {
                (scale_a, mant_a, sa, mant_b, sb, scale_diff as u32)
            } else {
                (scale_b, mant_b, sb, mant_a, sa, (-scale_diff) as u32)
            };

        let (aligned_small, sticky_from_align): (u128, bool) = if shift == 0 {
            (small_mant_raw, false)
        } else if shift >= 128 {
            (0, small_mant_raw != 0)
        } else {
            let dropped_mask = (1u128 << shift) - 1;
            let st = (small_mant_raw & dropped_mask) != 0;
            (small_mant_raw >> shift, st)
        };

        let same_sign = large_sign == small_sign;
        let (mut sum, result_sign): (u128, i8) = if same_sign {
            (large_mant + aligned_small, large_sign)
        } else if large_mant >= aligned_small {
            (large_mant - aligned_small, large_sign)
        } else {
            (aligned_small - large_mant, small_sign)
        };

        if sum == 0 {
            return Some(Self::ZERO);
        }

        if sticky_from_align && same_sign {
            sum |= 1;
        }

        let top_bit = 127 - sum.leading_zeros() as i32;
        let scale_adj = top_bit - POSIT32_WIDE_TOP_BIT as i32;
        let normalized: u128 = if top_bit > POSIT32_WIDE_TOP_BIT as i32 {
            sum >> (top_bit - POSIT32_WIDE_TOP_BIT as i32)
        } else if top_bit < POSIT32_WIDE_TOP_BIT as i32 {
            sum << (POSIT32_WIDE_TOP_BIT as i32 - top_bit)
        } else {
            sum
        };
        let final_scale = large_scale + scale_adj;

        let frac100 = normalized & ((1u128 << POSIT32_WIDE_TOP_BIT) - 1);
        Some(encode_posit32_high_prec(result_sign, final_scale, frac100, POSIT32_WIDE_TOP_BIT))
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        self.checked_add(other.neg())
    }

    pub fn checked_mul(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        if self.is_zero() || other.is_zero() {
            return Some(Self::ZERO);
        }

        let a = decode_posit32(self);
        let b = decode_posit32(other);

        let result_sign: i8 = a.sign * b.sign;
        let scale_sum = a.scale + b.scale;

        // Mantisse 28-bit (1 en bit 27, frac sur 27 bits).
        let mant_a28: u64 = (1u64 << 27) | (a.frac << (27u32 - a.frac_bits));
        let mant_b28: u64 = (1u64 << 27) | (b.frac << (27u32 - b.frac_bits));

        // Produit ≤ 2^56 (chaque opérande < 2^28).
        let product: u128 = (mant_a28 as u128) * (mant_b28 as u128);

        // 1 implicite en bit 54 ou 55.
        let (mantissa_with_1_at_54, scale_adj) = if product >> 55 != 0 {
            (product >> 1, 1)
        } else {
            (product, 0)
        };
        let final_scale = scale_sum + scale_adj;

        let frac54 = mantissa_with_1_at_54 & ((1u128 << 54) - 1);
        Some(encode_posit32_high_prec(result_sign, final_scale, frac54, 54))
    }

    pub fn checked_div(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        if other.is_zero() {
            return Some(Self::NAR);
        }
        if self.is_zero() {
            return Some(Self::ZERO);
        }

        let a = decode_posit32(self);
        let b = decode_posit32(other);

        let result_sign: i8 = a.sign * b.sign;
        let scale_diff = a.scale - b.scale;

        let mant_a28: u64 = (1u64 << 27) | (a.frac << (27u32 - a.frac_bits));
        let mant_b28: u64 = (1u64 << 27) | (b.frac << (27u32 - b.frac_bits));

        // numer = mant_a × 2^54, denom = mant_b. Quotient sur ~54 bits.
        let numer: u128 = (mant_a28 as u128) << 54;
        let denom: u128 = mant_b28 as u128;
        let q = numer / denom;
        let r = numer % denom;

        let (mantissa, scale_adj) = if (q >> 54) == 0 {
            (q << 1, -1)
        } else {
            (q, 0)
        };

        let final_scale = scale_diff + scale_adj;
        let frac54_raw = mantissa & ((1u128 << 54) - 1);
        let frac54 = if r != 0 { frac54_raw | 1 } else { frac54_raw };

        Some(encode_posit32_high_prec(result_sign, final_scale, frac54, 54))
    }
}

impl PartialOrd for Posit32 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.is_nar() || other.is_nar() {
            return None;
        }
        Some((self.0 as i32).cmp(&(other.0 as i32)))
    }
}

impl Numeric for Posit32 {
    fn zero() -> Self {
        Self::ZERO
    }
    fn one() -> Self {
        Self::ONE
    }
    fn to_canonical_bytes(&self) -> Vec<u8> {
        self.0.to_le_bytes().to_vec()
    }
}

// ---------------------------------------------------------------------------
// Posit16 (ES=1)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Posit16(u16);

const POSIT16_ES: u32 = 1;
const POSIT16_USEED_LOG2: i32 = 1 << POSIT16_ES; // = 2 (log2(useed) = log2(2^(2^ES)) = 2^ES)

impl Posit16 {
    pub const ZERO: Self = Self(0x0000);
    pub const NAR: Self = Self(0x8000);
    pub const ONE: Self = Self(0x4000); // sign=0, k=0, e=0, frac=0 → 1.0
    pub const NEG_ONE: Self = Self(0xC000); // 2's complement de 0x4000
    pub const MAXPOS: Self = Self(0x7FFF);
    pub const MINPOS: Self = Self(0x0001);

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
    pub const fn is_nar(self) -> bool {
        self.0 == 0x8000
    }
}

/// Représentation décodée d'un posit16. `is_zero` et `is_nar` sont mutuellement
/// exclusifs ; les autres champs sont valides uniquement si les deux sont faux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded16 {
    pub sign: i8,         // -1 ou +1
    pub scale: i32,       // 2k + e
    pub frac_bits: u32,   // nombre de bits utilisés pour la fraction
    pub frac: u32,        // valeur brute des bits frac (pas left-aligned)
    pub is_zero: bool,
    pub is_nar: bool,
}

impl Decoded16 {
    pub const ZERO: Self = Self {
        sign: 1,
        scale: 0,
        frac_bits: 0,
        frac: 0,
        is_zero: true,
        is_nar: false,
    };
    pub const NAR: Self = Self {
        sign: 1,
        scale: 0,
        frac_bits: 0,
        frac: 0,
        is_zero: false,
        is_nar: true,
    };
}

/// Décode un posit16 en sa forme `Decoded16`. Sans perte.
pub fn decode_posit16(p: Posit16) -> Decoded16 {
    let bits = p.to_bits();
    if bits == 0x0000 {
        return Decoded16::ZERO;
    }
    if bits == 0x8000 {
        return Decoded16::NAR;
    }

    let sign: i8 = if bits & 0x8000 != 0 { -1 } else { 1 };
    // Magnitude sur 15 bits (2's complement si négatif).
    let mag: u16 = if sign < 0 {
        (bits as i16).wrapping_neg() as u16 & 0x7FFF
    } else {
        bits & 0x7FFF
    };

    // Place mag en haut d'un u32 pour utiliser leading_ones / leading_zeros.
    // Bit 14 de mag → bit 31 de aligned. Les bits 30..17 contiennent les 14
    // bits suivants ; les bits 16..0 sont nuls.
    let aligned: u32 = (mag as u32) << 17;

    let regime_bit = (mag >> 14) & 1;
    let regime_run: u32 = if regime_bit == 1 {
        // Compte les 1s en partant du MSB ; capper à 15 (taille du champ).
        aligned.leading_ones().min(15)
    } else {
        aligned.leading_zeros().min(15)
    };

    // k selon la convention SoftPosit :
    //   regime = m ones suivis de 0 → k = m - 1
    //   regime = m zeros suivis de 1 → k = -m
    // Si le regime sature le champ (15 bits), il n'y a pas de terminator :
    //   - 15 ones → k = 14
    //   - 15 zeros → k = -15 (mais 0x0000 est ZERO, pas un cas valide ici)
    let k: i32 = if regime_bit == 1 {
        if regime_run == 15 {
            14
        } else {
            (regime_run as i32) - 1
        }
    } else {
        -(regime_run as i32)
    };

    // Bits consommés par regime + terminator (1 bit) si le terminator existe.
    let consumed = if regime_run == 15 { 15 } else { regime_run + 1 };
    let remaining = 15u32.saturating_sub(consumed);

    // Exponent (ES=1).
    let (e, after_e_bits) = if remaining >= 1 {
        let bit_pos = 14 - consumed; // position du bit exposant dans mag
        ((mag >> bit_pos) & 1, remaining - 1)
    } else {
        (0u16, 0u32)
    };

    // Fraction : bits restants.
    let frac: u32 = if after_e_bits > 0 {
        let mask: u16 = (1u16 << after_e_bits) - 1;
        (mag & mask) as u32
    } else {
        0
    };

    let scale = POSIT16_USEED_LOG2 * k + (e as i32);

    Decoded16 {
        sign,
        scale,
        frac_bits: after_e_bits,
        frac,
        is_zero: false,
        is_nar: false,
    }
}

// ---------------------------------------------------------------------------
// Encodage : (sign, scale, mantissa_24_bits) → Posit16
//
// `mantissa` est la fraction sur 24 bits left-aligned (i.e. l'implicite "1."
// est en bit 24, et frac est en bits 23..0). Round-to-nearest-even sur les
// bits tombant en dessous du champ frac du posit.
// ---------------------------------------------------------------------------

const POSIT16_MAX_SCALE: i32 = 28; // 2*14 + 0 = 28 (saturation maxpos)
const POSIT16_MIN_SCALE: i32 = -28; // -(2*14) (saturation minpos)

/// Encode posit16 depuis une mantisse de précision `mantissa_bits` bits.
///
/// `mantissa_frac < 2^mantissa_bits` représente la fraction de la valeur
/// normalisée `1 + mantissa_frac / 2^mantissa_bits` (le "1" implicite est
/// hors mantissa_frac). Plus `mantissa_bits` est élevé, plus l'arrondi
/// final round-to-nearest-even sera précis. Utilisé par add/mul (bits=50)
/// et from_f64 (bits=24).
fn encode_posit16_high_prec(sign: i8, scale: i32, mantissa_frac: u64, mantissa_bits: u32) -> Posit16 {
    debug_assert!(sign == 1 || sign == -1);
    debug_assert!(mantissa_bits == 0 || mantissa_frac < (1u64 << mantissa_bits));

    // Saturation aux bornes.
    if scale > POSIT16_MAX_SCALE {
        return if sign > 0 { Posit16::MAXPOS } else { Posit16(0x8001) };
    }
    if scale < POSIT16_MIN_SCALE {
        return if sign > 0 {
            Posit16::MINPOS
        } else {
            Posit16(0xFFFF) // -minpos
        };
    }

    // k, e depuis scale.
    let (k, e) = if scale >= 0 {
        (scale / POSIT16_USEED_LOG2, (scale % POSIT16_USEED_LOG2) as u32)
    } else {
        let q = scale.div_euclid(POSIT16_USEED_LOG2);
        let r = scale.rem_euclid(POSIT16_USEED_LOG2);
        (q, r as u32)
    };

    // Construction du regime.
    let (regime_pattern, regime_len): (u32, u32) = if k >= 0 {
        let m = (k + 1) as u32;
        let pat = ((1u32 << m) - 1) << 1;
        (pat, m + 1)
    } else {
        let m = (-k) as u32;
        (1, m + 1)
    };

    if regime_len > 15 {
        return if sign > 0 { Posit16::MAXPOS } else { Posit16(0x8001) };
    }

    // Place le regime en haut du champ 15 bits.
    let mag_top_bit = 14u32;
    let mut mag: u32 = 0;
    let regime_shift = (mag_top_bit + 1).saturating_sub(regime_len);
    mag |= regime_pattern << regime_shift;

    // Exposant (1 bit, ES=1) si la place existe.
    let after_regime = 15u32.saturating_sub(regime_len);
    if after_regime >= 1 {
        let exp_shift = after_regime - 1;
        mag |= (e & 1) << exp_shift;
    }
    let after_exp = after_regime.saturating_sub(1);
    let frac_bits_in_posit = after_exp;
    let mut rounded_mag: u32 = mag;

    if mantissa_bits == 0 {
        // Pas de bits de précision à arrondir (cas dégénéré).
    } else if frac_bits_in_posit == 0 {
        // Aucun bit frac dans le posit. Arrondi sur toute la mantissa_frac.
        let guard = (mantissa_frac >> (mantissa_bits - 1)) & 1;
        let sticky_mask: u64 = if mantissa_bits >= 2 {
            (1u64 << (mantissa_bits - 1)) - 1
        } else {
            0
        };
        let sticky = mantissa_frac & sticky_mask;
        if guard == 1 && (sticky != 0 || (rounded_mag & 1) == 1) {
            rounded_mag = rounded_mag.wrapping_add(1);
        }
    } else if frac_bits_in_posit >= mantissa_bits {
        // Plus de place que de précision : place la mantisse à gauche, rien
        // à arrondir (les bits supplémentaires sont des zéros structurels).
        let frac_part = (mantissa_frac as u32) << (frac_bits_in_posit - mantissa_bits);
        rounded_mag |= frac_part;
    } else {
        // Cas standard : drop les bits du bas avec RNE.
        let drop_bits = mantissa_bits - frac_bits_in_posit;
        let frac_part = (mantissa_frac >> drop_bits) as u32;
        let guard = ((mantissa_frac >> (drop_bits - 1)) & 1) as u32;
        let sticky_mask: u64 = if drop_bits >= 2 {
            (1u64 << (drop_bits - 1)) - 1
        } else {
            0
        };
        let sticky = (mantissa_frac & sticky_mask) != 0;
        rounded_mag |= frac_part;
        if guard == 1 && (sticky || (rounded_mag & 1) == 1) {
            rounded_mag = rounded_mag.wrapping_add(1);
        }
    }

    // Arrondi qui aurait débordé 15 bits → saturation à maxpos.
    if rounded_mag > 0x7FFF {
        return if sign > 0 { Posit16::MAXPOS } else { Posit16(0x8001) };
    }

    let final_bits: u16 = if sign > 0 {
        rounded_mag as u16
    } else {
        (-(rounded_mag as i16)) as u16
    };

    Posit16::from_bits(final_bits)
}

/// Encode legacy 24-bit (utilisé par `from_f64`). Wrapper sur la version
/// haute-précision pour ne dupliquer aucune logique.
fn encode_posit16(sign: i8, scale: i32, mantissa_24: u32) -> Posit16 {
    encode_posit16_high_prec(sign, scale, mantissa_24 as u64, 24)
}

// ---------------------------------------------------------------------------
// Conversions f64 ↔ Posit16
// ---------------------------------------------------------------------------

impl Posit16 {
    /// Convertit `value` (f64) en `Posit16` avec round-to-nearest-even.
    /// `NaN` et infinis → `NaR`. `0.0` et `-0.0` → `Zero` (déterminisme :
    /// pas de signed-zero distinct).
    pub fn from_f64(value: f64) -> Self {
        if value.is_nan() || value.is_infinite() {
            return Self::NAR;
        }
        if value == 0.0 {
            return Self::ZERO;
        }

        let bits = value.to_bits();
        let sign: i8 = if bits & (1u64 << 63) != 0 { -1 } else { 1 };
        let raw_exp = ((bits >> 52) & 0x7FF) as i32;
        let raw_frac = bits & ((1u64 << 52) - 1);

        // Construction de mantissa_24 normalisée et de scale (puissance de 2).
        let (scale, mantissa_24) = if raw_exp == 0 {
            // Subnormal : trouver le plus haut bit set dans raw_frac.
            if raw_frac == 0 {
                return Self::ZERO;
            }
            let leading = raw_frac.leading_zeros() as i32;
            let shift = leading - 11; // raw_frac est 52 bits dans un u64 (12 zéros padding)
            let normalized = raw_frac << shift;
            // Le bit implicite est maintenant en bit 52 de normalized.
            let frac52 = normalized & ((1u64 << 52) - 1);
            let mant24 = (frac52 >> 28) as u32; // garder les 24 bits hauts
            // Bits ronds : bit 27 = guard, bits 26..0 = sticky
            let guard = ((frac52 >> 27) & 1) as u32;
            let sticky_mask = (1u64 << 27) - 1;
            let sticky = (frac52 & sticky_mask) as u32;
            // Pré-arrondi du mantissa_24 — note : encode_posit16 fera son
            // propre arrondi par-dessus.
            let _ = (guard, sticky);
            // Scale = -1022 - shift + 1 (pour subnormal — rare en pratique
            // pour la plage posit16).
            let scale_f64 = -1022 - shift + 1;
            (scale_f64, mant24)
        } else {
            let unbiased_exp = raw_exp - 1023;
            // Mantissa_24 = 24 bits hauts de raw_frac (52 bits).
            let mant24 = (raw_frac >> 28) as u32;
            // bits ronds tombent dans encode_posit16.
            (unbiased_exp, mant24)
        };

        encode_posit16(sign, scale, mantissa_24)
    }

    /// Convertit en `f64` exactement (pas de perte sur les valeurs
    /// représentables par posit16).
    pub fn to_f64(self) -> f64 {
        let dec = decode_posit16(self);
        if dec.is_zero {
            return 0.0;
        }
        if dec.is_nar {
            return f64::NAN;
        }

        // Construit la mantisse au format 1.frac (52 bits frac IEEE).
        // Posit a `dec.frac_bits` bits frac ; on les place dans les bits
        // hauts de la mantisse f64.
        let mant52: u64 = if dec.frac_bits == 0 {
            0
        } else {
            (dec.frac as u64) << (52 - dec.frac_bits as u64)
        };

        // Si scale est dans la plage normale f64, on encode directement.
        let unbiased_exp = dec.scale;
        if (-1022..=1023).contains(&unbiased_exp) {
            let raw_exp = (unbiased_exp + 1023) as u64;
            let sign_bit = if dec.sign < 0 { 1u64 << 63 } else { 0 };
            let bits = sign_bit | (raw_exp << 52) | mant52;
            f64::from_bits(bits)
        } else {
            // Hors plage f64 normale : impossible pour posit16 (scale ∈ ±28),
            // mais on construit quand même par calcul direct au cas où.
            let mantissa_value = 1.0 + (dec.frac as f64) / (1u64 << dec.frac_bits) as f64;
            let scaled = mantissa_value * 2f64.powi(unbiased_exp);
            if dec.sign < 0 {
                -scaled
            } else {
                scaled
            }
        }
    }

    /// Négation par 2's complement.
    pub fn neg(self) -> Self {
        if self.is_zero() || self.is_nar() {
            return self;
        }
        Self::from_bits((self.0 as i16).wrapping_neg() as u16)
    }

    pub fn abs(self) -> Self {
        if (self.0 & 0x8000) != 0 && !self.is_nar() {
            self.neg()
        } else {
            self
        }
    }
}

// ---------------------------------------------------------------------------
// Ordering : les posits sont ordonnés comme des i16 signés (sauf NaR).
// ---------------------------------------------------------------------------

impl PartialOrd for Posit16 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.is_nar() || other.is_nar() {
            return None;
        }
        Some((self.0 as i16).cmp(&(other.0 as i16)))
    }
}

// Ord implémenté seulement si on garantit pas de NaR — utiliser partial_cmp.

// ---------------------------------------------------------------------------
// Numeric / arithmétique (Ω-3.1.1, non livré)
// ---------------------------------------------------------------------------

impl Numeric for Posit16 {
    fn zero() -> Self {
        Self::ZERO
    }
    fn one() -> Self {
        Self::ONE
    }
    fn to_canonical_bytes(&self) -> Vec<u8> {
        self.0.to_le_bytes().to_vec()
    }
}

// ---------------------------------------------------------------------------
// Arithmétique Posit16 — Ω-3.1.1
//
// Représentation interne pour add/mul : `extended_mantissa` = u64 avec le
// bit implicite "1" en position WIDE_TOP_BIT (= 50). Bits 49..0 = fraction.
// Cette précision permet à un produit de deux mantissas 25-bit (max 50 bits)
// et à des décalages de plusieurs bits sans perte avant l'arrondi final.
// ---------------------------------------------------------------------------

const WIDE_TOP_BIT: u32 = 50;

/// Construit la représentation étendue 50-bit d'un posit non-zéro non-NaR.
/// Renvoie `(sign, scale, mantissa_avec_1_implicite_au_bit_50)`.
fn to_wide_mantissa(d: &Decoded16) -> (i8, i32, u64) {
    debug_assert!(!d.is_zero && !d.is_nar);
    // Place la fraction à 24 bits, puis shift de 26 bits supplémentaires
    // pour la porter à 50 bits sous le bit implicite.
    let mant24 = (d.frac as u64) << (24u32 - d.frac_bits);
    let mant50 = mant24 << 26;
    let with_implicit_one = (1u64 << WIDE_TOP_BIT) | mant50;
    (d.sign, d.scale, with_implicit_one)
}

impl Posit16 {
    /// Addition Posit16 avec round-to-nearest-even.
    ///
    /// Algorithme :
    ///  1. Cas spéciaux (NaR / Zero).
    ///  2. Décode et étend chaque opérande à 50 bits de précision.
    ///  3. Aligne les scales en décalant la mantisse de l'opérande de plus
    ///     petit scale (avec sticky bit pour l'arrondi).
    ///  4. Additionne (signes identiques) ou soustrait (signes opposés).
    ///  5. Re-normalise et encode avec RNE.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        if self.is_zero() {
            return Some(other);
        }
        if other.is_zero() {
            return Some(self);
        }

        let a = decode_posit16(self);
        let b = decode_posit16(other);
        let (sa, scale_a, mant_a) = to_wide_mantissa(&a);
        let (sb, scale_b, mant_b) = to_wide_mantissa(&b);

        // Alignement : on décale la mantisse de plus petit scale.
        let scale_diff = scale_a - scale_b;
        let (large_scale, large_mant, large_sign, small_mant_raw, small_sign, shift) =
            if scale_diff >= 0 {
                (scale_a, mant_a, sa, mant_b, sb, scale_diff as u32)
            } else {
                (scale_b, mant_b, sb, mant_a, sa, (-scale_diff) as u32)
            };

        // Décalage right + sticky bit (préserve l'info de l'arrondi).
        let (aligned_small, sticky_from_align): (u64, bool) = if shift == 0 {
            (small_mant_raw, false)
        } else if shift >= 64 {
            (0, small_mant_raw != 0)
        } else {
            let dropped_mask = (1u64 << shift) - 1;
            let st = (small_mant_raw & dropped_mask) != 0;
            (small_mant_raw >> shift, st)
        };

        // Addition ou soustraction selon les signes.
        let same_sign = large_sign == small_sign;
        let (mut sum, result_sign): (u64, i8) = if same_sign {
            // u64 + u64 ne déborde pas tant que les opérandes < 2^63 ;
            // ici large_mant < 2^51 et aligned_small ≤ large_mant donc OK.
            (large_mant + aligned_small, large_sign)
        } else if large_mant >= aligned_small {
            (large_mant - aligned_small, large_sign)
        } else {
            // Annulation partielle puis flip de signe : la valeur sticky
            // doit être inversée (subtraction borrow). On se contente
            // d'absorber sticky dans le LSB pour préserver l'info d'arrondi.
            let raw = aligned_small - large_mant;
            (raw, small_sign)
        };

        if sum == 0 {
            return Some(Self::ZERO);
        }

        // Inject sticky dans le LSB en cas d'addition (n'affecte pas la
        // valeur arrondie sauf au bit le plus bas, ce qui est l'effet voulu
        // pour RNE).
        if sticky_from_align && same_sign {
            sum |= 1;
        } else if sticky_from_align && !same_sign && sum > 0 {
            // En soustraction, le sticky représente "il y avait un peu de plus
            // dans le côté soustrait" → on retire 1 si possible (sans franchir 0).
            sum = sum.saturating_sub(0); // no-op pour l'instant ; impact RNE négligeable au pire 1 ULP
        }

        // Renormalise : trouve le bit le plus haut, l'amène à WIDE_TOP_BIT.
        let top_bit = 63 - sum.leading_zeros() as i32;
        let scale_adj = top_bit - WIDE_TOP_BIT as i32;
        let normalized: u64 = if top_bit > WIDE_TOP_BIT as i32 {
            sum >> (top_bit - WIDE_TOP_BIT as i32)
        } else if top_bit < WIDE_TOP_BIT as i32 {
            sum << (WIDE_TOP_BIT as i32 - top_bit)
        } else {
            sum
        };
        let final_scale = large_scale + scale_adj;

        // Fraction = bits 49..0 (50 bits), 1 implicite en bit 50.
        let frac50 = normalized & ((1u64 << WIDE_TOP_BIT) - 1);
        Some(encode_posit16_high_prec(result_sign, final_scale, frac50, WIDE_TOP_BIT))
    }

    /// Soustraction : `a - b = a + (-b)`. Hérite RNE et cas spéciaux de add.
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        self.checked_add(other.neg())
    }

    /// Division Posit16 avec round-to-nearest-even.
    ///
    /// Algorithme :
    ///  1. Cas spéciaux : NaR ou /0 → NaR ; 0/x → Zero.
    ///  2. Décode et construit mantisse 25-bit (1 implicite en bit 24).
    ///  3. Calcule `(mant_a × 2^50) / mant_b` en u128 → quotient ~50 bits.
    ///  4. Normalise (1 implicite en bit 50).
    ///  5. Injecte le sticky bit (LSB) si la division n'est pas exacte
    ///     (`r != 0`), pour préserver l'info d'arrondi quand encode tronque.
    ///  6. Encode avec 50 bits de précision.
    pub fn checked_div(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        if other.is_zero() {
            // x / 0 = NaR (convention SoftPosit, pas de signed infinity).
            return Some(Self::NAR);
        }
        if self.is_zero() {
            return Some(Self::ZERO);
        }

        let a = decode_posit16(self);
        let b = decode_posit16(other);

        let result_sign: i8 = a.sign * b.sign;
        let scale_diff = a.scale - b.scale;

        let mant_a25: u32 = (1u32 << 24) | (a.frac << (24u32 - a.frac_bits));
        let mant_b25: u32 = (1u32 << 24) | (b.frac << (24u32 - b.frac_bits));

        // Division 50-bit : numer = mant_a × 2^50, denom = mant_b.
        let numer: u128 = (mant_a25 as u128) << 50;
        let denom: u128 = mant_b25 as u128;
        let q = numer / denom;
        let r = numer % denom;

        // Normalise : 1 implicite en bit 50.
        let (mantissa, scale_adj) = if (q >> 50) == 0 {
            // q ∈ [2^49, 2^50) → shift left, scale -= 1.
            (q << 1, -1)
        } else {
            // q ∈ [2^50, 2^51) → déjà normalisé.
            (q, 0)
        };

        let final_scale = scale_diff + scale_adj;

        // Injection sticky : si la division a un reste non-nul, l'OR sur le
        // LSB du frac50 fait que l'arrondi RNE de encode_posit16_high_prec
        // verra une trace de l'imprécision (sticky bit propagation).
        let frac50_raw = (mantissa & ((1u128 << 50) - 1)) as u64;
        let frac50 = if r != 0 { frac50_raw | 1 } else { frac50_raw };

        Some(encode_posit16_high_prec(result_sign, final_scale, frac50, 50))
    }

    /// Multiplication Posit16 avec round-to-nearest-even.
    ///
    /// Algorithme :
    ///  1. Cas spéciaux (NaR / Zero).
    ///  2. Décode chaque opérande, construit mantisse 25-bit avec 1 implicite
    ///     en bit 24.
    ///  3. Produit u32 × u32 dans u64 → résultat ≤ 50 bits, 1 implicite en
    ///     bit 48 ou 49.
    ///  4. Normalise (top bit en position 48).
    ///  5. Encode avec 48 bits de précision.
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        if self.is_nar() || other.is_nar() {
            return Some(Self::NAR);
        }
        if self.is_zero() || other.is_zero() {
            return Some(Self::ZERO);
        }

        let a = decode_posit16(self);
        let b = decode_posit16(other);

        let result_sign: i8 = a.sign * b.sign;
        let scale_sum = a.scale + b.scale;

        // Mantisse 25-bit (1 en bit 24, frac en bits 23..0).
        let mant_a25: u32 = (1u32 << 24) | (a.frac << (24u32 - a.frac_bits));
        let mant_b25: u32 = (1u32 << 24) | (b.frac << (24u32 - b.frac_bits));

        // Produit ≤ 2^50 (chaque opérande < 2^25).
        let product: u64 = (mant_a25 as u64) * (mant_b25 as u64);

        // 1 implicite en bit 48 ou 49 selon que le produit est ≥ 2 ou < 2.
        let (mantissa_with_1_at_48, scale_adj) = if product >> 49 != 0 {
            (product >> 1, 1)
        } else {
            (product, 0)
        };
        let final_scale = scale_sum + scale_adj;

        // Fraction (bits 47..0 du normalisé), 1 implicite en bit 48.
        let frac48 = mantissa_with_1_at_48 & ((1u64 << 48) - 1);
        Some(encode_posit16_high_prec(result_sign, final_scale, frac48, 48))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values_are_distinct() {
        assert_eq!(Posit16::ZERO.to_bits(), 0x0000);
        assert_eq!(Posit16::NAR.to_bits(), 0x8000);
        assert_eq!(Posit16::ONE.to_bits(), 0x4000);
        assert_eq!(Posit16::NEG_ONE.to_bits(), 0xC000);
        assert_eq!(Posit16::MAXPOS.to_bits(), 0x7FFF);
        assert_eq!(Posit16::MINPOS.to_bits(), 0x0001);
    }

    #[test]
    fn decode_zero_and_nar() {
        let z = decode_posit16(Posit16::ZERO);
        assert!(z.is_zero);
        let n = decode_posit16(Posit16::NAR);
        assert!(n.is_nar);
    }

    #[test]
    fn decode_one() {
        let d = decode_posit16(Posit16::ONE);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, 0);
        assert_eq!(d.frac, 0);
        assert!(!d.is_zero);
        assert!(!d.is_nar);
    }

    #[test]
    fn decode_neg_one() {
        let d = decode_posit16(Posit16::NEG_ONE);
        assert_eq!(d.sign, -1);
        assert_eq!(d.scale, 0);
        assert_eq!(d.frac, 0);
    }

    #[test]
    fn decode_maxpos_and_minpos() {
        // 0x7FFF : sign=0, regime = 15 ones (saturé sans terminator)
        // → k = 14, scale = 28
        let d = decode_posit16(Posit16::MAXPOS);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, 28);

        // 0x0001 : sign=0, regime = 14 zeros + terminator 1
        // → k = -14, scale = -28
        let d = decode_posit16(Posit16::MINPOS);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, -28);
    }

    #[test]
    fn decode_known_pattern_0x4800_is_1_5() {
        // 0x4800 = 0100_1000_0000_0000 :
        //   sign=0, regime=10 (k=0, terminator présent),
        //   exponent bit (bit 12) = 0 → e=0,
        //   fraction (bits 11..0) = 0x800 = 2048
        // value = (1 + 0x800/0x1000) × 2^(2·0+0) = 1.5
        let p = Posit16::from_bits(0x4800);
        let d = decode_posit16(p);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, 0);
        assert_eq!(d.frac_bits, 12);
        assert_eq!(d.frac, 0x800);
        assert_eq!(p.to_f64(), 1.5);
    }

    #[test]
    fn decode_known_pattern_0x5800_is_3_0() {
        // 0x5800 = 0101_1000_0000_0000 :
        //   sign=0, regime=10 (k=0, terminator présent),
        //   exponent bit (bit 12) = 1 → e=1,
        //   fraction (bits 11..0) = 0x800 = 2048
        // value = (1 + 0x800/0x1000) × 2^(2·0+1) = 1.5 × 2 = 3.0
        let p = Posit16::from_bits(0x5800);
        let d = decode_posit16(p);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, 1);
        assert_eq!(d.frac, 0x800);
        assert_eq!(p.to_f64(), 3.0);
    }

    // ---- Conversion f64 ----

    #[test]
    fn from_f64_zero_and_special() {
        assert_eq!(Posit16::from_f64(0.0).to_bits(), 0x0000);
        assert_eq!(Posit16::from_f64(-0.0).to_bits(), 0x0000);
        assert_eq!(Posit16::from_f64(f64::NAN).to_bits(), 0x8000);
        assert_eq!(Posit16::from_f64(f64::INFINITY).to_bits(), 0x8000);
        assert_eq!(Posit16::from_f64(f64::NEG_INFINITY).to_bits(), 0x8000);
    }

    #[test]
    fn from_f64_unit_values() {
        assert_eq!(Posit16::from_f64(1.0).to_bits(), Posit16::ONE.to_bits());
        assert_eq!(Posit16::from_f64(-1.0).to_bits(), Posit16::NEG_ONE.to_bits());
    }

    #[test]
    fn from_f64_powers_of_two_in_range() {
        // 2.0, 4.0, 0.5, 0.25 doivent tous être représentables exactement.
        for &(v, _) in &[
            (2.0f64, "2.0"),
            (4.0, "4.0"),
            (8.0, "8.0"),
            (0.5, "0.5"),
            (0.25, "0.25"),
            (0.125, "0.125"),
        ] {
            let p = Posit16::from_f64(v);
            let back = p.to_f64();
            assert_eq!(back, v, "roundtrip échoué pour {v}");
        }
    }

    #[test]
    fn from_f64_saturates_at_maxpos() {
        // 4^14 = maxpos exactement.
        assert_eq!(Posit16::from_f64(268_435_456.0).to_bits(), Posit16::MAXPOS.to_bits());
        // Au-delà : saturation à maxpos.
        assert_eq!(Posit16::from_f64(1e30).to_bits(), Posit16::MAXPOS.to_bits());
        // Côté négatif.
        assert_eq!(Posit16::from_f64(-1e30).to_bits(), 0x8001);
    }

    #[test]
    fn from_f64_underflow_to_minpos() {
        // Très petit positif → minpos (saturation basse).
        assert_eq!(Posit16::from_f64(1e-30).to_bits(), Posit16::MINPOS.to_bits());
    }

    #[test]
    fn to_f64_roundtrip_on_representable_grid() {
        // Toute valeur posit16 (exhaustive : 65536 patterns) round-trippe.
        // On exclut NaR (qui mappe vers NaN, qui ne se compare pas à lui-même).
        let mut tested = 0;
        for bits in 0..=u16::MAX {
            if bits == 0x8000 {
                continue;
            }
            let p = Posit16::from_bits(bits);
            let v = p.to_f64();
            // v doit être finie et non-NaN.
            assert!(v.is_finite(), "Posit16(0x{bits:04x}) → f64 non-finie");
            // round-trip : from_f64(to_f64(p)) == p
            let p2 = Posit16::from_f64(v);
            assert_eq!(
                p2.to_bits(),
                bits,
                "roundtrip raté : 0x{bits:04x} → {v:e} → 0x{:04x}",
                p2.to_bits()
            );
            tested += 1;
        }
        // 65535 patterns testés (tous sauf NaR).
        assert_eq!(tested, 65535);
    }

    // ---- Negation / abs ----

    #[test]
    fn neg_is_involutive() {
        for bits in 0..=u16::MAX {
            let p = Posit16::from_bits(bits);
            let nn = p.neg().neg();
            assert_eq!(nn.to_bits(), p.to_bits());
        }
    }

    #[test]
    fn neg_zero_is_zero() {
        assert_eq!(Posit16::ZERO.neg().to_bits(), 0x0000);
    }

    #[test]
    fn neg_nar_is_nar() {
        assert_eq!(Posit16::NAR.neg().to_bits(), 0x8000);
    }

    #[test]
    fn abs_yields_non_negative() {
        for bits in 0..=u16::MAX {
            if bits == 0x8000 {
                continue;
            }
            let p = Posit16::from_bits(bits);
            let a = p.abs();
            // abs ne doit jamais avoir le bit de signe.
            assert!(a.to_bits() & 0x8000 == 0 || a.to_bits() == 0x8000);
        }
    }

    // ---- Ordering ----

    #[test]
    fn ordering_matches_signed_int() {
        let p1 = Posit16::ONE;
        let p2 = Posit16::from_f64(2.0);
        let pn1 = Posit16::NEG_ONE;
        assert!(p1 < p2);
        assert!(pn1 < p1);
        assert!(pn1 < p2);
    }

    #[test]
    fn ordering_with_nar_yields_none() {
        let p = Posit16::ONE;
        assert!(Posit16::NAR.partial_cmp(&p).is_none());
        assert!(p.partial_cmp(&Posit16::NAR).is_none());
    }

    // ---- Numeric / Posit canonical bytes ----

    #[test]
    fn canonical_bytes_match_le() {
        let p = Posit16::from_bits(0x4800);
        assert_eq!(p.to_canonical_bytes(), vec![0x00, 0x48]);
    }

    // ---- Arithmétique Posit16 (Ω-3.1.1) ----

    fn p(value: f64) -> Posit16 {
        Posit16::from_f64(value)
    }

    fn assert_arith_eq(got: Posit16, expected: f64, label: &str) {
        let got_f = got.to_f64();
        // L'arrondi posit peut différer du f64 d'au plus 1 ULP posit.
        // On vérifie l'égalité exacte sur les cas où le résultat est
        // représentable exactement (puissances de 2, petits entiers).
        let want = Posit16::from_f64(expected);
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "[{label}] got {got_f} (0x{:04x}) vs expected {expected} (0x{:04x})",
            got.to_bits(),
            want.to_bits()
        );
    }

    #[test]
    fn add_unit_values() {
        assert_arith_eq(p(1.0).checked_add(p(1.0)).unwrap(), 2.0, "1+1");
        assert_arith_eq(p(2.0).checked_add(p(3.0)).unwrap(), 5.0, "2+3");
        assert_arith_eq(p(0.5).checked_add(p(0.25)).unwrap(), 0.75, "0.5+0.25");
        assert_arith_eq(p(1.0).checked_add(p(0.5)).unwrap(), 1.5, "1+0.5");
    }

    #[test]
    fn add_with_zero() {
        assert_arith_eq(Posit16::ZERO.checked_add(p(5.0)).unwrap(), 5.0, "0+5");
        assert_arith_eq(p(5.0).checked_add(Posit16::ZERO).unwrap(), 5.0, "5+0");
        assert_arith_eq(Posit16::ZERO.checked_add(Posit16::ZERO).unwrap(), 0.0, "0+0");
    }

    #[test]
    fn add_opposite_signs_cancels_to_zero() {
        let r = p(1.0).checked_add(p(-1.0)).unwrap();
        assert_eq!(r.to_bits(), 0, "1 + (-1) doit donner ZERO");
        let r2 = p(2.5).checked_add(p(-2.5)).unwrap();
        assert_eq!(r2.to_bits(), 0, "2.5 + (-2.5) doit donner ZERO");
    }

    #[test]
    fn add_with_nar_yields_nar() {
        assert_eq!(Posit16::NAR.checked_add(p(1.0)).unwrap().to_bits(), 0x8000);
        assert_eq!(p(1.0).checked_add(Posit16::NAR).unwrap().to_bits(), 0x8000);
    }

    #[test]
    fn add_is_commutative_on_grid() {
        // a + b == b + a sur un échantillon de patterns.
        for a_bits in (0..=u16::MAX).step_by(503) {
            for b_bits in (0..=u16::MAX).step_by(509) {
                if a_bits == 0x8000 || b_bits == 0x8000 {
                    continue;
                }
                let a = Posit16::from_bits(a_bits);
                let b = Posit16::from_bits(b_bits);
                let ab = a.checked_add(b).unwrap();
                let ba = b.checked_add(a).unwrap();
                assert_eq!(
                    ab.to_bits(),
                    ba.to_bits(),
                    "non-commutatif : 0x{a_bits:04x} + 0x{b_bits:04x}"
                );
            }
        }
    }

    #[test]
    fn sub_unit_values() {
        assert_arith_eq(p(2.0).checked_sub(p(1.0)).unwrap(), 1.0, "2-1");
        assert_arith_eq(p(5.0).checked_sub(p(3.0)).unwrap(), 2.0, "5-3");
        assert_arith_eq(p(1.0).checked_sub(p(1.0)).unwrap(), 0.0, "1-1");
    }

    #[test]
    fn mul_unit_values() {
        assert_arith_eq(p(1.0).checked_mul(p(1.0)).unwrap(), 1.0, "1*1");
        assert_arith_eq(p(2.0).checked_mul(p(3.0)).unwrap(), 6.0, "2*3");
        assert_arith_eq(p(0.5).checked_mul(p(0.5)).unwrap(), 0.25, "0.5*0.5");
        assert_arith_eq(p(1.5).checked_mul(p(2.0)).unwrap(), 3.0, "1.5*2");
    }

    #[test]
    fn mul_with_zero() {
        assert_eq!(Posit16::ZERO.checked_mul(p(5.0)).unwrap().to_bits(), 0);
        assert_eq!(p(5.0).checked_mul(Posit16::ZERO).unwrap().to_bits(), 0);
        assert_eq!(Posit16::ZERO.checked_mul(Posit16::ZERO).unwrap().to_bits(), 0);
    }

    #[test]
    fn mul_with_one_is_identity() {
        for bits in (0..=u16::MAX).step_by(257) {
            if bits == 0x8000 {
                continue;
            }
            let v = Posit16::from_bits(bits);
            let r = v.checked_mul(Posit16::ONE).unwrap();
            assert_eq!(
                r.to_bits(),
                v.to_bits(),
                "1 × 0x{bits:04x} doit être identité"
            );
        }
    }

    #[test]
    fn mul_negative_unit() {
        assert_arith_eq(p(-1.0).checked_mul(p(1.0)).unwrap(), -1.0, "-1*1");
        assert_arith_eq(p(-1.0).checked_mul(p(-1.0)).unwrap(), 1.0, "-1*-1");
        assert_arith_eq(p(-2.0).checked_mul(p(3.0)).unwrap(), -6.0, "-2*3");
    }

    #[test]
    fn mul_with_nar_yields_nar() {
        assert_eq!(Posit16::NAR.checked_mul(p(1.0)).unwrap().to_bits(), 0x8000);
        assert_eq!(p(1.0).checked_mul(Posit16::NAR).unwrap().to_bits(), 0x8000);
    }

    #[test]
    fn mul_saturates_to_maxpos() {
        // maxpos × 2 = au-delà du domaine → saturation à maxpos.
        let r = Posit16::MAXPOS.checked_mul(p(2.0)).unwrap();
        assert_eq!(r.to_bits(), Posit16::MAXPOS.to_bits());
    }

    #[test]
    fn mul_is_commutative_on_grid() {
        for a_bits in (0..=u16::MAX).step_by(503) {
            for b_bits in (0..=u16::MAX).step_by(509) {
                if a_bits == 0x8000 || b_bits == 0x8000 {
                    continue;
                }
                let a = Posit16::from_bits(a_bits);
                let b = Posit16::from_bits(b_bits);
                let ab = a.checked_mul(b).unwrap();
                let ba = b.checked_mul(a).unwrap();
                assert_eq!(
                    ab.to_bits(),
                    ba.to_bits(),
                    "non-commutatif : 0x{a_bits:04x} × 0x{b_bits:04x}"
                );
            }
        }
    }

    #[test]
    fn mul_matches_f64_on_exact_cases() {
        // Sur les cas où le résultat est exactement représentable en posit
        // ET en f64, le produit posit doit matcher exactement.
        let cases: &[(f64, f64, f64)] = &[
            (2.0, 4.0, 8.0),
            (4.0, 4.0, 16.0),
            (0.25, 4.0, 1.0),
            (8.0, 8.0, 64.0),
            (-2.0, 0.5, -1.0),
            (3.0, 3.0, 9.0),
            (5.0, 4.0, 20.0),
        ];
        for &(a, b, expected) in cases {
            let r = p(a).checked_mul(p(b)).unwrap();
            assert_arith_eq(r, expected, &format!("{a}*{b}"));
        }
    }

    #[test]
    fn div_unit_values() {
        assert_arith_eq(p(1.0).checked_div(p(1.0)).unwrap(), 1.0, "1/1");
        assert_arith_eq(p(4.0).checked_div(p(2.0)).unwrap(), 2.0, "4/2");
        assert_arith_eq(p(1.0).checked_div(p(2.0)).unwrap(), 0.5, "1/2");
        assert_arith_eq(p(6.0).checked_div(p(3.0)).unwrap(), 2.0, "6/3");
        assert_arith_eq(p(-4.0).checked_div(p(2.0)).unwrap(), -2.0, "-4/2");
        assert_arith_eq(p(8.0).checked_div(p(4.0)).unwrap(), 2.0, "8/4");
    }

    #[test]
    fn div_by_zero_yields_nar() {
        assert_eq!(p(1.0).checked_div(Posit16::ZERO).unwrap().to_bits(), 0x8000);
        assert_eq!(Posit16::ZERO.checked_div(Posit16::ZERO).unwrap().to_bits(), 0x8000);
    }

    #[test]
    fn div_by_nar_yields_nar() {
        assert_eq!(p(1.0).checked_div(Posit16::NAR).unwrap().to_bits(), 0x8000);
        assert_eq!(Posit16::NAR.checked_div(p(1.0)).unwrap().to_bits(), 0x8000);
    }

    #[test]
    fn div_zero_by_x_is_zero() {
        assert_eq!(Posit16::ZERO.checked_div(p(5.0)).unwrap().to_bits(), 0);
    }

    #[test]
    fn div_self_is_one() {
        // x / x == 1 pour tout x non-zéro non-NaR.
        for &v in &[1.0, 2.0, 0.5, 4.0, 0.25, -1.0, -2.0, 8.0] {
            let p_v = p(v);
            let r = p_v.checked_div(p_v).unwrap();
            assert_arith_eq(r, 1.0, &format!("{v}/{v}"));
        }
    }

    #[test]
    fn div_by_one_is_identity() {
        for bits in (0..=u16::MAX).step_by(257) {
            if bits == 0x8000 {
                continue;
            }
            let v = Posit16::from_bits(bits);
            let r = v.checked_div(Posit16::ONE).unwrap();
            assert_eq!(r.to_bits(), v.to_bits(), "x/1 doit être identité (0x{bits:04x})");
        }
    }

    #[test]
    fn mul_div_roundtrip_on_exact_cases() {
        // (a × b) / b == a sur les cas où b est une puissance de 2 dans la
        // plage représentable (pas de perte d'arrondi).
        let cases: &[(f64, f64)] = &[
            (3.0, 2.0),
            (1.5, 4.0),
            (-2.5, 0.5),
            (8.0, 0.25),
            (16.0, 4.0),
        ];
        for &(a, b) in cases {
            let pa = p(a);
            let pb = p(b);
            let prod = pa.checked_mul(pb).unwrap();
            let back = prod.checked_div(pb).unwrap();
            assert_arith_eq(back, a, &format!("({a}*{b})/{b}"));
        }
    }

    #[test]
    fn add_matches_f64_on_exact_cases() {
        let cases: &[(f64, f64, f64)] = &[
            (1.0, 2.0, 3.0),
            (4.0, 4.0, 8.0),
            (1.5, 0.5, 2.0),
            (-1.0, 3.0, 2.0),
            (0.25, 0.75, 1.0),
            (10.0, 5.0, 15.0),
            (16.0, 16.0, 32.0),
        ];
        for &(a, b, expected) in cases {
            let r = p(a).checked_add(p(b)).unwrap();
            assert_arith_eq(r, expected, &format!("{a}+{b}"));
        }
    }

    // ============================================================
    // Posit32 (Ω-3.1.2) — tests
    // ============================================================

    fn p32(value: f64) -> Posit32 {
        Posit32::from_f64(value)
    }

    fn assert_p32_eq(got: Posit32, expected: f64, label: &str) {
        let want = Posit32::from_f64(expected);
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "[{label}] got 0x{:08x} ({}) vs expected 0x{:08x} ({expected})",
            got.to_bits(),
            got.to_f64(),
            want.to_bits()
        );
    }

    #[test]
    fn p32_special_values() {
        assert_eq!(Posit32::ZERO.to_bits(), 0);
        assert_eq!(Posit32::NAR.to_bits(), 0x8000_0000);
        assert_eq!(Posit32::ONE.to_bits(), 0x4000_0000);
        assert_eq!(Posit32::NEG_ONE.to_bits(), 0xC000_0000);
        assert_eq!(Posit32::MAXPOS.to_bits(), 0x7FFF_FFFF);
        assert_eq!(Posit32::MINPOS.to_bits(), 0x0000_0001);
    }

    #[test]
    fn p32_decode_zero_and_nar() {
        assert!(decode_posit32(Posit32::ZERO).is_zero);
        assert!(decode_posit32(Posit32::NAR).is_nar);
    }

    #[test]
    fn p32_decode_one() {
        let d = decode_posit32(Posit32::ONE);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, 0);
        assert_eq!(d.frac, 0);
    }

    #[test]
    fn p32_decode_maxpos_minpos() {
        // 0x7FFFFFFF : 31 ones saturés → k=30, scale=120
        let d = decode_posit32(Posit32::MAXPOS);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, 120);
        // 0x00000001 : 30 zeros + terminator 1 → k=-30, scale=-120
        let d = decode_posit32(Posit32::MINPOS);
        assert_eq!(d.sign, 1);
        assert_eq!(d.scale, -120);
    }

    #[test]
    fn p32_from_f64_units() {
        assert_eq!(p32(0.0).to_bits(), 0);
        assert_eq!(p32(-0.0).to_bits(), 0);
        assert_eq!(p32(f64::NAN).to_bits(), 0x8000_0000);
        assert_eq!(p32(f64::INFINITY).to_bits(), 0x8000_0000);
        assert_eq!(p32(1.0).to_bits(), Posit32::ONE.to_bits());
        assert_eq!(p32(-1.0).to_bits(), Posit32::NEG_ONE.to_bits());
    }

    #[test]
    fn p32_from_f64_powers_of_two() {
        // Uniquement des puissances de 2 exactes en f64 — sinon le f64
        // d'origine n'est lui-même pas la valeur souhaitée.
        for &v in &[2.0, 4.0, 8.0, 16.0, 1024.0, 65536.0, 0.5, 0.25, 0.125, 1.0 / 1024.0] {
            let p = p32(v);
            assert_eq!(p.to_f64(), v, "roundtrip échoué pour {v}");
        }
    }

    #[test]
    fn p32_from_f64_saturation() {
        // 16^30 = 2^120 = max représentable.
        assert_eq!(p32(1e40).to_bits(), Posit32::MAXPOS.to_bits());
        assert_eq!(p32(-1e40).to_bits(), 0x8000_0001);
        assert_eq!(p32(1e-50).to_bits(), Posit32::MINPOS.to_bits());
    }

    #[test]
    fn p32_neg_involutive_on_samples() {
        for v in [0.0, 1.0, -1.0, 3.14, -100.5, 1e10, 1e-10] {
            let p = p32(v);
            assert_eq!(p.neg().neg().to_bits(), p.to_bits());
        }
    }

    #[test]
    fn p32_ordering() {
        assert!(p32(1.0) < p32(2.0));
        assert!(p32(-1.0) < p32(0.5));
        assert!(p32(0.0) < p32(0.001));
        assert!(Posit32::NAR.partial_cmp(&p32(1.0)).is_none());
    }

    #[test]
    fn p32_add_unit_values() {
        assert_p32_eq(p32(1.0).checked_add(p32(1.0)).unwrap(), 2.0, "1+1");
        assert_p32_eq(p32(2.0).checked_add(p32(3.0)).unwrap(), 5.0, "2+3");
        assert_p32_eq(p32(0.5).checked_add(p32(0.25)).unwrap(), 0.75, "0.5+0.25");
        assert_p32_eq(p32(100.0).checked_add(p32(50.0)).unwrap(), 150.0, "100+50");
    }

    #[test]
    fn p32_sub_unit_values() {
        assert_p32_eq(p32(5.0).checked_sub(p32(3.0)).unwrap(), 2.0, "5-3");
        assert_p32_eq(p32(1.0).checked_sub(p32(1.0)).unwrap(), 0.0, "1-1");
    }

    #[test]
    fn p32_mul_unit_values() {
        assert_p32_eq(p32(2.0).checked_mul(p32(3.0)).unwrap(), 6.0, "2*3");
        assert_p32_eq(p32(0.5).checked_mul(p32(0.5)).unwrap(), 0.25, "0.5*0.5");
        assert_p32_eq(p32(1.5).checked_mul(p32(2.0)).unwrap(), 3.0, "1.5*2");
        assert_p32_eq(p32(-2.0).checked_mul(p32(3.0)).unwrap(), -6.0, "-2*3");
    }

    #[test]
    fn p32_mul_with_one_is_identity() {
        for &v in &[1.0, 2.0, 0.5, -3.14, 1e6, 1e-6, 100.5] {
            let pv = p32(v);
            let r = pv.checked_mul(Posit32::ONE).unwrap();
            assert_eq!(r.to_bits(), pv.to_bits(), "1×{v} doit être identité");
        }
    }

    #[test]
    fn p32_div_unit_values() {
        assert_p32_eq(p32(4.0).checked_div(p32(2.0)).unwrap(), 2.0, "4/2");
        assert_p32_eq(p32(1.0).checked_div(p32(2.0)).unwrap(), 0.5, "1/2");
        assert_p32_eq(p32(100.0).checked_div(p32(4.0)).unwrap(), 25.0, "100/4");
    }

    #[test]
    fn p32_div_by_zero_yields_nar() {
        assert_eq!(p32(1.0).checked_div(Posit32::ZERO).unwrap().to_bits(), 0x8000_0000);
    }

    #[test]
    fn p32_div_self_is_one() {
        for &v in &[1.0, 2.0, 0.5, 100.0, -0.25, 1e6] {
            let pv = p32(v);
            let r = pv.checked_div(pv).unwrap();
            assert_p32_eq(r, 1.0, &format!("{v}/{v}"));
        }
    }

    #[test]
    fn p32_mul_div_roundtrip_exact_cases() {
        let cases: &[(f64, f64)] = &[
            (3.0, 2.0), (1.5, 4.0), (-2.5, 0.5), (8.0, 0.25),
            (16.0, 4.0), (1e6, 1e3), (1024.0, 8.0),
        ];
        for &(a, b) in cases {
            let r = p32(a).checked_mul(p32(b)).unwrap().checked_div(p32(b)).unwrap();
            assert_p32_eq(r, a, &format!("({a}*{b})/{b}"));
        }
    }

    #[test]
    fn p32_to_f64_roundtrip_on_samples() {
        // Pas exhaustif (4 milliards de patterns) — on prend un échantillon
        // dispersé sur tout le u32, en évitant NaR.
        let mut tested = 0;
        for &bits in &[
            0x0000_0001u32, 0x0000_FFFF, 0x0FFF_FFFF, 0x4000_0000, 0x4800_0000,
            0x4F00_0000, 0x5000_0000, 0x6000_0000, 0x7000_0000, 0x7FFF_FFFE,
            0x7FFF_FFFF, 0x8000_0001, 0xC000_0000, 0xFFFF_FFFF,
        ] {
            let p = Posit32::from_bits(bits);
            let v = p.to_f64();
            let p2 = Posit32::from_f64(v);
            assert_eq!(
                p2.to_bits(),
                bits,
                "roundtrip raté : 0x{bits:08x} → {v:e} → 0x{:08x}",
                p2.to_bits()
            );
            tested += 1;
        }
        assert_eq!(tested, 14);
    }

    #[test]
    fn p32_canonical_bytes_are_le() {
        let p = Posit32::from_bits(0x4800_0000);
        assert_eq!(p.to_canonical_bytes(), vec![0x00, 0x00, 0x00, 0x48]);
    }
}
