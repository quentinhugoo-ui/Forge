//! Atlas — phase A2 v1 (Atlas Cartographie ≤4 nœuds, INDEXÉ).
//!
//! Catalogue pré-calculé de programmes KASM canoniques. Construit
//! hors-ligne par `examples/atlas_a1.rs --build PATH`.
//!
//! ### Format binaire V1 (Φ.μ.7.11)
//!
//! ```
//! [magic 8B "ATLASV1\0"]
//! [count u32 LE]
//! [canonical_count u32 LE]
//! [canonical_inputs: canonical_count × i64 LE]
//! [entries: count × {
//!     [fp: 32B]
//!     [canonical_outputs: canonical_count × i64 LE]
//!     [prog_size: u16 LE]
//!     [prog_bytes: prog_size]
//! }]
//! ```
//!
//! ### Lookup
//!
//! - **O(1) hash lookup** quand les examples utilisateur matchent
//!   exactement les inputs canoniques (cas standard lab_runner).
//!   On hash le vecteur des outputs et on lookup dans une HashMap.
//! - **Fallback linear scan** sinon (compatibilité large).
//!
//! Mesure attendue : ~5-50 µs par lookup pour cas canonique
//! (vs ~33 ms en V0 linear scan, ×600-6000 speedup).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use sha2::{Digest, Sha256};

use crate::kasm::{execute, Program};

const MAGIC_V1: &[u8; 8] = b"ATLASV1\0";
const MAGIC_V2: &[u8; 8] = b"ATLASV2\0";

/// Φ.ν.7 — LiveAtlas persistent ATLASV2 file path. Single source of
/// truth for the runtime-growing atlas (replaces legacy `hot-atlas.bin`).
pub const LIVE_ATLAS_PATH: &str = ".codex-tmp/atlas-live.bin";

/// Φ.ν.7 — Legacy `hot-atlas.bin` path kept for one-shot migration into
/// ATLASV2. Read-only after migration; never written, never truncated.
pub const LEGACY_HOT_ATLAS_PATH: &str = ".codex-tmp/hot-atlas.bin";

/// Φ.μ.7.13 — `trait AtlasIngest` pour coordination Track A (Atlas
/// Cartographie) ↔ Track B (`MonsterNode::self_improve`, Φ.ν).
///
/// La session Φ.ν construit un cœur cognitif unifié (oracle + dispatch
/// + synthesis) dans `MonsterNode`. Quand `self_improve` découvre un
/// nouveau programme exact (train + holdout), elle peut l'ingérer dans
/// l'atlas via cette interface — sans coupler le module `monster::atlas`
/// au module `monster::self_improve`.
///
/// **L'atlas V1 actuel est read-only** (immutable on disk). Une future
/// implémentation `LiveAtlas` (V2+) pourrait :
/// 1. Maintenir un overlay d'entries en RAM en plus du backing file
/// 2. Implémenter `AtlasIngest::submit` pour grow runtime
/// 3. Periodically flush vers disque (snapshot lookup-stable)
///
/// Pour Φ.ν.3 : la session passe `Arc<dyn AtlasIngest>` à
/// `self_improve`. Si pas d'implémentation fournie (atlas read-only),
/// le `submit` est un no-op silencieux — pas d'erreur, juste pas
/// d'ingestion.
pub trait AtlasIngest: Send + Sync {
    /// Soumet un nouveau programme cartographique.
    /// `fingerprint` : SHA-256 des outputs canoniques sémantiques (32 B).
    /// `canonical_outputs` : outputs sur les `ATLAS_CANONICAL_INPUTS`
    ///                       (12 i64) pour permettre l'index hash O(1).
    /// `program` : programme canonique (bytes minimum-size form).
    ///
    /// Retourne `true` si l'entrée a été ingérée (nouvelle classe),
    /// `false` si déjà connue ou rejetée.
    fn submit(
        &self,
        fingerprint: [u8; 32],
        canonical_outputs: &[i64],
        program: &Program,
    ) -> bool;
}

/// Inputs canoniques pour l'index. **Doivent matcher
/// `lab_runner::build_diverse_inputs`** : sinon les lookups O(1)
/// échouent et on retombe sur linear scan.
pub const ATLAS_CANONICAL_INPUTS: [i64; 12] = [
    -7, -1, 1, 11, -100, 100, -987, 987, -12345, -50000, 12345, 50000,
];

/// Une entry stocke le fingerprint sémantique, le programme canonique,
/// et les outputs pré-calculés sur les inputs canoniques (pour le hash
/// lookup O(1)).
struct Entry {
    program_bytes: Vec<u8>,
}

