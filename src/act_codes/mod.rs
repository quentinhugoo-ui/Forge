//! Act Codes — the engineering/biology/gamedev math library that turns an
//! SDF design into verified physical artifacts, content-addressed so a
//! given (sub-geometry, computation, params) triple is never evaluated
//! twice across the whole session history.
//!
//! ## The workflow this module backs
//!
//! 1. The LLM talks to the user in natural language. If the user knows
//!    engineering math, the LLM hand-writes act codes directly. If not,
//!    step 2 covers them with the standard library.
//! 2. The LLM decomposes the request into sub-parts. For each sub-part it
//!    looks up an existing `ActCode` (or registers a new one) and runs it
//!    through the KASM-style ledger : `hash(sub_sdf, act_id, params)`. A
//!    hit returns the stored `Artifact` instantly — no recompute, at every
//!    level of the computation. Billions of redundant evals collapse.
//! 3. The engine returns the artifacts to the LLM, which uses the numbers
//!    to rewrite the SDF act code for visualization in INGEN Banger.
//!
//! ## Doctrine compliance
//!
//! - Zero external deps beyond `sha2` (already a Forge dependency) — the
//!   ledger key is the canonical persisted hash, not a RAM-only fingerprint.
//! - The SDF evaluator mirrors `scenes.ts` opcode-for-opcode so the CPU
//!   physics and the GPU raymarcher agree on the same geometry.
//! - Multi-level dedup : the planner hashes whole-scene AND per-sub-part,
//!   so mutating one component re-runs only the act codes that touch it.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub mod inertia;
pub mod modal;
pub mod planner;

// ---------------------------------------------------------------------------
// SDF op model — mirrors examples/forge_tauri_ui/ui/src/sections/banger/scenes.ts
// ---------------------------------------------------------------------------

pub type Vec3 = [f64; 3];

/// One SDF operation. Field semantics match scenes.ts::SdfOp exactly so a
/// scene authored on either side round-trips losslessly.
#[derive(Clone, Debug, PartialEq)]
pub enum SdfOp {
    Sphere { center: Vec3, radius: f64 },
    Box { center: Vec3, half_extents: Vec3 },
    Torus { center: Vec3, major_radius: f64, minor_radius: f64 },
    Capsule { a: Vec3, b: Vec3, radius: f64 },
    RoundedBox { center: Vec3, half_extents: Vec3, corner_radius: f64 },
    Union,
    Intersect,
    Diff,
    Smin { k: f64 },
}

impl SdfOp {
    /// Opcode integer, identical to scenes.ts OP_* constants.
    pub fn opcode(&self) -> u32 {
        match self {
            SdfOp::Sphere { .. } => 0,
            SdfOp::Box { .. } => 1,
            SdfOp::Torus { .. } => 2,
            SdfOp::Capsule { .. } => 3,
            SdfOp::RoundedBox { .. } => 4,
            SdfOp::Union => 10,
            SdfOp::Intersect => 11,
            SdfOp::Diff => 12,
            SdfOp::Smin { .. } => 13,
        }
    }

    /// Append this op's bytes to a hasher in a stable, ASCII-free layout so
    /// the ledger key is reproducible across runs and machines.
    fn hash_into(&self, h: &mut Sha256) {
        h.update(self.opcode().to_le_bytes());
        let mut push = |v: f64| h.update(v.to_le_bytes());
        match *self {
            SdfOp::Sphere { center, radius } => {
                for c in center { push(c); }
                push(radius);
            }
            SdfOp::Box { center, half_extents } => {
                for c in center { push(c); }
                for e in half_extents { push(e); }
            }
            SdfOp::Torus { center, major_radius, minor_radius } => {
                for c in center { push(c); }
                push(major_radius);
                push(minor_radius);
            }
            SdfOp::Capsule { a, b, radius } => {
                for c in a { push(c); }
                for c in b { push(c); }
                push(radius);
            }
            SdfOp::RoundedBox { center, half_extents, corner_radius } => {
                for c in center { push(c); }
                for e in half_extents { push(e); }
                push(corner_radius);
            }
            SdfOp::Union | SdfOp::Intersect | SdfOp::Diff => {}
            SdfOp::Smin { k } => push(k),
        }
    }
}

