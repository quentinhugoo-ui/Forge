//! Shared compute primitives for Forge sections.
//!
//! Trading, Immo, WebExplorer analysis, Atlas programs and future labs should
//! reuse this layer instead of growing parallel cache/hash/mask machinery in
//! each feature file. The goal is simple: identical inputs produce one stable
//! key, known sub-results are auto-injected, and only new work reaches CPU/GPU.

use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputeCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub avoided_units: usize,
    pub stage_elapsed_us: u128,
}

impl ComputeCacheStats {
    pub fn delta(self, before: ComputeCacheStats) -> ComputeCacheStats {
        ComputeCacheStats {
            hits: self.hits.saturating_sub(before.hits),
            misses: self.misses.saturating_sub(before.misses),
            avoided_units: self.avoided_units.saturating_sub(before.avoided_units),
            stage_elapsed_us: self.stage_elapsed_us.saturating_sub(before.stage_elapsed_us),
        }
    }

    pub fn record_hit(&mut self, avoided_units: usize, elapsed: Duration) {
        self.hits = self.hits.saturating_add(1);
        self.avoided_units = self.avoided_units.saturating_add(avoided_units);
        self.stage_elapsed_us = self.stage_elapsed_us.saturating_add(elapsed.as_micros());
    }

    pub fn record_miss(&mut self, elapsed: Duration) {
        self.misses = self.misses.saturating_add(1);
        self.stage_elapsed_us = self.stage_elapsed_us.saturating_add(elapsed.as_micros());
    }

    pub fn lookups(self) -> usize {
        self.hits.saturating_add(self.misses)
    }

    pub fn hit_rate(self) -> f64 {
        let lookups = self.lookups();
        if lookups == 0 {
            0.0
        } else {
            self.hits as f64 / lookups as f64
        }
    }
}

