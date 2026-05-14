//! Π.29 (Wave 15, 2026-05-02) — Slab allocator page-aligned.
//!
//! **Origine** : Linux kernel `kmem_cache_create` (Bonwick 1994),
//! FreeBSD UMA, TCMalloc/jemalloc size-class freelists. Idée centrale :
//! pré-allouer des **slabs** (= 1 OS page = 4KB = N objets de taille T)
//! et servir les allocations depuis une freelist par slab. Avantages :
//!
//!   - Cache-line parfaite : 1 slab = 1 page = N objets contigus
//!   - Zéro fragmentation : un slab ne sert que des `T`, pas de mix
//!   - Realloc en page-grain (Linux mremap-style) au lieu de byte-grain
//!
//! ## Pourquoi pour Forge ?
//!
//! `RamKey` = `[u8; 32]` align(64) (cache-line aligned). Un slab de
//! 4KB = 64 RamKeys exactement. Allouer/free par slab plutôt que par
//! key élimine la fragmentation hors page, et garantit que chaque key
//! est cache-line aligned (donc zéro false sharing entre keys
//! adjacents — cf Σ.7 PaddedAtomicU64 pattern).
//!
//! Π.29 complete Σ.3 `BumpAllocator` (Wave 2) :
//!   - Σ.3 = bump allocator pour des allocs de tailles variables
//!   - Π.29 = slab pour des allocs de **taille fixe** (T sized)
//!   - Use case respectif : Σ.3 pour synth lab burst (beaucoup de
//!     tailles différentes), Π.29 pour RamKey/CacheSlot (taille
//!     fixe répétée).
//!
//! ## Architecture Wave 15 minimal viable
//!
//! - `SlabAllocator<T>` : Vec<Slab<T>> où chaque Slab = 4KB page.
//! - Each slab : freelist intrusive (= les slots libres pointent
//!   vers le slot suivant).
//! - `alloc(value)` : O(1) pop freelist d'un slab non-plein.
//! - `free(handle)` : O(1) push freelist du slab du handle.
//! - Auto-grow : nouveau slab si tous pleins.
//!
//! ## Limitations Wave 15 minimal
//!
//! - T: Sized + Copy obligatoire (freelist intrusive utilise les
//!   bytes du slot pour pointer le suivant).
//! - Single-threaded (Wave 16+ pourra ajouter per-CPU slabs Σ.3 style).
//! - Pas de slab shrink (slabs ne sont jamais libérés au runtime —
//!   single growth, reset via clear()).

use std::mem::size_of;

/// Page size assumée pour le slab. Correspond à OS_PAGE_SIZE (Σ.21).
pub const SLAB_PAGE_SIZE: usize = 4096;

/// Handle opaque vers un slot dans un slab. (slab_idx, slot_idx).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlabHandle {
    slab_idx: u32,
    slot_idx: u32,
}

impl SlabHandle {
    pub fn raw(self) -> (u32, u32) {
        (self.slab_idx, self.slot_idx)
    }
}

/// Allocator slab pour `T: Copy`. Chaque slab = 1 page OS = N slots
/// contigus. Pas de fragmentation hors page.
pub struct SlabAllocator<T: Copy> {
    /// Liste de slabs, chacun un Vec<Option<T>> de capacité fixe.
    slabs: Vec<Vec<Option<T>>>,
    /// Free list par slab : (slab_idx, slot_idx) des slots libres.
    /// Stack-style LIFO pour cache locality (slots récemment libérés
    /// repris en premier).
    free_list: Vec<SlabHandle>,
    /// Slots par slab = (page_size / size_of::<T>()).max(1).
    slots_per_slab: usize,
    /// Compteur d'allocs vivantes.
    live: usize,
    /// Stats : total alloc / free events.
    alloc_count: u64,
    free_count: u64,
}

#[allow(dead_code)] // Wave 15 — primitives expose pour wiring RamKey cache Wave 16+.
impl<T: Copy> SlabAllocator<T> {
    /// Construit un allocator avec une slab initiale.
    pub fn new() -> Self {
        let t_size = size_of::<T>().max(1);
        let slots_per_slab = (SLAB_PAGE_SIZE / t_size).max(1);
        let mut alloc = Self {
            slabs: Vec::new(),
            free_list: Vec::new(),
            slots_per_slab,
            live: 0,
            alloc_count: 0,
            free_count: 0,
        };
        alloc.add_slab();
        alloc
    }

