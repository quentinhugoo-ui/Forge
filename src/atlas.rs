//! Unified content-addressed atlas for redundancy hashes + cross-session
//! result memoization.
//!
//! Single block, single file, single API. Stores ALL hashes that the
//! redundancy pipeline mines (CSE / trace / sub-tree / peek) AND the
//! `(program × input) → result_hash` triples that allow MonsterNode to
//! return a Hit on dispatch without recomputing — across sessions.
//!
//! Both ForgeBackend (Tauri) and MonsterNode (lib core) hold an `Arc<Atlas>`
//! through `MonsterNode::attach_atlas` so a single source of truth backs
//! every "déjà vu ?" question.
//!
//! ## File format (append-only, variable-length records)
//!
//!   record = kind:u8 || payload
//!   kind ∈ {1..=4} → payload = hash:[u8;32]                (33 bytes)
//!   kind ∈ {5..=9} → payload = key:[u8;32] || value:[u8;20] (53 bytes)
//!
//! Kinds 6..=9 are legacy tags (FEATURE/TRADE/SCORE/OPMEMO) collapsed
//! into RESULT in M1.5 ; the reader still recognizes them so old files
//! parse cleanly, but new writes only use RESULT (kind 5). No header,
//! no version field — kind expansion is the schema evolution mechanism.
//!
//! ## In-memory state
//!
//!   `seen`    : `HashSet<[u8;33]>` for kinds 1..=4 (CSE/TRACE/SUBTREE/PEEK)
//!   `results` : `HashMap<[u8;32], [u8;20]>` for kind 5 (RESULT lookups)
//!
//! Both are loaded once at `open()`, kept consistent with the file via the
//! internal Mutex (file) and RwLocks (collections).

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use crate::fast_hash::FastBuildHasher;

fn mix_u64(mut hash: u64, value: u64) -> u64 {
    hash ^= value;
    hash = hash.wrapping_mul(0x100000001b3);
    hash
}

fn alpha_raw_feature_matrix_schema_hash(
    bars_len: u32,
    feature_count: u8,
    schema_version: u8,
) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    hash = mix_u64(hash, bars_len as u64);
    hash = mix_u64(hash, feature_count as u64);
    mix_u64(hash, schema_version as u64)
}

/// Atlas writer buffer size. Each value-bearing record is 53 bytes,
/// each hash-only record is 33 bytes. With 256 KiB the BufWriter
/// amortizes ~4 900 value writes per syscall flush, cutting the
/// per-record write() cost from one syscall to ~one buffer-copy.
///
/// Trade-off : up to 256 KiB of just-recorded entries can be lost on
/// process kill before the next flush. For an atlas (cache layer),
/// this is acceptable — those entries get recomputed and re-written
/// on the next session. Critical writes can call `flush()` explicitly.
const ATLAS_WRITE_BUFFER_BYTES: usize = 256 * 1024;

pub mod kind {
    pub const CSE: u8 = 1;
    pub const TRACE: u8 = 2;
    pub const SUBTREE: u8 = 3;
    pub const PEEK: u8 = 4;
    /// Unified value-bearing kind (M1.5+). `(func_hash || input_hash)
    /// → 20-byte payload`. Used for cross-session memoization of any
    /// `apply()`-style call : program output hash, packed trade
    /// outcome, scored loss, op-level memo — any computation whose
    /// identity is captured by a 32-byte key.
    pub const RESULT: u8 = 5;

    // Legacy kind tags 6..=9 (FEATURE/TRADE/SCORE/OPMEMO) were
    // collapsed into RESULT in M1.5 (commit d6fe1e7) and the
    // fallback readers were dropped in M2. The file format reader
    // still recognizes those bytes as value-bearing for backward
    // compat on read — see `kind_has_value`. New writes always use
    // RESULT ; pre-migration entries with the legacy tags remain
    // physically in the file but become unreachable through the
    // new keying scheme (acceptable cache loss : atlas re-warms
    // on first run after upgrade).
}

