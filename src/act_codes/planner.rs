//! Planner — decompose an engineering body into independent sub-parts,
//! run each through the ledger, and recombine the artifacts.
//!
//! This is the layer that realises the user's workflow step 2 : "il y a
//! sûrement plusieurs parties à travailler indépendamment ; chaque partie
//! doit passer par une batterie de tests". A body is described as a list
//! of named, self-contained `SubPart`s (each a complete SDF op list with
//! its own material density). The planner :
//!
//!   - hashes each sub-part INDEPENDENTLY → mutating one component only
//!     re-runs the act codes that touch it, every other sub-part is served
//!     from the ledger at 0 cost ;
//!   - runs the requested act codes per sub-part ;
//!   - recombines the per-part inertia into a whole-body tensor via the
//!     parallel-axis (Huygens–Steiner) theorem — exact for a heterogeneous
//!     assembly without the SDF carrying material tags.

use super::inertia::InertiaActCode;
use super::{ActLedger, SdfOp, Vec3};

/// One independently-analysable component of a body.
#[derive(Clone, Debug)]
pub struct SubPart {
    pub label: String,
    /// Material density (kg/m³).
    pub density: f64,
    /// Self-contained SDF op list yielding this part as a standalone solid.
    pub ops: Vec<SdfOp>,
}

/// Per-part result plus whether it was served from the ledger.
#[derive(Clone, Debug)]
pub struct PartResult {
    pub label: String,
    pub mass: f64,
    pub com: Vec3,
    pub tensor: [f64; 6],
    pub from_cache: bool,
}

/// Consolidated whole-body mass properties + the dedup statistics that
/// prove the KASM ledger is doing its job.
#[derive(Clone, Debug)]
pub struct PlanReport {
    pub parts: Vec<PartResult>,
    pub total_mass: f64,
    pub global_com: Vec3,
    pub global_tensor: [f64; 6],
    pub hits: u64,
    pub misses: u64,
}

/// Parse the inertia fields back out of the ledger's JSON artifact line.
/// We wrote it, so the format is fixed — a tiny hand parser avoids pulling
/// in a JSON dependency (Forge zero-dep doctrine).
fn parse_inertia(json: &str) -> Option<(f64, Vec3, [f64; 6])> {
    let mass = grab_number(json, "\"mass\":")?;
    let com = grab_array(json, "\"com\":[", 3)?;
    let t = grab_array(json, "\"tensor\":[", 6)?;
    Some((mass, [com[0], com[1], com[2]], [t[0], t[1], t[2], t[3], t[4], t[5]]))
}

fn grab_number(s: &str, key: &str) -> Option<f64> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let end = rest.find(|c: char| c == ',' || c == '}' || c == ']').unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
}

fn grab_array(s: &str, key: &str, n: usize) -> Option<Vec<f64>> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let end = rest.find(']')?;
    let vals: Vec<f64> = rest[..end].split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if vals.len() == n { Some(vals) } else { None }
}

/// Run the inertia act code over every sub-part (with each part's density),
/// then recombine into a whole-body tensor via the parallel-axis theorem.
///
/// `samples` is the Monte-Carlo budget per part ; `seed` seeds the shared
/// deterministic RNG lineage. The ledger memoises per-part, so calling this
/// twice — or after mutating a single component — re-runs only what changed.
pub fn run_inertia_plan(
    ledger: &mut ActLedger,
    parts: &[SubPart],
    samples: u32,
    seed: u64,
) -> std::io::Result<PlanReport> {
    let mut results = Vec::with_capacity(parts.len());
    let h0 = ledger.hits;
    let m0 = ledger.misses;

    for part in parts {
        let code = InertiaActCode::with(samples, part.density, seed);
        let (json, from_cache) = ledger.run_cached(&code, &part.ops)?;
        let (mass, com, tensor) = parse_inertia(&json).unwrap_or((0.0, [0.0; 3], [0.0; 6]));
        results.push(PartResult { label: part.label.clone(), mass, com, tensor, from_cache });
    }

    // Whole-body COM = mass-weighted mean of part COMs.
    let total_mass: f64 = results.iter().map(|r| r.mass).sum();
    let mut gcom = [0.0f64; 3];
    if total_mass > 0.0 {
        for r in &results {
            for i in 0..3 { gcom[i] += r.mass * r.com[i]; }
        }
        for i in 0..3 { gcom[i] /= total_mass; }
    }

    // Whole-body tensor = Σ [ part tensor (about its own COM) + Huygens
    // shift from the part COM to the global COM ].
    let mut g = [0.0f64; 6]; // [Ixx, Iyy, Izz, Ixy, Ixz, Iyz]
    for r in &results {
        let d = [r.com[0] - gcom[0], r.com[1] - gcom[1], r.com[2] - gcom[2]];
        g[0] += r.tensor[0] + r.mass * (d[1]*d[1] + d[2]*d[2]);
        g[1] += r.tensor[1] + r.mass * (d[0]*d[0] + d[2]*d[2]);
        g[2] += r.tensor[2] + r.mass * (d[0]*d[0] + d[1]*d[1]);
        g[3] += r.tensor[3] - r.mass * d[0] * d[1];
        g[4] += r.tensor[4] - r.mass * d[0] * d[2];
        g[5] += r.tensor[5] - r.mass * d[1] * d[2];
    }

    Ok(PlanReport {
        parts: results,
        total_mass,
        global_com: gcom,
        global_tensor: g,
        hits: ledger.hits - h0,
        misses: ledger.misses - m0,
    })
}

