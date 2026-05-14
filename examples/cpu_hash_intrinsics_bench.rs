use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::hint::black_box;
use std::time::{Duration, Instant};

use scan::fast_hash::FastBuildHasher;
use sha2::{Digest, Sha256};

fn main() {
    let mib = std::env::var("FORGE_HASH_BENCH_MIB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(64)
        .max(1);
    let rounds = std::env::var("FORGE_HASH_BENCH_ROUNDS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(5)
        .max(1);
    let bytes_len = mib * 1024 * 1024;
    let data = deterministic_bytes(bytes_len);

    println!("Forge CPU hash intrinsics bench");
    println!("buffer={} MiB rounds={} total={} MiB", mib, rounds, mib * rounds);
    print_cpu_features();

    let fnv = bench_u64("fnv1a64_current", &data, rounds, fnv1a64);
    print_u64(fnv);

    let sha = bench_32("sha2_sha256_current", &data, rounds, sha256_current);
    print_32(sha);

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sse4.2") {
            let crc = bench_u64("crc32_sse4_2_hw", &data, rounds, |bytes| unsafe {
                crc32_sse42(bytes)
            });
            print_u64(crc);
        } else {
            println!("crc32_sse4_2_hw: SKIPPED (sse4.2 unavailable)");
        }

        if std::is_x86_feature_detected!("aes") {
            let aes = bench_128("aes_ni_fingerprint", &data, rounds, |bytes| unsafe {
                aes_ni_fingerprint(bytes)
            });
            print_128(aes);
        } else {
            println!("aes_ni_fingerprint: SKIPPED (aes unavailable)");
        }
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        println!("crc32_sse4_2_hw: SKIPPED (non-x86 target)");
        println!("aes_ni_fingerprint: SKIPPED (non-x86 target)");
    }

    let map_keys = std::env::var("FORGE_HASHMAP_BENCH_KEYS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(300_000)
        .max(1);
    println!();
    println!("Atlas-like HashMap bench");
    println!("keys={} key_size=32B", map_keys);
    let keys = deterministic_keys(map_keys);
    print_map(bench_hash_map(
        "hashmap_randomstate_current",
        &keys,
        RandomState::new(),
    ));
    print_map(bench_hash_map(
        "hashmap_fastbuildhasher_crc32",
        &keys,
        FastBuildHasher,
    ));
}

fn deterministic_bytes(len: usize) -> Vec<u8> {
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let v = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn deterministic_keys(len: usize) -> Vec<[u8; 32]> {
    let mut x = 0xd6e8_feb8_6659_fd93u64;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let mut key = [0u8; 32];
        for chunk in key.chunks_exact_mut(8) {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            let v = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
            chunk.copy_from_slice(&v.to_le_bytes());
        }
        out.push(key);
    }
    out
}

fn print_cpu_features() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        println!(
            "cpu_features: sse4.2={} aes={} sha={}",
            std::is_x86_feature_detected!("sse4.2"),
            std::is_x86_feature_detected!("aes"),
            std::is_x86_feature_detected!("sha")
        );
    }
}

fn bench_u64(
    name: &'static str,
    bytes: &[u8],
    rounds: usize,
    mut f: impl FnMut(&[u8]) -> u64,
) -> BenchU64 {
    black_box(f(bytes));
    let elapsed = time_rounds(rounds, || {
        black_box(f(black_box(bytes)));
    });
    BenchU64 {
        name,
        elapsed,
        bytes: bytes.len() * rounds,
        value: f(bytes),
    }
}

fn bench_128(
    name: &'static str,
    bytes: &[u8],
    rounds: usize,
    mut f: impl FnMut(&[u8]) -> [u8; 16],
) -> Bench128 {
    black_box(f(bytes));
    let elapsed = time_rounds(rounds, || {
        black_box(f(black_box(bytes)));
    });
    Bench128 {
        name,
        elapsed,
        bytes: bytes.len() * rounds,
        value: f(bytes),
    }
}

fn bench_32(
    name: &'static str,
    bytes: &[u8],
    rounds: usize,
    mut f: impl FnMut(&[u8]) -> [u8; 32],
) -> Bench32 {
    black_box(f(bytes));
    let elapsed = time_rounds(rounds, || {
        black_box(f(black_box(bytes)));
    });
    Bench32 {
        name,
        elapsed,
        bytes: bytes.len() * rounds,
        value: f(bytes),
    }
}

fn time_rounds(rounds: usize, mut f: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..rounds {
        f();
    }
    start.elapsed()
}