// ---- SDF primitives (mirror the WGSL in ingen-render.ts) ------------------

#[inline]
fn sub(a: Vec3, b: Vec3) -> Vec3 { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
#[inline]
fn length(v: Vec3) -> f64 { (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt() }

#[inline]
fn sd_sphere(p: Vec3, r: f64) -> f64 { length(p) - r }

#[inline]
fn sd_box(p: Vec3, b: Vec3) -> f64 {
    let q = [p[0].abs()-b[0], p[1].abs()-b[1], p[2].abs()-b[2]];
    let outside = length([q[0].max(0.0), q[1].max(0.0), q[2].max(0.0)]);
    let inside = q[0].max(q[1].max(q[2])).min(0.0);
    outside + inside
}

#[inline]
fn sd_torus(p: Vec3, big_r: f64, small_r: f64) -> f64 {
    let q = [(p[0]*p[0] + p[1]*p[1]).sqrt() - big_r, p[2]];
    (q[0]*q[0] + q[1]*q[1]).sqrt() - small_r
}

#[inline]
fn sd_capsule(p: Vec3, a: Vec3, b: Vec3, r: f64) -> f64 {
    let pa = sub(p, a);
    let ba = sub(b, a);
    let h = (dot(pa, ba) / dot(ba, ba).max(1e-9)).clamp(0.0, 1.0);
    length([pa[0]-ba[0]*h, pa[1]-ba[1]*h, pa[2]-ba[2]*h]) - r
}

#[inline]
fn sd_rounded_box(p: Vec3, b: Vec3, r: f64) -> f64 {
    let q = [p[0].abs()-b[0]+r, p[1].abs()-b[1]+r, p[2].abs()-b[2]+r];
    let outside = length([q[0].max(0.0), q[1].max(0.0), q[2].max(0.0)]);
    let inside = q[0].max(q[1].max(q[2])).min(0.0);
    outside + inside - r
}

#[inline]
fn dot(a: Vec3, b: Vec3) -> f64 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }

#[inline]
fn smin_k(a: f64, b: f64, k: f64) -> f64 {
    let kk = k.max(1e-4);
    let h = (0.5 + 0.5 * (b - a) / kk).clamp(0.0, 1.0);
    (b * (1.0 - h) + a * h) - kk * h * (1.0 - h)
}

/// Evaluate the signed distance of `ops` (postfix stack machine) at point p.
/// Identical traversal to the WGSL `scene()` — primitives push, binary ops
/// fold the top two stack entries.
pub fn eval_scene(ops: &[SdfOp], p: Vec3) -> f64 {
    let mut stack: Vec<f64> = Vec::with_capacity(16);
    for op in ops {
        match *op {
            SdfOp::Sphere { center, radius } => stack.push(sd_sphere(sub(p, center), radius)),
            SdfOp::Box { center, half_extents } => stack.push(sd_box(sub(p, center), half_extents)),
            SdfOp::Torus { center, major_radius, minor_radius } =>
                stack.push(sd_torus(sub(p, center), major_radius, minor_radius)),
            SdfOp::Capsule { a, b, radius } => stack.push(sd_capsule(p, a, b, radius)),
            SdfOp::RoundedBox { center, half_extents, corner_radius } =>
                stack.push(sd_rounded_box(sub(p, center), half_extents, corner_radius)),
            SdfOp::Union => { let b = stack.pop().unwrap_or(1e9); let a = stack.pop().unwrap_or(1e9); stack.push(a.min(b)); }
            SdfOp::Intersect => { let b = stack.pop().unwrap_or(1e9); let a = stack.pop().unwrap_or(1e9); stack.push(a.max(b)); }
            SdfOp::Diff => { let b = stack.pop().unwrap_or(1e9); let a = stack.pop().unwrap_or(1e9); stack.push(a.max(-b)); }
            SdfOp::Smin { k } => { let b = stack.pop().unwrap_or(1e9); let a = stack.pop().unwrap_or(1e9); stack.push(smin_k(a, b, k)); }
        }
    }
    stack.first().copied().unwrap_or(1e9)
}

/// Axis-aligned bounding box of an op list (loose — sums primitive extents).
/// Used to size the Monte-Carlo integration domain.
pub fn scene_aabb(ops: &[SdfOp]) -> (Vec3, Vec3) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut grow = |c: Vec3, r: f64| {
        for i in 0..3 {
            lo[i] = lo[i].min(c[i] - r);
            hi[i] = hi[i].max(c[i] + r);
        }
    };
    for op in ops {
        match *op {
            SdfOp::Sphere { center, radius } => grow(center, radius),
            SdfOp::Box { center, half_extents } | SdfOp::RoundedBox { center, half_extents, .. } => {
                let r = half_extents[0].max(half_extents[1]).max(half_extents[2]);
                grow(center, r);
            }
            SdfOp::Torus { center, major_radius, minor_radius } => grow(center, major_radius + minor_radius),
            SdfOp::Capsule { a, b, radius } => { grow(a, radius); grow(b, radius); }
            _ => {}
        }
    }
    if !lo[0].is_finite() { (lo, hi) = ([-1.0; 3], [1.0; 3]); }
    (lo, hi)
}

