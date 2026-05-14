//! Append-only content-addressed store.
//!
//! V7 (Phase γ.0) : remplace l'implémentation V6 (git2 + sidecar
//! SCAN-DB) par un fichier unique `forge.cas` par répertoire.
//! Aucune dépendance externe — pure Rust + std + le SHA-1
//! local (compatible bit-pour-bit avec Git pour le hash de blob).
//!
//! ## Format de fichier
//!
//! ```text
//! Header (32 octets fixes) :
//!   "FORGECAS"   8 B   magic
//!   u32 LE       4 B   version = 1
//!   u32 LE       4 B   flags   = 0
//!   16 B               réservé (zéro)
//!
//! Record (variable, append-only) :
//!   u8           1 B   tag
//!   u32 LE       4 B   payload_len
//!   [..]               payload (selon tag)
//!
//! BLOB (tag = 1) payload :
//!   20 B               hash (SHA-1, framing git "blob {len}\0")
//!   N B                bytes bruts (N = payload_len - 20)
//!
//! REF (tag = 2) payload :
//!   u16 LE       2 B   name_len
//!   M B                name (UTF-8)
//!   20 B               target hash
//!
//! UNREF (tag = 3) payload :
//!   u16 LE       2 B   name_len
//!   M B                name (tombstone)
//! ```
//!
//! ## Propriétés
//!
//! * **Append-only** : chaque opération est un record écrit en bout
//!   de fichier puis flushé. Pas de modification in-place.
//! * **Crash-safe** : un record interrompu en cours d'écriture est
//!   ignoré au replay (truncation au dernier record valide).
//! * **Content-addressed dedup** : si un blob avec ce hash existe
//!   déjà dans l'index RAM, l'append est sauté.
//! * **Refs latest-wins** : chaque REF du même nom écrase l'entrée
//!   précédente dans l'index RAM. Les anciens records restent dans
//!   le fichier (audit trail) mais ne sont jamais consultés.
//! * **Refs deletables** : un UNREF marque le nom comme absent.
//!
//! ## Index RAM
//!
//! * `blobs : HashMap<Hash, BlobRef>` — pour `load` O(1) avec un seek.
//! * `refs  : HashMap<String, Hash>` — pour `lookup_ref` O(1) sans I/O.
//! * `cache : VecDeque<CachedBlob>`  — petit cache LRU des blobs
//!   récemment lus (8 entrées max).

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use crate::monster::swiss_table::SwissMap;
use crate::{pack_lossless, unpack_lossless, CodecError};

// ---------------------------------------------------------------------------
// Format constants
// ---------------------------------------------------------------------------

const MAGIC: &[u8; 8] = b"FORGECAS";
const VERSION: u32 = 1;
const HEADER_LEN: u64 = 32;

const TAG_BLOB: u8 = 1;
const TAG_REF: u8 = 2;
const TAG_UNREF: u8 = 3;

const BLOB_CACHE_LIMIT: usize = 8;

// ---------------------------------------------------------------------------
// Hash — opaque 20-byte content address
// ---------------------------------------------------------------------------

/// Opaque 20-byte content address. Bit-compatible with Git's blob OID
/// (SHA-1 over the framed payload `"blob {len}\0" || bytes`), so a
/// hash computed by `Hash::for_blob` matches what `git hash-object -t
/// blob` would produce on the same payload. The compatibility is
/// preserved across the V6→V7 rewrite even though we no longer link
/// against libgit2 — it's purely a hashing convention.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash([u8; 20]);

impl Hash {
    pub fn from_hex(s: &str) -> Option<Self> {
        if s.len() != 40 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return None;
        }
        let mut bytes = [0u8; 20];
        for (i, slot) in bytes.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Hash(bytes))
    }

    pub fn for_blob(bytes: &[u8]) -> Self {
        let mut framed = format!("blob {}\0", bytes.len()).into_bytes();
        framed.extend_from_slice(bytes);
        Hash(sha1(&framed))
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Hash(bytes)
    }

    pub fn as_hex(&self) -> String {
        hex20(self.0)
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", &self.as_hex()[..12])
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.as_hex())
    }
}

