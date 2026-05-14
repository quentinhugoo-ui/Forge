//! Fast non-cryptographic hashing for in-memory tables and prefilters.
//!
//! This module deliberately does NOT replace Forge's canonical persisted
//! hashes. It is only for RAM hash tables, cache prefilters, and local
//! fingerprints where collisions are still resolved by exact key equality.

use std::hash::{BuildHasher, Hasher};

#[derive(Clone, Copy, Debug, Default)]
pub struct FastBuildHasher;

impl BuildHasher for FastBuildHasher {
    type Hasher = FastHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FastHasher::default()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FastHasher {
    state: u64,
}

impl Default for FastHasher {
    #[inline]
    fn default() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        final_mix64(self.state)
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.state = fast_hash_append(self.state, bytes);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write(&[i]);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write(&i.to_le_bytes());
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8);
    }

    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.write_u16(i as u16);
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.write_u32(i as u32);
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.write_usize(i as usize);
    }
}

#[inline]
pub fn fast_fingerprint64(bytes: &[u8]) -> u64 {
    final_mix64(fast_hash_append(0xcbf2_9ce4_8422_2325, bytes))
}

#[inline]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[inline]
fn fast_hash_append(state: u64, bytes: &[u8]) -> u64 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sse4.2") {
            // Safety: guarded by runtime feature detection.
            return unsafe { crc32_sse42_append(state, bytes) };
        }
    }
    fnv1a64_append(state, bytes)
}

#[inline]
fn fnv1a64_append(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32_sse42_append(state: u64, bytes: &[u8]) -> u64 {
    use std::arch::x86_64::{_mm_crc32_u64, _mm_crc32_u8};

    let mut lo = state;
    let mut hi = (state >> 32) ^ 0xa5a5_a5a5;
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().unwrap());
        lo = _mm_crc32_u64(lo, word);
        hi = _mm_crc32_u64(hi, word.rotate_left(32) ^ 0x9e37_79b9_7f4a_7c15);
    }
    for &byte in chunks.remainder() {
        lo = _mm_crc32_u8(lo as u32, byte) as u64;
        hi = _mm_crc32_u8(hi as u32, byte ^ 0xa5) as u64;
    }
    (hi << 32) | (lo & 0xffff_ffff)
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32_sse42_append(state: u64, bytes: &[u8]) -> u64 {
    use std::arch::x86::{_mm_crc32_u32, _mm_crc32_u8};

    let mut lo = state as u32;
    let mut hi = ((state >> 32) as u32) ^ 0xa5a5_a5a5;
    let mut chunks = bytes.chunks_exact(4);
    for chunk in &mut chunks {
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        lo = _mm_crc32_u32(lo, word);
        hi = _mm_crc32_u32(hi, word.rotate_left(16) ^ 0x9e37_79b9);
    }
    for &byte in chunks.remainder() {
        lo = _mm_crc32_u8(lo, byte);
        hi = _mm_crc32_u8(hi, byte ^ 0xa5);
    }
    ((hi as u64) << 32) | lo as u64
}

#[inline]
fn final_mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    #[test]
    fn fast_hash_is_deterministic() {
        assert_eq!(fast_fingerprint64(b"forge"), fast_fingerprint64(b"forge"));
        assert_ne!(fast_fingerprint64(b"forge"), fast_fingerprint64(b"forge!"));
    }

    #[test]
    fn fast_hasher_supports_hash_trait() {
        let mut h1 = FastBuildHasher.build_hasher();
        (42u64, "alpha").hash(&mut h1);
        let mut h2 = FastBuildHasher.build_hasher();
        (42u64, "alpha").hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
