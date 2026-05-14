//! Σ.3 (Wave 2, 2026-05-02) — Bump allocator pour le synthétiseur lab.
//!
//! **Origine** : arena allocators (jemalloc, bumpalo, region-based
//! memory mgmt). Idée centrale : on alloue en avançant un pointeur
//! linéairement dans un buffer pré-réservé. Pas de free individuel —
//! seulement un `reset()` global qui rewinde le pointeur. Pour des
//! workloads burst (synthèse de candidats KASM dans le lab), c'est
//! ×5-50 plus rapide qu'un allocateur général.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le lab synthétiseur génère 5000-15000 candidats KASM par run, dont
//! 95+ % sont rejetés sous 100 µs. Chaque candidat alloue son `Vec<Node>`,
//! puis le drop. Le système d'allocation général (`malloc`/jemalloc)
//! paie ~30-80 ns par alloc + ~50 ns par drop. Sur 15 k candidats × 8
//! nodes/programme moyen = 120 k allocs/run = ~10 ms perdus.
//!
//! Avec un bump : alloc = 1 cmpxchg + offset += size = ~3 ns. Pas de
//! drop individuel — seulement un `reset()` global au début de chaque
//! tour du lab. Gain attendu : ×30-100 sur les allocs synthétiseur.
//!
//! ## Architecture Wave 2 minimal viable
//!
//! - Capacité fixe au boot (Mo configurables, default = 4 MiB)
//! - Alloc atomique via `fetch_add` sur l'offset (multi-thread-safe
//!   sans lock)
//! - `reset()` zero-cost (offset = 0, pas de zero-fill — les writes
//!   suivants écrasent)
//! - `try_alloc(layout)` retourne `None` si OOM → fallback caller
//! - Pas de free individuel (Wave 2 minimal — pas de tagged pointers)
//!
//! ## Limitations Wave 2 minimal
//!
//! - Pas de generic typed alloc (juste `&mut [u8]` slabs)
//! - Pas de Drop runner pour types qui en ont besoin (les utilisateurs
//!   doivent garantir Plain-Old-Data ou faire les drops eux-mêmes)
//! - Reset NON-thread-safe (single-thread point d'orchestration)

use std::alloc::Layout;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Capacité par défaut : 4 MiB. Pour le lab à 15 k candidats × 8 nodes
/// × 8 bytes = ~1 MiB ; 4 MiB laisse 4× margin.
#[allow(dead_code)] // Exposé comme constante publique pour callers Wave 11+.
pub const DEFAULT_BUMP_CAPACITY: usize = 4 * 1024 * 1024;

/// Bump allocator multi-thread-safe (lock-free fetch_add).
///
/// Layout interne : un `Vec<u8>` de capacité fixe, un `AtomicUsize`
/// pour l'offset courant. Toutes les allocs avancent l'offset
/// atomiquement ; le reset le ramène à 0.
pub struct BumpAllocator {
    /// Slab de mémoire brute. `Box<[u8]>` est un slice owned : pointeur
    /// + longueur, sans la 3ème word de capacity. Garantit que `as_ptr`
    /// retourne bien le data pointer (et pas un pointeur vers la
    /// struct Vec).
    buffer: Box<[u8]>,
    /// Capacité totale (constant après new()).
    capacity: usize,
    /// Offset courant. Lecture/écriture via `fetch_add(Relaxed)`.
    offset: AtomicUsize,
    /// Compteur d'allocations totales (statistique).
    alloc_count: AtomicUsize,
    /// Compteur de resets totaux.
    reset_count: AtomicUsize,
}

// SAFETY: `BumpAllocator` est conçu pour être thread-safe via les
// atomics sur `offset`. Le buffer interne est en `UnsafeCell<Vec<u8>>`
// mais on n'écrit jamais sur des bytes partagés (chaque alloc reçoit
// un slot disjoint via fetch_add).
unsafe impl Send for BumpAllocator {}
unsafe impl Sync for BumpAllocator {}

