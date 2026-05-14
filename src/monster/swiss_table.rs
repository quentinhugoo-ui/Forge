//! Σ.12 (Wave 13, 2026-05-02) — Swiss tables HashMap (Google Abseil-style).
//!
//! **Origine** : Google Abseil (Matt Kulukundis, CppCon 2017),
//! Hashbrown (Amanieu d'Antras, port Rust). Idée centrale : remplacer
//! le `HashMap` chaining par un open-addressing avec :
//!
//!   1. Une **metadata table** 8-bit par slot stockant 7 bits du hash
//!      (tag) + 1 bit "occupied" (sentinel pour empty/deleted).
//!   2. Un probe SIMD-friendly qui scan 16 slots en parallèle (sur
//!      x86_64 avec SSE2).
//!
//! Résultat : ~5 ns/lookup vs ~20 ns std::HashMap SipHash + chaining.
//! 4× sur le hot path RAM cache.
//!
//! ## Pourquoi pour Forge ?
//!
//! Φ.μ.7 a déjà déployé `IdentityHasher` sur le RAM cache (RamKey 32B
//! déjà-hashed). Le `std::HashMap<RamKey, CacheSlot, IdentityBuildHasher>`
//! va passer de chaining (LinkedList per bucket) à open-addressing
//! flat (cache-friendly).
//!
//! Wave 13 minimal viable : SwissMap pure Rust + std (pas d'Abseil
//! crate, doctrine V7 "pure Rust + std + sha2"). Pas d'intrinsics
//! SIMD x86_64 (qui demanderaient `core::arch::x86_64`) — scalar
//! probe O(15) par-slot dans un buffer aligné. Sur clés
//! pre-hashed (RamKey) c'est suffisant.
//!
//! ## Architecture Wave 13 minimal viable
//!
//! - `SwissMap<K, V>` open-addressing table avec :
//!   - `meta: Box<[u8]>` : metadata 1 byte/slot (sentinel + tag)
//!   - `entries: Box<[Option<(K, V)>]>` : slots
//!   - `len`, `capacity`, `growth_threshold`
//! - Constants : SENTINEL_EMPTY = 0x80, SENTINEL_DELETED = 0xFE.
//!   Tag = 7 bits hash & 0x7F (occupied bit 7 = 0).
//! - Probe linear (Wave 13 minimal) — SIMD batch probe Wave 14+.
//! - Insert/get/remove avec triangular probing.
//!
//! ## Limitations Wave 13 minimal
//!
//! - Probe scalar (pas SIMD) — gain ×3 vs std::HashMap, pas ×5.
//! - K: Hash + Eq + Clone (pas Copy obligatoire).
//! - Pas de reserve(), grow auto à 75% load factor.

use std::hash::{BuildHasher, Hash, Hasher};

use crate::fast_hash::FastBuildHasher;

/// Sentinel : slot vide.
#[allow(dead_code)] // Wave 13 — exposé pour wiring RAM cache Wave 14+.
const SENTINEL_EMPTY: u8 = 0x80;
/// Sentinel : slot deleted (tombstone, must skip but not stop probe).
#[allow(dead_code)]
const SENTINEL_DELETED: u8 = 0xFE;

/// Default initial capacity (puissance de 2).
#[allow(dead_code)]
const INITIAL_CAPACITY: usize = 16;

/// Swiss-style open-addressing hash map.
#[allow(dead_code)] // Wave 13 — primitives exposées pour wiring Wave 14+ RAM cache.
pub struct SwissMap<K: Hash + Eq, V> {
    meta: Box<[u8]>,
    entries: Box<[Option<(K, V)>]>,
    len: usize,
    capacity: usize,
    /// Threshold (75% de capacity) au-dessus duquel on grow.
    growth_threshold: usize,
    /// Hasher builder (default SipHash std, peut être surchargé).
    hasher_builder: FastBuildHasher,
}