/// Catalogue indexé par hash du vecteur d'outputs canoniques.
pub struct Atlas {
    entries: Vec<Entry>,
    canonical_inputs: Vec<i64>,
    /// Index hash : `outputs vector -> liste d'entry indices`. Plusieurs
    /// programmes peuvent partager les mêmes outputs canoniques (rare
    /// car semantic_fingerprint dédupe déjà à 16 inputs canoniques —
    /// mais ces 16 ≠ nos 12 lab inputs, donc collisions possibles).
    by_outputs: HashMap<Vec<i64>, Vec<usize>>,
}

impl Atlas {
    /// Charge l'atlas V1 depuis un fichier binaire.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        if bytes.len() < 16 || &bytes[..8] != MAGIC_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad atlas magic (expected ATLASV1)",
            ));
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let canonical_count =
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;

        let mut cursor = 16;
        if cursor + canonical_count * 8 > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated canonical inputs header",
            ));
        }
        let mut canonical_inputs = Vec::with_capacity(canonical_count);
        for _ in 0..canonical_count {
            let v = i64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            canonical_inputs.push(v);
            cursor += 8;
        }

        let mut entries = Vec::with_capacity(count);
        let mut by_outputs: HashMap<Vec<i64>, Vec<usize>> = HashMap::with_capacity(count);

        for i in 0..count {
            if cursor + 32 + canonical_count * 8 + 2 > bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated atlas entry",
                ));
            }
            let mut _fp = [0u8; 32];
            _fp.copy_from_slice(&bytes[cursor..cursor + 32]);
            cursor += 32;
            let mut canonical_outputs = Vec::with_capacity(canonical_count);
            for _ in 0..canonical_count {
                let v = i64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
                canonical_outputs.push(v);
                cursor += 8;
            }
            let prog_size =
                u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
            cursor += 2;
            if cursor + prog_size > bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated atlas program",
                ));
            }
            let program_bytes = bytes[cursor..cursor + prog_size].to_vec();
            cursor += prog_size;

            by_outputs
                .entry(canonical_outputs.clone())
                .or_default()
                .push(i);

            entries.push(Entry { program_bytes });
        }

        Ok(Self {
            entries,
            canonical_inputs,
            by_outputs,
        })
    }

    /// Écrit l'atlas V1. Trie par fingerprint avant écriture.
    /// `entries` : vec de `(fp_32B, canonical_outputs, program_bytes)`.
    /// `canonical_inputs` : les inputs utilisés pour calculer les outputs.
    pub fn write(
        path: impl AsRef<Path>,
        canonical_inputs: &[i64],
        mut entries: Vec<(Vec<u8>, Vec<i64>, Vec<u8>)>,
    ) -> io::Result<()> {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut file = fs::File::create(path)?;
        file.write_all(MAGIC_V1)?;
        file.write_all(&(entries.len() as u32).to_le_bytes())?;
        file.write_all(&(canonical_inputs.len() as u32).to_le_bytes())?;
        for &input in canonical_inputs {
            file.write_all(&input.to_le_bytes())?;
        }
        for (fp, outputs, prog) in &entries {
            if fp.len() != 32 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "fingerprint must be 32 bytes",
                ));
            }
            if outputs.len() != canonical_inputs.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "canonical_outputs length must match canonical_inputs",
                ));
            }
            if prog.len() > u16::MAX as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "program too large for u16 size field",
                ));
            }
            file.write_all(fp)?;
            for &out in outputs {
                file.write_all(&out.to_le_bytes())?;
            }
            file.write_all(&(prog.len() as u16).to_le_bytes())?;
            file.write_all(prog)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn canonical_inputs(&self) -> &[i64] {
        &self.canonical_inputs
    }

    /// Cherche un programme qui matche TOUS les examples utilisateur.
    ///
    /// Stratégie :
    /// 1. **Fast path O(1)** : si les examples sont alignés sur les
    ///    inputs canoniques (même séquence, même ordre — typique
    ///    `lab_runner`), hash lookup direct. **Pas de fallback linear
    ///    scan** : si l'atlas ne contient pas la classe sémantique,
    ///    inutile de scanner — par construction c'est définitif.
    /// 2. **Slow path O(N)** : SEULEMENT si les inputs ne matchent
    ///    pas le préfixe canonique (cas non-lab), linear scan avec
    ///    short-circuit.
    pub fn find_for_examples(&self, examples: &[(i64, i64)]) -> Option<Program> {
        if examples.is_empty() {
            return None;
        }

        if self.examples_align_canonical(examples) {
            // Hash lookup ONLY — no linear fallback.
            // L'atlas est exhaustif sur ≤4 nœuds : si la classe n'y
            // est pas, scan linéaire ne la trouvera pas non plus
            // (ce sont les MÊMES entries via path différent).
            return self.lookup_canonical_aligned(examples);
        }

        // Cas non-aligné : linear scan fallback.
        self.linear_scan(examples)
    }

    /// Test : les `canonical_inputs.len()` premiers examples ont les
    /// inputs canoniques dans le bon ordre.
    fn examples_align_canonical(&self, examples: &[(i64, i64)]) -> bool {
        if examples.len() < self.canonical_inputs.len() {
            return false;
        }
        for (i, &expected_input) in self.canonical_inputs.iter().enumerate() {
            if examples[i].0 != expected_input {
                return false;
            }
        }
        true
    }

    /// Suppose `examples_align_canonical(examples) == true`. Hash
    /// lookup direct via le HashMap d'index.
    fn lookup_canonical_aligned(&self, examples: &[(i64, i64)]) -> Option<Program> {
        let user_outputs: Vec<i64> = examples
            .iter()
            .take(self.canonical_inputs.len())
            .map(|(_, y)| *y)
            .collect();
        let candidates = self.by_outputs.get(&user_outputs)?;
        for &idx in candidates {
            let entry = &self.entries[idx];
            let prog = match Program::from_bytes(&entry.program_bytes) {
                Ok(p) => p,
                Err(_) => continue,
            };
            // Si user fournit > canonical inputs, vérifier les supplémentaires.
            if examples.len() > self.canonical_inputs.len() {
                if !program_matches_examples(&prog, &examples[self.canonical_inputs.len()..]) {
                    continue;
                }
            }
            return Some(prog);
        }
        None
    }

    fn linear_scan(&self, examples: &[(i64, i64)]) -> Option<Program> {
        for entry in &self.entries {
            let prog = match Program::from_bytes(&entry.program_bytes) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if program_matches_examples(&prog, examples) {
                return Some(prog);
            }
        }
        None
    }

    /// Inspectable pour tests : retourne le nombre de buckets dans l'index.
    #[doc(hidden)]
    pub fn index_buckets(&self) -> usize {
        self.by_outputs.len()
    }
}

