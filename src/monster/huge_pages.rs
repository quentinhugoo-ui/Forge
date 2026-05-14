//! Σ.22 (Wave 16, 2026-05-02) — Huge Pages 2MB hint API.
//!
//! **Origine** : Linux `MAP_HUGETLB` / `madvise(MADV_HUGEPAGE)`,
//! Windows `MEM_LARGE_PAGES`. Pattern utilisé par databases (Oracle
//! SGA, PostgreSQL HugePages, MySQL huge_tlb_page) pour réduire le
//! TLB pressure d'un facteur ×512.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le hot-atlas RAM (post-Π.25 MmapStore) peut faire 100-500 MB.
//! Avec des pages OS standard 4KB, scanner cet atlas = 25k-125k
//! TLB entries traversées → TLB miss massif (~100 cycles each).
//!
//! Avec huge pages 2MB :
//!   - 1 entrée TLB couvre 2MB au lieu de 4KB
//!   - 100MB hot-atlas tient dans 50 entrées TLB (vs 25600)
//!   - Le TLB CPU (1024-2048 entries L1) couvre tout l'atlas en cache
//!   - Quasi-élimination des TLB misses sur les accès atlas
//!
//! Gain attendu : -10 à -20% sur les workloads atlas-heavy.
//!
//! ## Doctrine V7 vs huge pages
//!
//! Vraie alloc huge pages nécessite :
//!   - Linux : `mmap(MAP_HUGETLB)` ou `madvise(MADV_HUGEPAGE)` via libc
//!   - Windows : `VirtualAlloc(MEM_LARGE_PAGES)` via winapi crate
//!
//! V7 doctrine (pure Rust + std + sha2) interdit ces dependencies.
//! Wave 16 livre donc **l'API stable** (`HugePageBuffer`,
//! `request_huge_pages_hint`) avec backing `Box<[u8]>` standard.
//! Quand Wave 17+ acceptera libc dep (ou écrira raw syscalls Linux),
//! le backing pourra basculer vers vraie huge page sans casser
//! l'API caller.
//!
//! ## Architecture Wave 16 minimal viable
//!
//! - `HugePageBuffer` wrapper Box<[u8]> avec metadata huge-page hint.
//! - `request_huge_pages_hint(size)` retourne best-effort buffer.
//! - `huge_page_size_hint()` retourne 2MB constant (target hint).
//! - `is_huge_pages_active()` retourne false en V7 (pas de syscall).
//! - Stats observability : `HugePageStats { backing_size,
//!   page_size_hint, active_huge_pages }`.
//!
//! ## Limitations Wave 16 minimal
//!
//! - **Pas de vraie huge page** — backing standard 4KB pages. L'API
//!   est stable mais le runtime fallback est identique à un Box<[u8]>.
//! - Wave 17+ avec libc/winapi dep peut activer le backing huge page
//!   réel sans casser cette API.
//! - Pas d'auto-allocation 2MB-aligned (alignement standard 16-byte).

/// Taille hint pour huge pages (Linux/Windows = 2MB classique).
pub const HUGE_PAGE_SIZE_HINT: usize = 2 * 1024 * 1024;

/// Stats observability d'un HugePageBuffer.
#[allow(dead_code)] // Wave 16 — backing_size lu via debug print Wave 18+.
#[derive(Debug, Clone, Copy)]
pub struct HugePageStats {
    pub backing_size: usize,
    pub page_size_hint: usize,
    /// Wave 16 : toujours false (pas de syscall huge page actif).
    /// Wave 17+ avec libc dep : true si MAP_HUGETLB succeeded.
    pub active_huge_pages: bool,
}

/// Buffer avec hint huge pages 2MB. Wave 16 : backing Box<[u8]>
/// standard (les pages 4KB sont allouées par malloc/mmap normal).
/// L'API est stable pour upgrade transparent vers vraie huge page
/// en Wave 17+.
#[derive(Debug)]
pub struct HugePageBuffer {
    backing: Box<[u8]>,
    /// Active hint = true si on demande huge pages via hint syscall.
    /// Wave 16 : toujours false (pas de hint syscall envoyé sans libc).
    huge_page_active: bool,
}

#[allow(dead_code)] // Wave 16 — API stable pour upgrade Wave 17+.
impl HugePageBuffer {
    /// Demande un buffer de `size` bytes avec hint huge pages.
    /// Wave 16 fallback : Vec<u8> standard. API stable pour upgrade
    /// Wave 17+.
    pub fn new(size: usize) -> Self {
        // Wave 16 minimal : malloc standard. Wave 17+ pourra ajouter
        // un cfg(target_os = "linux") path avec mmap MAP_HUGETLB
        // via raw syscall ou libc dep.
        let backing = vec![0u8; size].into_boxed_slice();
        Self {
            backing,
            huge_page_active: false,
        }
    }

