//! Π.13 (Wave 2, 2026-05-02) — Lua tables auto-array/hash hybrid.
//!
//! **Origine** : Lua 5 (Roberto Ierusalimschy). Idée centrale : une
//! "table" Lua est UNE structure unifiée, mais avec deux backends
//! opaques côté implem :
//!   - **array part** : Vec<T> contigu pour clés 1..=N denses
//!   - **hash part** : HashMap pour les clés sparses ou non-numériques
//! Le runtime décide automatiquement où va chaque clé selon sa
//! densité, et migre dynamiquement quand un seuil est franchi.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le RAM cache `MonsterNode` indexe par `RamKey` qui est un blob
//! de 32 bytes (CallKey hash). Pour les workloads "calls denses sur
//! une fenêtre récente", un `Vec` indexé par `seqno % N` serait
//! ×3 plus rapide que la HashMap (pas de hash, pas de chain walk).
//! Pour les workloads "calls sparses", la HashMap reste la bonne
//! abstraction.
//!
//! Une LuaTable hybride donne le best-of-both sans demander à
//! l'utilisateur d'arbitrer en amont.
//!
//! ## Architecture Wave 2 minimal viable
//!
//! - Clés : `i64` (universelles dans KASM)
//! - Valeurs : génériques `V: Clone`
//! - Array part : `Vec<Option<V>>` indexée par clé positive [0, N)
//! - Hash part : `HashMap<i64, V>` pour le reste
//! - Migration : si une clé tombe dans [0, ARRAY_CAP) on la met dans
//!   l'array part ; sinon dans la hash part. ARRAY_CAP grandit par
//!   doublement quand >50% rempli ; ne réduit jamais (Wave 2 minimal).
//!
//! ## Limitations Wave 2 minimal
//!
//! - Clés i64 seulement (pas de string keys)
//! - Pas de garbage collection sur l'array part (slots None gardés)
//! - Migration array→hash absente (un fois ARRAY_CAP grandi, il ne
//!   shrinke pas — économie de migration cycles)

use std::collections::HashMap;

/// Capacité initiale de l'array part. Choisie petite pour minimiser
/// le footprint des tables peu utilisées.
const INITIAL_ARRAY_CAP: usize = 8;
/// Seuil de fill ratio pour étendre l'array part (50%).
const ARRAY_GROW_NUM: usize = 1;
const ARRAY_GROW_DEN: usize = 2;
/// Capacité maximale de l'array part (au-delà → toujours hash).
const MAX_ARRAY_CAP: usize = 1 << 20; // 1 MiB de slots Option<V>.

/// Table Lua-style auto-array/hash hybride.
pub struct LuaTable<V: Clone> {
    /// Array part : indexée par clé positive (clé i64 cast en usize).
    array: Vec<Option<V>>,
    /// Hash part : pour clés négatives, hors array_cap, ou collisions
    /// avec la zone array temporairement.
    hash: HashMap<i64, V>,
    /// Compteur de slots array effectivement occupés (pas la capacité).
    array_count: usize,
    /// Stats : opérations totales.
    ops: u64,
    /// Stats : hits dans l'array part vs hash part.
    array_hits: u64,
    hash_hits: u64,
}