fn program_matches_examples(prog: &Program, examples: &[(i64, i64)]) -> bool {
    for &(input, want) in examples {
        let bytes = match execute(prog, &input.to_le_bytes()) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let got = match bytes
            .get(..8)
            .and_then(|c| c.try_into().ok())
            .map(i64::from_le_bytes)
        {
            Some(v) => v,
            None => return false,
        };
        if got != want {
            return false;
        }
    }
    true
}

// ─────────────────────────────────────────────────────────────────────────
// Φ.ν.7 — LiveAtlas (ATLASV2 indexed, prog_bytes externalized via forge.cas)
// ─────────────────────────────────────────────────────────────────────────
//
// Doctrine fusion :
//   • HotAtlas (RAM cache `HashMap<u64, Program>`)            — fast lookup
//   • AtlasIngest channel (was `NoopAtlasIngest`)             — dedup persist
//   • `hot-atlas.bin` custom binary format                    — 250 B/entry
// → all replaced by **one** `LiveAtlas` keyed dually:
//   • `fnv64(canonical_outputs)` for the hot path             — preserved
//   • `SHA-256(canonical_outputs)` 32B for dedup + persist    — Trust DDC
// → on-disk format ATLASV2: `(fp32, 12×i64 outputs, prog_hash 20B)`,
//   ~148 B/entry, prog bytes externalized via `Store::store()`.
//
// Design choices :
//   • Trusting Trust DDC — append-only journal anchored in fp32 hash
//   • Hash collision = canonicalisation gratuite
//   • Behavioural fingerprint > structural
//   • forge.cas as cryptographic audit substrate
//   • Erlang let-it-crash — submit() returns bool, flush silent on Drop

/// FNV-1a 64-bit hash. Preserved bit-stable from the legacy
/// `lab::fnv64` to keep migrated `hot-atlas.bin` keys consistent.
pub(crate) fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// FNV-64 over the LE-packed canonical outputs vector (12×i64 = 96 bytes).
/// **Hot-path key** — `target_fingerprint(target) == fnv64_outputs(target_outputs)`
/// when `target_outputs == canonical_outputs(prog_with_exact_holdout)`.
pub fn fnv64_outputs(outputs: &[i64]) -> u64 {
    let mut buf = [0u8; 96];
    for (i, &y) in outputs.iter().take(12).enumerate() {
        buf[i * 8..i * 8 + 8].copy_from_slice(&y.to_le_bytes());
    }
    fnv64(&buf)
}

/// Execute `program` on `ATLAS_CANONICAL_INPUTS` and return the 12 outputs.
/// `None` if any execution fails or output isn't a valid i64.
pub fn canonical_outputs(program: &Program) -> Option<Vec<i64>> {
    ATLAS_CANONICAL_INPUTS
        .iter()
        .map(|&x| {
            let bytes = execute(program, &x.to_le_bytes()).ok()?;
            let head: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
            Some(i64::from_le_bytes(head))
        })
        .collect()
}