// ---------------------------------------------------------------------------
// Artifact — the typed result of an act code
// ---------------------------------------------------------------------------

/// The output of running an act code. Each variant serialises to a compact
/// ASCII JSON object for the ledger and the final physics report.
#[derive(Clone, Debug, PartialEq)]
pub enum Artifact {
    /// Mass (kg), centre of mass (m), inertia tensor (kg·m²) as the 6 unique
    /// entries of the symmetric matrix : [Ixx, Iyy, Izz, Ixy, Ixz, Iyz].
    Inertia { mass: f64, com: Vec3, tensor: [f64; 6] },
    /// A scalar list (frequencies, temperatures, …) tagged by a label.
    Scalars { label: String, values: Vec<f64> },
}

impl Artifact {
    /// Serialise to a single-line ASCII JSON object (ledger + report use this).
    pub fn to_json(&self) -> String {
        match self {
            Artifact::Inertia { mass, com, tensor } => format!(
                "{{\"kind\":\"inertia\",\"mass\":{},\"com\":[{},{},{}],\"tensor\":[{},{},{},{},{},{}]}}",
                fj(*mass), fj(com[0]), fj(com[1]), fj(com[2]),
                fj(tensor[0]), fj(tensor[1]), fj(tensor[2]), fj(tensor[3]), fj(tensor[4]), fj(tensor[5]),
            ),
            Artifact::Scalars { label, values } => {
                let vs: Vec<String> = values.iter().map(|v| fj(*v)).collect();
                format!("{{\"kind\":\"scalars\",\"label\":\"{}\",\"values\":[{}]}}", label, vs.join(","))
            }
        }
    }
}

/// JSON-safe float : finite decimal, no Unicode, no scientific 'e' surprises
/// for the common magnitudes we deal with (mm..m, grams..kg).
pub(crate) fn fj(v: f64) -> String {
    if !v.is_finite() { return "0".into(); }
    if v == 0.0 { return "0".into(); }
    let s = format!("{:.9}", v);
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" { "0".into() } else { t.into() }
}

// ---------------------------------------------------------------------------
// ActCode trait — one engineering computation
// ---------------------------------------------------------------------------

/// A single math/engineering computation over an SDF sub-geometry. The id
/// + params hash into the ledger key so two identical requests collapse.
pub trait ActCode {
    /// Stable identifier, e.g. "inertia.mc.v1". Part of the ledger key.
    fn id(&self) -> &'static str;
    /// Stable bytes describing this code's parameters (sample count, grid…).
    fn params_bytes(&self) -> Vec<u8>;
    /// Run the computation over `ops` and return the artifact.
    fn run(&self, ops: &[SdfOp]) -> Artifact;
}