/// Returns `true` for kinds that carry a 20-byte value alongside their
/// 32-byte key. Kinds 1..=4 are 33-byte hash-only records ; kinds
/// 5..=9 are 53-byte value records (RESULT plus legacy
/// FEATURE/TRADE/SCORE/OPMEMO tags retained for old-file readability).
fn kind_has_value(k: u8) -> bool {
    (5..=9).contains(&k)
}

const HASH_RECORD_LEN: usize = 33; // kind:1 + hash:32
const RESULT_PAYLOAD_LEN: usize = 52; // key:32 + result:20
const RESULT_RECORD_LEN: usize = 1 + RESULT_PAYLOAD_LEN; // 53 bytes total

pub struct Atlas {
    path: PathBuf,
    /// Append-only writer wrapped in a `BufWriter` to amortize
    /// per-record write() syscalls. Reads at startup happen via a
    /// separate scope before this writer is created.
    file: Mutex<BufWriter<File>>,
    seen: RwLock<HashSet<[u8; HASH_RECORD_LEN], FastBuildHasher>>,
    /// Generic key→value store, indexed by `(kind, key)`. Holds RESULT,
    /// FEATURE, TRADE, SCORE — anything that fits in 20 bytes.
    values: RwLock<HashMap<(u8, [u8; 32]), [u8; 20], FastBuildHasher>>,
}

impl Atlas {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let mut seen = HashSet::with_hasher(FastBuildHasher);
        let mut values: HashMap<(u8, [u8; 32]), [u8; 20], FastBuildHasher> =
            HashMap::with_hasher(FastBuildHasher);
        let mut kind_buf = [0u8; 1];
        let mut hash_buf = [0u8; 32];
        let mut value_payload = [0u8; RESULT_PAYLOAD_LEN];
        loop {
            match file.read_exact(&mut kind_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            let k = kind_buf[0];
            if kind_has_value(k) {
                file.read_exact(&mut value_payload)?;
                let mut key = [0u8; 32];
                let mut res = [0u8; 20];
                key.copy_from_slice(&value_payload[..32]);
                res.copy_from_slice(&value_payload[32..]);
                values.insert((k, key), res);
            } else {
                file.read_exact(&mut hash_buf)?;
                let mut record = [0u8; HASH_RECORD_LEN];
                record[0] = k;
                record[1..].copy_from_slice(&hash_buf);
                seen.insert(record);
            }
        }
        file.seek(SeekFrom::End(0))?;
        let writer = BufWriter::with_capacity(ATLAS_WRITE_BUFFER_BYTES, file);
        Ok(Self {
            path,
            file: Mutex::new(writer),
            seen: RwLock::new(seen),
            values: RwLock::new(values),
        })
    }