#[allow(dead_code)] // Wave 2 — primitives exposées pour wiring Wave 11+ RAM cache.
impl<V: Clone> LuaTable<V> {
    pub fn new() -> Self {
        Self {
            array: vec![None; INITIAL_ARRAY_CAP],
            hash: HashMap::new(),
            array_count: 0,
            ops: 0,
            array_hits: 0,
            hash_hits: 0,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let array_cap = cap.min(MAX_ARRAY_CAP).max(INITIAL_ARRAY_CAP);
        Self {
            array: vec![None; array_cap],
            hash: HashMap::with_capacity(cap.saturating_sub(array_cap)),
            array_count: 0,
            ops: 0,
            array_hits: 0,
            hash_hits: 0,
        }
    }

    /// Nombre total de paires (key, value) actuellement stockées.
    pub fn len(&self) -> usize {
        self.array_count + self.hash.len()
    }

    /// Vrai si vide.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Capacité actuelle de l'array part.
    pub fn array_capacity(&self) -> usize {
        self.array.len()
    }

    /// Stats : (ops_totales, array_hits, hash_hits).
    pub fn stats(&self) -> (u64, u64, u64) {
        (self.ops, self.array_hits, self.hash_hits)
    }

    /// Insert (key, value). Retourne l'ancienne valeur si présente.
    pub fn insert(&mut self, key: i64, value: V) -> Option<V> {
        self.ops += 1;
        if Self::fits_in_array(key, self.array.len()) {
            let idx = key as usize;
            let prev = std::mem::replace(&mut self.array[idx], Some(value));
            if prev.is_none() {
                self.array_count += 1;
            }
            return prev;
        }
        // Cas borderline : clé positive proche de la capacité actuelle
        // → tenter la migration grow+place.
        if key >= 0
            && (key as usize) < MAX_ARRAY_CAP
            && self.should_grow_for_key(key as usize)
        {
            self.grow_array_to_fit(key as usize);
            // Re-vérifier : maintenant la clé doit fitter.
            if Self::fits_in_array(key, self.array.len()) {
                let idx = key as usize;
                let prev = std::mem::replace(&mut self.array[idx], Some(value));
                if prev.is_none() {
                    self.array_count += 1;
                }
                return prev;
            }
        }
        // Sinon → hash part.
        self.hash.insert(key, value)
    }

    /// Lecture par clé.
    pub fn get(&mut self, key: i64) -> Option<&V> {
        self.ops += 1;
        if Self::fits_in_array(key, self.array.len()) {
            let idx = key as usize;
            if let Some(ref v) = self.array[idx] {
                self.array_hits += 1;
                return Some(v);
            }
            return None;
        }
        if let Some(v) = self.hash.get(&key) {
            self.hash_hits += 1;
            return Some(v);
        }
        None
    }

    /// Suppression par clé. Retourne l'ancienne valeur si présente.
    pub fn remove(&mut self, key: i64) -> Option<V> {
        self.ops += 1;
        if Self::fits_in_array(key, self.array.len()) {
            let idx = key as usize;
            let prev = self.array[idx].take();
            if prev.is_some() {
                self.array_count -= 1;
            }
            return prev;
        }
        self.hash.remove(&key)
    }

    fn fits_in_array(key: i64, cap: usize) -> bool {
        key >= 0 && (key as usize) < cap
    }

    fn should_grow_for_key(&self, key_usize: usize) -> bool {
        // Heuristique : on grandit si la clé est ≤ 4× la capacité actuelle
        // ET que le fill ratio de l'array justifie l'expansion.
        if key_usize >= MAX_ARRAY_CAP {
            return false;
        }
        let cur_cap = self.array.len();
        let target_cap = (key_usize + 1).next_power_of_two().min(MAX_ARRAY_CAP);
        // Pas de croissance > 4× en un coup pour éviter les sauts massifs
        // sur une clé pathologique.
        if target_cap > cur_cap.saturating_mul(4) {
            return false;
        }
        // Fill ratio acceptable si on grossit de moins de 4×.
        let fill_after = self.array_count * ARRAY_GROW_DEN;
        fill_after >= target_cap * ARRAY_GROW_NUM / 4
            || target_cap <= cur_cap.saturating_mul(2)
    }

    fn grow_array_to_fit(&mut self, key_usize: usize) {
        let mut new_cap = self.array.len().max(INITIAL_ARRAY_CAP);
        while new_cap <= key_usize && new_cap < MAX_ARRAY_CAP {
            new_cap = new_cap.saturating_mul(2);
        }
        new_cap = new_cap.min(MAX_ARRAY_CAP);
        if new_cap > self.array.len() {
            self.array.resize(new_cap, None);
        }
    }
}

impl<V: Clone> Default for LuaTable<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_dense_keys_use_array() {
        let mut t: LuaTable<u32> = LuaTable::new();
        for i in 0..5i64 {
            t.insert(i, (i * 10) as u32);
        }
        for i in 0..5i64 {
            assert_eq!(t.get(i).copied(), Some((i * 10) as u32));
        }
        let (_, array_hits, hash_hits) = t.stats();
        assert_eq!(array_hits, 5, "5 lectures denses → 5 array hits");
        assert_eq!(hash_hits, 0, "aucun hash hit attendu sur clés denses");
    }

    #[test]
    fn lua_negative_keys_use_hash() {
        let mut t: LuaTable<i64> = LuaTable::new();
        t.insert(-1, 100);
        t.insert(-100, 200);
        assert_eq!(t.get(-1).copied(), Some(100));
        assert_eq!(t.get(-100).copied(), Some(200));
        let (_, array_hits, hash_hits) = t.stats();
        assert_eq!(hash_hits, 2);
        assert_eq!(array_hits, 0);
    }

    #[test]
    fn lua_sparse_large_key_uses_hash() {
        let mut t: LuaTable<u64> = LuaTable::new();
        // Clé énorme → ne tient pas dans l'array (heuristique grow >4×).
        t.insert(1_000_000_000, 42);
        assert_eq!(t.get(1_000_000_000).copied(), Some(42));
        let (_, _, hash_hits) = t.stats();
        assert_eq!(hash_hits, 1);
    }

    #[test]
    fn lua_grow_array_doubles_until_key_fits() {
        let mut t: LuaTable<u32> = LuaTable::new();
        let initial = t.array_capacity();
        // Insère clés 0..15 — l'array doit grossir au-delà de 8.
        for i in 0..15i64 {
            t.insert(i, i as u32);
        }
        assert!(t.array_capacity() > initial,
            "array doit grandir pour accepter clés denses au-delà de la cap initiale");
        // Toutes les clés doivent être retrouvées.
        for i in 0..15i64 {
            assert_eq!(t.get(i).copied(), Some(i as u32));
        }
    }

    #[test]
    fn lua_remove_decrements_count() {
        let mut t: LuaTable<u32> = LuaTable::new();
        t.insert(0, 10);
        t.insert(1, 20);
        t.insert(-5, 30);
        assert_eq!(t.len(), 3);
        assert_eq!(t.remove(0), Some(10));
        assert_eq!(t.len(), 2);
        assert_eq!(t.remove(-5), Some(30));
        assert_eq!(t.len(), 1);
        assert_eq!(t.remove(999), None);
    }

    #[test]
    fn lua_insert_returns_previous() {
        let mut t: LuaTable<u32> = LuaTable::new();
        assert_eq!(t.insert(0, 100), None);
        assert_eq!(t.insert(0, 200), Some(100));
        assert_eq!(t.get(0).copied(), Some(200));
    }

    #[test]
    fn lua_hybrid_workload_correctness() {
        // Mix de clés denses (0..50) et sparses (1M+, négatives).
        let mut t: LuaTable<i64> = LuaTable::new();
        for i in 0..50i64 {
            t.insert(i, i * 2);
        }
        t.insert(-1, -100);
        t.insert(1_000_000, 999);
        // Toutes lookups doivent fonctionner.
        for i in 0..50i64 {
            assert_eq!(t.get(i).copied(), Some(i * 2));
        }
        assert_eq!(t.get(-1).copied(), Some(-100));
        assert_eq!(t.get(1_000_000).copied(), Some(999));
        assert_eq!(t.len(), 52);
    }
}