/// Compute the content-addressed ledger key for (act_code, ops).
/// Key = sha256(act_id || params || each op's stable bytes).
pub fn ledger_key(code: &dyn ActCode, ops: &[SdfOp]) -> String {
    let mut h = Sha256::new();
    h.update(code.id().as_bytes());
    h.update(b"\x00");
    h.update(code.params_bytes());
    h.update(b"\x00");
    for op in ops { op.hash_into(&mut h); }
    let digest = h.finalize();
    // 16-byte (128-bit) hex prefix — collision-free for any realistic
    // act-code corpus, half the bytes of the full digest.
    let mut s = String::with_capacity(32);
    for b in &digest[..16] {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ---------------------------------------------------------------------------
// ActLedger — content-addressed result cache, the KASM dedup at work
// ---------------------------------------------------------------------------

/// Append-only JSONL ledger : `<key>\t<artifact-json>\n`. Loaded once into a
/// HashMap, queried in O(1). This is the layer that makes "never compute the
/// same thing twice" real — at the whole-scene level AND the sub-part level,
/// since the planner hashes every sub-geometry independently.
pub struct ActLedger {
    path: PathBuf,
    cache: HashMap<String, String>, // key -> artifact json
    pub hits: u64,
    pub misses: u64,
}

impl ActLedger {
    /// Open (or create) a ledger file, loading existing entries into RAM.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut cache = HashMap::new();
        if path.exists() {
            let f = fs::File::open(&path)?;
            for line in BufReader::new(f).lines() {
                let line = line?;
                if let Some((k, v)) = line.split_once('\t') {
                    cache.insert(k.to_string(), v.to_string());
                }
            }
        }
        Ok(Self { path, cache, hits: 0, misses: 0 })
    }

    /// Run `code` over `ops`, returning a cached artifact JSON on a hit or
    /// computing + persisting it on a miss. The return tuple is
    /// (artifact_json, was_hit) so callers can report the dedup ratio.
    pub fn run_cached(&mut self, code: &dyn ActCode, ops: &[SdfOp]) -> std::io::Result<(String, bool)> {
        let key = ledger_key(code, ops);
        if let Some(v) = self.cache.get(&key) {
            self.hits += 1;
            return Ok((v.clone(), true));
        }
        self.misses += 1;
        let artifact = code.run(ops);
        let json = artifact.to_json();
        let mut f = OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{}\t{}", key, json)?;
        self.cache.insert(key, json.clone());
        Ok((json, false))
    }

    pub fn entry_count(&self) -> usize { self.cache.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_spheres() -> Vec<SdfOp> {
        vec![
            SdfOp::Sphere { center: [-0.7, 0.0, 0.0], radius: 0.7 },
            SdfOp::Sphere { center: [0.7, 0.0, 0.0], radius: 0.7 },
            SdfOp::Smin { k: 5.0 },
        ]
    }

    #[test]
    fn eval_matches_primitive_at_known_points() {
        // Single sphere radius 1 at origin : distance at (2,0,0) is 1.
        let ops = vec![SdfOp::Sphere { center: [0.0; 3], radius: 1.0 }];
        assert!((eval_scene(&ops, [2.0, 0.0, 0.0]) - 1.0).abs() < 1e-9);
        assert!((eval_scene(&ops, [0.0, 0.0, 0.0]) + 1.0).abs() < 1e-9);
    }

    #[test]
    fn diff_removes_material() {
        // Big sphere minus small sphere → point inside the hole is outside.
        let ops = vec![
            SdfOp::Sphere { center: [0.0; 3], radius: 1.0 },
            SdfOp::Sphere { center: [0.0; 3], radius: 0.5 },
            SdfOp::Diff,
        ];
        assert!(eval_scene(&ops, [0.0, 0.0, 0.0]) > 0.0); // hollow centre
        assert!(eval_scene(&ops, [0.75, 0.0, 0.0]) < 0.0); // in the shell
    }

    #[test]
    fn ledger_key_is_stable_and_param_sensitive() {
        let a = two_spheres();
        let mut b = a.clone();
        b[2] = SdfOp::Smin { k: 6.0 }; // different blend
        let code = inertia::InertiaActCode::default();
        let ka = ledger_key(&code, &a);
        let ka2 = ledger_key(&code, &a);
        let kb = ledger_key(&code, &b);
        assert_eq!(ka, ka2, "same scene → same key");
        assert_ne!(ka, kb, "different op param → different key");
        assert_eq!(ka.len(), 32, "16-byte hex prefix");
    }

    #[test]
    fn aabb_bounds_two_spheres() {
        let (lo, hi) = scene_aabb(&two_spheres());
        assert!(lo[0] <= -1.4 && hi[0] >= 1.4);
    }
}
