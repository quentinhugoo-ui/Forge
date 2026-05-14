//! CPU bit-manipulation helpers for hot paths.
//!
//! The public functions keep portable scalar fallbacks and use x86_64
//! intrinsics only after runtime feature detection. They are intended for
//! in-memory scoring/filtering, not for changing persisted identities.

#[inline]
pub fn popcount_u64(value: u64) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("popcnt") {
            // Safety: guarded by runtime feature detection.
            return unsafe { popcount_u64_hw(value) };
        }
    }
    value.count_ones()
}

#[inline]
pub fn popcount_slice_u64(values: &[u64]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("popcnt") {
            // Safety: guarded by runtime feature detection.
            return unsafe { popcount_slice_u64_hw(values) };
        }
    }
    values.iter().map(|v| u64::from(v.count_ones())).sum()
}

#[inline]
pub fn popcount_slice_i64(values: &[i64]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("popcnt") {
            // Safety: guarded by runtime feature detection.
            return unsafe { popcount_slice_i64_hw(values) };
        }
    }
    values.iter().map(|v| u64::from((*v as u64).count_ones())).sum()
}

#[inline]
pub fn leading_zeros_u64(value: u64) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("lzcnt") {
            // Safety: guarded by runtime feature detection.
            return unsafe { leading_zeros_u64_hw(value) };
        }
    }
    value.leading_zeros()
}

#[inline]
pub fn trailing_zeros_u64(value: u64) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("bmi1") {
            // Safety: guarded by runtime feature detection.
            return unsafe { trailing_zeros_u64_hw(value) };
        }
    }
    value.trailing_zeros()
}

#[inline]
pub fn pext_u64(value: u64, mask: u64) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("bmi2") {
            // Safety: guarded by runtime feature detection.
            return unsafe { pext_u64_hw(value, mask) };
        }
    }
    pext_u64_soft(value, mask)
}

#[inline]
pub fn pdep_u64(value: u64, mask: u64) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("bmi2") {
            // Safety: guarded by runtime feature detection.
            return unsafe { pdep_u64_hw(value, mask) };
        }
    }
    pdep_u64_soft(value, mask)
}

#[inline]
pub fn extract_and_deposit_u64(value: u64, mask: u64) -> u64 {
    pdep_u64(pext_u64(value, mask), mask)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn popcount_u64_hw(value: u64) -> u32 {
    use std::arch::x86_64::_popcnt64;

    _popcnt64(value as i64) as u32
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn popcount_slice_u64_hw(values: &[u64]) -> u64 {
    use std::arch::x86_64::_popcnt64;

    values.iter().map(|&v| _popcnt64(v as i64) as u64).sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn popcount_slice_i64_hw(values: &[i64]) -> u64 {
    use std::arch::x86_64::_popcnt64;

    values.iter().map(|&v| _popcnt64(v) as u64).sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "lzcnt")]
unsafe fn leading_zeros_u64_hw(value: u64) -> u32 {
    use std::arch::x86_64::_lzcnt_u64;

    _lzcnt_u64(value) as u32
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi1")]
unsafe fn trailing_zeros_u64_hw(value: u64) -> u32 {
    use std::arch::x86_64::_tzcnt_u64;

    _tzcnt_u64(value) as u32
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi2")]
unsafe fn pext_u64_hw(value: u64, mask: u64) -> u64 {
    use std::arch::x86_64::_pext_u64;

    _pext_u64(value, mask)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi2")]
unsafe fn pdep_u64_hw(value: u64, mask: u64) -> u64 {
    use std::arch::x86_64::_pdep_u64;

    _pdep_u64(value, mask)
}

fn pext_u64_soft(value: u64, mut mask: u64) -> u64 {
    let mut out = 0u64;
    let mut bit = 1u64;
    while mask != 0 {
        let lowest = mask & mask.wrapping_neg();
        if value & lowest != 0 {
            out |= bit;
        }
        mask &= mask - 1;
        bit <<= 1;
    }
    out
}

fn pdep_u64_soft(value: u64, mut mask: u64) -> u64 {
    let mut out = 0u64;
    let mut bit = 1u64;
    while mask != 0 {
        let lowest = mask & mask.wrapping_neg();
        if value & bit != 0 {
            out |= lowest;
        }
        mask &= mask - 1;
        bit <<= 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popcount_matches_standard() {
        let values = [
            0,
            1,
            u64::MAX,
            0x1234_5678_9abc_def0,
            0x8000_0000_0000_0001,
        ];
        for value in values {
            assert_eq!(popcount_u64(value), value.count_ones());
        }
        assert_eq!(
            popcount_slice_u64(&values),
            values.iter().map(|v| u64::from(v.count_ones())).sum()
        );
        let signed = values.map(|value| value as i64);
        assert_eq!(
            popcount_slice_i64(&signed),
            signed
                .iter()
                .map(|v| u64::from((*v as u64).count_ones()))
                .sum()
        );
    }

    #[test]
    fn zero_count_helpers_match_standard() {
        let values = [0, 1, 2, 8, u64::MAX, 0x0100_0000_0000_0000];
        for value in values {
            assert_eq!(leading_zeros_u64(value), value.leading_zeros());
            assert_eq!(trailing_zeros_u64(value), value.trailing_zeros());
        }
    }

    #[test]
    fn pext_pdep_roundtrip_matches_masked_value() {
        let cases = [
            (0b1011_0010u64, 0b1111_0000u64),
            (0x1234_5678_9abc_def0, 0x5555_5555_5555_5555),
            (u64::MAX, 0x0001_0001_0001_0001),
            (0, u64::MAX),
        ];
        for (value, mask) in cases {
            assert_eq!(pdep_u64(pext_u64(value, mask), mask), value & mask);
        }
    }
}
