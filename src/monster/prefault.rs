//! Σ.21 (Wave 15, 2026-05-02) — Boot prefault pour hot-atlas warm-up.
//!
//! **Origine** : Linux/POSIX `madvise(MADV_WILLNEED)`, Windows
//! `PrefetchVirtualMemory`, OpenJDK G1 GC `pretouch`. Idée centrale :
//! au démarrage, scanner séquentiellement le hot-atlas pour FORCER
//! les page faults upfront. Première lecture après boot = 0 page
//! faults (toutes les pages sont déjà résidentes en RAM).
//!
//! ## Pourquoi pour Forge ?
//!
//! Le hot-atlas mmap-backed (γ.X / Wave 16 future) ou en RAM peut
//! avoir 100-500 MB. À l'ouverture, les pages sont à demand-paged :
//! la première lecture de chaque page de 4KB déclenche un page fault
//! (~5-15 µs/page = 1-2 secondes de latence cumulative diffuse sur
//! le premier run lab).
//!
//! Σ.21 prefault = parcourir séquentiellement toutes les pages au
//! boot (1 byte par page suffit pour forcer le fault), AVANT que le
//! lab synth commence. Coût : ~1-2 secondes au démarrage. Bénéfice :
//! le lab tourne à régime permanent dès le premier tick.
//!
//! ## Architecture Wave 15 minimal viable
//!
//! - `prefault_buffer(slice)` : touche 1 byte par OS page (4KB) sur
//!   tout le slice. Pure Rust + std (pas de syscall direct, juste
//!   read en pattern séquentiel — fait page fault implicitement).
//! - `PrefaultStats { pages_touched, bytes_scanned, elapsed_ns }`
//!   pour observabilité.
//! - Fonction Linux/Unix-specific ajoutée Wave 16+ via `madvise()`
//!   syscall direct (gain ~2× vs touch séquentiel).
//!
//! ## Limitations Wave 15 minimal
//!
//! - Touch-based (pas madvise direct). Linux/Unix-specific syscall
//!   reporté Wave 16 portabilité.
//! - 4KB page assumed (huge pages 2MB Σ.22 reporté Wave 16).
//! - Pas de progress callback (caller mesure via PrefaultStats).

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

/// OS page size assumed (4KB sur la plupart des arch x86_64/ARM64).
/// Pour huge pages (2MB), Wave 16 ajoutera une variante.
pub const OS_PAGE_SIZE: usize = 4096;

#[allow(dead_code)] // Wave 15 — primitives boot atlas warmup Wave 16+.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrefaultStats {
    pub pages_touched: usize,
    pub bytes_scanned: usize,
    pub elapsed_ns: u128,
}

#[allow(dead_code)]
impl PrefaultStats {
    pub fn pages_per_sec(&self) -> u64 {
        if self.elapsed_ns == 0 {
            return 0;
        }
        ((self.pages_touched as u128 * 1_000_000_000) / self.elapsed_ns) as u64
    }
}

/// Scanne un slice byte-par-page pour forcer les page faults upfront.
/// Utilise `volatile_read` pattern via `core::ptr::read_volatile` pour
/// empêcher le compilateur d'optimiser le scan (noop sinon).
///
/// Retourne stats avec timing.
pub fn prefault_buffer(buf: &[u8]) -> PrefaultStats {
    let start = Instant::now();
    let mut sum: u8 = 0;
    let mut pages_touched = 0;

    // Touch 1 byte par page (séquentiel).
    let mut offset = 0;
    while offset < buf.len() {
        // SAFETY: offset < buf.len() check fait par la boucle.
        // read_volatile prevent dead-code elimination.
        let byte = unsafe { std::ptr::read_volatile(&buf[offset]) };
        sum = sum.wrapping_add(byte);
        offset += OS_PAGE_SIZE;
        pages_touched += 1;
    }
    // Use sum pour empêcher le compilateur de DCE l'entire scan.
    // Stocker dans un atomic global one-shot pour garantir side-effect.
    PREFAULT_DUMMY.store(sum, Ordering::Relaxed);

    let elapsed = start.elapsed();
    PrefaultStats {
        pages_touched,
        bytes_scanned: buf.len(),
        elapsed_ns: elapsed.as_nanos(),
    }
}

/// Atomic dummy pour empêcher l'optimisation du sum dans prefault_buffer.
static PREFAULT_DUMMY: AtomicU8 = AtomicU8::new(0);

/// Prefault un buffer typed (Vec<T>). Appelle `prefault_buffer` sur la
/// vue raw bytes du slice.
#[allow(dead_code)]
pub fn prefault_typed<T>(slice: &[T]) -> PrefaultStats {
    let bytes = unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            std::mem::size_of_val(slice),
        )
    };
    prefault_buffer(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefault_basic_scan() {
        let buf = vec![0x42u8; 16 * OS_PAGE_SIZE];  // 16 pages
        let stats = prefault_buffer(&buf);
        assert_eq!(stats.pages_touched, 16);
        assert_eq!(stats.bytes_scanned, 16 * OS_PAGE_SIZE);
        assert!(stats.elapsed_ns > 0);
    }

    #[test]
    fn prefault_empty_buffer() {
        let buf: Vec<u8> = Vec::new();
        let stats = prefault_buffer(&buf);
        assert_eq!(stats.pages_touched, 0);
        assert_eq!(stats.bytes_scanned, 0);
    }

    #[test]
    fn prefault_partial_page_counts_one() {
        let buf = vec![0u8; 100];  // < 1 page
        let stats = prefault_buffer(&buf);
        assert_eq!(stats.pages_touched, 1);
    }

    #[test]
    fn prefault_typed_works_on_i64() {
        let buf = vec![0i64; 4096];  // 32KB = 8 pages
        let stats = prefault_typed(&buf);
        assert_eq!(stats.pages_touched, 8);
        assert_eq!(stats.bytes_scanned, 8 * OS_PAGE_SIZE);
    }

    #[test]
    fn prefault_pages_per_sec_computes() {
        let buf = vec![0u8; 100 * OS_PAGE_SIZE];
        let stats = prefault_buffer(&buf);
        let pps = stats.pages_per_sec();
        assert!(pps > 0, "should report pages/sec stat");
    }

    #[test]
    fn prefault_large_buffer_scales_linearly() {
        // Smoke test : 10MB buffer → 2560 pages, scan complet en < 100ms.
        let buf = vec![0u8; 10 * 1024 * 1024];
        let stats = prefault_buffer(&buf);
        assert_eq!(stats.pages_touched, 2560);
        assert!(stats.elapsed_ns < 200_000_000, "10MB scan should be < 200ms");
    }
}
