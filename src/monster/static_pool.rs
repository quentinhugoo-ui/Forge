//! Π.7 (Wave 2, 2026-05-02) — TigerBeetle static memory pool.
//!
//! **Origine** : TigerBeetle DB (Joran Dirk Greef, 2020-). Doctrine
//! "static memory only" : tout est pré-alloué au démarrage, jamais
//! d'`malloc` en runtime. Conséquence : latence prévisible (pas de
//! GC ni de page fault), jamais d'OOM en steady state, audit
//! complet de la mémoire utilisée.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le `MonsterNode` cache RAM peut grossir indéfiniment (le governor
//! aide mais coûte des locks). Pour le hot path GPU↔CPU dispatch,
//! on veut un pool de slots fixe alloué une fois — `try_take()`
//! retourne un handle ou rien, `release(handle)` rend le slot.
//! Aucune allocation runtime, aucune fragmentation.
//!
//! ## Architecture Wave 2 minimal viable
//!
//! - Pool de N slots `Slot<T>` pré-alloués (Vec<UnsafeCell<MaybeUninit<T>>>)
//! - Free list via `Vec<u32>` de handles disponibles
//! - `try_take(value)` retourne `PoolHandle` (u32) ou None
//! - `release(handle)` retourne le slot dans la free list
//! - `get(handle) -> &T` accès O(1) lecture
//! - Single-thread pour Wave 2 minimal (pas de Mutex — l'utilisateur
//!   serialise lui-même ou utilise un Mutex<StaticPool> si besoin).
//!
//! ## Limitations Wave 2 minimal
//!
//! - Single-thread (pas d'atomics sur la free list)
//! - Pas de generation counter → ABA possible si l'utilisateur réutilise
//!   un handle libéré (handle "live" garanti seulement entre take/release)
//! - T doit être Copy ou drop-safe (pas de Drop run sur release Wave 2)

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;

/// Handle opaque vers un slot du pool. u32 pour rester compact (4G
/// slots max — largement assez pour Forge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PoolHandle(u32);

impl PoolHandle {
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// Pool statique de capacité fixe.
pub struct StaticPool<T: Copy> {
    slots: Vec<UnsafeCell<MaybeUninit<T>>>,
    free_list: Vec<u32>,
    /// Stats : compteur d'opérations take / release.
    take_count: u64,
    release_count: u64,
}

#[allow(dead_code)] // Wave 2 — primitives exposées pour wiring Wave 11+ MonsterNode pool.
impl<T: Copy> StaticPool<T> {
    /// Construit un pool de `cap` slots disponibles.
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.min(u32::MAX as usize);
        let mut slots = Vec::with_capacity(cap);
        let mut free_list = Vec::with_capacity(cap);
        for i in 0..cap {
            slots.push(UnsafeCell::new(MaybeUninit::uninit()));
            // Free list en LIFO — les derniers libérés sont repris en
            // premier (cache locality favorable).
            free_list.push((cap - 1 - i) as u32);
        }
        Self {
            slots,
            free_list,
            take_count: 0,
            release_count: 0,
        }
    }

    /// Capacité totale (constante après new).
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Nombre de slots actuellement libres.
    pub fn free(&self) -> usize {
        self.free_list.len()
    }

    /// Nombre de slots actuellement occupés.
    pub fn used(&self) -> usize {
        self.capacity() - self.free()
    }

    /// Stats : (takes_totaux, releases_totaux).
    pub fn stats(&self) -> (u64, u64) {
        (self.take_count, self.release_count)
    }

    /// Tente d'acquérir un slot. Retourne None si le pool est plein.
    pub fn try_take(&mut self, value: T) -> Option<PoolHandle> {
        let idx = self.free_list.pop()?;
        // SAFETY: idx vient de free_list — c'est un slot libre, donc
        // pas accédé par un autre handle vivant.
        unsafe {
            let slot = &mut *self.slots[idx as usize].get();
            slot.write(value);
        }
        self.take_count += 1;
        Some(PoolHandle(idx))
    }

    /// Libère un slot. SAFETY contract : le handle doit avoir été
    /// retourné par `try_take` et pas encore release. Pas de double-
    /// release detection Wave 2 minimal.
    pub fn release(&mut self, handle: PoolHandle) {
        debug_assert!((handle.0 as usize) < self.slots.len(),
            "handle out of pool range");
        debug_assert!(!self.free_list.contains(&handle.0),
            "double release of handle {}", handle.0);
        self.free_list.push(handle.0);
        self.release_count += 1;
    }