    /// Flush the in-memory write buffer to disk. Callers that need
    /// strong durability (e.g. before reopening the same atlas in a
    /// secondary handle) should call this explicitly. The atlas is
    /// also flushed on `Drop` automatically, but `Drop` errors are
    /// silently swallowed.
    pub fn flush(&self) -> io::Result<()> {
        let mut writer = self.file.lock().expect("atlas file poisoned");
        writer.flush()?;
        writer.get_mut().sync_data()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Records `(kind, hash)` for the simple-hash kinds (CSE/TRACE/SUBTREE/PEEK).
    /// Returns `Ok(true)` if new (not seen before), `Ok(false)` if already known.
    /// File write only happens on the new path — strictly append-only and idempotent.
    pub fn record(&self, kind: u8, hash: &[u8; 32]) -> io::Result<bool> {
        debug_assert!(
            !kind_has_value(kind),
            "use record_with_value for value-bearing kinds"
        );
        let mut key = [0u8; HASH_RECORD_LEN];
        key[0] = kind;
        key[1..].copy_from_slice(hash);
        if !self.seen.write().expect("atlas seen poisoned").insert(key) {
            return Ok(false);
        }
        self.file
            .lock()
            .expect("atlas file poisoned")
            .write_all(&key)?;
        Ok(true)
    }

    /// Generic key→value record for any value-bearing kind. The 20-byte
    /// `value` is interpreted by the caller — it can be a SHA-1 result
    /// hash (kind RESULT), a packed feature/trade/score tuple (kinds
    /// FEATURE/TRADE/SCORE), or anything else that fits in 20 bytes.
    pub fn record_with_value(
        &self,
        kind: u8,
        key: &[u8; 32],
        value: &[u8; 20],
    ) -> io::Result<bool> {
        debug_assert!(
            kind_has_value(kind),
            "record_with_value only accepts value-bearing kinds"
        );
        {
            // Single Entry API call instead of contains_key+insert : one
            // hash compute + one bucket probe per write, half the cost.
            let mut values = self.values.write().expect("atlas values poisoned");
            match values.entry((kind, *key)) {
                Entry::Occupied(_) => return Ok(false),
                Entry::Vacant(slot) => {
                    slot.insert(*value);
                }
            }
        }
        let mut record = [0u8; RESULT_RECORD_LEN];
        record[0] = kind;
        record[1..33].copy_from_slice(key);
        record[33..].copy_from_slice(value);
        self.file
            .lock()
            .expect("atlas file poisoned")
            .write_all(&record)?;
        Ok(true)
    }

    /// Generic key→value lookup. Returns the 20-byte value if present.
    pub fn lookup_with_value(&self, kind: u8, key: &[u8; 32]) -> Option<[u8; 20]> {
        self.values
            .read()
            .expect("atlas values poisoned")
            .get(&(kind, *key))
            .copied()
    }

    /// Stores `(key) → result_hash` for cross-session result memoization.
    /// Returns `Ok(true)` if the key is new, `Ok(false)` if it was already
    /// mapped (the existing mapping is left untouched — first-write-wins).
    pub fn record_result(
        &self,
        key: &[u8; 32],
        result_hash: &[u8; 20],
    ) -> io::Result<bool> {
        self.record_with_value(kind::RESULT, key, result_hash)
    }

    pub fn contains(&self, kind: u8, hash: &[u8; 32]) -> bool {
        debug_assert!(
            !kind_has_value(kind),
            "use lookup_with_value for value-bearing kinds"
        );
        let mut key = [0u8; HASH_RECORD_LEN];
        key[0] = kind;
        key[1..].copy_from_slice(hash);
        self.seen.read().expect("atlas seen poisoned").contains(&key)
    }

    /// Returns the persisted result hash for a `(program, input)` key, if any.
    /// Caller loads the actual bytes from the `Store` via the returned hash.
    pub fn lookup_result(&self, key: &[u8; 32]) -> Option<[u8; 20]> {
        self.lookup_with_value(kind::RESULT, key)
    }

    pub fn count_kind(&self, kind: u8) -> usize {
        if kind_has_value(kind) {
            return self
                .values
                .read()
                .expect("atlas values poisoned")
                .keys()
                .filter(|(k, _)| *k == kind)
                .count();
        }
        self.seen
            .read()
            .expect("atlas seen poisoned")
            .iter()
            .filter(|k| k[0] == kind)
            .count()
    }

    pub fn total(&self) -> usize {
        let seen = self.seen.read().expect("atlas seen poisoned").len();
        let values = self.values.read().expect("atlas values poisoned").len();
        seen + values
    }

    /// Compose a 32-byte key for a per-bar feature value. Layout:
    ///   `[file_hash:8][feature_id:1][bar_index:4][zero_pad:19]`
    pub fn feature_key(file_hash: u64, feature_id: u8, bar_index: u32) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8] = feature_id;
        key[9..13].copy_from_slice(&bar_index.to_le_bytes());
        key
    }

    /// Compose a 32-byte key for any deterministic blob-backed result.
    ///
    /// RESULT points to a Store blob. Callers choose an 8-byte namespace and
    /// put every semantic/version/config bit that changes the blob format into
    /// `schema_hash`. This is the generic path for replacing huge scalar Atlas
    /// fan-out with one content-addressed matrix/vector/table artifact.
    pub fn blob_result_key(
        namespace: &[u8; 8],
        source_hash: u64,
        schema_hash: u64,
        start: u32,
        end: u32,
    ) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(namespace);
        key[8..16].copy_from_slice(&source_hash.to_le_bytes());
        key[16..24].copy_from_slice(&schema_hash.to_le_bytes());
        key[24..28].copy_from_slice(&start.to_le_bytes());
        key[28..32].copy_from_slice(&end.to_le_bytes());
        key
    }

    /// Backward-compatible named key for the full Alpha raw feature matrix.
    pub fn alpha_raw_feature_matrix_key(
        file_hash: u64,
        start: u32,
        end: u32,
        bars_len: u32,
        feature_count: u8,
        schema_version: u8,
    ) -> [u8; 32] {
        let schema_hash = alpha_raw_feature_matrix_schema_hash(
            bars_len,
            feature_count,
            schema_version,
        );
        Self::blob_result_key(b"FRAWMTX1", file_hash, schema_hash, start, end)
    }

    /// Compose a 32-byte key for a trade simulation outcome. Layout:
    ///   `[file_hash:8][bar_index:4][direction:1][sl_milli:4]
    ///    [tp_milli:4][spread_milli:4][horizon:2][zero_pad:5]`
    /// Prices are quantized at 1e-3 so OANDA-style spreads like 0.008 are
    /// preserved exactly in the cache key.
    pub fn trade_key(
        file_hash: u64,
        bar_index: u32,
        direction: u8,
        sl_points: f64,
        tp_points: f64,
        spread_points: f64,
        horizon: u16,
    ) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..12].copy_from_slice(&bar_index.to_le_bytes());
        key[12] = direction;
        let sl_milli = (sl_points * 1000.0).round() as i32;
        let tp_milli = (tp_points * 1000.0).round() as i32;
        let spread_milli = (spread_points * 1000.0).round() as i32;
        key[13..17].copy_from_slice(&sl_milli.to_le_bytes());
        key[17..21].copy_from_slice(&tp_milli.to_le_bytes());
        key[21..25].copy_from_slice(&spread_milli.to_le_bytes());
        key[25..27].copy_from_slice(&horizon.to_le_bytes());
        key
    }

    /// Compose a 32-byte key for derived binary LONG/SHORT opportunity labels.
    /// Layout:
    ///   `[file_hash:8][bar_index:4][sl_milli:4][tp_milli:4][spread_milli:4][horizon:2][pad:6]`
    /// Stable across sessions for any `(file, bar, sl, tp, spread, horizon)` combination.
    pub fn label_key(
        file_hash: u64,
        bar_index: u32,
        sl_points: f64,
        tp_points: f64,
        spread_points: f64,
        horizon: u16,
    ) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..12].copy_from_slice(&bar_index.to_le_bytes());
        let sl_milli = (sl_points * 1000.0).round() as i32;
        let tp_milli = (tp_points * 1000.0).round() as i32;
        let spread_milli = (spread_points * 1000.0).round() as i32;
        key[12..16].copy_from_slice(&sl_milli.to_le_bytes());
        key[16..20].copy_from_slice(&tp_milli.to_le_bytes());
        key[20..24].copy_from_slice(&spread_milli.to_le_bytes());
        key[24..26].copy_from_slice(&horizon.to_le_bytes());
        key
    }

    /// Pack an `f64` value into a 20-byte slot (8 bytes value + 12 bytes
    /// zero pad). Used by FEATURE-kind entries.
    pub fn pack_f64(value: f64) -> [u8; 20] {
        let mut out = [0u8; 20];
        out[..8].copy_from_slice(&value.to_le_bytes());
        out
    }

    pub fn unpack_f64(value: &[u8; 20]) -> f64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&value[..8]);
        f64::from_le_bytes(bytes)
    }

    /// Pack a trade outcome `(pnl_points, exit_reason, bars_held)` into
    /// a 20-byte slot: `[pnl:f64=8][exit:u8=1][bars_held:u8=1][pad:10]`.
    pub fn pack_trade(pnl_points: f64, exit_reason: u8, bars_held: u8) -> [u8; 20] {
        let mut out = [0u8; 20];
        out[..8].copy_from_slice(&pnl_points.to_le_bytes());
        out[8] = exit_reason;
        out[9] = bars_held;
        out
    }

    pub fn unpack_trade(value: &[u8; 20]) -> (f64, u8, u8) {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&value[..8]);
        (f64::from_le_bytes(bytes), value[8], value[9])
    }

    /// Pack binary LONG/SHORT labels into a 20-byte slot.
    pub fn pack_binary_labels(long_label: i64, short_label: i64) -> [u8; 20] {
        let mut out = [0u8; 20];
        out[0] = if long_label != 0 { 1 } else { 0 };
        out[1] = if short_label != 0 { 1 } else { 0 };
        out
    }

    pub fn unpack_binary_labels(value: &[u8; 20]) -> (i64, i64) {
        (i64::from(value[0] != 0), i64::from(value[1] != 0))
    }

    /// Compose a 32-byte key for an op-level memo entry. Layout:
    ///   `[op_byte:1][input_i64:8][zero_pad:23]`
    /// Stable across sessions for any (op, input) pair the interpreter
    /// has already evaluated.
    pub fn opmemo_key(op_byte: u8, input: i64) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[0] = op_byte;
        key[1..9].copy_from_slice(&input.to_le_bytes());
        key
    }

    /// Pack an `i64` value into a 20-byte slot (8 bytes value + 12 pad).
    pub fn pack_i64(value: i64) -> [u8; 20] {
        let mut out = [0u8; 20];
        out[..8].copy_from_slice(&value.to_le_bytes());
        out
    }

    pub fn unpack_i64(value: &[u8; 20]) -> i64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&value[..8]);
        i64::from_le_bytes(bytes)
    }

    /// Compose a 32-byte lookup key from `(func_hash || input_bytes)`. The
    /// canonical packing used by MonsterNode dispatch + `synth_atlas_warm_estimate`:
    /// 20 bytes of `Hash::as_bytes()` followed by up to 12 bytes of input
    /// (zero-padded if shorter). For inputs longer than 12 bytes (rare for
    /// scalar workloads), only the first 12 bytes participate — reasonable
    /// since inputs at this scale share `func` AND a stable prefix.
    pub fn result_key(func_bytes: &[u8; 20], input_bytes: &[u8]) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..20].copy_from_slice(func_bytes);
        let n = input_bytes.len().min(12);
        key[20..20 + n].copy_from_slice(&input_bytes[..n]);
        key
    }

    /// Compose a 32-byte key for a synth pair score bundle. Layout:
    ///   `[left_fp:8][right_fp:8][targets_fp:8][n_examples:4][pad:4]`
    /// Stable across sessions for any `(left outputs, right outputs,
    /// target outputs, example count)` combination. The blob stored via
    /// RESULT contains the 9 opcode scores for that pair.
    pub fn pair_score_key(
        left_fp: u64,
        right_fp: u64,
        targets_fp: u64,
        n_examples: u32,
    ) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&left_fp.to_le_bytes());
        key[8..16].copy_from_slice(&right_fp.to_le_bytes());
        key[16..24].copy_from_slice(&targets_fp.to_le_bytes());
        key[24..28].copy_from_slice(&n_examples.to_le_bytes());
        key
    }

    /// Compose a 32-byte key for a single synth opcode score.
    /// Layout:
    ///   `[left_fp:8][right_fp:8][targets_fp:8][n_examples:4][op:1][pad:3]`
    pub fn pair_op_score_key(
        left_fp: u64,
        right_fp: u64,
        targets_fp: u64,
        n_examples: u32,
        op: u8,
    ) -> [u8; 32] {
        let mut key = Self::pair_score_key(left_fp, right_fp, targets_fp, n_examples);
        key[28] = op;
        key
    }

    /// Compose a 32-byte key for one confluence feature scalar.
    /// Layout:
    ///   `[file_hash:8][modelset_fp:8][feature_id:1][bar_index:4][pad:11]`
    /// `modelset_fp` must summarize the exact stage-1 LONG/SHORT detector set
    /// used to derive the confluence row.
    pub fn confluence_feature_key(
        file_hash: u64,
        modelset_fp: u64,
        feature_id: u8,
        bar_index: u32,
    ) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..16].copy_from_slice(&modelset_fp.to_le_bytes());
        key[16] = feature_id;
        key[17..21].copy_from_slice(&bar_index.to_le_bytes());
        key
    }

    /// Compose a 32-byte key for one persisted prediction row.
    /// Layout:
    ///   `[file_hash:8][program_hash:20][bar_index:4]`
    /// Stable for any `(file, detector program, bar)` triple.
    pub fn prediction_key(
        file_hash: u64,
        program_hash: &[u8; 20],
        bar_index: u32,
    ) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..28].copy_from_slice(program_hash);
        key[28..32].copy_from_slice(&bar_index.to_le_bytes());
        key
    }

    /// Compose a 32-byte key for a persisted final alpha-synth selection
    /// artifact. Layout:
    ///   `[file_hash:8][request_fp:8][pad:16]`
    /// `request_fp` must summarize the user-facing synthesis request
    /// parameters so repeated runs on the same file/params reuse the same slot.
    pub fn alpha_selection_key(file_hash: u64, request_fp: u64) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..16].copy_from_slice(&request_fp.to_le_bytes());
        key
    }

    /// Compose a 32-byte key for a persisted alpha pre-start artifact.
    /// Layout:
    ///   `[file_hash:8]["APRE3":5][pad:19]`
    /// The blob referenced by RESULT contains the parsed bars and raw
    /// VWAP feature rows prepared during pre-start inspect.
    pub fn alpha_prestart_key(file_hash: u64) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..13].copy_from_slice(b"APRE3");
        key
    }

    /// Compose a 32-byte key for a persisted alpha inspect report.
    /// Layout:
    ///   `[file_hash:8]["AINSPT2":7][pad:17]`
    /// The blob referenced by RESULT contains the serialized
    /// `ComputationPlanReport` for `reverse_synth_alpha`, allowing
    /// cross-session instant replay of the full pre-start analysis.
    pub fn alpha_inspect_key(file_hash: u64) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..15].copy_from_slice(b"AINSPT2");
        key
    }

    /// Compose a 32-byte key for one persisted final decision row.
    /// Layout:
    ///   `[file_hash:8][decision_fp:8][bar_index:4][pad:12]`
    /// `decision_fp` must summarize the exact LONG/SHORT detector pair
    /// whose merged decision stream is being cached.
    pub fn decision_key(file_hash: u64, decision_fp: u64, bar_index: u32) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&file_hash.to_le_bytes());
        key[8..16].copy_from_slice(&decision_fp.to_le_bytes());
        key[16..20].copy_from_slice(&bar_index.to_le_bytes());
        key
    }

    /// Convenience helper: pad a `u64` trace hash to a 32-byte record key.
    pub fn pad_u64(value: u64) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..8].copy_from_slice(&value.to_le_bytes());
        out
    }
}