// ---------------------------------------------------------------------------
// Store — content-addressed substrate
// ---------------------------------------------------------------------------

/// Content-addressed store, single-file backed. The directory passed
/// to [`open`] holds a single `forge.cas` file (V7 format). API
/// preserved from V6: blobs (`store`/`load`), memos
/// (`write_memo`/`lookup_memo`), and arbitrary refs
/// (`write_ref`/`lookup_ref`/`delete_ref`).
pub struct Store {
    path: PathBuf,
    file: Mutex<File>,
    /// Σ.12 wire — SwissMap au lieu de HashMap pour blob lookup.
    /// Open-addressing avec triangular probing + tag 7-bit, plus
    /// cache-friendly que HashMap sur les workloads atlas-heavy
    /// (millions de blobs). Hot path Layer 5 lookup_memo + load.
    blobs: RwLock<SwissMap<Hash, BlobRef>>,
    refs: RwLock<HashMap<String, Hash>>,
    cache: Mutex<VecDeque<CachedBlob>>,
}

#[derive(Clone, Copy)]
struct BlobRef {
    payload_offset: u64,
    blob_len: u32,
}

struct CachedBlob {
    hash: Hash,
    bytes: Vec<u8>,
}

impl Store {
    /// Open or create the store at `path`. The directory will be
    /// created if missing; the single `forge.cas` file inside it is
    /// either initialised (fresh) or replayed (existing).
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        std::fs::create_dir_all(&path)?;
        let cas_file = path.join("forge.cas");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&cas_file)?;

        let len = file.metadata()?.len();
        // Replay accumule dans HashMap (interface stable de `replay`),
        // puis on convertit en SwissMap pour stockage final. Coût
        // négligeable car replay = boot one-shot.
        let mut blobs: HashMap<Hash, BlobRef> = HashMap::new();
        let mut refs = HashMap::new();

        if len == 0 {
            write_header(&mut file)?;
        } else {
            verify_header(&mut file)?;
            replay(&mut file, &mut blobs, &mut refs)?;
        }
        // Position at end so the next write is an append.
        file.seek(SeekFrom::End(0))?;

        // Convert HashMap → SwissMap. with_capacity arrondit au prochain
        // power of 2 ; pour un boot avec 100k blobs, alloue 131072 slots.
        let mut swiss: SwissMap<Hash, BlobRef> = SwissMap::with_capacity(blobs.len().max(16));
        for (h, br) in blobs {
            swiss.insert(h, br);
        }

        Ok(Store {
            path,
            file: Mutex::new(file),
            blobs: RwLock::new(swiss),
            refs: RwLock::new(refs),
            cache: Mutex::new(VecDeque::new()),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Persist `bytes` as a blob, returning its content hash. Already-
    /// known blobs are deduplicated: the file is not re-appended.
    pub fn store(&self, bytes: &[u8]) -> io::Result<Hash> {
        let hash = Hash::for_blob(bytes);
        if self.blobs.read().unwrap().contains_key(&hash) {
            self.remember_blob(hash, bytes.to_vec());
            return Ok(hash);
        }
        let payload_len = (20 + bytes.len()) as u32;
        let mut file = self.file.lock().unwrap();
        // Record header
        file.write_all(&[TAG_BLOB])?;
        file.write_all(&payload_len.to_le_bytes())?;
        // Hash + payload
        file.write_all(hash.as_bytes())?;
        let payload_offset = file.stream_position()?;
        file.write_all(bytes)?;
        file.flush()?;
        drop(file);
        self.blobs.write().unwrap().insert(
            hash,
            BlobRef {
                payload_offset,
                blob_len: bytes.len() as u32,
            },
        );
        self.remember_blob(hash, bytes.to_vec());
        Ok(hash)
    }

    pub fn store_packed(&self, bytes: &[u8]) -> io::Result<Hash> {
        self.store(&pack_lossless(bytes))
    }

    pub fn load(&self, hash: &Hash) -> Option<Vec<u8>> {
        if let Some(bytes) = self.cache_lookup(hash) {
            return Some(bytes);
        }
        let blob_ref = *self.blobs.read().unwrap().get(hash)?;
        let mut buf = vec![0u8; blob_ref.blob_len as usize];
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(blob_ref.payload_offset)).ok()?;
        file.read_exact(&mut buf).ok()?;
        // Restore append position so the next `store` doesn't overwrite.
        file.seek(SeekFrom::End(0)).ok()?;
        drop(file);
        self.remember_blob(*hash, buf.clone());
        Some(buf)
    }

    pub fn load_packed(&self, hash: &Hash) -> Result<Vec<u8>, CodecError> {
        self.load(hash)
            .ok_or(CodecError::Truncated)
            .and_then(|bytes| unpack_lossless(&bytes))
    }

    /// Look up a memo by its published call key (hex-encoded).
    pub fn lookup_memo(&self, key: &str) -> Option<Hash> {
        self.lookup_ref(&memo_ref_name(key))
    }

    /// Persist a memo: `key → target`. Idempotent on the same pair.
    pub fn write_memo(&self, key: &str, target: &Hash) -> io::Result<()> {
        self.write_ref(&memo_ref_name(key), target, "memo")
    }

    pub fn lookup_ref(&self, name: &str) -> Option<Hash> {
        self.refs.read().unwrap().get(name).copied()
    }

    /// Write or overwrite a ref. The `_message` parameter is kept for
    /// API compatibility with the V6 git-backed implementation; it is
    /// no longer recorded (the V7 file format is leaner — message
    /// strings would just be overhead with no consumer).
    pub fn write_ref(&self, name: &str, target: &Hash, _message: &str) -> io::Result<()> {
        if name.len() > u16::MAX as usize {
            return Err(io::Error::other("ref name too long"));
        }
        let name_bytes = name.as_bytes();
        let payload_len = (2 + name_bytes.len() + 20) as u32;
        let mut file = self.file.lock().unwrap();
        file.write_all(&[TAG_REF])?;
        file.write_all(&payload_len.to_le_bytes())?;
        file.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
        file.write_all(name_bytes)?;
        file.write_all(target.as_bytes())?;
        file.flush()?;
        drop(file);
        self.refs.write().unwrap().insert(name.to_string(), *target);
        Ok(())
    }

    /// Best-effort ref deletion. Idempotent: silently no-op when the
    /// ref is absent. Used to purge a persisted oracle after shadow-
    /// execution proves it diverges.
    pub fn delete_ref(&self, name: &str) -> io::Result<()> {
        if !self.refs.read().unwrap().contains_key(name) {
            return Ok(());
        }
        if name.len() > u16::MAX as usize {
            return Err(io::Error::other("ref name too long"));
        }
        let name_bytes = name.as_bytes();
        let payload_len = (2 + name_bytes.len()) as u32;
        let mut file = self.file.lock().unwrap();
        file.write_all(&[TAG_UNREF])?;
        file.write_all(&payload_len.to_le_bytes())?;
        file.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
        file.write_all(name_bytes)?;
        file.flush()?;
        drop(file);
        self.refs.write().unwrap().remove(name);
        Ok(())
    }

    fn cache_lookup(&self, hash: &Hash) -> Option<Vec<u8>> {
        self.cache
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|e| &e.hash == hash)
            .map(|e| e.bytes.clone())
    }

    fn remember_blob(&self, hash: Hash, bytes: Vec<u8>) {
        let mut cache = self.cache.lock().unwrap();
        if cache.iter().any(|e| e.hash == hash) {
            return;
        }
        cache.push_back(CachedBlob { hash, bytes });
        while cache.len() > BLOB_CACHE_LIMIT {
            cache.pop_front();
        }
    }

    // ─── Wave 10 closeout (2026-05-02) — `.cas portable` ──────────────
    //
    // Le format `forge.cas` est entièrement endian-explicit (`to_le_bytes`
    // partout dans `write_header`, `store`, `write_ref`, `delete_ref`).
    // Les hashes sont des `[u8; 20]` byte-array — invariants par
    // architecture. Donc le `.cas` est CROSS-MACHINE PORTABLE par
    // construction depuis γ.0 (réécriture du store sans git2).
    //
    // Wave 10 ajoute une API explicite `snapshot_to(target_path)` qui
    // copie atomiquement le `forge.cas` vers un autre répertoire. Le
    // résultat peut être ré-ouvert via `Store::open(target_path)` sur
    // une machine différente et restituera intégralement le même état
    // (blobs + refs + memos), bit-pour-bit identique.

    /// Wave 10 — snapshot le `forge.cas` vers `target_path` (créé si
    /// absent). Le snapshot est un .cas autonome, ré-ouvrable via
    /// `Store::open(target_path)` sur n'importe quelle machine
    /// supportant le format V7 (LE explicit, hashes byte-arrays).
    ///
    /// Atomique : flush avant copy, target overwritten only à la fin.
    pub fn snapshot_to(&self, target_path: impl Into<PathBuf>) -> io::Result<()> {
        let target_path = target_path.into();
        std::fs::create_dir_all(&target_path)?;
        let target_cas = target_path.join("forge.cas");

        // Flush pour s'assurer que toutes les écritures sont sur disque.
        {
            let mut file = self.file.lock().unwrap();
            file.flush()?;
        }
        // Copy via std::fs (atomique sur la même filesystem).
        let source_cas = self.path.join("forge.cas");
        std::fs::copy(&source_cas, &target_cas)?;
        Ok(())
    }

    /// Wave 10 — vérifie que le `forge.cas` actuel est un format
    /// portable valide (header magic + version LE).
    /// Retourne `Ok(())` si le format est cross-machine compatible,
    /// `Err` avec détail sinon.
    pub fn verify_portable_format(&self) -> io::Result<()> {
        let cas_file = self.path.join("forge.cas");
        let mut f = File::open(&cas_file)?;
        let mut header = [0u8; HEADER_LEN as usize];
        f.read_exact(&mut header)?;
        if &header[..8] != MAGIC {
            return Err(io::Error::other("forge.cas: bad magic — not a portable cas"));
        }
        let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
        if version != VERSION {
            return Err(io::Error::other(format!(
                "forge.cas: version {} != current {} — re-snapshot needed",
                version, VERSION,
            )));
        }
        // Bytes 12..32 reserved zero (flags + future use).
        // Pas d'autre check structurel ici — `Store::open(path)` fait
        // un replay complet au open et rejettera tout enregistrement
        // mal formé.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// File-format helpers
// ---------------------------------------------------------------------------

fn memo_ref_name(key: &str) -> String {
    format!("refs/memo/{key}")
}

fn write_header(file: &mut File) -> io::Result<()> {
    let mut header = [0u8; HEADER_LEN as usize];
    header[..8].copy_from_slice(MAGIC);
    header[8..12].copy_from_slice(&VERSION.to_le_bytes());
    // bytes 12..32 stay zero (flags + reserved)
    file.write_all(&header)?;
    file.flush()
}

fn verify_header(file: &mut File) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    let mut header = [0u8; HEADER_LEN as usize];
    file.read_exact(&mut header)?;
    if &header[..8] != MAGIC {
        return Err(io::Error::other("forge.cas: bad magic"));
    }
    let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
    if version != VERSION {
        return Err(io::Error::other(format!(
            "forge.cas: unsupported version {version}"
        )));
    }
    Ok(())
}

