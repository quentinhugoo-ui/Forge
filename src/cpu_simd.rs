//! CPU SIMD helpers for hot in-memory scoring loops.
//!
//! These routines are deliberately conservative: AVX2 is used only when
//! runtime feature detection says it is available. The vector path checks
//! each subtraction for signed overflow and falls back to scalar `i128`
//! arithmetic if an extreme case would make the SIMD result non-exact.

#[inline]
pub fn loss_i64_abs_sum(outputs: &[i64], targets: &[i64]) -> u128 {
    debug_assert_eq!(outputs.len(), targets.len());

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::env::var_os("FORGE_ENABLE_AVX2_LOSS").is_some()
            && std::is_x86_feature_detected!("avx2")
        {
            // Safety: guarded by runtime AVX2 detection. The implementation
            // detects overflow/diff=i64::MIN and falls back before summing.
            return unsafe { loss_i64_abs_sum_avx2(outputs, targets) };
        }
    }

    loss_i64_abs_sum_scalar(outputs, targets)
}

#[inline]
pub fn loss_i64_abs_sum_scalar(outputs: &[i64], targets: &[i64]) -> u128 {
    outputs
        .iter()
        .zip(targets)
        .map(|(got, want)| ((*got as i128) - (*want as i128)).unsigned_abs())
        .sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn loss_i64_abs_sum_avx2(outputs: &[i64], targets: &[i64]) -> u128 {
    use std::arch::x86_64::{
        __m256i, _mm256_and_si256, _mm256_cmpeq_epi64, _mm256_cmpgt_epi64, _mm256_loadu_si256,
        _mm256_movemask_epi8, _mm256_or_si256, _mm256_set1_epi64x, _mm256_setzero_si256,
        _mm256_storeu_si256, _mm256_sub_epi64, _mm256_xor_si256,
    };

    let mut total = 0u128;
    let mut i = 0usize;
    let zero = _mm256_setzero_si256();
    let min_i64 = _mm256_set1_epi64x(i64::MIN);
    while i + 4 <= outputs.len() {
        let got = _mm256_loadu_si256(outputs.as_ptr().add(i) as *const __m256i);
        let want = _mm256_loadu_si256(targets.as_ptr().add(i) as *const __m256i);
        let diff = _mm256_sub_epi64(got, want);
        let sign_changed_inputs = _mm256_xor_si256(got, want);
        let sign_changed_result = _mm256_xor_si256(got, diff);
        let overflow_bits = _mm256_and_si256(sign_changed_inputs, sign_changed_result);
        let overflow = _mm256_cmpgt_epi64(zero, overflow_bits);
        let is_min = _mm256_cmpeq_epi64(diff, min_i64);
        if _mm256_movemask_epi8(_mm256_or_si256(overflow, is_min)) != 0 {
            return loss_i64_abs_sum_scalar(outputs, targets);
        }
        let sign = _mm256_cmpgt_epi64(zero, diff);
        let abs = _mm256_sub_epi64(_mm256_xor_si256(diff, sign), sign);
        let mut lanes = [0i64; 4];
        _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, abs);
        total += lanes.iter().map(|&v| v as u128).sum::<u128>();
        i += 4;
    }
    total + loss_i64_abs_sum_scalar(&outputs[i..], &targets[i..])
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "avx2")]
unsafe fn loss_i64_abs_sum_avx2(outputs: &[i64], targets: &[i64]) -> u128 {
    use std::arch::x86::{
        __m256i, _mm256_and_si256, _mm256_cmpeq_epi64, _mm256_cmpgt_epi64, _mm256_loadu_si256,
        _mm256_movemask_epi8, _mm256_or_si256, _mm256_set1_epi64x, _mm256_setzero_si256,
        _mm256_storeu_si256, _mm256_sub_epi64, _mm256_xor_si256,
    };

    let mut total = 0u128;
    let mut i = 0usize;
    let zero = _mm256_setzero_si256();
    let min_i64 = _mm256_set1_epi64x(i64::MIN);
    while i + 4 <= outputs.len() {
        let got = _mm256_loadu_si256(outputs.as_ptr().add(i) as *const __m256i);
        let want = _mm256_loadu_si256(targets.as_ptr().add(i) as *const __m256i);
        let diff = _mm256_sub_epi64(got, want);
        let sign_changed_inputs = _mm256_xor_si256(got, want);
        let sign_changed_result = _mm256_xor_si256(got, diff);
        let overflow_bits = _mm256_and_si256(sign_changed_inputs, sign_changed_result);
        let overflow = _mm256_cmpgt_epi64(zero, overflow_bits);
        let is_min = _mm256_cmpeq_epi64(diff, min_i64);
        if _mm256_movemask_epi8(_mm256_or_si256(overflow, is_min)) != 0 {
            return loss_i64_abs_sum_scalar(outputs, targets);
        }
        let sign = _mm256_cmpgt_epi64(zero, diff);
        let abs = _mm256_sub_epi64(_mm256_xor_si256(diff, sign), sign);
        let mut lanes = [0i64; 4];
        _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, abs);
        total += lanes.iter().map(|&v| v as u128).sum::<u128>();
        i += 4;
    }
    total + loss_i64_abs_sum_scalar(&outputs[i..], &targets[i..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_loss_matches_scalar_for_i32_range_values() {
        let outputs = (-2048..2048).map(|v| (v * 17) as i64).collect::<Vec<_>>();
        let targets = (-2048..2048).rev().map(|v| (v * 11) as i64).collect::<Vec<_>>();
        assert_eq!(
            loss_i64_abs_sum(&outputs, &targets),
            loss_i64_abs_sum_scalar(&outputs, &targets)
        );
    }

    #[test]
    fn simd_loss_falls_back_for_extreme_i64_values() {
        let outputs = [i64::MIN, i64::MAX, 0, -7];
        let targets = [i64::MAX, i64::MIN, -3, 11];
        assert_eq!(
            loss_i64_abs_sum(&outputs, &targets),
            loss_i64_abs_sum_scalar(&outputs, &targets)
        );
    }
}
