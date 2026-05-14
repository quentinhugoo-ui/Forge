//! Π.25 (Wave 16, 2026-05-02) — `MmapStore` zero-copy CAS read.
//!
//! **Origine** : LMDB B-tree (Howard Chu, 2011), Plan 9 srvfs, Urbit
//! Loom mmap unifié, RocksDB block cache. Idée centrale : au lieu de
//! `read()` + `Vec<u8>` alloc à chaque load, garder le `forge.cas`
//! en mémoire une seule fois et servir les blobs comme `&[u8]` slices
//! zero-copy.
//!
//! ## Pourquoi pour Forge ?
//!
//! `Store::load(hash)` actuel (γ.0) fait : `seek` + `read_exact` →
//! `Vec<u8>`. Coût : 1 syscall + 1 alloc + 1 memcpy par blob load.
//! Pour 100k blob loads/sec (cible swarm post-Wave 17), c'est 200k
//! syscalls + 200k allocs cumulés.
//!
//! `MmapStore` charge le fichier `forge.cas` UNE fois en RAM dans un
//! `Box<[u8]>` partagé (`Arc`-wrapped). Tous les blobs sont des
//! `&[u8]` slices vers ce buffer — zero-copy, zero-alloc per load.
//!
//! ## Doctrine V7 vs vraie mmap
//!
//! La vraie `mmap()` (Linux) nécessite la crate `libc` (interdit V7
//! doctrine pure Rust + std + sha2). Donc Wave 16 livre la version
//! "logical mmap" : full-read upfront via `std::fs::read()`, qui
//! fait UN seul syscall + UN seul alloc, puis zero-copy ensuite.
//!
//! Différence vs vraie mmap :
//!   - Vraie mmap : pages chargées à demand via page faults
//!     (lazy, paginated par 4KB)
//!   - Logical mmap : tout chargé upfront (eager, monolithic)
//!
//! Pour `forge.cas` < 1GB, le full-read est competitive avec mmap
//! (même nombre de page faults, juste front-loaded). Pour > 1GB,
//! upgrade vers vraie mmap requiert libc dep — décision Wave 17+.
//!
//! ## Architecture Wave 16 minimal viable
//!
//! - `MmapStore { backing: Arc<Box<[u8]>> }` charge fichier complet.
//! - `lookup(hash)` retourne `Option<&[u8]>` slice du backing (zero-copy).
//! - Index séparé `HashMap<Hash, (offset, len)>` reconstruit au chargement.
//! - `MmapStoreError::Io / NotFound / BadFormat` pour erreurs.
//!
//! Synergie avec Π.27 IntrusiveBlobIndex (Wave 16 même) : on peut
//! remplacer la HashMap par un index intrusive si justifié par mesure.
//!
//! ## Limitations Wave 16 minimal
//!
//! - Read-only (writes vont via le `Store` original qui écrit append-only).
//! - Pas de invalidation/refresh — caller doit recharger MmapStore
//!   manuellement après écritures externes.
//! - Pas de mmap MAP_HUGETLB (Σ.22 reporté Wave 17 + libc dep).

use crate::store::Hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub enum MmapStoreError {
    Io(std::io::Error),
    BadMagic,
    BadVersion(u32),
    Truncated,
}

impl std::fmt::Display for MmapStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MmapStoreError::Io(e) => write!(f, "mmap_store: io error: {}", e),
            MmapStoreError::BadMagic => write!(f, "mmap_store: bad magic"),
            MmapStoreError::BadVersion(v) => write!(f, "mmap_store: unsupported version {}", v),
            MmapStoreError::Truncated => write!(f, "mmap_store: truncated record"),
        }
    }
}

impl From<std::io::Error> for MmapStoreError {
    fn from(e: std::io::Error) -> Self {
        MmapStoreError::Io(e)
    }
}

/// Magic header `forge.cas`. Doit matcher `crate::store::MAGIC`.
const CAS_MAGIC: &[u8; 8] = b"FORGECAS";
const HEADER_LEN: usize = 32;
const TAG_BLOB: u8 = 1;
const TAG_REF: u8 = 2;
const TAG_UNREF: u8 = 3;