impl PlanReport {
    /// Serialise the whole report to ASCII JSON for the physics file the
    /// LLM reads back to mutate the SDF.
    pub fn to_json(&self) -> String {
        let mut parts_json = Vec::with_capacity(self.parts.len());
        for p in &self.parts {
            parts_json.push(format!(
                "{{\"label\":\"{}\",\"mass\":{},\"com\":[{},{},{}],\"from_cache\":{}}}",
                p.label, super::fj(p.mass),
                super::fj(p.com[0]), super::fj(p.com[1]), super::fj(p.com[2]),
                p.from_cache,
            ));
        }
        format!(
            "{{\"total_mass\":{},\"global_com\":[{},{},{}],\"global_tensor\":[{},{},{},{},{},{}],\"ledger\":{{\"hits\":{},\"misses\":{}}},\"parts\":[{}]}}",
            super::fj(self.total_mass),
            super::fj(self.global_com[0]), super::fj(self.global_com[1]), super::fj(self.global_com[2]),
            super::fj(self.global_tensor[0]), super::fj(self.global_tensor[1]), super::fj(self.global_tensor[2]),
            super::fj(self.global_tensor[3]), super::fj(self.global_tensor[4]), super::fj(self.global_tensor[5]),
            self.hits, self.misses,
            parts_json.join(","),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two identical unit masses at ±x : COM at origin, and the parallel-
    /// axis recombination must produce a non-zero Izz that exceeds the sum
    /// of the individual (about-own-COM) tensors — proving the shift fires.
    #[test]
    fn two_spheres_combine_with_parallel_axis() {
        let mk = |x: f64| SubPart {
            label: format!("s{}", x),
            density: 1000.0,
            ops: vec![SdfOp::Sphere { center: [x, 0.0, 0.0], radius: 0.2 }],
        };
        let parts = vec![mk(-0.5), mk(0.5)];
        let dir = std::env::temp_dir().join("forge_act_test_pa.jsonl");
        let _ = std::fs::remove_file(&dir);
        let mut ledger = ActLedger::open(&dir).unwrap();
        let rep = run_inertia_plan(&mut ledger, &parts, 400_000, 1).unwrap();

        assert!(rep.global_com[0].abs() < 0.01, "symmetric COM should be ~0");
        // Each sphere's own Izz is small ; the displaced combination is
        // dominated by m d² terms → much larger.
        let per_part_izz_sum: f64 = rep.parts.iter().map(|p| p.tensor[2]).sum();
        assert!(rep.global_tensor[2] > per_part_izz_sum * 2.0, "parallel axis should dominate");
        let _ = std::fs::remove_file(&dir);
    }

    /// The same plan run twice : second run is 100% ledger hits.
    #[test]
    fn second_run_is_all_cache_hits() {
        let parts = vec![SubPart {
            label: "ball".into(),
            density: 1000.0,
            ops: vec![SdfOp::Sphere { center: [0.0; 3], radius: 0.3 }],
        }];
        let dir = std::env::temp_dir().join("forge_act_test_cache.jsonl");
        let _ = std::fs::remove_file(&dir);
        let mut ledger = ActLedger::open(&dir).unwrap();

        let first = run_inertia_plan(&mut ledger, &parts, 300_000, 9).unwrap();
        assert_eq!(first.misses, 1, "first run computes");
        assert_eq!(first.hits, 0);

        let second = run_inertia_plan(&mut ledger, &parts, 300_000, 9).unwrap();
        assert_eq!(second.hits, 1, "second run is a hit");
        assert_eq!(second.misses, 0, "nothing recomputed");
        let _ = std::fs::remove_file(&dir);
    }
}