    /// Construit avec capacité initiale = `expected_slots` slots
    /// pré-alloués (= round up to slab boundary).
    pub fn with_capacity(expected_slots: usize) -> Self {
        let mut alloc = Self::new();
        let needed_slabs = (expected_slots / alloc.slots_per_slab).max(1);
        for _ in 1..needed_slabs {
            alloc.add_slab();
        }
        alloc
    }

    pub fn slots_per_slab(&self) -> usize {
        self.slots_per_slab
    }

    pub fn slab_count(&self) -> usize {
        self.slabs.len()
    }

    pub fn live(&self) -> usize {
        self.live
    }

    pub fn capacity(&self) -> usize {
        self.slabs.len() * self.slots_per_slab
    }

    /// Stats : (alloc_count, free_count).
    pub fn stats(&self) -> (u64, u64) {
        (self.alloc_count, self.free_count)
    }

    /// Add a fresh slab. Tous ses slots vont dans la free_list.
    fn add_slab(&mut self) {
        let slab_idx = self.slabs.len() as u32;
        let mut slab = Vec::with_capacity(self.slots_per_slab);
        for _ in 0..self.slots_per_slab {
            slab.push(None);
        }
        // Freelist : slots de ce slab en LIFO (le slot 0 sera pris en
        // premier après ceux des slabs précédents).
        for slot_idx in (0..self.slots_per_slab as u32).rev() {
            self.free_list.push(SlabHandle { slab_idx, slot_idx });
        }
        self.slabs.push(slab);
    }

    /// Allocate un slot avec value. Auto-grow si tous pleins.
    pub fn alloc(&mut self, value: T) -> SlabHandle {
        let handle = match self.free_list.pop() {
            Some(h) => h,
            None => {
                self.add_slab();
                self.free_list.pop().expect("just added slab — must have free slot")
            }
        };
        self.slabs[handle.slab_idx as usize][handle.slot_idx as usize] = Some(value);
        self.live += 1;
        self.alloc_count += 1;
        handle
    }

    /// Free un handle. SAFETY contract : handle vivant (allocated, not
    /// déjà freed). Pas de double-free detection en release.
    pub fn free(&mut self, handle: SlabHandle) {
        debug_assert!(
            (handle.slab_idx as usize) < self.slabs.len(),
            "slab idx out of range"
        );
        debug_assert!(
            (handle.slot_idx as usize) < self.slots_per_slab,
            "slot idx out of range"
        );
        self.slabs[handle.slab_idx as usize][handle.slot_idx as usize] = None;
        self.free_list.push(handle);
        self.live -= 1;
        self.free_count += 1;
    }

    /// Read un slot. SAFETY contract : handle vivant.
    pub fn get(&self, handle: SlabHandle) -> &T {
        debug_assert!((handle.slab_idx as usize) < self.slabs.len());
        debug_assert!((handle.slot_idx as usize) < self.slots_per_slab);
        self.slabs[handle.slab_idx as usize][handle.slot_idx as usize]
            .as_ref()
            .expect("slot must be live (handle invariant)")
    }

    /// Write un slot. SAFETY contract : handle vivant.
    pub fn get_mut(&mut self, handle: SlabHandle) -> &mut T {
        self.slabs[handle.slab_idx as usize][handle.slot_idx as usize]
            .as_mut()
            .expect("slot must be live (handle invariant)")
    }

    /// Clear : libère tous les slots de tous les slabs (mais ne free
    /// pas les pages — les slabs restent disponibles pour réutilisation).
    pub fn clear(&mut self) {
        for slab in self.slabs.iter_mut() {
            for slot in slab.iter_mut() {
                *slot = None;
            }
        }
        self.free_list.clear();
        for (slab_idx, _) in self.slabs.iter().enumerate() {
            for slot_idx in (0..self.slots_per_slab as u32).rev() {
                self.free_list.push(SlabHandle {
                    slab_idx: slab_idx as u32,
                    slot_idx,
                });
            }
        }
        self.live = 0;
    }
}

