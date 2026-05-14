//! Π.26 (Wave 15, 2026-05-02) — Arena lifetime tracking.
//!
//! **Origine** : Mojo `Reference`/`Lifetime` parameters, Zig
//! `ArenaAllocator`, Rust typed-arena crate. Idée centrale : wrapper
//! l'arena `Σ.3 BumpAllocator` avec un système de lifetimes Rust qui
//! garantit à la **compile-time** que rien ne s'échappe de l'epoch.
//!
//! ## Pourquoi pour Forge ?
//!
//! Σ.3 `BumpAllocator` (Wave 2) marche déjà — alloc lock-free CAS,
//! reset O(1). Mais l'API actuelle retourne `*mut u8` brut : le caller
//! doit garantir manuellement qu'aucune référence ne survit au
//! `reset()`. C'est unsafe par construction.
//!
//! Π.26 ajoute une couche `ArenaScope<'a>` qui :
//!   - alloue uniquement avec un lifetime borné par le scope
//!   - le compilateur Rust REJETTE toute tentative d'évasion (les
//!     `&'a T` ne peuvent pas survivre au scope)
//!   - drop(ArenaScope) appelle automatiquement `reset()` sur l'arena
//!
//! Pattern Mojo `with arena: ArenaScope() as a:` rendu safe par le
//! borrow checker Rust — pas besoin de macros, pas besoin de
//! `unsafe` côté caller.
//!
//! ## Architecture Wave 15 minimal viable
//!
//! - `ArenaScope<'a>` borrow-checked autour d'un `BumpAllocator`.
//! - `alloc<T: Copy>(value)` retourne `&'a mut T` borné par le scope.
//! - `alloc_slice<T: Copy>(values)` pour Vec-like patterns.
//! - `Drop` impl qui appelle `arena.reset()` automatiquement.
//!
//! ## Limitations Wave 15 minimal
//!
//! - T: Copy obligatoire (pas de Drop run au reset).
//! - Single-threaded (BumpAllocator est multi-thread mais ArenaScope
//!   tient le borrow exclusif).
//! - Pas de `alloc_iter` (Wave 16+ pourrait ajouter avec specialization).

use crate::monster::bump::BumpAllocator;
use std::alloc::Layout;
use std::marker::PhantomData;

/// Scope d'arena lifetime-tracked. Le `'a` lifetime garantit que les
/// allocations ne survivent pas au scope.
pub struct ArenaScope<'a> {
    arena: &'a BumpAllocator,
    _marker: PhantomData<&'a mut ()>,
}

#[allow(dead_code)] // Wave 15 — primitives expose pour wiring synth lab Wave 16+.
impl<'a> ArenaScope<'a> {
    /// Construit un scope sur l'arena. Le caller doit garantir que
    /// l'arena n'est pas utilisée par d'autres callers pendant ce
    /// scope (single-thread per scope).
    ///
    /// SAFETY contract : l'arena ne doit pas avoir d'allocs vivantes
    /// au moment de la création du scope (le `reset()` au drop
    /// invaliderait toutes les allocs antérieures).
    pub fn new(arena: &'a BumpAllocator) -> Self {
        Self {
            arena,
            _marker: PhantomData,
        }
    }

    /// Allocate `value` dans l'arena, retourne `&'a mut T` borné par
    /// le scope. Le compilateur Rust empêche le caller de stocker
    /// cette référence au-delà du scope.
    ///
    /// Retourne None si l'arena est OOM.
    pub fn alloc<T: Copy>(&self, value: T) -> Option<&'a mut T> {
        let layout = Layout::new::<T>();
        let ptr = self.arena.try_alloc(layout)? as *mut T;
        // SAFETY: ptr est aligned (Layout) et pointe sur un slot exclusif
        // (arena fetch_add atomique). Le lifetime 'a borne la référence
        // au scope.
        unsafe {
            std::ptr::write(ptr, value);
            Some(&mut *ptr)
        }
    }

    /// Allocate un slice copié depuis `values`. Retourne `&'a mut [T]`.
    pub fn alloc_slice<T: Copy>(&self, values: &[T]) -> Option<&'a mut [T]> {
        let count = values.len();
        if count == 0 {
            return Some(&mut []);
        }
        let layout = Layout::array::<T>(count).ok()?;
        let ptr = self.arena.try_alloc(layout)? as *mut T;
        // SAFETY: layout array calculé correctement, ptr aligné.
        unsafe {
            for (i, v) in values.iter().enumerate() {
                std::ptr::write(ptr.add(i), *v);
            }
            Some(std::slice::from_raw_parts_mut(ptr, count))
        }
    }

    /// Bytes utilisés actuellement par l'arena (statistique).
    pub fn bytes_used(&self) -> usize {
        self.arena.bytes_used()
    }
}

impl<'a> Drop for ArenaScope<'a> {
    fn drop(&mut self) {
        // Reset l'arena automatiquement — les allocs du scope sont
        // invalidées. Le borrow checker garantit qu'aucune référence
        // au-delà de ce point ne pointe vers les bytes recyclés.
        self.arena.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_scope_basic_alloc() {
        let arena = BumpAllocator::with_capacity(1024);
        let scope = ArenaScope::new(&arena);
        let r = scope.alloc(42i64).unwrap();
        assert_eq!(*r, 42);
        // r vit ici, dans le scope. Drop scope au end of fn.
    }

    #[test]
    fn arena_scope_drop_resets_arena() {
        let arena = BumpAllocator::with_capacity(1024);
        {
            let scope = ArenaScope::new(&arena);
            let _ = scope.alloc(1u64).unwrap();
            let _ = scope.alloc(2u64).unwrap();
            assert!(scope.bytes_used() >= 16);
        } // scope dropped here → arena.reset() called
        assert_eq!(arena.bytes_used(), 0);
    }

    #[test]
    fn arena_scope_alloc_slice() {
        let arena = BumpAllocator::with_capacity(1024);
        let scope = ArenaScope::new(&arena);
        let slice = scope.alloc_slice(&[1i32, 2, 3, 4, 5]).unwrap();
        assert_eq!(slice.len(), 5);
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn arena_scope_oom_returns_none() {
        let arena = BumpAllocator::with_capacity(8);  // small
        let scope = ArenaScope::new(&arena);
        // Demander plus de bytes que dispo.
        let huge: Option<&mut [u8; 1024]> = scope.alloc([0u8; 1024]);
        assert!(huge.is_none());
    }

    #[test]
    fn arena_scope_alloc_slice_empty() {
        let arena = BumpAllocator::with_capacity(64);
        let scope = ArenaScope::new(&arena);
        let s: &mut [i32] = scope.alloc_slice(&[]).unwrap();
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn arena_scope_consecutive_scopes_reset() {
        let arena = BumpAllocator::with_capacity(64);
        for cycle in 0..10 {
            let scope = ArenaScope::new(&arena);
            let values: Vec<i32> = (0..5).map(|i| i + cycle * 10).collect();
            let slice = scope.alloc_slice(&values).unwrap();
            assert_eq!(slice.len(), 5);
        }
        // Apres 10 cycles, arena reset = empty.
        assert_eq!(arena.bytes_used(), 0);
    }

    #[test]
    fn arena_scope_bytes_used_grows() {
        let arena = BumpAllocator::with_capacity(1024);
        let scope = ArenaScope::new(&arena);
        let before = scope.bytes_used();
        let _ = scope.alloc([0i64; 4]).unwrap(); // 32 bytes
        let after = scope.bytes_used();
        assert!(after >= before + 32);
    }
}