fn bench_hash_map<S>(name: &'static str, keys: &[[u8; 32]], hasher: S) -> MapBench
where
    S: BuildHasher + Clone,
{
    let mut map: HashMap<[u8; 32], u64, S> =
        HashMap::with_capacity_and_hasher(keys.len(), hasher);
    let start = Instant::now();
    for (idx, key) in keys.iter().enumerate() {
        map.insert(*key, idx as u64);
    }
    let insert_elapsed = start.elapsed();

    let start = Instant::now();
    let mut acc = 0u64;
    for key in keys {
        acc ^= *map.get(key).expect("inserted key must be present");
    }
    let lookup_elapsed = start.elapsed();
    black_box(acc);

    MapBench {
        name,
        len: keys.len(),
        insert_elapsed,
        lookup_elapsed,
        checksum: acc,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn sha256_current(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32_sse42(bytes: &[u8]) -> u64 {
    use std::arch::x86_64::{_mm_crc32_u64, _mm_crc32_u8};

    let mut crc = 0u64;
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().unwrap());
        crc = _mm_crc32_u64(crc, word);
    }
    for &byte in chunks.remainder() {
        crc = _mm_crc32_u8(crc as u32, byte) as u64;
    }
    crc
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32_sse42(bytes: &[u8]) -> u64 {
    use std::arch::x86::{_mm_crc32_u32, _mm_crc32_u8};

    let mut crc = 0u32;
    let mut chunks = bytes.chunks_exact(4);
    for chunk in &mut chunks {
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        crc = _mm_crc32_u32(crc, word);
    }
    for &byte in chunks.remainder() {
        crc = _mm_crc32_u8(crc, byte);
    }
    crc as u64
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "aes")]
unsafe fn aes_ni_fingerprint(bytes: &[u8]) -> [u8; 16] {
    use std::arch::x86_64::{
        __m128i, _mm_aesenc_si128, _mm_loadu_si128, _mm_set1_epi8, _mm_storeu_si128, _mm_xor_si128,
    };

    let key = _mm_set1_epi8(0x5a);
    let mut state = _mm_set1_epi8(0x36);
    let mut chunks = bytes.chunks_exact(16);
    for chunk in &mut chunks {
        let block = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
        state = _mm_aesenc_si128(_mm_xor_si128(state, block), key);
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut tail = [0u8; 16];
        tail[..rem.len()].copy_from_slice(rem);
        let block = _mm_loadu_si128(tail.as_ptr() as *const __m128i);
        state = _mm_aesenc_si128(_mm_xor_si128(state, block), key);
    }
    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, state);
    out
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "aes")]
unsafe fn aes_ni_fingerprint(bytes: &[u8]) -> [u8; 16] {
    use std::arch::x86::{
        __m128i, _mm_aesenc_si128, _mm_loadu_si128, _mm_set1_epi8, _mm_storeu_si128, _mm_xor_si128,
    };

    let key = _mm_set1_epi8(0x5a);
    let mut state = _mm_set1_epi8(0x36);
    let mut chunks = bytes.chunks_exact(16);
    for chunk in &mut chunks {
        let block = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
        state = _mm_aesenc_si128(_mm_xor_si128(state, block), key);
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut tail = [0u8; 16];
        tail[..rem.len()].copy_from_slice(rem);
        let block = _mm_loadu_si128(tail.as_ptr() as *const __m128i);
        state = _mm_aesenc_si128(_mm_xor_si128(state, block), key);
    }
    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, state);
    out
}

struct BenchU64 {
    name: &'static str,
    elapsed: Duration,
    bytes: usize,
    value: u64,
}

struct Bench128 {
    name: &'static str,
    elapsed: Duration,
    bytes: usize,
    value: [u8; 16],
}

struct Bench32 {
    name: &'static str,
    elapsed: Duration,
    bytes: usize,
    value: [u8; 32],
}

struct MapBench {
    name: &'static str,
    len: usize,
    insert_elapsed: Duration,
    lookup_elapsed: Duration,
    checksum: u64,
}

fn print_u64(result: BenchU64) {
    println!(
        "{}: {:.3} ms | {:.2} GiB/s | {:.3} ns/byte | value={:016x}",
        result.name,
        result.elapsed.as_secs_f64() * 1000.0,
        gib_per_sec(result.bytes, result.elapsed),
        ns_per_byte(result.bytes, result.elapsed),
        result.value
    );
}

fn print_128(result: Bench128) {
    println!(
        "{}: {:.3} ms | {:.2} GiB/s | {:.3} ns/byte | value={}",
        result.name,
        result.elapsed.as_secs_f64() * 1000.0,
        gib_per_sec(result.bytes, result.elapsed),
        ns_per_byte(result.bytes, result.elapsed),
        hex(&result.value)
    );
}

fn print_32(result: Bench32) {
    println!(
        "{}: {:.3} ms | {:.2} GiB/s | {:.3} ns/byte | value={}",
        result.name,
        result.elapsed.as_secs_f64() * 1000.0,
        gib_per_sec(result.bytes, result.elapsed),
        ns_per_byte(result.bytes, result.elapsed),
        hex(&result.value)
    );
}

fn print_map(result: MapBench) {
    println!(
        "{}: insert {:.3} ms ({:.1} ns/key) | lookup {:.3} ms ({:.1} ns/key) | checksum={:016x}",
        result.name,
        result.insert_elapsed.as_secs_f64() * 1000.0,
        result.insert_elapsed.as_nanos() as f64 / result.len as f64,
        result.lookup_elapsed.as_secs_f64() * 1000.0,
        result.lookup_elapsed.as_nanos() as f64 / result.len as f64,
        result.checksum
    );
}

fn gib_per_sec(bytes: usize, elapsed: Duration) -> f64 {
    (bytes as f64 / 1024.0 / 1024.0 / 1024.0) / elapsed.as_secs_f64()
}

fn ns_per_byte(bytes: usize, elapsed: Duration) -> f64 {
    elapsed.as_nanos() as f64 / bytes as f64
}

fn hex(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(LUT[(byte >> 4) as usize] as char);
        out.push(LUT[(byte & 0x0f) as usize] as char);
    }
    out
}