    /// Demande un buffer aligné sur HUGE_PAGE_SIZE_HINT (2MB). Wave 16
    /// minimal : pas d'alignement réel (alloc standard ~16-byte aligned).
    /// Wave 17+ pourra implémenter avec aligned_alloc ou mmap MAP_HUGETLB.
    pub fn new_aligned(size: usize) -> Self {
        // Wave 16 : alignement non garanti au-delà de l'alignement
        // malloc standard. Caller peut vérifier via `is_huge_aligned()`.
        Self::new(size)
    }

    /// Construit un HugePageBuffer en prenant ownership d'un Box existant.
    /// Utilisé par MmapStore pour wrapper le backing forge.cas avec le
    /// hint huge pages — au lieu de double-allouer + copier, on wrap.
    /// Wave 17+ pourra adapter ce path pour migrer le buffer existant
    /// vers une page huge via remap (madvise MADV_COLLAPSE Linux 6.0+).
    pub fn from_box(backing: Box<[u8]>) -> Self {
        Self {
            backing,
            huge_page_active: false,
        }
    }

    /// Slice immutable du backing.
    pub fn as_slice(&self) -> &[u8] {
        &self.backing
    }

    /// Slice mutable.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.backing
    }

    /// Bytes total.
    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn is_empty(&self) -> bool {
        self.backing.is_empty()
    }

    /// Vrai si l'allocation est alignée sur HUGE_PAGE_SIZE_HINT.
    /// Wave 16 : presque toujours false (alloc standard).
    pub fn is_huge_aligned(&self) -> bool {
        let ptr = self.backing.as_ptr() as usize;
        ptr % HUGE_PAGE_SIZE_HINT == 0
    }

    /// Vrai si vraie huge page activée. Wave 16 : toujours false.
    pub fn huge_page_active(&self) -> bool {
        self.huge_page_active
    }

    /// Snapshot stats.
    pub fn stats(&self) -> HugePageStats {
        HugePageStats {
            backing_size: self.backing.len(),
            page_size_hint: HUGE_PAGE_SIZE_HINT,
            active_huge_pages: self.huge_page_active,
        }
    }
}

/// Constante hint pour callers qui veulent vérifier la taille cible
/// huge page sans instancier un buffer.
#[allow(dead_code)]
pub fn huge_page_size_hint() -> usize {
    HUGE_PAGE_SIZE_HINT
}

/// Vrai si Forge runtime supporte les huge pages réelles. Wave 16 :
/// toujours false (V7 doctrine). Wave 17+ avec libc dep : true sur
/// Linux avec madvise/MAP_HUGETLB.
#[allow(dead_code)]
pub fn is_huge_pages_supported() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_page_size_is_2mb() {
        assert_eq!(HUGE_PAGE_SIZE_HINT, 2 * 1024 * 1024);
        assert_eq!(huge_page_size_hint(), 2 * 1024 * 1024);
    }

    #[test]
    fn huge_page_buffer_new_size() {
        let buf = HugePageBuffer::new(4096);
        assert_eq!(buf.len(), 4096);
        assert_eq!(buf.as_slice().len(), 4096);
    }

    #[test]
    fn huge_page_buffer_writable() {
        let mut buf = HugePageBuffer::new(64);
        for (i, b) in buf.as_mut_slice().iter_mut().enumerate() {
            *b = i as u8;
        }
        assert_eq!(buf.as_slice()[42], 42);
    }

    #[test]
    fn huge_page_v7_fallback_active_false() {
        // V7 doctrine : aucune syscall huge page active en Wave 16.
        let buf = HugePageBuffer::new(4096);
        assert!(!buf.huge_page_active());
        assert!(!is_huge_pages_supported());
    }

    #[test]
    fn huge_page_stats_consistent() {
        let buf = HugePageBuffer::new(8192);
        let s = buf.stats();
        assert_eq!(s.backing_size, 8192);
        assert_eq!(s.page_size_hint, HUGE_PAGE_SIZE_HINT);
        assert!(!s.active_huge_pages);
    }

    #[test]
    fn huge_page_empty_buffer() {
        let buf = HugePageBuffer::new(0);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn huge_page_aligned_check() {
        // Wave 16 : alignement standard malloc, presque jamais 2MB-aligned.
        let buf = HugePageBuffer::new_aligned(4096);
        // Ne fait pas d'assertion sur alignement (env-dependent).
        // L'API expose juste un check.
        let _aligned = buf.is_huge_aligned();
    }

    #[test]
    fn huge_page_large_alloc_works() {
        // 8MB buffer, fits in standard malloc.
        let buf = HugePageBuffer::new(8 * 1024 * 1024);
        assert_eq!(buf.len(), 8 * 1024 * 1024);
    }
}