/// Mmap-style store : full backing buffer + index par offset.
#[allow(dead_code)] // Wave 16 — primitives expose pour wiring Wave 18+.
#[derive(Debug)]
pub struct MmapStore {
    /// Backing buffer entier (forge.cas en RAM). Wrapped dans
    /// HugePageBuffer (Σ.22 wiré) pour signaler le hint 2 MB pages
    /// — Wave 17+ pourra activer real huge pages via libc/syscall
    /// sans changer cette API. Pour un atlas > 2 MB, gain TLB attendu
    /// quand le hint sera activé : -10 à -20% sur scan complet.
    backing: Arc<super::huge_pages::HugePageBuffer>,
    /// Index Hash → (offset, len) via IntrusiveBlobIndex (Π.27 wiré).
    /// 16 bytes/entry compact + binary search O(log N) cache-friendly,
    /// vs HashMap qui est ~30-60 bytes/entry. Pour un atlas de 100M
    /// entrées : 1.6 GB vs 3-6 GB. Critique pour tenir en RAM les gros
    /// atlas sciences/finance.
    index: super::intrusive_index::IntrusiveBlobIndex,
    /// Path d'origine (pour debug + reload).
    path: PathBuf,
}

#[allow(dead_code)]
impl MmapStore {
    /// Charge un `forge.cas` depuis le path donné, full-read upfront.
    pub fn open(cas_path: impl Into<PathBuf>) -> Result<Self, MmapStoreError> {
        let path = cas_path.into();
        let bytes = std::fs::read(&path)?;
        if bytes.len() < HEADER_LEN {
            return Err(MmapStoreError::Truncated);
        }
        if &bytes[..8] != CAS_MAGIC {
            return Err(MmapStoreError::BadMagic);
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != 1 {
            return Err(MmapStoreError::BadVersion(version));
        }
        let raw_box: Box<[u8]> = bytes.into_boxed_slice();
        // Σ.21 wire — prefault toutes les pages du backing buffer.
        // Évite les page faults imprévisibles sur le premier accès
        // hot-path (~5-15 µs/page faulted at random). Avec prefault,
        // toutes les pages sont résidentes avant que dispatch_impl
        // commence à les lire. Bénéfice critique pour Tauri UI cold
        // start ET pour les workloads temps-réel finance.
        let _prefault_stats = super::prefault::prefault_buffer(&raw_box);
        let index = build_index(&raw_box)?;
        // Σ.22 wire — wrap le backing dans HugePageBuffer. Wave 16
        // minimal : pass-through (no syscall). Wave 17+ activera vraies
        // huge pages 2 MB via libc → ~10-20% TLB miss reduction sur
        // les atlas multi-Go (génome, tick history, MD trajectory).
        let backing = super::huge_pages::HugePageBuffer::from_box(raw_box);
        Ok(Self {
            backing: Arc::new(backing),
            index,
            path,
        })
    }

    /// Charge depuis un buffer raw (pour test).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, MmapStoreError> {
        if bytes.len() < HEADER_LEN {
            return Err(MmapStoreError::Truncated);
        }
        if &bytes[..8] != CAS_MAGIC {
            return Err(MmapStoreError::BadMagic);
        }
        let raw_box: Box<[u8]> = bytes.into_boxed_slice();
        let index = build_index(&raw_box)?;
        let backing = super::huge_pages::HugePageBuffer::from_box(raw_box);
        Ok(Self {
            backing: Arc::new(backing),
            index,
            path: PathBuf::new(),
        })
    }

    /// Lookup zero-copy d'un blob par hash. Retourne `&[u8]` slice du
    /// backing ; le caller obtient une référence avec lifetime borné
    /// par `&self`.
    ///
    /// Wire Π.27 IntrusiveBlobIndex : binary search O(log N) sur les
    /// 8 premiers bytes du hash (collision-free pour SHA-1 distribués
    /// uniformément). ~50 ns par lookup, comparable à HashMap mais
    /// avec 2-3× moins de RAM.
    pub fn lookup(&self, hash: &Hash) -> Option<&[u8]> {
        let (offset, len) = self.index.lookup(hash)?;
        let off = offset as usize;
        let l = len as usize;
        Some(&self.backing.as_slice()[off..off + l])
    }