#[allow(dead_code)] // Wave 2 — primitives exposées pour wiring Wave 11+.
impl BumpAllocator {
    /// Construit un bump allocator de capacité `cap` bytes.
    pub fn with_capacity(cap: usize) -> Self {
        // Pré-réserve toute la capacité — pas de realloc dynamique.
        // `vec![0u8; cap]` initialise zéros (vs `set_len` UB sur uninit).
        let buffer: Box<[u8]> = vec![0u8; cap].into_boxed_slice();
        Self {
            buffer,
            capacity: cap,
            offset: AtomicUsize::new(0),
            alloc_count: AtomicUsize::new(0),
            reset_count: AtomicUsize::new(0),
        }
    }

    /// Capacité totale en bytes.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Bytes utilisés actuellement.
    pub fn bytes_used(&self) -> usize {
        self.offset.load(Ordering::Acquire).min(self.capacity)
    }

    /// Stats : (allocs_totales, resets_totales).
    pub fn stats(&self) -> (usize, usize) {
        (
            self.alloc_count.load(Ordering::Relaxed),
            self.reset_count.load(Ordering::Relaxed),
        )
    }

    /// Tente d'allouer un slab de taille `size` aligné sur `align`.
    /// Retourne `None` si OOM (offset + size dépasse capacité).
    ///
    /// Le pointeur retourné est valide jusqu'au prochain `reset()`.
    /// Pas de drop individuel possible.
    pub fn try_alloc(&self, layout: Layout) -> Option<*mut u8> {
        let size = layout.size();
        let align = layout.align();
        if size == 0 {
            // Pas d'alloc nulle au sens C, mais Rust autorise ZST.
            // Retourne le buffer base aligné — toujours valide pour
            // un slice de longueur 0.
            let base = self.buffer.as_ptr() as *mut u8;
            return Some(base);
        }

        // Boucle CAS pour gérer l'alignement : on lit l'offset courant,
        // on calcule l'offset tel que base + offset soit aligné sur
        // `align`, on tente le fetch_add — si la capacité est dépassée,
        // on retourne None (pas d'écriture engagée).
        let base_addr = self.buffer.as_ptr() as usize;
        loop {
            let cur = self.offset.load(Ordering::Acquire);
            // base + aligned doit être divisible par align.
            let abs = base_addr.wrapping_add(cur);
            let pad = (align - (abs & (align - 1))) & (align - 1);
            let aligned = cur + pad;
            let new_offset = aligned + size;
            if new_offset > self.capacity {
                return None;
            }
            // Tenter de poser le nouveau offset.
            match self.offset.compare_exchange(
                cur, new_offset, Ordering::AcqRel, Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.alloc_count.fetch_add(1, Ordering::Relaxed);
                    // Cast away const : on traite le buffer comme une
                    // arena multi-thread où chaque slot disjoint est
                    // écrit une seule fois. Pas de aliasing concret.
                    let base = self.buffer.as_ptr() as *mut u8;
                    // SAFETY: aligned < capacity, donc base+aligned est
                    // dans le buffer. Le slot [aligned..aligned+size]
                    // est disjoint de tous les autres slots alloués
                    // (garanti par fetch_add/CAS atomique).
                    let ptr = unsafe { base.add(aligned) };
                    return Some(ptr);
                }
                Err(_) => {
                    // Concurrent alloc, retry.
                    continue;
                }
            }
        }
    }

    /// Reset — rewinde l'offset à 0. NON thread-safe : appeler depuis
    /// un point d'orchestration single-thread (e.g. fin du tour lab).
    /// Toute alloc précédente devient invalide ; il appartient à
    /// l'appelant de garantir que plus aucun pointer n'est en vol.
    pub fn reset(&self) {
        self.offset.store(0, Ordering::Release);
        self.reset_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Variante typée : alloue un `T` initialisé à `value`, retourne
    /// une référence. ATTENTION : pas de Drop run au reset() — donc
    /// `T` doit être Plain-Old-Data (Copy ou compatible).
    ///
    /// SAFETY: l'appelant garantit que `T: Copy` ou n'a pas besoin de
    /// Drop pour la correction de son usage. La doctrine V7 (zéro
    /// dépendance, primitives compactes) rend cette restriction
    /// acceptable.
    pub fn alloc_copy<T: Copy>(&self, value: T) -> Option<&mut T> {
        let layout = Layout::new::<T>();
        let ptr = self.try_alloc(layout)? as *mut T;
        // SAFETY: ptr est aligné (try_alloc respecte Layout::align)
        // et pointe sur un slot exclusif (fetch_add atomique).
        unsafe {
            std::ptr::write(ptr, value);
            Some(&mut *ptr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn bump_basic_alloc() {
        let bump = BumpAllocator::with_capacity(1024);
        assert_eq!(bump.bytes_used(), 0);
        let layout = Layout::from_size_align(64, 8).unwrap();
        let p1 = bump.try_alloc(layout).unwrap();
        assert!(!p1.is_null());
        assert_eq!(bump.bytes_used(), 64);
        let p2 = bump.try_alloc(layout).unwrap();
        assert!(!p2.is_null());
        assert_ne!(p1, p2);
        assert_eq!(bump.bytes_used(), 128);
    }

    #[test]
    fn bump_reset_rewinds_offset() {
        let bump = BumpAllocator::with_capacity(1024);
        let layout = Layout::from_size_align(256, 8).unwrap();
        let _p = bump.try_alloc(layout).unwrap();
        assert_eq!(bump.bytes_used(), 256);
        bump.reset();
        assert_eq!(bump.bytes_used(), 0);
        let (allocs, resets) = bump.stats();
        assert_eq!(allocs, 1);
        assert_eq!(resets, 1);
    }

    #[test]
    fn bump_oom_returns_none() {
        let bump = BumpAllocator::with_capacity(128);
        let layout = Layout::from_size_align(256, 8).unwrap();
        assert!(bump.try_alloc(layout).is_none());
    }

    #[test]
    fn bump_alloc_copy_writes_value() {
        let bump = BumpAllocator::with_capacity(1024);
        let r = bump.alloc_copy(42i64).unwrap();
        assert_eq!(*r, 42);
        let r2 = bump.alloc_copy([1u8, 2, 3, 4]).unwrap();
        assert_eq!(*r2, [1, 2, 3, 4]);
    }

    #[test]
    fn bump_concurrent_allocs_disjoint() {
        // 4 threads × 1000 allocs de 8 bytes chacun = 32 000 bytes.
        // Aucun deux threads ne doit obtenir le même pointeur.
        let bump = Arc::new(BumpAllocator::with_capacity(64 * 1024));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let b = Arc::clone(&bump);
            handles.push(thread::spawn(move || {
                let mut ptrs = Vec::new();
                let layout = Layout::from_size_align(8, 8).unwrap();
                for _ in 0..1000 {
                    if let Some(p) = b.try_alloc(layout) {
                        ptrs.push(p as usize);
                    }
                }
                ptrs
            }));
        }
        let mut all_ptrs = Vec::new();
        for h in handles {
            all_ptrs.extend(h.join().unwrap());
        }
        let unique: std::collections::HashSet<_> = all_ptrs.iter().copied().collect();
        assert_eq!(unique.len(), all_ptrs.len(),
            "tous les pointeurs alloués concurrentiellement doivent être disjoints");
        let (allocs, _) = bump.stats();
        assert_eq!(allocs, all_ptrs.len(),
            "compteur d'allocs cohérent avec le nombre de retours non-None");
    }

    #[test]
    fn bump_alignment_is_respected() {
        let bump = BumpAllocator::with_capacity(1024);
        let layout1 = Layout::from_size_align(1, 1).unwrap();
        let layout64 = Layout::from_size_align(64, 64).unwrap();
        // 1 byte d'abord
        let _ = bump.try_alloc(layout1).unwrap();
        // Puis un 64-byte aligné
        let p = bump.try_alloc(layout64).unwrap();
        assert_eq!((p as usize) % 64, 0, "alloc 64-byte aligned doit l'être");
    }
}