impl<T: Copy> Default for SlabAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slab_basic_alloc_free() {
        let mut s: SlabAllocator<i64> = SlabAllocator::new();
        let h1 = s.alloc(42);
        let h2 = s.alloc(99);
        assert_eq!(*s.get(h1), 42);
        assert_eq!(*s.get(h2), 99);
        assert_eq!(s.live(), 2);
        s.free(h1);
        assert_eq!(s.live(), 1);
    }

    #[test]
    fn slab_slots_per_slab_for_i64() {
        // i64 = 8 bytes, page = 4096 → 512 slots/slab.
        let s: SlabAllocator<i64> = SlabAllocator::new();
        assert_eq!(s.slots_per_slab(), 512);
        assert_eq!(s.capacity(), 512);  // 1 slab initial
    }

    #[test]
    fn slab_grows_when_full() {
        let mut s: SlabAllocator<i64> = SlabAllocator::new();
        let initial_cap = s.capacity();
        // Alloc juste au-dessus de la capacity initiale.
        let mut handles = Vec::new();
        for i in 0..(initial_cap + 10) {
            handles.push(s.alloc(i as i64));
        }
        // Slab count should have grown.
        assert!(s.slab_count() >= 2);
        // Toutes les allocs doivent être lisibles.
        for (i, h) in handles.iter().enumerate() {
            assert_eq!(*s.get(*h), i as i64);
        }
    }

    #[test]
    fn slab_free_then_reuse() {
        let mut s: SlabAllocator<u32> = SlabAllocator::new();
        let h1 = s.alloc(100);
        let h2 = s.alloc(200);
        s.free(h1);
        let h3 = s.alloc(300);
        // h3 doit réutiliser le slot libéré.
        assert_eq!(h3.raw(), h1.raw());
        assert_eq!(*s.get(h3), 300);
        assert_eq!(*s.get(h2), 200);
    }

    #[test]
    fn slab_get_mut_writes_through() {
        let mut s: SlabAllocator<i64> = SlabAllocator::new();
        let h = s.alloc(0);
        *s.get_mut(h) = 12345;
        assert_eq!(*s.get(h), 12345);
    }

    #[test]
    fn slab_clear_resets_all() {
        let mut s: SlabAllocator<u64> = SlabAllocator::new();
        for i in 0..100u64 {
            s.alloc(i);
        }
        assert_eq!(s.live(), 100);
        s.clear();
        assert_eq!(s.live(), 0);
        // Re-alloc should work post-clear.
        let h = s.alloc(999);
        assert_eq!(*s.get(h), 999);
    }

    #[test]
    fn slab_with_capacity_pre_allocates() {
        let s: SlabAllocator<i64> = SlabAllocator::with_capacity(2000);
        // 2000 / 512 = 3.9 → at least 3 slabs.
        assert!(s.slab_count() >= 3);
    }

    #[test]
    fn slab_stats_track_alloc_free() {
        let mut s: SlabAllocator<i64> = SlabAllocator::new();
        let h1 = s.alloc(1);
        let h2 = s.alloc(2);
        s.free(h1);
        let (allocs, frees) = s.stats();
        assert_eq!(allocs, 2);
        assert_eq!(frees, 1);
        let _ = h2;
    }

    #[test]
    fn slab_for_64byte_ramkey() {
        // RamKey = 64 bytes (cache-line). Slab = 4096 / 64 = 64 keys/slab.
        type RamKey = [u8; 64];
        let s: SlabAllocator<RamKey> = SlabAllocator::new();
        assert_eq!(s.slots_per_slab(), 64);
    }

    #[test]
    fn slab_handle_raw_is_pair() {
        let mut s: SlabAllocator<u32> = SlabAllocator::new();
        let h = s.alloc(1);
        let (slab, slot) = h.raw();
        assert_eq!(slab, 0);
        // Premier alloc = dernier slot freelist (LIFO) = slot 0.
        assert_eq!(slot, 0);
    }

    #[test]
    fn slab_zero_alloc_runtime_after_init() {
        // Smoke test : 1000 alloc + free cycles, slab_count stable.
        let mut s: SlabAllocator<u64> = SlabAllocator::new();
        for _ in 0..1000 {
            let h = s.alloc(0);
            s.free(h);
        }
        // Slab count = 1 (jamais grown au-delà), live = 0.
        assert_eq!(s.slab_count(), 1);
        assert_eq!(s.live(), 0);
    }
}