impl Drop for Atlas {
    /// Best-effort flush of the in-memory write buffer to the OS on
    /// drop. Errors are intentionally swallowed — `Drop` cannot
    /// propagate `io::Result`, and the atlas is a cache layer where
    /// losing the very last 256 KiB on a crash is acceptable. Callers
    /// that require strong durability must call `flush()` explicitly
    /// while still holding the `Atlas` (or its owning `Arc<Atlas>`).
    fn drop(&mut self) {
        if let Ok(mut writer) = self.file.lock() {
            let _ = writer.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("forge-atlas-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    /// Microbenchmark that exercises the BufWriter amortization on a
    /// FEATURE-style write workload. Marked `#[ignore]` so it doesn't
    /// run in the default `cargo test` pass — invoke with
    /// `cargo test --release atlas_write_throughput_bench --
    ///   --ignored --nocapture` to get the timing print.
    ///
    /// The test deliberately uses 100 000 unique records so the
    /// duplicate-shortcut never fires and every call exercises the
    /// full lock + write path. On the BufWriter implementation this
    /// should land in the millions of writes/sec range ; on the
    /// pre-BufWriter (raw File) implementation the same workload
    /// drops by a large factor due to per-record write() syscalls.
    #[test]
    #[ignore]
    fn atlas_write_throughput_bench() {
        const N: u32 = 100_000;
        let path = fresh_path("perf-buf");
        let atlas = Atlas::open(&path).expect("open");
        let t0 = std::time::Instant::now();
        for i in 0..N {
            let key = Atlas::feature_key(0xCAFEBABE_u64, 1, i);
            let value = Atlas::pack_f64(i as f64);
            let _ = atlas.record_with_value(kind::RESULT, &key, &value);
        }
        atlas.flush().expect("flush");
        let elapsed = t0.elapsed();
        let throughput = (N as f64) / elapsed.as_secs_f64();
        println!(
            "atlas_write_throughput_bench: {} records in {:.3} ms ({:.0} records/sec, {:.0} ns/record)",
            N,
            elapsed.as_secs_f64() * 1000.0,
            throughput,
            elapsed.as_nanos() as f64 / N as f64,
        );
        assert_eq!(atlas.count_kind(kind::RESULT), N as usize);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn atlas_records_new_and_skips_duplicates() {
        let path = fresh_path("dup");
        let atlas = Atlas::open(&path).expect("open");
        let h = [42u8; 32];
        assert!(atlas.record(kind::CSE, &h).unwrap());
        assert!(!atlas.record(kind::CSE, &h).unwrap());
        assert!(atlas.contains(kind::CSE, &h));
        assert!(!atlas.contains(kind::TRACE, &h));
        assert_eq!(atlas.count_kind(kind::CSE), 1);
        assert_eq!(atlas.total(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn atlas_persists_across_open_close() {
        let path = fresh_path("persist");
        {
            let atlas = Atlas::open(&path).expect("open1");
            atlas.record(kind::CSE, &[1u8; 32]).unwrap();
            atlas.record(kind::TRACE, &Atlas::pad_u64(0xDEADBEEF)).unwrap();
            atlas.record(kind::SUBTREE, &[3u8; 32]).unwrap();
        }
        let atlas = Atlas::open(&path).expect("open2");
        assert_eq!(atlas.total(), 3);
        assert!(atlas.contains(kind::CSE, &[1u8; 32]));
        assert!(atlas.contains(kind::TRACE, &Atlas::pad_u64(0xDEADBEEF)));
        assert!(atlas.contains(kind::SUBTREE, &[3u8; 32]));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn atlas_separates_kinds_by_first_byte() {
        let path = fresh_path("kinds");
        let atlas = Atlas::open(&path).expect("open");
        let h = [7u8; 32];
        atlas.record(kind::CSE, &h).unwrap();
        atlas.record(kind::TRACE, &h).unwrap();
        atlas.record(kind::SUBTREE, &h).unwrap();
        atlas.record(kind::PEEK, &h).unwrap();
        assert_eq!(atlas.count_kind(kind::CSE), 1);
        assert_eq!(atlas.count_kind(kind::TRACE), 1);
        assert_eq!(atlas.count_kind(kind::SUBTREE), 1);
        assert_eq!(atlas.count_kind(kind::PEEK), 1);
        assert_eq!(atlas.total(), 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn atlas_persists_results_across_sessions() {
        let path = fresh_path("results");
        let func = [11u8; 20];
        let input = 42i64.to_le_bytes();
        let key = Atlas::result_key(&func, &input);
        let result = [0xABu8; 20];
        {
            let atlas = Atlas::open(&path).expect("open1");
            assert!(atlas.lookup_result(&key).is_none());
            assert!(atlas.record_result(&key, &result).unwrap());
            assert!(!atlas.record_result(&key, &result).unwrap()); // idempotent
            assert_eq!(atlas.lookup_result(&key), Some(result));
        }
        let atlas = Atlas::open(&path).expect("open2");
        assert_eq!(atlas.lookup_result(&key), Some(result));
        assert_eq!(atlas.count_kind(kind::RESULT), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn atlas_result_keys_distinguish_inputs() {
        let path = fresh_path("result-keys");
        let atlas = Atlas::open(&path).expect("open");
        let func = [22u8; 20];
        let k1 = Atlas::result_key(&func, &1i64.to_le_bytes());
        let k2 = Atlas::result_key(&func, &2i64.to_le_bytes());
        assert_ne!(k1, k2);
        atlas.record_result(&k1, &[0x11u8; 20]).unwrap();
        atlas.record_result(&k2, &[0x22u8; 20]).unwrap();
        assert_eq!(atlas.lookup_result(&k1), Some([0x11u8; 20]));
        assert_eq!(atlas.lookup_result(&k2), Some([0x22u8; 20]));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn trade_keys_distinguish_tp_values() {
        let k1 = Atlas::trade_key(0xABCD, 42, 1, 0.09, 0.07, 0.008, 6);
        let k2 = Atlas::trade_key(0xABCD, 42, 1, 0.09, 0.11, 0.008, 6);
        assert_ne!(k1, k2);
    }

    #[test]
    fn trade_keys_distinguish_spread_values() {
        let k1 = Atlas::trade_key(0xABCD, 42, 1, 0.09, 0.07, 0.006, 6);
        let k2 = Atlas::trade_key(0xABCD, 42, 1, 0.09, 0.07, 0.010, 6);
        assert_ne!(k1, k2);
    }

    #[test]
    fn label_keys_distinguish_tp_and_spread_values() {
        let k1 = Atlas::label_key(0xABCD, 42, 0.07, 0.07, 0.008, 6);
        let k2 = Atlas::label_key(0xABCD, 42, 0.07, 0.09, 0.008, 6);
        let k3 = Atlas::label_key(0xABCD, 42, 0.07, 0.07, 0.010, 6);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn binary_label_pack_roundtrip() {
        let packed = Atlas::pack_binary_labels(1, 0);
        assert_eq!(Atlas::unpack_binary_labels(&packed), (1, 0));
        let packed = Atlas::pack_binary_labels(0, 1);
        assert_eq!(Atlas::unpack_binary_labels(&packed), (0, 1));
    }

    #[test]
    fn pair_score_keys_distinguish_pair_shape() {
        let k1 = Atlas::pair_score_key(11, 22, 33, 1440);
        let k2 = Atlas::pair_score_key(11, 22, 33, 2880);
        let k3 = Atlas::pair_score_key(11, 44, 33, 1440);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn pair_op_score_keys_distinguish_opcode() {
        let k1 = Atlas::pair_op_score_key(11, 22, 33, 1440, 1);
        let k2 = Atlas::pair_op_score_key(11, 22, 33, 1440, 7);
        assert_ne!(k1, k2);
    }

    #[test]
    fn confluence_feature_keys_distinguish_modelset_feature_and_bar() {
        let k1 = Atlas::confluence_feature_key(0xAA, 0xBB, 3, 42);
        let k2 = Atlas::confluence_feature_key(0xAA, 0xCC, 3, 42);
        let k3 = Atlas::confluence_feature_key(0xAA, 0xBB, 4, 42);
        let k4 = Atlas::confluence_feature_key(0xAA, 0xBB, 3, 99);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
        assert_ne!(k1, k4);
    }

    #[test]
    fn prediction_keys_distinguish_program_and_bar() {
        let p1 = [0x11; 20];
        let p2 = [0x22; 20];
        let k1 = Atlas::prediction_key(0xAA, &p1, 42);
        let k2 = Atlas::prediction_key(0xAA, &p2, 42);
        let k3 = Atlas::prediction_key(0xAA, &p1, 99);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn alpha_selection_keys_distinguish_request_fingerprint() {
        let k1 = Atlas::alpha_selection_key(0xAA, 0x11);
        let k2 = Atlas::alpha_selection_key(0xAA, 0x22);
        let k3 = Atlas::alpha_selection_key(0xBB, 0x11);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn alpha_prestart_keys_distinguish_file_hash() {
        let k1 = Atlas::alpha_prestart_key(0xAA);
        let k2 = Atlas::alpha_prestart_key(0xBB);
        assert_ne!(k1, k2);
    }

    #[test]
    fn alpha_inspect_keys_distinguish_file_hash() {
        let k1 = Atlas::alpha_inspect_key(0xAA);
        let k2 = Atlas::alpha_inspect_key(0xBB);
        let k3 = Atlas::alpha_prestart_key(0xAA);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn decision_keys_distinguish_pair_and_bar() {
        let k1 = Atlas::decision_key(0xAA, 0x11, 42);
        let k2 = Atlas::decision_key(0xAA, 0x22, 42);
        let k3 = Atlas::decision_key(0xAA, 0x11, 99);
        let k4 = Atlas::decision_key(0xBB, 0x11, 42);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
        assert_ne!(k1, k4);
    }
}