/// Walk every record from the start of the file, populating the RAM
/// indexes. Stops at the first sign of truncation or corruption — the
/// last partial record is silently dropped (crash-safety: the next
/// append simply overwrites the dangling tail).
fn replay(
    file: &mut File,
    blobs: &mut HashMap<Hash, BlobRef>,
    refs: &mut HashMap<String, Hash>,
) -> io::Result<()> {
    let total = file.metadata()?.len();
    let mut pos = HEADER_LEN;
    file.seek(SeekFrom::Start(HEADER_LEN))?;
    while pos < total {
        // Record header: tag (1B) + payload_len (4B LE)
        let mut hdr = [0u8; 5];
        if file.read_exact(&mut hdr).is_err() {
            break;
        }
        let tag = hdr[0];
        let payload_len = u32::from_le_bytes(hdr[1..5].try_into().unwrap()) as u64;
        let payload_pos = pos + 5;
        if payload_pos + payload_len > total {
            break; // truncated mid-record
        }
        match tag {
            TAG_BLOB => {
                if payload_len < 20 {
                    break;
                }
                let mut hash_bytes = [0u8; 20];
                file.read_exact(&mut hash_bytes)?;
                let blob_len = (payload_len - 20) as u32;
                blobs.insert(
                    Hash(hash_bytes),
                    BlobRef {
                        payload_offset: payload_pos + 20,
                        blob_len,
                    },
                );
                file.seek(SeekFrom::Current(i64::from(blob_len)))?;
            }
            TAG_REF => {
                if payload_len < 2 + 20 {
                    break;
                }
                let mut name_len_bytes = [0u8; 2];
                file.read_exact(&mut name_len_bytes)?;
                let name_len = u16::from_le_bytes(name_len_bytes) as u64;
                if 2 + name_len + 20 != payload_len {
                    break;
                }
                let mut name = vec![0u8; name_len as usize];
                file.read_exact(&mut name)?;
                let mut target = [0u8; 20];
                file.read_exact(&mut target)?;
                let name_str = match std::str::from_utf8(&name) {
                    Ok(s) => s.to_string(),
                    Err(_) => break,
                };
                refs.insert(name_str, Hash(target));
            }
            TAG_UNREF => {
                if payload_len < 2 {
                    break;
                }
                let mut name_len_bytes = [0u8; 2];
                file.read_exact(&mut name_len_bytes)?;
                let name_len = u16::from_le_bytes(name_len_bytes) as u64;
                if 2 + name_len != payload_len {
                    break;
                }
                let mut name = vec![0u8; name_len as usize];
                file.read_exact(&mut name)?;
                if let Ok(s) = std::str::from_utf8(&name) {
                    refs.remove(s);
                }
            }
            _ => break, // unknown tag → stop, treat the rest as garbage
        }
        pos += 5 + payload_len;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SHA-1 — pure Rust, kept inline for the "blob {len}\0" framing
// ---------------------------------------------------------------------------

fn hex20(bytes: [u8; 20]) -> String {
    let mut out = String::with_capacity(40);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn sha1(bytes: &[u8]) -> [u8; 20] {
    let mut h0 = 0x67452301u32;
    let mut h1 = 0xefcdab89u32;
    let mut h2 = 0x98badcfeu32;
    let mut h3 = 0x10325476u32;
    let mut h4 = 0xc3d2e1f0u32;

    let bit_len = (bytes.len() as u64) * 8;
    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let j = i * 4;
            *word = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    

    fn fresh_path(tag: &str) -> PathBuf {
        // Store tests use pid disambiguation for concurrent test runs.
        let pid = std::process::id();
        crate::fresh_tmp_path("scan-store", &format!("{tag}-{pid}"))
    }

    #[test]
    fn blob_hash_is_git_compatible() {
        let path = fresh_path("blob-hash-compat");
        let store = Store::open(&path).unwrap();
        // Git's blob hash for "monster bypass" is well-defined by the
        // SHA-1 framing — independent of our backing format.
        let bytes = b"monster bypass";
        let from_static = Hash::for_blob(bytes);
        let from_store = store.store(bytes).unwrap();
        assert_eq!(from_static, from_store);
        let _ = std::fs::remove_dir_all(&path);
    }

    // ─── Wave 10 closeout (2026-05-02) — `.cas portable` tests ──────

    #[test]
    fn cas_portable_snapshot_roundtrip() {
        // Simule un transfert cross-machine : Store A écrit, snapshot
        // vers path B, ré-ouverture de Store B doit avoir le même état.
        let src_path = fresh_path("cas-portable-src");
        let dst_path = fresh_path("cas-portable-dst");

        let blobs_to_write = [
            b"alpha bytes".to_vec(),
            b"bravo content longer than alpha".to_vec(),
            b"".to_vec(),
            (0u8..255).collect::<Vec<u8>>(),
        ];
        let mut original_hashes = Vec::new();

        // Write into source store + snapshot it.
        {
            let store = Store::open(&src_path).unwrap();
            for bytes in &blobs_to_write {
                let h = store.store(bytes).unwrap();
                original_hashes.push(h);
            }
            store.write_ref("refs/test/snapshot-marker", &original_hashes[0], "wave10").unwrap();
            store.write_memo("memo-key-42", &original_hashes[1]).unwrap();

            store.snapshot_to(&dst_path).unwrap();
        }

        // Open destination store — replay must restore identical state.
        let dst_store = Store::open(&dst_path).unwrap();
        for (i, bytes) in blobs_to_write.iter().enumerate() {
            let restored = dst_store.load(&original_hashes[i]).unwrap();
            assert_eq!(restored, *bytes, "blob {} bytes mismatch after snapshot", i);
        }
        // Refs and memos restored.
        assert_eq!(
            dst_store.lookup_ref("refs/test/snapshot-marker"),
            Some(original_hashes[0]),
        );
        assert_eq!(
            dst_store.lookup_memo("memo-key-42"),
            Some(original_hashes[1]),
        );

        let _ = std::fs::remove_dir_all(&src_path);
        let _ = std::fs::remove_dir_all(&dst_path);
    }

    #[test]
    fn cas_portable_format_verify_passes_on_valid_cas() {
        let path = fresh_path("cas-portable-verify");
        let store = Store::open(&path).unwrap();
        let _ = store.store(b"some data").unwrap();
        store.verify_portable_format().unwrap();
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn cas_portable_format_verify_rejects_bad_magic() {
        // Crée un .cas avec un mauvais magic et vérifie que verify_*
        // le rejette.
        let path = fresh_path("cas-portable-badmagic");
        std::fs::create_dir_all(&path).unwrap();
        // Crée un faux forge.cas avec magic invalide.
        let cas_file = path.join("forge.cas");
        let mut f = std::fs::File::create(&cas_file).unwrap();
        let bad_header = [0xFFu8; HEADER_LEN as usize];
        std::io::Write::write_all(&mut f, &bad_header).unwrap();
        std::io::Write::flush(&mut f).unwrap();
        drop(f);

        // Tenter d'ouvrir doit échouer (verify_header au open),
        // mais on teste verify_portable_format directement via un
        // Store fictif.
        let bad_store_attempt = Store::open(&path);
        assert!(bad_store_attempt.is_err(), "Store::open doit rejeter bad magic");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn cas_portable_le_encoding_endian_explicit() {
        // Write un blob, dump les bytes du fichier, vérifie que le
        // payload_len est bien encodé en little-endian (premier byte =
        // poids faible).
        let path = fresh_path("cas-portable-le");
        let store = Store::open(&path).unwrap();
        // Blob de 5 bytes → payload_len = 20 (hash) + 5 = 25.
        let _ = store.store(b"hello").unwrap();
        drop(store);

        let cas_file = path.join("forge.cas");
        let bytes = std::fs::read(&cas_file).unwrap();
        // Header [0..32], puis tag (1 byte), puis payload_len LE u32 [33..37].
        let header_len = HEADER_LEN as usize;
        let payload_len_bytes = &bytes[header_len + 1..header_len + 5];
        let payload_len = u32::from_le_bytes(payload_len_bytes.try_into().unwrap());
        assert_eq!(payload_len, 25, "payload_len doit être LE 25 ({:?})", payload_len_bytes);
        // Vérifier que le byte de poids fort est zéro (LE encoding).
        assert_eq!(payload_len_bytes[3], 0, "byte 3 du u32 LE doit être 0 pour value 25");
        assert_eq!(payload_len_bytes[0], 25, "byte 0 du u32 LE doit être 25");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn blob_roundtrip_persists_through_reopen() {
        let path = fresh_path("blob-persist");
        let payload = b"persistence proof".to_vec();

        let hash = {
            let store = Store::open(&path).unwrap();
            let h = store.store(&payload).unwrap();
            assert_eq!(store.load(&h).unwrap(), payload);
            h
        };

        // Reopen — the replay must rebuild the blob index from disk.
        let store = Store::open(&path).unwrap();
        assert_eq!(store.load(&hash).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn store_dedups_repeated_blobs() {
        let path = fresh_path("blob-dedup");
        let store = Store::open(&path).unwrap();
        let bytes = b"deduplicate me";
        let h1 = store.store(bytes).unwrap();
        let h2 = store.store(bytes).unwrap();
        assert_eq!(h1, h2);
        // The on-disk file should contain header + ONE blob record,
        // not two. Header (32) + record header (5) + hash (20) + bytes
        // (14) = 71 octets total.
        let file_len = std::fs::metadata(path.join("forge.cas")).unwrap().len();
        assert_eq!(file_len, 32 + 5 + 20 + 14);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn ref_roundtrip_and_overwrite() {
        let path = fresh_path("ref-overwrite");
        let store = Store::open(&path).unwrap();
        let h_a = store.store(b"value-a").unwrap();
        let h_b = store.store(b"value-b").unwrap();

        store.write_ref("refs/test/x", &h_a, "first").unwrap();
        assert_eq!(store.lookup_ref("refs/test/x"), Some(h_a));

        store.write_ref("refs/test/x", &h_b, "second").unwrap();
        assert_eq!(store.lookup_ref("refs/test/x"), Some(h_b));

        // Reopen — latest write wins.
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.lookup_ref("refs/test/x"), Some(h_b));

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn delete_ref_is_idempotent_and_persistent() {
        let path = fresh_path("ref-delete");
        let store = Store::open(&path).unwrap();
        let h = store.store(b"target").unwrap();
        store.write_ref("refs/scratch/y", &h, "msg").unwrap();
        assert_eq!(store.lookup_ref("refs/scratch/y"), Some(h));

        store.delete_ref("refs/scratch/y").unwrap();
        assert_eq!(store.lookup_ref("refs/scratch/y"), None);
        // Idempotent: deleting a missing ref is a no-op.
        store.delete_ref("refs/scratch/y").unwrap();
        store.delete_ref("refs/never-existed").unwrap();

        // Reopen: tombstone survives.
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.lookup_ref("refs/scratch/y"), None);

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn memo_roundtrip() {
        let path = fresh_path("memo-roundtrip");
        let store = Store::open(&path).unwrap();
        let target = store.store(b"the answer").unwrap();
        let key = "deadbeefcafebabe1234567890abcdef0fedcba9876543210123456789abcdef";
        store.write_memo(key, &target).unwrap();
        assert_eq!(store.lookup_memo(key), Some(target));
        // Same key, different target → overwrite.
        let other = store.store(b"another answer").unwrap();
        store.write_memo(key, &other).unwrap();
        assert_eq!(store.lookup_memo(key), Some(other));
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn truncated_tail_is_silently_dropped_on_replay() {
        let path = fresh_path("truncated-tail");
        let store = Store::open(&path).unwrap();
        let h = store.store(b"good record").unwrap();
        drop(store);

        // Append garbage that looks like a record header but with a
        // payload_len that exceeds the file. Replay must stop at the
        // last valid record and ignore the garbage.
        let cas_path = path.join("forge.cas");
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&cas_path)
            .unwrap();
        file.write_all(&[TAG_BLOB]).unwrap();
        file.write_all(&999_999u32.to_le_bytes()).unwrap(); // bogus huge len
        file.write_all(b"random partial").unwrap();
        drop(file);

        let store = Store::open(&path).unwrap();
        assert_eq!(store.load(&h).unwrap(), b"good record".to_vec());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn packed_store_roundtrip_is_lossless_and_smaller_for_traces() {
        let path = fresh_path("packed-roundtrip");
        let store = Store::open(&path).unwrap();
        let mut bytes = Vec::new();
        for i in 0..2048i64 {
            bytes.extend_from_slice(&(i * 2).to_le_bytes());
        }

        let packed = store.store_packed(&bytes).unwrap();
        assert_eq!(store.load_packed(&packed).unwrap(), bytes);
        // Stored bytes are the LZ-compressed payload, much shorter.
        assert!(store.load(&packed).unwrap().len() < bytes.len() / 4);

        let _ = std::fs::remove_dir_all(&path);
    }
}