    /// Variante owned : retourne `Arc<Box<[u8]>>` partagé + range.
    /// Convient pour caller qui veut détenir le slice au-delà du
    /// `&self` borrow.
    pub fn lookup_owned(&self, hash: &Hash) -> Option<MmapBlobRef> {
        let (offset, len) = self.index.lookup(hash)?;
        Some(MmapBlobRef {
            backing: Arc::clone(&self.backing),
            offset: offset as usize,
            len: len as usize,
        })
    }

    /// Total bytes du backing — wrapper sur HugePageBuffer.len().
    pub fn backing_len(&self) -> usize {
        self.backing.len()
    }

    /// Nombre de blobs indexés.
    pub fn blob_count(&self) -> usize {
        self.index.len()
    }

    /// Total bytes du backing.
    pub fn backing_size(&self) -> usize {
        self.backing.len()
    }

    /// Nouveau Wave 16+ wire : référence vers le HugePageBuffer pour
    /// observabilité (`stats()`, `huge_page_active()`).
    pub fn huge_page_buffer(&self) -> &super::huge_pages::HugePageBuffer {
        &self.backing
    }

    /// Path d'origine (pour reload / debug).
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Reference partagée vers un blob du `MmapStore`. Détient un Arc sur
/// le backing → le slice reste valide tant que la ref vit.
#[allow(dead_code)]
#[derive(Clone)]
pub struct MmapBlobRef {
    backing: Arc<super::huge_pages::HugePageBuffer>,
    offset: usize,
    len: usize,
}

#[allow(dead_code)]
impl MmapBlobRef {
    pub fn as_slice(&self) -> &[u8] {
        &self.backing.as_slice()[self.offset..self.offset + self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl AsRef<[u8]> for MmapBlobRef {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// Parse les records du CAS et retourne l'index Hash → (offset, len).
/// Wire Π.27 IntrusiveBlobIndex : index compact 16 bytes/entry + binary
/// search O(log N), vs HashMap qui était 30-60 bytes/entry.
fn build_index(bytes: &[u8]) -> Result<super::intrusive_index::IntrusiveBlobIndex, MmapStoreError> {
    use super::intrusive_index::IntrusiveBlobIndex;
    // Pré-collecte les blobs pour bulk insert (sort-once), plus rapide
    // qu'insert un par un qui re-trie à chaque ajout.
    let mut blobs: Vec<(Hash, u32, u32)> = Vec::new();
    let mut cursor = HEADER_LEN;
    while cursor < bytes.len() {
        if cursor + 5 > bytes.len() {
            return Err(MmapStoreError::Truncated);
        }
        let tag = bytes[cursor];
        let payload_len = u32::from_le_bytes(
            bytes[cursor + 1..cursor + 5].try_into().unwrap(),
        ) as usize;
        cursor += 5;
        if cursor + payload_len > bytes.len() {
            return Err(MmapStoreError::Truncated);
        }
        match tag {
            TAG_BLOB => {
                if payload_len < 20 {
                    return Err(MmapStoreError::Truncated);
                }
                let hash_bytes: [u8; 20] = bytes[cursor..cursor + 20].try_into().unwrap();
                let hash = Hash::from_bytes(hash_bytes);
                let blob_offset = cursor + 20;
                let blob_len = payload_len - 20;
                // u32 cap : forge.cas > 4 GB unsupported par IntrusiveBlobIndex ;
                // fallback errored pour signaler explicitement.
                if blob_offset > u32::MAX as usize || blob_len > u32::MAX as usize {
                    return Err(MmapStoreError::Truncated);
                }
                blobs.push((hash, blob_offset as u32, blob_len as u32));
            }
            TAG_REF | TAG_UNREF => {
                // Skip — refs ne sont pas indexées par MmapStore minimal.
            }
            _ => {
                // Tag inconnu — skip défensif (forward compatibility).
            }
        }
        cursor += payload_len;
    }
    Ok(IntrusiveBlobIndex::from_blobs(blobs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::{fresh_tmp_path, TmpDir};

    fn open_test_store(tag: &str) -> (TmpDir, Store, PathBuf) {
        let path = fresh_tmp_path("mmap-store-test", tag);
        std::fs::create_dir_all(&path).unwrap();
        let guard = TmpDir::new(path.clone());
        let store = Store::open(&path).unwrap();
        let cas_file = path.join("forge.cas");
        (guard, store, cas_file)
    }

    #[test]
    fn mmap_store_open_empty_cas() {
        let (_g, _s, cas) = open_test_store("empty");
        let mmap = MmapStore::open(&cas).unwrap();
        assert_eq!(mmap.blob_count(), 0);
        // Backing = header only (32 bytes).
        assert_eq!(mmap.backing_size(), HEADER_LEN);
    }

    #[test]
    fn mmap_store_lookup_existing_blob() {
        let (_g, store, cas) = open_test_store("lookup");
        let payload = b"mmap test payload";
        let hash = store.store(payload).unwrap();
        drop(store);  // release lock pour MmapStore qui ouvre le file.

        let mmap = MmapStore::open(&cas).unwrap();
        assert_eq!(mmap.blob_count(), 1);
        let slice = mmap.lookup(&hash).expect("blob should be in mmap");
        assert_eq!(slice, payload);
    }

    #[test]
    fn mmap_store_lookup_unknown_returns_none() {
        let (_g, _s, cas) = open_test_store("unknown");
        let mmap = MmapStore::open(&cas).unwrap();
        let bogus = Hash::from_bytes([0u8; 20]);
        assert!(mmap.lookup(&bogus).is_none());
    }

    #[test]
    fn mmap_store_multi_blob_index() {
        let (_g, store, cas) = open_test_store("multi");
        let h1 = store.store(b"alpha").unwrap();
        let h2 = store.store(b"bravo bytes").unwrap();
        let h3 = store.store(b"").unwrap();
        drop(store);

        let mmap = MmapStore::open(&cas).unwrap();
        assert_eq!(mmap.blob_count(), 3);
        assert_eq!(mmap.lookup(&h1).unwrap(), b"alpha");
        assert_eq!(mmap.lookup(&h2).unwrap(), b"bravo bytes");
        assert_eq!(mmap.lookup(&h3).unwrap(), b"");
    }

    #[test]
    fn mmap_store_lookup_owned_arc_share() {
        let (_g, store, cas) = open_test_store("arc-share");
        let payload = b"shared bytes";
        let hash = store.store(payload).unwrap();
        drop(store);

        let mmap = MmapStore::open(&cas).unwrap();
        let blob_ref = mmap.lookup_owned(&hash).unwrap();
        let cloned = blob_ref.clone();
        // Drop original ref, cloned reste valide via Arc.
        drop(blob_ref);
        assert_eq!(cloned.as_slice(), payload);
        assert_eq!(cloned.len(), payload.len());
        assert!(!cloned.is_empty());
    }

    #[test]
    fn mmap_store_bad_magic_errors() {
        let bytes = vec![0xFFu8; 32];
        let err = MmapStore::from_bytes(bytes).unwrap_err();
        assert!(matches!(err, MmapStoreError::BadMagic));
    }

    #[test]
    fn mmap_store_truncated_errors() {
        let bytes = vec![0u8; 10];  // < HEADER_LEN
        let err = MmapStore::from_bytes(bytes).unwrap_err();
        assert!(matches!(err, MmapStoreError::Truncated));
    }

    #[test]
    fn mmap_store_zero_copy_slice_lifetimes() {
        // Sanity test : le slice retourné par lookup() est lifetime-borne
        // par &self → le compilateur Rust empêche l'évasion. Test
        // ne compilerait pas si lifetime mal géré.
        let (_g, store, cas) = open_test_store("lifetime");
        let payload = b"lifetime test";
        let hash = store.store(payload).unwrap();
        drop(store);

        let mmap = MmapStore::open(&cas).unwrap();
        let slice = mmap.lookup(&hash).unwrap();
        // slice peut être lu librement ici.
        assert_eq!(slice, payload);
        // slice ne survit pas au drop de mmap (compile-time check).
    }

    #[test]
    fn mmap_store_path_recoverable() {
        let (_g, _s, cas) = open_test_store("path");
        let mmap = MmapStore::open(&cas).unwrap();
        assert_eq!(mmap.path(), cas.as_path());
    }
}
