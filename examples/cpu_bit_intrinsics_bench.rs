use std::hint::black_box;
use std::time::{Duration, Instant};

fn main() {
    let len = std::env::var("FORGE_BIT_BENCH_LEN")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(8 * 1024 * 1024)
        .max(1);
    let rounds = std::env::var("FORGE_BIT_BENCH_ROUNDS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(8)
        .max(1);
    let data = deterministic_u64(len);
    let masks = deterministic_masks(len);

    println!("Forge CPU bit intrinsics bench");
    println!("items={} rounds={} total_items={}", len, rounds, len * rounds);
    print_cpu_features();

    let pop_kernighan = bench("popcount_kernighan_fallback", rounds, len, || {
        popcount_kernighan(&data)
    });
    print_result(&pop_kernighan, None);

    let pop_current = bench("popcount_rust_count_ones", rounds, len, || {
        popcount_count_ones(&data)
    });
    print_result(&pop_current, Some(pop_kernighan.ns_per_item()));

    let scan_current = bench("scan_bits_trailing_zeros", rounds, len, || {
        scan_bits_current(&data)
    });
    print_result(&scan_current, None);

    let lz_tz_current = bench("leading_trailing_zeros_current", rounds, len, || {
        leading_trailing_current(&data)
    });
    print_result(&lz_tz_current, None);

    let pext_soft = bench("pext_pdep_software", rounds, len, || {
        pext_pdep_software_batch(&data, &masks)
    });
    print_result(&pext_soft, None);

    #[cfg(target_arch = "x86_64")]
    unsafe {
        if std::is_x86_feature_detected!("popcnt") {
            let pop_hw = bench("popcnt_hw", rounds, len, || popcnt_hw(&data));
            assert_eq!(pop_current.value, pop_hw.value);
            print_result(&pop_hw, Some(pop_current.ns_per_item()));
        } else {
            println!("popcnt_hw: SKIPPED (popcnt unavailable)");
        }

        if std::is_x86_feature_detected!("bmi1") {
            let scan_bmi1 = bench("scan_bits_tzcnt_blsr_bmi1", rounds, len, || {
                scan_bits_bmi1(&data)
            });
            assert_eq!(scan_current.value, scan_bmi1.value);
            print_result(&scan_bmi1, Some(scan_current.ns_per_item()));
        } else {
            println!("scan_bits_tzcnt_blsr_bmi1: SKIPPED (bmi1 unavailable)");
        }

        if std::is_x86_feature_detected!("lzcnt") {
            let lzcnt_hw = bench("leading_trailing_lzcnt_tzcnt", rounds, len, || {
                leading_trailing_lzcnt(&data)
            });
            assert_eq!(lz_tz_current.value, lzcnt_hw.value);
            print_result(&lzcnt_hw, Some(lz_tz_current.ns_per_item()));
        } else {
            println!("leading_trailing_lzcnt_tzcnt: SKIPPED (lzcnt unavailable)");
        }

        if std::is_x86_feature_detected!("bmi2") {
            let pext_hw = bench("pext_pdep_bmi2_hw", rounds, len, || {
                pext_pdep_bmi2(&data, &masks)
            });
            assert_eq!(pext_soft.value, pext_hw.value);
            print_result(&pext_hw, Some(pext_soft.ns_per_item()));
        } else {
            println!("pext_pdep_bmi2_hw: SKIPPED (bmi2 unavailable)");
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        println!("x86_64 bit intrinsics: SKIPPED (non-x86_64 target)");
    }
}

fn print_cpu_features() {
    #[cfg(target_arch = "x86_64")]
    {
        println!(
            "cpu_features: popcnt={} bmi1={} bmi2={} lzcnt={}",
            std::is_x86_feature_detected!("popcnt"),
            std::is_x86_feature_detected!("bmi1"),
            std::is_x86_feature_detected!("bmi2"),
            std::is_x86_feature_detected!("lzcnt"),
        );
    }
}

fn deterministic_u64(len: usize) -> Vec<u64> {
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        out.push(x.wrapping_mul(0x2545_f491_4f6c_dd1d));
    }
    out
}

fn deterministic_masks(len: usize) -> Vec<u64> {
    deterministic_u64(len)
        .into_iter()
        .map(|v| (v & 0x5555_5555_5555_5555) | 0x0001_0001_0001_0001)
        .collect()
}

fn popcount_kernighan(data: &[u64]) -> u64 {
    let mut total = 0u64;
    for &value in data {
        let mut x = value;
        while x != 0 {
            x &= x - 1;
            total += 1;
        }
    }
    total
}

fn popcount_count_ones(data: &[u64]) -> u64 {
    data.iter().map(|v| u64::from(v.count_ones())).sum()
}

fn scan_bits_current(data: &[u64]) -> u64 {
    let mut total = 0u64;
    for &value in data {
        let mut x = value;
        while x != 0 {
            let bit = x.trailing_zeros() as u64;
            total = total.wrapping_add(bit);
            x &= x - 1;
        }
    }
    total
}

fn leading_trailing_current(data: &[u64]) -> u64 {
    data.iter()
        .map(|v| u64::from(v.leading_zeros()) + u64::from(v.trailing_zeros()))
        .sum()
}

fn pext_pdep_software_batch(data: &[u64], masks: &[u64]) -> u64 {
    data.iter()
        .zip(masks)
        .fold(0u64, |acc, (&value, &mask)| {
            let extracted = pext_software(value, mask);
            acc ^ pdep_software(extracted, mask)
        })
}

fn pext_software(mut value: u64, mut mask: u64) -> u64 {
    let mut out = 0u64;
    let mut bit = 1u64;
    while mask != 0 {
        let lowest = mask & mask.wrapping_neg();
        if value & lowest != 0 {
            out |= bit;
        }
        mask &= mask - 1;
        bit <<= 1;
        value &= !lowest;
    }
    out
}

fn pdep_software(value: u64, mut mask: u64) -> u64 {
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

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn popcnt_hw(data: &[u64]) -> u64 {
    use std::arch::x86_64::_popcnt64;

    data.iter()
        .map(|&v| _popcnt64(v as i64) as u64)
        .sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi1")]
unsafe fn scan_bits_bmi1(data: &[u64]) -> u64 {
    use std::arch::x86_64::{_blsr_u64, _tzcnt_u64};

    let mut total = 0u64;
    for &value in data {
        let mut x = value;
        while x != 0 {
            total = total.wrapping_add(_tzcnt_u64(x));
            x = _blsr_u64(x);
        }
    }
    total
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "lzcnt")]
unsafe fn leading_trailing_lzcnt(data: &[u64]) -> u64 {
    use std::arch::x86_64::{_lzcnt_u64, _tzcnt_u64};

    data.iter()
        .map(|&v| _lzcnt_u64(v) + _tzcnt_u64(v))
        .sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi2")]
unsafe fn pext_pdep_bmi2(data: &[u64], masks: &[u64]) -> u64 {
    use std::arch::x86_64::{_pdep_u64, _pext_u64};

    data.iter()
        .zip(masks)
        .fold(0u64, |acc, (&value, &mask)| {
            let extracted = _pext_u64(value, mask);
            acc ^ _pdep_u64(extracted, mask)
        })
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

fn print_result(result: &BenchResult, baseline_ns: Option<f64>) {
    let ns = result.ns_per_item();
    let speedup = baseline_ns.map(|base| base / ns).unwrap_or(1.0);
    println!(
        "{}: {:.3} ms | {:.3} ns/item | {:.2} Gitems/s | speedup={:.2}x | value={:016x}",
        result.name,
        result.elapsed.as_secs_f64() * 1000.0,
        ns,
        result.items as f64 / result.elapsed.as_secs_f64() / 1_000_000_000.0,
        speedup,
        result.value
    );
}