    /// Lecture immutable du slot. SAFETY contract : handle vivant.
    pub fn get(&self, handle: PoolHandle) -> &T {
        debug_assert!((handle.0 as usize) < self.slots.len());
        // SAFETY: handle vivant ⇒ slot écrit + pas de releases en cours.
        unsafe {
            let slot = &*self.slots[handle.0 as usize].get();
            &*slot.as_ptr()
        }
    }

    /// Lecture mutable du slot. SAFETY contract : handle vivant + pas
    /// d'autre référence active sur ce slot.
    pub fn get_mut(&mut self, handle: PoolHandle) -> &mut T {
        debug_assert!((handle.0 as usize) < self.slots.len());
        // SAFETY: handle vivant + &mut self exclu autres références.
        unsafe {
            let slot = &mut *self.slots[handle.0 as usize].get();
            &mut *slot.as_mut_ptr()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_basic_take_release() {
        let mut pool: StaticPool<u64> = StaticPool::with_capacity(4);
        assert_eq!(pool.capacity(), 4);
        assert_eq!(pool.free(), 4);
        let h1 = pool.try_take(42).unwrap();
        let h2 = pool.try_take(43).unwrap();
        assert_eq!(*pool.get(h1), 42);
        assert_eq!(*pool.get(h2), 43);
        assert_eq!(pool.used(), 2);
        pool.release(h1);
        assert_eq!(pool.free(), 3);
        let (takes, releases) = pool.stats();
        assert_eq!(takes, 2);
        assert_eq!(releases, 1);
    }

    #[test]
    fn pool_oom_returns_none() {
        let mut pool: StaticPool<u32> = StaticPool::with_capacity(2);
        let _h1 = pool.try_take(1).unwrap();
        let _h2 = pool.try_take(2).unwrap();
        assert!(pool.try_take(3).is_none(), "pool plein → None");
    }

    #[test]
    fn pool_release_then_reuse() {
        let mut pool: StaticPool<u32> = StaticPool::with_capacity(2);
        let h1 = pool.try_take(10).unwrap();
        let _h2 = pool.try_take(20).unwrap();
        assert!(pool.try_take(30).is_none());
        pool.release(h1);
        // Slot libéré doit être réutilisable.
        let h3 = pool.try_take(99).unwrap();
        assert_eq!(*pool.get(h3), 99);
    }

    #[test]
    fn pool_get_mut_writes_through() {
        let mut pool: StaticPool<u64> = StaticPool::with_capacity(2);
        let h = pool.try_take(0).unwrap();
        *pool.get_mut(h) = 12345;
        assert_eq!(*pool.get(h), 12345);
    }

    #[test]
    fn pool_lifo_order_for_cache_locality() {
        // free_list est LIFO : le dernier libéré est repris en premier.
        let mut pool: StaticPool<u32> = StaticPool::with_capacity(4);
        let h1 = pool.try_take(1).unwrap();
        let h2 = pool.try_take(2).unwrap();
        let h3 = pool.try_take(3).unwrap();
        pool.release(h1);
        pool.release(h2);
        // Reprise : h2 (dernier release) puis h1.
        let h_a = pool.try_take(99).unwrap();
        let h_b = pool.try_take(100).unwrap();
        assert_eq!(h_a.raw(), h2.raw(), "LIFO : dernier release repris en premier");
        assert_eq!(h_b.raw(), h1.raw());
        let _ = h3;
    }

    #[test]
    fn pool_zero_alloc_runtime_after_new() {
        // Smoke test : prendre/libérer 1000 fois ne fait pas grossir
        // les structures internes (capacités fixes).
        let mut pool: StaticPool<u64> = StaticPool::with_capacity(8);
        let cap_slots = pool.slots.capacity();
        let cap_free = pool.free_list.capacity();
        for _ in 0..1000 {
            let h = pool.try_take(0).unwrap();
            pool.release(h);
        }
        assert_eq!(pool.slots.capacity(), cap_slots,
            "capacité slots inchangée — pas d'alloc runtime");
        assert_eq!(pool.free_list.capacity(), cap_free,
            "capacité free_list inchangée — pas d'alloc runtime");
    }
}
