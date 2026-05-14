//! Ω-5.0H-B — Content-Addressed Memory Fabric Simulator.
//!
//! Détournement *non armé* des concepts hardware identifiés dans
//! `docs/OMEGA_RAM_INTROSPECTION_IDEAS.md` :
//!
//!   * Idée #14 — FPGA DRAM controller indexé par hash de contenu plutôt
//!     que par adresse physique.
//!   * Idée #10 — Battering-RAM-style interposer qui réécrit le bus
//!     mémoire pour exposer un content-addressing au niveau silicon.
//!
//! **Sécurité** : ce module est PUREMENT logique. Aucun Rowhammer, aucun
//! DMA hors-process, aucun cold-boot, aucune lecture RAM cross-process.
//! On modélise le comportement *attendu* d'un substrat content-addressed
//! comme une structure de données Rust ordinaire — c'est la sandbox de
//! validation qui précédera tout effort hardware réel.
//!
//! Le simulateur expose :
//!   * Un store content-addressed (`hash → bytes`, immuable).
//!   * Un mapping virtuel (`VirtualAddr → ContentHash`) qui peut être
//!     remappé/migré sans copier les bytes.
//!   * Une allocation déterministe de `PhysicalSlot` par hash.
//!   * Un TLB minimal (`VirtualAddr` ever-resolved set) pour distinguer
//!     les premiers accès (miss) des accès subséquents (hit).
//!   * Un `fabric_hash` canonique invariant sous l'ordre d'insertion.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

const HASH_DOMAIN: &[u8] = b"SCAN-OMEGA-FABRIC-PAGE-V1";
const FABRIC_HASH_DOMAIN: &[u8] = b"SCAN-OMEGA-FABRIC-STATE-V1";
const TLB_CAPACITY: usize = 64;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Hash content-addressed sur 32 bytes (sha256 domain-separated).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Calcule le hash d'un blob avec un domaine de séparation explicite.
    /// Utiliser ce constructeur garantit qu'aucune collision ne peut être
    /// induite depuis un autre contexte (autre crate, autre niveau de hash).
    pub fn for_bytes(bytes: &[u8]) -> Self {
        let mut h = Sha256::new();
        h.update(HASH_DOMAIN);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
        let result = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        Self(out)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Adresse virtuelle = poignée logique. Aucune sémantique physique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualAddr(pub u64);

/// Slot physique alloué dans le substrat simulé. Monotone, déterministe
/// dans l'ordre d'insertion d'un hash unique. **Pas inclus dans
/// `fabric_hash`** — c'est un détail d'allocation runtime, pas un état
/// content-addressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalSlot(pub u64);

/// Page indexée par contenu. Les bytes sont **immuables une fois indexés** —
/// aucune API publique du fabric ne permet de les mutater.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FabricPage {
    pub hash: ContentHash,
    pub bytes: Vec<u8>,
}

/// Compteurs runtime du fabric.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FabricMetrics {
    /// Resolves cache hits (TLB hits).
    pub hits: u64,
    /// Resolves cache misses (TLB misses, premier accès à une addr).
    pub misses: u64,
    /// Nombre de remaps explicites ou implicites (insert sur addr déjà mappée).
    pub remaps: u64,
    /// Dédoublonnages : insertion d'un hash déjà présent dans le store.
    pub dedupes: u64,
}

/// Erreur retournée par les opérations de remap/migrate.
#[derive(Debug, PartialEq, Eq)]
pub enum FabricError {
    /// `remap` cible un hash absent du page store.
    UnknownHash(ContentHash),
    /// `migrate_addr` source non mappée.
    UnknownAddr(VirtualAddr),
}

impl std::fmt::Display for FabricError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FabricError::UnknownHash(h) => {
                write!(f, "fabric: unknown content hash {:02x?}", &h.0[..8])
            }
            FabricError::UnknownAddr(a) => write!(f, "fabric: unknown virtual addr {}", a.0),
        }
    }
}

impl std::error::Error for FabricError {}

// ---------------------------------------------------------------------------
// Fabric
// ---------------------------------------------------------------------------

/// Substrat mémoire content-addressed simulé.
///
/// Garanties :
///   * Pages immuables une fois indexées (aucune mutation publique).
///   * Dédoublonnage automatique : deux insertions avec mêmes bytes
///     partagent le même slot et le même hash.
///   * `migrate_addr` / `remap` n'effectuent **aucune copie de bytes**,
///     uniquement la modification du mapping.
///   * `fabric_hash` est un hash canonique de l'état content-addressed
///     (pages + mapping), invariant sous l'ordre d'insertion.
#[derive(Debug, Default)]
pub struct ContentAddressedFabric {
    pages: BTreeMap<ContentHash, FabricPage>,
    mapping: BTreeMap<VirtualAddr, ContentHash>,
    slots: BTreeMap<ContentHash, PhysicalSlot>,
    next_slot: u64,
    tlb: Vec<VirtualAddr>,
    metrics: FabricMetrics,
}

