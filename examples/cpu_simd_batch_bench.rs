use std::hint::black_box;
use std::time::{Duration, Instant};

use scan::cpu_simd::{loss_i64_abs_sum, loss_i64_abs_sum_scalar};

fn main() {
    let len = std::env::var("FORGE_SIMD_BENCH_LEN")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(16 * 1024 * 1024)
        .max(16);
    let rounds = std::env::var("FORGE_SIMD_BENCH_ROUNDS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(8)
        .max(1);

    let a = deterministic_i32(len, 0x9e37_79b9_7f4a_7c15);
    let b = deterministic_i32(len, 0xd6e8_feb8_6659_fd93);

    println!("Forge CPU SIMD batch bench");
    println!("operation=L1 sum(abs(a[i]-b[i])) over i32 feature batches");
    println!("items={} rounds={} total_items={}", len, rounds, len * rounds);
    print_cpu_features();

    let scalar = bench("scalar_i32", rounds, len, || scalar_l1_i32(&a, &b));
    print_result(&scalar, None);

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe {
        if std::is_x86_feature_detected!("avx2") {
            let avx2 = bench("avx2_i32x8", rounds, len, || avx2_l1_i32(&a, &b));
            assert_eq!(scalar.value, avx2.value);
            print_result(&avx2, Some(scalar.ns_per_item()));
        } else {
            println!("avx2_i32x8: SKIPPED (avx2 unavailable)");
        }

        if std::is_x86_feature_detected!("avx512f") {
            let avx512 = bench("avx512_i32x16", rounds, len, || avx512_l1_i32(&a, &b));
            assert_eq!(scalar.value, avx512.value);
            print_result(&avx512, Some(scalar.ns_per_item()));
        } else {
            println!("avx512_i32x16: SKIPPED (avx512f unavailable)");
        }
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        println!("avx2_i32x8: SKIPPED (non-x86 target)");
        println!("avx512_i32x16: SKIPPED (non-x86 target)");
    }

    println!("neon_i32x4: SKIPPED on this build (requires ARM/AArch64 target)");
    println!("sve2: SKIPPED on this build (requires AArch64 SVE2 target)");

    println!();
    println!("Installed Forge loss() helper bench");
    let outputs = a.iter().map(|&v| i64::from(v)).collect::<Vec<_>>();
    let targets = b.iter().map(|&v| i64::from(v)).collect::<Vec<_>>();
    let scalar_loss = bench("forge_loss_scalar", rounds, len, || {
        loss_i64_abs_sum_scalar(&outputs, &targets) as u64
    });
    print_result(&scalar_loss, None);
    let simd_loss = bench("forge_loss_runtime_avx2", rounds, len, || {
        loss_i64_abs_sum(&outputs, &targets) as u64
    });
    assert_eq!(scalar_loss.value, simd_loss.value);
    print_result(&simd_loss, Some(scalar_loss.ns_per_item()));
}

fn deterministic_i32(len: usize, mut x: u64) -> Vec<i32> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        let v = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
        out.push(((v & 0x0fff) as i32) - 2048);
    }
    out
}

fn print_cpu_features() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        println!(
            "cpu_features: avx={} avx2={} avx512f={} avx512bw={} avx512vl={} fma={}",
            std::is_x86_feature_detected!("avx"),
            std::is_x86_feature_detected!("avx2"),
            std::is_x86_feature_detected!("avx512f"),
            std::is_x86_feature_detected!("avx512bw"),
            std::is_x86_feature_detected!("avx512vl"),
            std::is_x86_feature_detected!("fma"),
        );
    }
}

fn scalar_l1_i32(a: &[i32], b: &[i32]) -> u64 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| i64::from(x - y).abs() as u64)
        .sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn avx2_l1_i32(a: &[i32], b: &[i32]) -> u64 {
    use std::arch::x86_64::{
        __m256i, _mm256_add_epi32, _mm256_loadu_si256, _mm256_setzero_si256, _mm256_srai_epi32,
        _mm256_storeu_si256, _mm256_sub_epi32, _mm256_xor_si256,
    };

    let mut total = 0u64;
    let mut i = 0usize;
    while i + 8 <= a.len() {
        let mut acc = _mm256_setzero_si256();
        let chunk_end = (i + 8 * 4096).min(a.len() & !7);
        while i < chunk_end {
            let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
            let diff = _mm256_sub_epi32(va, vb);
            let sign = _mm256_srai_epi32(diff, 31);
            let abs = _mm256_sub_epi32(_mm256_xor_si256(diff, sign), sign);
            acc = _mm256_add_epi32(acc, abs);
            i += 8;
        }
        let mut lanes = [0i32; 8];
        _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, acc);
        total += lanes.iter().map(|&v| v as u64).sum::<u64>();
    }
    total + scalar_l1_i32(&a[i..], &b[i..])
}