pub fn compute_cache_key(stage: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(stage.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    format!("{stage}:{}", hex(&hasher.finalize()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeSurface {
    Home,
    Trading,
    Immo,
    Banger,
    WebExplorer,
    Atlas,
    Other(&'static str),
}

impl ComputeSurface {
    pub fn namespace(self) -> &'static str {
        match self {
            ComputeSurface::Home => "home",
            ComputeSurface::Trading => "trading",
            ComputeSurface::Immo => "immo",
            ComputeSurface::Banger => "banger",
            ComputeSurface::WebExplorer => "webexplorer",
            ComputeSurface::Atlas => "atlas",
            ComputeSurface::Other(namespace) => namespace,
        }
    }

    pub fn stage_key(self, stage: &str, parts: &[&str]) -> String {
        let scoped_stage = format!("{}:{stage}", self.namespace());
        compute_cache_key(&scoped_stage, parts)
    }
}

pub fn compact_hash(key: &str) -> String {
    if key.len() <= 58 {
        key.to_string()
    } else {
        format!("{}..{}", &key[..38], &key[key.len() - 16..])
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskRef {
    pub namespace: String,
    pub hash: String,
    pub len: usize,
    pub popcount: usize,
}

impl MaskRef {
    pub fn key(&self) -> &str {
        &self.hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedBitMask {
    len: usize,
    words: Vec<u64>,
    popcount: usize,
}

impl PackedBitMask {
    pub fn empty(len: usize) -> Self {
        let words = vec![0; len.div_ceil(64)];
        Self {
            len,
            words,
            popcount: 0,
        }
    }

    pub fn from_predicate(mut len: usize, mut predicate: impl FnMut(usize) -> bool) -> Self {
        if len == 0 {
            len = 0;
        }
        let mut mask = Self::empty(len);
        for index in 0..len {
            if predicate(index) {
                mask.set(index, true);
            }
        }
        mask
    }

    pub fn from_indices(len: usize, indices: impl IntoIterator<Item = usize>) -> Self {
        let mut mask = Self::empty(len);
        for index in indices {
            mask.set(index, true);
        }
        mask
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.popcount == 0
    }

    pub fn popcount(&self) -> usize {
        self.popcount
    }

    pub fn words(&self) -> &[u64] {
        &self.words
    }

    pub fn contains(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }
        let word = index / 64;
        let bit = index % 64;
        (self.words[word] & (1_u64 << bit)) != 0
    }

    pub fn set(&mut self, index: usize, value: bool) {
        if index >= self.len {
            return;
        }
        let word = index / 64;
        let bit = index % 64;
        let flag = 1_u64 << bit;
        let was_set = (self.words[word] & flag) != 0;
        match (was_set, value) {
            (false, true) => {
                self.words[word] |= flag;
                self.popcount += 1;
            }
            (true, false) => {
                self.words[word] &= !flag;
                self.popcount -= 1;
            }
            _ => {}
        }
    }

    pub fn iter_ones(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(move |(word_index, word)| {
            OneBits {
                base: word_index * 64,
                word: *word,
                len: self.len,
            }
        })
    }

    pub fn intersection(&self, other: &PackedBitMask) -> PackedBitMask {
        assert_eq!(self.len, other.len, "mask length mismatch");
        let words: Vec<u64> = self
            .words
            .iter()
            .zip(&other.words)
            .map(|(left, right)| left & right)
            .collect();
        Self::from_words(self.len, words)
    }

    pub fn union(&self, other: &PackedBitMask) -> PackedBitMask {
        assert_eq!(self.len, other.len, "mask length mismatch");
        let words: Vec<u64> = self
            .words
            .iter()
            .zip(&other.words)
            .map(|(left, right)| left | right)
            .collect();
        Self::from_words(self.len, words)
    }

    pub fn and_not(&self, other: &PackedBitMask) -> PackedBitMask {
        assert_eq!(self.len, other.len, "mask length mismatch");
        let words: Vec<u64> = self
            .words
            .iter()
            .zip(&other.words)
            .map(|(left, right)| left & !right)
            .collect();
        Self::from_words(self.len, words)
    }

    pub fn mask_ref(&self, namespace: &str) -> MaskRef {
        let mut hasher = Sha256::new();
        hasher.update(namespace.as_bytes());
        hasher.update([0]);
        hasher.update((self.len as u64).to_le_bytes());
        for word in &self.words {
            hasher.update(word.to_le_bytes());
        }
        MaskRef {
            namespace: namespace.to_string(),
            hash: format!("{namespace}:mask:{}", hex(&hasher.finalize())),
            len: self.len,
            popcount: self.popcount,
        }
    }

    fn from_words(len: usize, mut words: Vec<u64>) -> Self {
        let expected = len.div_ceil(64);
        words.truncate(expected);
        words.resize(expected, 0);
        if let Some(last) = words.last_mut() {
            let trailing = len % 64;
            if trailing != 0 {
                *last &= (1_u64 << trailing) - 1;
            }
        }
        let popcount = words.iter().map(|word| word.count_ones() as usize).sum();
        Self {
            len,
            words,
            popcount,
        }
    }
}

struct OneBits {
    base: usize,
    word: u64,
    len: usize,
}

impl Iterator for OneBits {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.word != 0 {
            let bit = self.word.trailing_zeros() as usize;
            self.word &= self.word - 1;
            let index = self.base + bit;
            if index < self.len {
                return Some(index);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompactOpcode {
    pub op: u16,
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

impl CompactOpcode {
    pub const fn new(op: u16, a: i32, b: i32, c: i32) -> Self {
        Self { op, a, b, c }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactProgram {
    pub namespace: String,
    pub ops: Vec<CompactOpcode>,
    pub display_formula: Vec<String>,
}

impl CompactProgram {
    pub fn program_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.namespace.as_bytes());
        for op in &self.ops {
            hasher.update(op.op.to_le_bytes());
            hasher.update(op.a.to_le_bytes());
            hasher.update(op.b.to_le_bytes());
            hasher.update(op.c.to_le_bytes());
        }
        format!("{}:program:{}", self.namespace, hex(&hasher.finalize()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeBackend {
    Auto,
    Cpu,
    Cuda,
    Wgpu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDecision {
    pub selected: ComputeBackend,
    pub gpu_attempted: bool,
    pub fallback_reason: Option<String>,
}

impl BackendDecision {
    pub fn cpu() -> Self {
        Self {
            selected: ComputeBackend::Cpu,
            gpu_attempted: false,
            fallback_reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridAxis {
    pub name: String,
    pub len: usize,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnePassGridPlan {
    pub namespace: String,
    pub kernel: String,
    pub input_hash: String,
    pub axes: Vec<GridAxis>,
    pub backend: ComputeBackend,
}

impl OnePassGridPlan {
    pub fn stage_key(&self) -> String {
        let axis_refs: Vec<String> = self
            .axes
            .iter()
            .map(|axis| format!("{}:{}:{}", axis.name, axis.len, axis.hash))
            .collect();
        let axis_ref_slices: Vec<&str> = axis_refs.iter().map(String::as_str).collect();
        let axes_hash = compute_cache_key("grid_axes:v1", &axis_ref_slices);
        compute_cache_key(
            "one_pass_grid:v1",
            &[
                &self.namespace,
                &self.kernel,
                &self.input_hash,
                &format!("backend={:?}", self.backend),
                &axes_hash,
            ],
        )
    }

    pub fn work_items(&self) -> usize {
        self.axes
            .iter()
            .fold(1_usize, |acc, axis| acc.saturating_mul(axis.len.max(1)))
    }
}

pub fn hash_f64_grid(namespace: &str, values: &[f64]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update([0]);
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    format!("{namespace}:grid:{}", hex(&hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable_and_order_sensitive() {
        let left = compute_cache_key("stage", &["a", "b"]);
        let same = compute_cache_key("stage", &["a", "b"]);
        let different = compute_cache_key("stage", &["b", "a"]);
        assert_eq!(left, same);
        assert_ne!(left, different);
    }

    #[test]
    fn mask_ref_reuses_set_algebra_without_materializing_entries() {
        let even = PackedBitMask::from_predicate(10, |idx| idx % 2 == 0);
        let high = PackedBitMask::from_indices(10, [4, 5, 6, 7, 8, 9]);
        let both = even.intersection(&high);
        assert_eq!(both.iter_ones().collect::<Vec<_>>(), vec![4, 6, 8]);
        let mask_ref = both.mask_ref("test");
        assert_eq!(mask_ref.popcount, 3);
        assert_eq!(mask_ref.len, 10);
        assert!(mask_ref.hash.starts_with("test:mask:"));
    }

    #[test]
    fn grid_plan_keys_include_axes_and_backend() {
        let plan = OnePassGridPlan {
            namespace: "strategy".to_string(),
            kernel: "mfe_reduce_tp_grid".to_string(),
            input_hash: "series:abc".to_string(),
            axes: vec![
                GridAxis {
                    name: "mask".to_string(),
                    len: 128,
                    hash: "mask:1".to_string(),
                },
                GridAxis {
                    name: "tp".to_string(),
                    len: 24,
                    hash: hash_f64_grid("tp", &[0.1, 0.2]),
                },
            ],
            backend: ComputeBackend::Cuda,
        };
        assert_eq!(plan.work_items(), 128 * 24);
        assert!(plan.stage_key().starts_with("one_pass_grid:v1:"));
    }

    #[test]
    fn surface_stage_keys_are_scoped_by_section() {
        let home = ComputeSurface::Home.stage_key("run", &["same"]);
        let banger = ComputeSurface::Banger.stage_key("run", &["same"]);
        assert_ne!(home, banger);
        assert!(home.starts_with("home:run:"));
        assert!(banger.starts_with("banger:run:"));
    }
}