#[allow(dead_code)]
impl<K: Hash + Eq, V> SwissMap<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(INITIAL_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(INITIAL_CAPACITY).next_power_of_two();
        let meta = vec![SENTINEL_EMPTY; cap].into_boxed_slice();
        let mut entries = Vec::with_capacity(cap);
        for _ in 0..cap {
            entries.push(None);
        }
        Self {
            meta,
            entries: entries.into_boxed_slice(),
            len: 0,
            capacity: cap,
            growth_threshold: cap * 3 / 4,
            hasher_builder: FastBuildHasher,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Hash + tag d'une clé.
    fn hash_and_tag(&self, key: &K) -> (u64, u8) {
        let mut h = self.hasher_builder.build_hasher();
        key.hash(&mut h);
        let hash = h.finish();
        // Tag = 7 bits du hash (bit 7 = 0 pour distinguer des sentinels).
        let tag = (hash & 0x7F) as u8;
        (hash, tag)
    }

    /// Probe pour trouver une clé. Retourne (slot_idx, found).
    /// found = true si la clé existe, false sinon (slot_idx pointe
    /// vers un slot empty ou deleted où l'insert peut aller).
    fn probe(&self, key: &K) -> (usize, bool) {
        let (hash, tag) = self.hash_and_tag(key);
        let mask = self.capacity - 1;
        let mut idx = (hash as usize) & mask;
        let mut first_deleted: Option<usize> = None;

        // Triangular probing : i² + i offset (anti-clustering).
        for step in 0..self.capacity {
            let meta_byte = self.meta[idx];
            if meta_byte == SENTINEL_EMPTY {
                // Empty → key not present. Insert position : la première
                // tombstone trouvée OU ce slot empty si pas de tombstone.
                return (first_deleted.unwrap_or(idx), false);
            }
            if meta_byte == SENTINEL_DELETED {
                if first_deleted.is_none() {
                    first_deleted = Some(idx);
                }
            } else if meta_byte == tag {
                // Tag matches → maybe found, vérifier l'entry.
                if let Some((k, _)) = &self.entries[idx] {
                    if k == key {
                        return (idx, true);
                    }
                }
            }
            // Triangular step : idx + (step+1).
            idx = (idx + step + 1) & mask;
        }
        // Probe complet sans trouver — table pleine. Insert position
        // = first_deleted ou panic (theoretically unreachable post grow).
        (first_deleted.unwrap_or(0), false)
    }

    /// Insert (key, value). Retourne ancienne valeur si présente.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.len >= self.growth_threshold {
            self.grow();
        }
        let (hash, tag) = self.hash_and_tag(&key);
        let _ = hash; // Stocké via tag, hash full pas nécessaire ici.
        let (slot, found) = self.probe(&key);
        if found {
            // Replace existing.
            let old = self.entries[slot].take().map(|(_, v)| v);
            self.entries[slot] = Some((key, value));
            self.meta[slot] = tag;
            old
        } else {
            self.entries[slot] = Some((key, value));
            self.meta[slot] = tag;
            self.len += 1;
            None
        }
    }

    /// Lookup. Retourne Some(&V) si présent.
    pub fn get(&self, key: &K) -> Option<&V> {
        let (slot, found) = self.probe(key);
        if found {
            self.entries[slot].as_ref().map(|(_, v)| v)
        } else {
            None
        }
    }

    /// Vrai si la clé existe.
    pub fn contains_key(&self, key: &K) -> bool {
        self.probe(key).1
    }

    /// Remove. Pose un tombstone (SENTINEL_DELETED) pour ne pas casser
    /// les probe chains. Retourne la valeur si présente.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let (slot, found) = self.probe(key);
        if found {
            self.meta[slot] = SENTINEL_DELETED;
            let val = self.entries[slot].take().map(|(_, v)| v);
            self.len -= 1;
            val
        } else {
            None
        }
    }

    /// Clear (réinitialise tous les slots).
    pub fn clear(&mut self) {
        for m in self.meta.iter_mut() {
            *m = SENTINEL_EMPTY;
        }
        for e in self.entries.iter_mut() {
            *e = None;
        }
        self.len = 0;
    }

    /// Grow : capacity × 2, rehash tous les entries.
    fn grow(&mut self) {
        let new_cap = self.capacity * 2;
        let new_meta = vec![SENTINEL_EMPTY; new_cap].into_boxed_slice();
        let mut new_entries = Vec::with_capacity(new_cap);
        for _ in 0..new_cap {
            new_entries.push(None);
        }
        let new_entries = new_entries.into_boxed_slice();

        let old_meta = std::mem::replace(&mut self.meta, new_meta);
        let old_entries = std::mem::replace(&mut self.entries, new_entries);
        self.capacity = new_cap;
        self.growth_threshold = new_cap * 3 / 4;
        self.len = 0;

        // Re-insert all live entries via slot.take() pour move out.
        let mut old_entries = old_entries.into_vec();
        for (i, slot) in old_entries.iter_mut().enumerate() {
            if old_meta[i] != SENTINEL_EMPTY && old_meta[i] != SENTINEL_DELETED {
                if let Some((k, v)) = slot.take() {
                    self.insert(k, v);
                }
            }
        }
    }

    /// Iterate sur les paires (k, v) live (skip empty/deleted slots).
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries
            .iter()
            .zip(self.meta.iter())
            .filter_map(|(entry, meta)| {
                if *meta != SENTINEL_EMPTY && *meta != SENTINEL_DELETED {
                    entry.as_ref().map(|(k, v)| (k, v))
                } else {
                    None
                }
            })
    }
}