impl ContentAddressedFabric {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insère un blob et le mappe à `addr`. Retourne le `ContentHash`.
    /// Si le blob existe déjà → dédoublonnage (pas de nouvelle copie).
    /// Si `addr` était déjà mappée → comptabilisé comme `remap`.
    pub fn insert(&mut self, addr: VirtualAddr, bytes: Vec<u8>) -> ContentHash {
        let hash = ContentHash::for_bytes(&bytes);
        if self.pages.contains_key(&hash) {
            self.metrics.dedupes += 1;
        } else {
            self.pages.insert(hash, FabricPage { hash, bytes });
            self.slots.insert(hash, PhysicalSlot(self.next_slot));
            self.next_slot += 1;
        }
        if self.mapping.insert(addr, hash).is_some() {
            self.metrics.remaps += 1;
            self.invalidate_tlb(addr);
        }
        hash
    }

    /// Résout `addr` en page. Met à jour le TLB et les métriques.
    /// Premier accès = miss, suivants = hit.
    pub fn resolve(&mut self, addr: VirtualAddr) -> Option<&FabricPage> {
        let hash = *self.mapping.get(&addr)?;
        if self.tlb.iter().any(|a| *a == addr) {
            self.metrics.hits += 1;
        } else {
            self.metrics.misses += 1;
            self.tlb.push(addr);
            if self.tlb.len() > TLB_CAPACITY {
                self.tlb.remove(0);
            }
        }
        self.pages.get(&hash)
    }

    /// Repointe `addr` vers `hash`. Erreur si le hash n'est pas dans le store.
    pub fn remap(&mut self, addr: VirtualAddr, hash: ContentHash) -> Result<(), FabricError> {
        if !self.pages.contains_key(&hash) {
            return Err(FabricError::UnknownHash(hash));
        }
        if self.mapping.insert(addr, hash).is_some() {
            self.metrics.remaps += 1;
        }
        self.invalidate_tlb(addr);
        Ok(())
    }

    /// Migre le mapping de `from` vers `to`. Aucune copie de bytes.
    /// Après migration, `from` est démappée ; `to` pointe vers le hash
    /// d'origine. Erreur si `from` n'existe pas.
    pub fn migrate_addr(
        &mut self,
        from: VirtualAddr,
        to: VirtualAddr,
    ) -> Result<(), FabricError> {
        let hash = self
            .mapping
            .remove(&from)
            .ok_or(FabricError::UnknownAddr(from))?;
        if self.mapping.insert(to, hash).is_some() {
            self.metrics.remaps += 1;
        }
        self.invalidate_tlb(from);
        self.invalidate_tlb(to);
        Ok(())
    }

    pub fn metrics(&self) -> FabricMetrics {
        self.metrics
    }

    /// Hash canonique de l'état content-addressed.
    ///
    /// Construction :
    ///   1. Domain separator `SCAN-OMEGA-FABRIC-STATE-V1`.
    ///   2. Pages triées par `ContentHash` (ordre BTreeMap natif).
    ///   3. Mapping trié par `VirtualAddr` (ordre BTreeMap natif).
    ///
    /// Le `next_slot`, le TLB, et les métriques runtime ne sont **PAS**
    /// inclus — ils dépendent de l'historique d'opérations, pas de l'état
    /// content-addressed final.
    pub fn fabric_hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(FABRIC_HASH_DOMAIN);
        h.update((self.pages.len() as u64).to_le_bytes());
        for (hash, page) in &self.pages {
            h.update(hash.as_bytes());
            h.update((page.bytes.len() as u64).to_le_bytes());
            h.update(&page.bytes);
        }
        h.update((self.mapping.len() as u64).to_le_bytes());
        for (addr, hash) in &self.mapping {
            h.update(addr.0.to_le_bytes());
            h.update(hash.as_bytes());
        }
        let result = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    /// Slot physique alloué pour un hash, si présent. Utile pour debug ;
    /// pas inclus dans `fabric_hash` (allocation runtime).
    pub fn physical_slot_for(&self, hash: &ContentHash) -> Option<PhysicalSlot> {
        self.slots.get(hash).copied()
    }