#[cfg(target_arch = "x86")]
#[target_feature(enable = "avx2")]
unsafe fn avx2_l1_i32(a: &[i32], b: &[i32]) -> u64 {
    use std::arch::x86::{
        __m256i, _mm256_add_epi32, _mm256_loadu_si256, _mm256_setzero_si256, _mm256_srai_epi32,
        _mm256_storeu_si256, _mm256_sub_epi32, _mm256_xor_si256,
    };

    let mut total = 0u64;
    let mut i = 0usize;
    while i + 8 <= a.len() {
        let mut acc = _mm256_setzero_si256();
        let chunk_end = (i + 8 * 4096).min(a.len() & !7);
        while i < chunk_end {
            let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
            let diff = _mm256_sub_epi32(va, vb);
            let sign = _mm256_srai_epi32(diff, 31);
            let abs = _mm256_sub_epi32(_mm256_xor_si256(diff, sign), sign);
            acc = _mm256_add_epi32(acc, abs);
            i += 8;
        }
        let mut lanes = [0i32; 8];
        _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, acc);
        total += lanes.iter().map(|&v| v as u64).sum::<u64>();
    }
    total + scalar_l1_i32(&a[i..], &b[i..])
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn avx512_l1_i32(a: &[i32], b: &[i32]) -> u64 {
    use std::arch::x86_64::{
        __m512i, _mm512_add_epi32, _mm512_loadu_si512, _mm512_setzero_si512, _mm512_srai_epi32,
        _mm512_storeu_si512, _mm512_sub_epi32, _mm512_xor_si512,
    };

    let mut total = 0u64;
    let mut i = 0usize;
    while i + 16 <= a.len() {
        let mut acc = _mm512_setzero_si512();
        let chunk_end = (i + 16 * 4096).min(a.len() & !15);
        while i < chunk_end {
            let va = _mm512_loadu_si512(a.as_ptr().add(i) as *const __m512i);
            let vb = _mm512_loadu_si512(b.as_ptr().add(i) as *const __m512i);
            let diff = _mm512_sub_epi32(va, vb);
            let sign = _mm512_srai_epi32(diff, 31);
            let abs = _mm512_sub_epi32(_mm512_xor_si512(diff, sign), sign);
            acc = _mm512_add_epi32(acc, abs);
            i += 16;
        }
        let mut lanes = [0i32; 16];
        _mm512_storeu_si512(lanes.as_mut_ptr() as *mut __m512i, acc);
        total += lanes.iter().map(|&v| v as u64).sum::<u64>();
    }
    total + scalar_l1_i32(&a[i..], &b[i..])
}

#[cfg(target_arch = "x86")]
unsafe fn avx512_l1_i32(_a: &[i32], _b: &[i32]) -> u64 {
    unreachable!("AVX-512 bench is only compiled for x86_64")
}

fn bench(name: &'static str, rounds: usize, len: usize, mut f: impl FnMut() -> u64) -> BenchResult {
    let expected = black_box(f());
    let start = Instant::now();
    let mut value = 0u64;
    for _ in 0..rounds {
        value ^= black_box(f());
    }
    let elapsed = start.elapsed();
    black_box(value);
    BenchResult {
        name,
        elapsed,
        items: len * rounds,
        value: expected,
    }
}

struct BenchResult {
    name: &'static str,
    elapsed: Duration,
    items: usize,
    value: u64,
}

impl BenchResult {
    fn ns_per_item(&self) -> f64 {
        self.elapsed.as_nanos() as f64 / self.items as f64
    }
}

fn print_result(result: &BenchResult, scalar_ns: Option<f64>) {
    let ns = result.ns_per_item();
    let speedup = scalar_ns.map(|base| base / ns).unwrap_or(1.0);
    println!(
        "{}: {:.3} ms | {:.3} ns/item | {:.2} Gitems/s | speedup={:.2}x | value={}",
        result.name,
        result.elapsed.as_secs_f64() * 1000.0,
        ns,
        result.items as f64 / result.elapsed.as_secs_f64() / 1_000_000_000.0,
        speedup,
        result.value
    );
}