/// SHA-256 32B fingerprint of canonical outputs. Persistent dedup key
/// for `LiveAtlas` and the `AtlasIngest` channel. Domain-separated by
/// the `forge-self-improve-output-fp-v1\0` prefix to prevent cross-context
/// collisions.
pub fn output_fingerprint(outputs: &[i64]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"forge-self-improve-output-fp-v1\0");
    for y in outputs {
        h.update(y.to_le_bytes());
    }
    h.finalize().into()
}

/// Φ.ν.7.b — forge.cas externalization **amputated** (Via Negativa +
/// experimental amputation). Prior design stored `prog_hash: [u8;20]` per entry
/// and relied on `Store::store/load` for program bytes — measured -39 %
/// iter/sec regression vs the legacy HotAtlas baseline (666 → 403). Root
/// cause: `Mutex<File>` contention across 6 threads + lazy-load forge.cas
/// reads on first lookup. The audit benefit was hypothetical, the perf
/// cost was real. Mapping Liste A → A1 Stockfish NNUE: weights live with
/// the engine, not externalized. Forge atlas same.
///
/// Each entry now holds the program directly via `Arc<Program>` in RAM,
/// serialized inline in ATLASV2 at flush. One file, one abstraction (A6
/// Plan 9 everything-is-a-file).
#[derive(Clone)]
struct LiveEntry {
    fp32: [u8; 32],
    canonical_outputs: Vec<i64>,
    program: Arc<Program>,
}

#[derive(Default)]
pub struct LiveAtlasCounters {
    pub loaded: AtomicU64,
    pub submitted: AtomicU64,
    pub accepted: AtomicU64,
    pub dedup_rejects: AtomicU64,
    pub hits_hot: AtomicU64,
    pub migrated: AtomicU64,
}

pub struct LiveAtlas {
    canonical_inputs: Vec<i64>,
    flush_path: Option<PathBuf>,
    state: RwLock<LiveAtlasInner>,
    pub counters: LiveAtlasCounters,
}

struct LiveAtlasInner {
    entries: Vec<LiveEntry>,
    by_fp32: HashSet<[u8; 32]>,
    /// fnv64(outs) → entry index. The single hot-path lookup index. The
    /// program is `entries[idx].program` (Arc<Program>) — RAM-resident
    /// since Φ.ν.7.b amputated forge.cas externalization. No lazy loads,
    /// no cold cache : matches the baseline HotAtlas semantics directly.
    by_fnv64: HashMap<u64, usize>,
    dirty: bool,
}

impl LiveAtlas {
    /// Open (or create) the LiveAtlas backed by `flush_path` (ATLASV2).
    /// Performs one-shot migration from `LEGACY_HOT_ATLAS_PATH` if the
    /// V2 file doesn't exist yet — preserves the 166 K + flywheel state
    /// accumulated across the Φ.μ phases.
    pub fn open(flush_path: impl Into<PathBuf>) -> io::Result<Self> {
        let flush_path = flush_path.into();
        let counters = LiveAtlasCounters::default();

        // One-shot migration from legacy `hot-atlas.bin` (Φ.μ.7.11). Only
        // triggers when `flush_path` is the canonical production path —
        // test paths and other ad-hoc paths skip migration so they don't
        // get polluted by a stale legacy file in CWD.
        let is_canonical = flush_path == PathBuf::from(LIVE_ATLAS_PATH);
        let legacy = PathBuf::from(LEGACY_HOT_ATLAS_PATH);
        if is_canonical && !flush_path.exists() && legacy.exists() {
            let n = migrate_legacy_hot_atlas(&legacy, &flush_path)?;
            counters.migrated.store(n as u64, Ordering::Relaxed);
        }

        let inner = if flush_path.exists() {
            Self::load_v2(&flush_path)?
        } else {
            LiveAtlasInner {
                entries: Vec::new(),
                by_fp32: HashSet::new(),
                by_fnv64: HashMap::new(),
                dirty: false,
            }
        };
        counters
            .loaded
            .store(inner.entries.len() as u64, Ordering::Relaxed);

        Ok(Self {
            canonical_inputs: ATLAS_CANONICAL_INPUTS.to_vec(),
            flush_path: Some(flush_path),
            state: RwLock::new(inner),
            counters,
        })
    }

    /// Transient (no flush, no persist) — used in self_improve when no
    /// disk path is available, or in tests.
    pub fn transient() -> Self {
        Self {
            canonical_inputs: ATLAS_CANONICAL_INPUTS.to_vec(),
            flush_path: None,
            state: RwLock::new(LiveAtlasInner {
                entries: Vec::new(),
                by_fp32: HashSet::new(),
                by_fnv64: HashMap::new(),
                dirty: false,
            }),
            counters: LiveAtlasCounters::default(),
        }
    }