    fn invalidate_tlb(&mut self, addr: VirtualAddr) {
        self.tlb.retain(|a| *a != addr);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fab() -> ContentAddressedFabric {
        ContentAddressedFabric::new()
    }

    #[test]
    fn same_bytes_same_hash() {
        let bytes = vec![1, 2, 3, 4, 5];
        let h1 = ContentHash::for_bytes(&bytes);
        let h2 = ContentHash::for_bytes(&bytes);
        assert_eq!(h1, h2);

        // Et via le fabric : deux insertions de mêmes bytes à des addrs
        // différentes produisent le même hash.
        let mut f = fab();
        let a = f.insert(VirtualAddr(1), vec![10, 20, 30]);
        let b = f.insert(VirtualAddr(2), vec![10, 20, 30]);
        assert_eq!(a, b);
    }

    #[test]
    fn different_bytes_different_hash() {
        let h1 = ContentHash::for_bytes(b"hello");
        let h2 = ContentHash::for_bytes(b"world");
        assert_ne!(h1, h2);

        let mut f = fab();
        let a = f.insert(VirtualAddr(1), vec![1, 2, 3]);
        let b = f.insert(VirtualAddr(2), vec![4, 5, 6]);
        assert_ne!(a, b);
    }

    #[test]
    fn insert_then_resolve() {
        let mut f = fab();
        let bytes = vec![42, 99, 7];
        let h = f.insert(VirtualAddr(1), bytes.clone());
        let page = f.resolve(VirtualAddr(1)).expect("must resolve");
        assert_eq!(page.hash, h);
        assert_eq!(page.bytes, bytes);
    }

    #[test]
    fn duplicate_insert_dedupes() {
        let mut f = fab();
        let bytes = vec![1, 2, 3];
        f.insert(VirtualAddr(1), bytes.clone());
        f.insert(VirtualAddr(2), bytes.clone());
        f.insert(VirtualAddr(3), bytes);
        // Trois mappings, une seule page physique.
        assert_eq!(f.mapping.len(), 3);
        assert_eq!(f.pages.len(), 1);
        assert_eq!(f.metrics().dedupes, 2);
    }

    #[test]
    fn remap_changes_address_without_copy() {
        let mut f = fab();
        let h = f.insert(VirtualAddr(1), vec![7, 8, 9]);
        let pages_before = f.pages.len();

        // Remap addr=2 vers le même hash → pas de nouvelle page.
        f.remap(VirtualAddr(2), h).expect("remap");
        assert_eq!(f.pages.len(), pages_before, "remap doit pas créer de page");

        // Les deux addrs résolvent au même contenu.
        let p1_bytes = f.resolve(VirtualAddr(1)).unwrap().bytes.clone();
        let p2_bytes = f.resolve(VirtualAddr(2)).unwrap().bytes.clone();
        assert_eq!(p1_bytes, p2_bytes);
        assert_eq!(p1_bytes, vec![7, 8, 9]);
    }

    #[test]
    fn migrate_addr_preserves_content_hash() {
        let mut f = fab();
        let h = f.insert(VirtualAddr(10), vec![100, 101, 102]);
        f.migrate_addr(VirtualAddr(10), VirtualAddr(20)).expect("migrate");

        // L'ancienne addr est démappée.
        assert!(f.resolve(VirtualAddr(10)).is_none());
        // La nouvelle addr résout vers le même hash.
        let page = f.resolve(VirtualAddr(20)).expect("new addr");
        assert_eq!(page.hash, h);
        assert_eq!(page.bytes, vec![100, 101, 102]);
        // Une seule page physique reste.
        assert_eq!(f.pages.len(), 1);
    }

    #[test]
    fn unknown_addr_returns_none() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![1]);
        // Addr jamais insérée.
        assert!(f.resolve(VirtualAddr(9999)).is_none());
    }

    #[test]
    fn unknown_hash_remap_errors() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![1, 2, 3]);
        // Hash arbitraire jamais inséré.
        let bogus = ContentHash::from_bytes([0xAA; 32]);
        let result = f.remap(VirtualAddr(2), bogus);
        assert!(matches!(result, Err(FabricError::UnknownHash(_))));
    }

    #[test]
    fn fabric_hash_stable() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![1, 2]);
        f.insert(VirtualAddr(2), vec![3, 4]);
        let h1 = f.fabric_hash();
        let h2 = f.fabric_hash();
        let h3 = f.fabric_hash();
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);

        // Accès en lecture (resolve) ne change pas le hash canonique
        // (les métriques mutent mais ne participent pas au hash).
        let _ = f.resolve(VirtualAddr(1));
        let _ = f.resolve(VirtualAddr(1));
        let h4 = f.fabric_hash();
        assert_eq!(h1, h4, "fabric_hash doit être insensible aux resolves");
    }

    #[test]
    fn fabric_hash_order_independent() {
        // Insertion ordre A → B
        let mut a = fab();
        a.insert(VirtualAddr(1), vec![1, 2, 3]);
        a.insert(VirtualAddr(2), vec![4, 5, 6]);
        a.insert(VirtualAddr(3), vec![7, 8, 9]);

        // Insertion ordre B → A → C (et donc allocation slots différente)
        let mut b = fab();
        b.insert(VirtualAddr(2), vec![4, 5, 6]);
        b.insert(VirtualAddr(1), vec![1, 2, 3]);
        b.insert(VirtualAddr(3), vec![7, 8, 9]);

        // Même état content-addressed final → même fabric_hash.
        assert_eq!(a.fabric_hash(), b.fabric_hash());
    }

    #[test]
    fn tlb_hits_after_first_resolve() {
        let mut f = fab();
        f.insert(VirtualAddr(1), vec![42]);

        // Premier resolve = miss.
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        assert_eq!(f.metrics().misses, 1);
        assert_eq!(f.metrics().hits, 0);

        // Trois resolves de plus = trois hits.
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        assert_eq!(f.metrics().hits, 3);
        assert_eq!(f.metrics().misses, 1);

        // Une remap invalide le TLB pour cette addr → next resolve = miss.
        let h = ContentHash::for_bytes(&[42]);
        f.remap(VirtualAddr(1), h).unwrap();
        let _ = f.resolve(VirtualAddr(1)).unwrap();
        assert_eq!(f.metrics().misses, 2);
    }

    #[test]
    fn immutable_content_cannot_be_mutated_through_api() {
        let mut f = fab();
        let original = vec![1, 2, 3, 4, 5];
        let h = f.insert(VirtualAddr(1), original.clone());

        // Snapshot des bytes après insertion.
        let snap = f.resolve(VirtualAddr(1)).unwrap().bytes.clone();
        assert_eq!(snap, original);

        // Insertions répétées avec mêmes bytes ne modifient pas la page.
        for _ in 0..5 {
            f.insert(VirtualAddr(1), original.clone());
        }
        let after_reinserts = f.resolve(VirtualAddr(1)).unwrap().bytes.clone();
        assert_eq!(after_reinserts, original);

        // Insertion avec bytes différents crée une NOUVELLE page (hash
        // différent), n'altère pas la page d'origine.
        let h2 = f.insert(VirtualAddr(2), vec![9, 9, 9]);
        assert_ne!(h, h2);

        // La page originale a toujours ses bytes intacts.
        let page1 = f.pages.get(&h).expect("page hash should still exist");
        assert_eq!(page1.bytes, original);

        // Migration ne touche pas non plus aux bytes.
        f.migrate_addr(VirtualAddr(1), VirtualAddr(3)).unwrap();
        let migrated = f.resolve(VirtualAddr(3)).unwrap().bytes.clone();
        assert_eq!(migrated, original);
    }

    // ---- Tests bonus pour blindage supplémentaire ----

    #[test]
    fn physical_slot_is_assigned_per_unique_hash() {
        let mut f = fab();
        let h1 = f.insert(VirtualAddr(1), vec![1]);
        let h2 = f.insert(VirtualAddr(2), vec![2]);
        let s1 = f.physical_slot_for(&h1).unwrap();
        let s2 = f.physical_slot_for(&h2).unwrap();
        assert_ne!(s1, s2);

        // Insérer le même contenu ne crée pas un nouveau slot.
        let h3 = f.insert(VirtualAddr(3), vec![1]);
        assert_eq!(h3, h1);
        assert_eq!(f.physical_slot_for(&h3).unwrap(), s1);
    }

    #[test]
    fn migrate_unknown_addr_errors() {
        let mut f = fab();
        let result = f.migrate_addr(VirtualAddr(1), VirtualAddr(2));
        assert!(matches!(result, Err(FabricError::UnknownAddr(_))));
    }

    #[test]
    fn empty_fabric_hash_is_well_defined() {
        let f1 = fab();
        let f2 = fab();
        assert_eq!(f1.fabric_hash(), f2.fabric_hash());
    }

    #[test]
    fn hash_for_bytes_uses_domain_separation() {
        // Domaine garantit qu'un hash de bytes au sein du fabric ne peut
        // pas accidentellement collisionner avec un hash externe sur les
        // mêmes bytes (différent domaine, différent préfixe).
        let h_fabric = ContentHash::for_bytes(b"abc");
        let mut raw = Sha256::new();
        raw.update(b"abc");
        let raw_result: [u8; 32] = raw.finalize().into();
        assert_ne!(h_fabric.as_bytes(), &raw_result);
    }
}