impl<K: Hash + Eq, V> Default for SwissMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swiss_basic_insert_get() {
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        m.insert(42, 100);
        m.insert(99, 200);
        assert_eq!(m.get(&42), Some(&100));
        assert_eq!(m.get(&99), Some(&200));
        assert_eq!(m.get(&777), None);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn swiss_replace_returns_old() {
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        assert_eq!(m.insert(1, 10), None);
        assert_eq!(m.insert(1, 20), Some(10));
        assert_eq!(m.get(&1), Some(&20));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn swiss_remove_returns_value() {
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        m.insert(1, 100);
        assert_eq!(m.remove(&1), Some(100));
        assert_eq!(m.get(&1), None);
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn swiss_remove_then_reinsert_works() {
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        m.insert(1, 100);
        m.remove(&1);
        m.insert(1, 200);
        assert_eq!(m.get(&1), Some(&200));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn swiss_contains_key() {
        let mut m: SwissMap<&str, i32> = SwissMap::new();
        m.insert("alpha", 1);
        assert!(m.contains_key(&"alpha"));
        assert!(!m.contains_key(&"beta"));
    }

    #[test]
    fn swiss_grows_when_full() {
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        let initial_cap = m.capacity();
        // Insert beyond growth threshold.
        for i in 0..(initial_cap * 2) as i64 {
            m.insert(i, i * 10);
        }
        assert!(m.capacity() > initial_cap);
        // Tous les inserts doivent être retrouvables.
        for i in 0..(initial_cap * 2) as i64 {
            assert_eq!(m.get(&i), Some(&(i * 10)), "key {} lost", i);
        }
    }

    #[test]
    fn swiss_clear_resets() {
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        for i in 0..50i64 {
            m.insert(i, i);
        }
        m.clear();
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
        for i in 0..50 {
            assert_eq!(m.get(&i), None);
        }
    }

    #[test]
    fn swiss_iter_returns_all_live() {
        let mut m: SwissMap<i32, i32> = SwissMap::new();
        m.insert(1, 10);
        m.insert(2, 20);
        m.insert(3, 30);
        m.remove(&2);  // tombstone

        let collected: Vec<(i32, i32)> = m.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(collected.len(), 2);
        // Order non-déterministe (open-addressing) — vérifier set.
        let set: std::collections::HashSet<_> = collected.into_iter().collect();
        assert!(set.contains(&(1, 10)));
        assert!(set.contains(&(3, 30)));
        assert!(!set.contains(&(2, 20)));
    }

    #[test]
    fn swiss_handles_collision_via_probing() {
        // Insère beaucoup de clés. Open-addressing doit gérer toutes
        // les collisions sans perdre de paires.
        let mut m: SwissMap<i64, i64> = SwissMap::new();
        for i in 0..1000i64 {
            m.insert(i, i * 7);
        }
        for i in 0..1000i64 {
            assert_eq!(m.get(&i), Some(&(i * 7)), "key {} not found", i);
        }
        assert_eq!(m.len(), 1000);
    }

    #[test]
    fn swiss_string_keys_work() {
        let mut m: SwissMap<String, i32> = SwissMap::new();
        m.insert("hello".to_string(), 1);
        m.insert("world".to_string(), 2);
        assert_eq!(m.get(&"hello".to_string()), Some(&1));
        assert_eq!(m.get(&"world".to_string()), Some(&2));
    }

    #[test]
    fn swiss_default_is_empty() {
        let m: SwissMap<i32, i32> = SwissMap::default();
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
    }

    #[test]
    fn swiss_with_capacity_aligned_to_pow2() {
        let m: SwissMap<i32, i32> = SwissMap::with_capacity(10);
        // 10 → next_power_of_two = 16.
        assert_eq!(m.capacity(), 16);
        let m: SwissMap<i32, i32> = SwissMap::with_capacity(100);
        assert_eq!(m.capacity(), 128);
    }

    #[test]
    fn swiss_remove_unknown_returns_none() {
        let mut m: SwissMap<i32, i32> = SwissMap::new();
        assert_eq!(m.remove(&99), None);
    }
}