    fn load_v2(path: &Path) -> io::Result<LiveAtlasInner> {
        let bytes = fs::read(path)?;
        if bytes.len() < 16 || &bytes[..8] != MAGIC_V2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad atlas magic (expected ATLASV2)",
            ));
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let canonical_count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if canonical_count != ATLAS_CANONICAL_INPUTS.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ATLASV2 canonical_count mismatch",
            ));
        }
        let mut cur = 16;
        if cur + canonical_count * 8 > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated canonical inputs",
            ));
        }
        // Skip canonical inputs header (we use ATLAS_CANONICAL_INPUTS as authority).
        cur += canonical_count * 8;

        let mut entries: Vec<LiveEntry> = Vec::with_capacity(count);
        let mut by_fp32: HashSet<[u8; 32]> = HashSet::with_capacity(count);
        let mut by_fnv64: HashMap<u64, usize> = HashMap::with_capacity(count);

        for i in 0..count {
            if cur + 32 + canonical_count * 8 + 2 > bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated atlas entry header",
                ));
            }
            let mut fp32 = [0u8; 32];
            fp32.copy_from_slice(&bytes[cur..cur + 32]);
            cur += 32;
            let mut outs = Vec::with_capacity(canonical_count);
            for _ in 0..canonical_count {
                outs.push(i64::from_le_bytes(bytes[cur..cur + 8].try_into().unwrap()));
                cur += 8;
            }
            let prog_size =
                u16::from_le_bytes(bytes[cur..cur + 2].try_into().unwrap()) as usize;
            cur += 2;
            if cur + prog_size > bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated atlas program bytes",
                ));
            }
            let prog_bytes = &bytes[cur..cur + prog_size];
            cur += prog_size;
            // Skip malformed entries silently (E6 let-it-crash).
            let prog = match Program::from_bytes(prog_bytes) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let fnv_key = fnv64_outputs(&outs);
            by_fp32.insert(fp32);
            by_fnv64.entry(fnv_key).or_insert(i);
            entries.push(LiveEntry {
                fp32,
                canonical_outputs: outs,
                program: Arc::new(prog),
            });
        }

        Ok(LiveAtlasInner {
            entries,
            by_fp32,
            by_fnv64,
            dirty: false,
        })
    }

    /// Submit a new program. Returns `true` if it's a new fp32 class
    /// (accepted), `false` if dedup-rejected or invalid input.
    ///
    /// Submit a new program. Returns `true` if it's a new fp32 class
    /// (accepted), `false` if dedup-rejected or invalid input.
    /// Φ.ν.7.b — RAM-only insert; flush() serializes to atlas-live.bin
    /// inline (no forge.cas dependency, no Mutex<File> contention).
    pub fn submit(&self, fp32: [u8; 32], outputs: &[i64], program: &Program) -> bool {
        self.counters.submitted.fetch_add(1, Ordering::Relaxed);
        if outputs.len() != self.canonical_inputs.len() {
            return false;
        }
        // Fast path: dedup check under read lock.
        if self.state.read().unwrap().by_fp32.contains(&fp32) {
            self.counters.dedup_rejects.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let fnv_key = fnv64_outputs(outputs);
        let arc_prog = Arc::new(program.clone());

        let mut st = self.state.write().unwrap();
        if !st.by_fp32.insert(fp32) {
            // Race: another thread submitted between read-lock and write-lock.
            self.counters.dedup_rejects.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let idx = st.entries.len();
        st.entries.push(LiveEntry {
            fp32,
            canonical_outputs: outputs.to_vec(),
            program: arc_prog,
        });
        st.by_fnv64.entry(fnv_key).or_insert(idx);
        st.dirty = true;
        self.counters.accepted.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Φ.1 — Bulk submit. Same per-entry semantics as
    /// [`LiveAtlas::submit`] but acquires the write lock **once** for
    /// the entire batch. Critical for the `dispatch_batch` ingest path:
    /// hundreds of `Computed` results can be promoted into the atlas in
    /// a single lock window instead of N round-trips.
    ///
    /// `entries` is consumed by reference (each `(fp32, outputs, program)`
    /// tuple). Returns the count of newly-accepted classes; entries with
    /// duplicate fingerprints or wrong-length outputs are silently
    /// rejected (mirrors `submit`'s contract).
    pub fn submit_batch(
        &self,
        entries: &[([u8; 32], Vec<i64>, Program)],
    ) -> usize {
        if entries.is_empty() {
            return 0;
        }
        self.counters
            .submitted
            .fetch_add(entries.len() as u64, Ordering::Relaxed);

        let canonical_len = self.canonical_inputs.len();
        let mut accepted = 0usize;
        let mut rejected = 0u64;
        let mut st = self.state.write().unwrap();
        for (fp32, outputs, program) in entries {
            if outputs.len() != canonical_len {
                rejected += 1;
                continue;
            }
            if !st.by_fp32.insert(*fp32) {
                rejected += 1;
                continue;
            }
            let fnv_key = fnv64_outputs(outputs);
            let idx = st.entries.len();
            st.entries.push(LiveEntry {
                fp32: *fp32,
                canonical_outputs: outputs.clone(),
                program: Arc::new(program.clone()),
            });
            st.by_fnv64.entry(fnv_key).or_insert(idx);
            accepted += 1;
        }
        if accepted > 0 {
            st.dirty = true;
            self.counters
                .accepted
                .fetch_add(accepted as u64, Ordering::Relaxed);
        }
        if rejected > 0 {
            self.counters
                .dedup_rejects
                .fetch_add(rejected, Ordering::Relaxed);
        }
        accepted
    }

    /// Hot-path lookup keyed by `fnv64(canonical_outputs)`. RAM-only,
    /// O(1) HashMap + Arc clone. No I/O, no allocation beyond the Arc bump.
    pub fn lookup_hot(&self, fnv_key: u64) -> Option<Arc<Program>> {
        let st = self.state.read().unwrap();
        let idx = *st.by_fnv64.get(&fnv_key)?;
        let arc = Arc::clone(&st.entries[idx].program);
        drop(st);
        self.counters.hits_hot.fetch_add(1, Ordering::Relaxed);
        Some(arc)
    }

    /// Persist current state to `flush_path` (ATLASV2 inline). No-op if
    /// transient or not dirty.
    pub fn flush(&self) -> io::Result<()> {
        let path = match &self.flush_path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };
        let entries_snapshot = {
            let mut st = self.state.write().unwrap();
            if !st.dirty {
                return Ok(());
            }
            st.dirty = false;
            st.entries.clone()
        };
        write_atlas_v2(&path, &self.canonical_inputs, &entries_snapshot)
    }

    pub fn len(&self) -> usize {
        self.state.read().unwrap().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.state.read().unwrap().entries.is_empty()
    }

    /// Φ.ν.7c — read-only walk over (program, canonical_outputs) for offline
    /// dendritic analysis (ramée extraction + sève comparison). The closure
    /// runs under a read lock; keep it cheap.
    pub fn for_each_entry<F: FnMut(&Program, &[i64])>(&self, mut f: F) {
        let st = self.state.read().unwrap();
        for entry in &st.entries {
            f(&entry.program, &entry.canonical_outputs);
        }
    }
}

impl Drop for LiveAtlas {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl AtlasIngest for LiveAtlas {
    fn submit(&self, fp: [u8; 32], outs: &[i64], prog: &Program) -> bool {
        LiveAtlas::submit(self, fp, outs, prog)
    }
}

fn write_atlas_v2(path: &Path, canonical_inputs: &[i64], entries: &[LiveEntry]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    // Pre-size for ~250 B avg per program inline; over-allocation is cheap.
    let header = 16 + canonical_inputs.len() * 8;
    let est_entry = 32 + canonical_inputs.len() * 8 + 2 + 256;
    let mut buf: Vec<u8> = Vec::with_capacity(header + entries.len() * est_entry);
    buf.extend_from_slice(MAGIC_V2);
    buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    buf.extend_from_slice(&(canonical_inputs.len() as u32).to_le_bytes());
    for &x in canonical_inputs {
        buf.extend_from_slice(&x.to_le_bytes());
    }
    let mut sorted: Vec<&LiveEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.fp32.cmp(&b.fp32));
    for e in sorted {
        let prog_bytes = e.program.bytes();
        if prog_bytes.len() > u16::MAX as usize {
            // Defensive: KASM programs ≤ 4096 nodes, well under u16 max.
            continue;
        }
        buf.extend_from_slice(&e.fp32);
        for &y in &e.canonical_outputs {
            buf.extend_from_slice(&y.to_le_bytes());
        }
        buf.extend_from_slice(&(prog_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(prog_bytes);
    }
    fs::write(path, &buf)
}

/// One-shot migration from the legacy `hot-atlas.bin` custom format
/// (`[count u32][fp_u64 u64][len u32][prog_bytes]...`) into ATLASV2 inline.
/// Each program is re-executed on `ATLAS_CANONICAL_INPUTS` to derive
/// `fp32`. Programs that fail to execute are dropped silently. The legacy
/// file is **left untouched** (rollback safety).
pub fn migrate_legacy_hot_atlas(
    hot_path: &Path,
    atlas_v2_path: &Path,
) -> io::Result<usize> {
    let data = fs::read(hot_path)?;
    if data.len() < 4 {
        return Ok(0);
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut pos = 4usize;
    let mut entries: Vec<LiveEntry> = Vec::with_capacity(count);
    let mut seen: HashSet<[u8; 32]> = HashSet::with_capacity(count);
    for _ in 0..count {
        if pos + 12 > data.len() {
            break;
        }
        // Skip legacy fp_u64 (8 B); fnv64 will be recomputed from outputs.
        pos += 8;
        let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if pos + len > data.len() {
            break;
        }
        let prog_bytes = &data[pos..pos + len];
        pos += len;
        let prog = match Program::from_bytes(prog_bytes) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let outs = match canonical_outputs(&prog) {
            Some(o) => o,
            None => continue,
        };
        let fp32 = output_fingerprint(&outs);
        if !seen.insert(fp32) {
            continue;
        }
        entries.push(LiveEntry {
            fp32,
            canonical_outputs: outs,
            program: Arc::new(prog),
        });
    }
    write_atlas_v2(atlas_v2_path, &ATLAS_CANONICAL_INPUTS, &entries)?;
    Ok(entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Target, Ty};
    use std::path::PathBuf;
    

    fn fresh_path(tag: &str) -> PathBuf {
        // Atlas is a single .bin file, not a directory.
        let mut p = crate::fresh_tmp_path("atlas-test", tag);
        let stem = p.file_name().unwrap().to_str().unwrap().to_string();
        p.set_file_name(format!("{stem}.bin"));
        p
    }

    fn affine_program() -> Program {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(7),
            Node::mul(0, 1),
            Node::const_i64(3),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ];
        Program::new(Target::Cpu, 1, 1, 1024, nodes).unwrap()
    }

    fn affine_canonical_outputs() -> Vec<i64> {
        // y = 7x + 3 sur ATLAS_CANONICAL_INPUTS
        ATLAS_CANONICAL_INPUTS.iter().map(|x| x.wrapping_mul(7).wrapping_add(3)).collect()
    }

    #[test]
    fn atlas_v1_roundtrip() {
        let prog = affine_program();
        let outputs = affine_canonical_outputs();
        let path = fresh_path("v1-roundtrip");
        Atlas::write(
            &path,
            &ATLAS_CANONICAL_INPUTS,
            vec![(vec![0x42u8; 32], outputs.clone(), prog.bytes().to_vec())],
        )
        .unwrap();
        let atlas = Atlas::open(&path).unwrap();
        assert_eq!(atlas.len(), 1);
        assert_eq!(atlas.canonical_inputs(), &ATLAS_CANONICAL_INPUTS[..]);
        assert_eq!(atlas.index_buckets(), 1);
    }

    #[test]
    fn atlas_v1_o1_canonical_lookup() {
        let prog = affine_program(); // y = 7x + 3
        let outputs = affine_canonical_outputs();
        let path = fresh_path("v1-o1");
        Atlas::write(
            &path,
            &ATLAS_CANONICAL_INPUTS,
            vec![(vec![0u8; 32], outputs, prog.bytes().to_vec())],
        )
        .unwrap();
        let atlas = Atlas::open(&path).unwrap();

        // Cas canonique : examples == canonical_inputs en ordre
        let examples: Vec<(i64, i64)> = ATLAS_CANONICAL_INPUTS
            .iter()
            .map(|x| (*x, x.wrapping_mul(7).wrapping_add(3)))
            .collect();
        let found = atlas.find_for_examples(&examples);
        assert!(found.is_some(), "O(1) canonical lookup devrait matcher");
    }

    #[test]
    fn atlas_v1_falls_back_to_linear_scan() {
        let prog = affine_program(); // y = 7x + 3
        let outputs = affine_canonical_outputs();
        let path = fresh_path("v1-linear");
        Atlas::write(
            &path,
            &ATLAS_CANONICAL_INPUTS,
            vec![(vec![0u8; 32], outputs, prog.bytes().to_vec())],
        )
        .unwrap();
        let atlas = Atlas::open(&path).unwrap();

        // Examples NON-canoniques (ordre/inputs différents)
        let examples = (0..5i64).map(|x| (x, x.wrapping_mul(7) + 3)).collect::<Vec<_>>();
        let found = atlas.find_for_examples(&examples);
        assert!(found.is_some(), "linear scan fallback devrait matcher");
    }

    #[test]
    fn atlas_v1_misses_when_no_match() {
        let prog = affine_program(); // y = 7x + 3
        let outputs = affine_canonical_outputs();
        let path = fresh_path("v1-miss");
        Atlas::write(
            &path,
            &ATLAS_CANONICAL_INPUTS,
            vec![(vec![0u8; 32], outputs, prog.bytes().to_vec())],
        )
        .unwrap();
        let atlas = Atlas::open(&path).unwrap();

        // y = 2x + 1, ne matche pas
        let examples = (0..5i64).map(|x| (x, x.wrapping_mul(2) + 1)).collect::<Vec<_>>();
        let found = atlas.find_for_examples(&examples);
        assert!(found.is_none());
    }

    #[test]
    fn atlas_v1_rejects_v0_or_bad_magic() {
        let path = fresh_path("v1-badmagic");
        fs::write(&path, b"ATLASV0\0").unwrap();
        assert!(Atlas::open(&path).is_err(), "V0 atlas n'est plus accepté");
    }

    #[test]
    fn live_atlas_submit_dedup_and_lookup_hot() {
        let live = LiveAtlas::transient();
        let prog = affine_program();
        let outs = canonical_outputs(&prog).unwrap();
        let fp32 = output_fingerprint(&outs);
        let fnv = fnv64_outputs(&outs);

        assert!(live.submit(fp32, &outs, &prog), "first submit accepted");
        assert!(!live.submit(fp32, &outs, &prog), "second submit dedup");
        assert_eq!(live.len(), 1);

        let hit = live.lookup_hot(fnv).expect("hot path lookup hit");
        assert_eq!(hit.bytes(), prog.bytes());
        assert_eq!(live.counters.hits_hot.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn live_atlas_persist_and_reload_v2() {
        let atlas_path = fresh_path("v2-persist");
        let prog = affine_program();
        let outs = canonical_outputs(&prog).unwrap();
        let fp32 = output_fingerprint(&outs);
        let fnv = fnv64_outputs(&outs);

        // First instance: submit + drop → flush.
        {
            let live = LiveAtlas::open(&atlas_path).unwrap();
            assert_eq!(live.len(), 0);
            assert!(live.submit(fp32, &outs, &prog));
        } // Drop flushes.

        assert!(atlas_path.exists(), "ATLASV2 file written on Drop");

        // Second instance: reload from disk. Φ.ν.7.b — programs read inline,
        // RAM-resident from boot (no forge.cas dependency).
        {
            let live = LiveAtlas::open(&atlas_path).unwrap();
            assert_eq!(live.len(), 1);
            assert_eq!(live.counters.loaded.load(Ordering::Relaxed), 1);
            let hit = live.lookup_hot(fnv).expect("hot path lookup hit");
            assert_eq!(hit.bytes(), prog.bytes());
            assert_eq!(live.counters.hits_hot.load(Ordering::Relaxed), 1);
        }
    }

    #[test]
    fn live_atlas_rejects_bad_canonical_count() {
        let live = LiveAtlas::transient();
        let prog = affine_program();
        let bad_outs = vec![0i64; 5]; // wrong length
        let fp32 = output_fingerprint(&bad_outs);
        assert!(!live.submit(fp32, &bad_outs, &prog));
        assert_eq!(live.len(), 0);
    }

    #[test]
    fn live_atlas_v2_rejects_bad_magic() {
        let atlas_path = fresh_path("v2-bad-magic");
        fs::create_dir_all(atlas_path.parent().unwrap()).ok();
        fs::write(&atlas_path, b"ATLASV0\0\0\0\0\0\0\0\0\0").unwrap();
        assert!(LiveAtlas::open(&atlas_path).is_err());
    }

    #[test]
    fn live_atlas_migrates_legacy_hot_atlas() {
        // Build a legacy hot-atlas.bin payload by hand:
        // [count u32][fp_u64 u64][prog_len u32][prog_bytes]...
        let prog = affine_program();
        let pb = prog.bytes();
        let mut legacy = Vec::new();
        legacy.extend_from_slice(&1u32.to_le_bytes());
        legacy.extend_from_slice(&0xDEADBEEFu64.to_le_bytes());
        legacy.extend_from_slice(&(pb.len() as u32).to_le_bytes());
        legacy.extend_from_slice(pb);

        let legacy_path = fresh_path("legacy-hotatlas");
        let atlas_path = fresh_path("v2-migrated");
        fs::create_dir_all(legacy_path.parent().unwrap()).ok();
        fs::write(&legacy_path, &legacy).unwrap();

        let n = migrate_legacy_hot_atlas(&legacy_path, &atlas_path).unwrap();
        assert_eq!(n, 1);
        assert!(atlas_path.exists());
        assert!(legacy_path.exists(), "legacy file untouched");

        // Reload through LiveAtlas — full pipeline.
        let live = LiveAtlas::open(&atlas_path).unwrap();
        assert_eq!(live.len(), 1);
        let outs = canonical_outputs(&prog).unwrap();
        let fnv = fnv64_outputs(&outs);
        assert!(live.lookup_hot(fnv).is_some());
    }
}
