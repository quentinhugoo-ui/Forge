//! Packs — domain bundles of act codes the LLM launches by name.
//!
//! The Atlas (the MCP dropdown in the left panel) lists *packs*, not
//! individual act codes. A pack groups dozens-to-hundreds of math/test
//! computations that share an engineering domain — structural, thermal,
//! rigid-body, electromagnetics … When the LLM picks a pack it spends a
//! few hundred tokens reading the TINY catalogue (id + domain + one-line
//! description + calc count), then issues a single "run this pack" command.
//! Forge KASM expands the pack into its thousands of constituent
//! computations and runs them internally through the content-addressed
//! ledger — the LLM never reads the thousands of lines of math code, and
//! the dedup means re-running across design variants costs almost nothing.
//!
//! This is the realisation of the user's Atlas vision : "le LLM sait qu'il
//! contient des milliers de lignes de maths mais sans dépense de tokens à
//! les lire, tout ce qu'il fait c'est exécuter ce pack".
//!
//! A pack's `build()` returns boxed `ActCode` trait objects — typically a
//! parameter SWEEP (densities, mesh resolutions, convection regimes…) so
//! one pack run is already dozens of solves ; crossed with the planner's
//! sub-parts and design variants it scales into the millions, and the
//! ledger serves the overwhelming majority from cache.

use super::inertia::InertiaActCode;
use super::modal::ModalActCode;
use super::thermal::{HeatSource, ThermalActCode};
use super::{ActCode, ActLedger, SdfOp};

/// A domain bundle. `build` is a function pointer so the registry is cheap
/// to construct and carries no per-pack heap until a pack is actually run.
pub struct Pack {
    pub id: &'static str,
    pub domain: &'static str,
    pub description: &'static str,
    pub build: fn() -> Vec<Box<dyn ActCode>>,
}

/// Result of running every act code in a pack over one geometry.
pub struct PackReport {
    pub pack_id: String,
    /// (act_code_id, artifact_json, from_cache) per computation.
    pub runs: Vec<(String, String, bool)>,
    pub hits: u64,
    pub misses: u64,
}

impl PackReport {
    pub fn total(&self) -> usize { self.runs.len() }
}

// ---------------------------------------------------------------------------
// Domain packs. Each `build()` is a parameter sweep — the "thousands of
// lines of math" the LLM never reads.
// ---------------------------------------------------------------------------

/// Rigid-body dynamics : mass-properties confidence. Sweeps Monte-Carlo
/// sample counts (convergence) × candidate material densities (sensitivity).
fn rigid_body_pack() -> Vec<Box<dyn ActCode>> {
    // Densities spanning the realistic material palette (foam → brass).
    let densities = [
        900.0, 1050.0, 1250.0, 1400.0, 1600.0, 1850.0, 2500.0,
        2700.0, 4500.0, 7800.0, 8500.0, 11340.0,
    ];
    // Sample-count convergence ladder.
    let samples = [250_000u32, 500_000, 1_000_000, 2_000_000];
    let mut v: Vec<Box<dyn ActCode>> = Vec::new();
    for &d in &densities {
        for &s in &samples {
            v.push(Box::new(InertiaActCode::with(s, d, 0xC0FFEE)));
        }
    }
    v // 12 × 4 = 48 computations
}

/// Structural dynamics : vibration spectrum. Sweeps mesh resolution
/// (convergence) × material wave speed (ABS, PLA, nylon, aluminium, steel,
/// CFRP) × mode count.
fn structural_pack() -> Vec<Box<dyn ActCode>> {
    // c = sqrt(E/rho) for common print/build materials (m/s).
    let wave_speeds = [
        1480.0,  // ABS
        1700.0,  // PLA
        1300.0,  // nylon
        5100.0,  // aluminium
        5900.0,  // steel
        7000.0,  // CFRP (in-plane)
    ];
    let grids = [40u32, 56, 72, 96];
    let mut v: Vec<Box<dyn ActCode>> = Vec::new();
    for &c in &wave_speeds {
        for &g in &grids {
            v.push(Box::new(ModalActCode::with(g, 8, c, (g as f32 * 0.9) as u32 + 24)));
        }
    }
    v // 6 × 4 = 24 computations
}

/// Thermal management : steady-state hotspot. Sweeps the convection regime
/// (sealed → natural → forced → liquid) × source power scenarios (idle →
/// peak compute + motors). Finds the cooling threshold that keeps the RPi
/// below its throttle point.
fn thermal_pack() -> Vec<Box<dyn ActCode>> {
    let h_convs = [4.0, 8.0, 12.0, 20.0, 35.0, 60.0, 100.0]; // sealed → liquid
    // Source scenarios : (rpi_w, motor_w_each).
    let scenarios = [(2.0, 2.0), (3.0, 5.0), (4.0, 8.0), (5.0, 12.0)];
    let mut v: Vec<Box<dyn ActCode>> = Vec::new();
    for &h in &h_convs {
        for &(rpi, motor) in &scenarios {
            // RPi at centre + 4 motors on a 0.053 m ring at z=-0.008.
            let mut sources = vec![HeatSource { pos: [0.0, 0.0, 0.0], watts: rpi, radius: 0.018 }];
            for k in 0..4u32 {
                let theta = (k as f64) * std::f64::consts::FRAC_PI_2;
                sources.push(HeatSource {
                    pos: [0.0528 * theta.cos(), 0.0528 * theta.sin(), -0.008],
                    watts: motor,
                    radius: 0.020,
                });
            }
            v.push(Box::new(ThermalActCode::new(40, 0.17, h, 25.0, sources)));
        }
    }
    v // 7 × 4 = 28 computations
}

/// The static pack registry. Cheap to build (fn pointers only).
pub fn registry() -> Vec<Pack> {
    vec![
        Pack {
            id: "rigid_body",
            domain: "Rigid-Body Dynamics",
            description: "Mass, centre of mass and inertia tensor — Monte-Carlo convergence × material density sweep.",
            build: rigid_body_pack,
        },
        Pack {
            id: "structural",
            domain: "Structural Dynamics",
            description: "Vibration eigenspectrum (flutter avoidance) — mesh-resolution convergence × material wave-speed sweep.",
            build: structural_pack,
        },
        Pack {
            id: "thermal",
            domain: "Thermal Management",
            description: "Steady-state hotspot — convection regime (sealed→liquid) × power-scenario sweep to find the cooling threshold.",
            build: thermal_pack,
        },
    ]
}

/// Look up a pack by id.
pub fn find(id: &str) -> Option<Pack> {
    registry().into_iter().find(|p| p.id == id)
}

/// The token-cheap catalogue the LLM reads. One small JSON object per pack —
/// id, domain, description, and how many computations the pack expands into.
/// The LLM never sees the math code, only this index.
pub fn catalog_json() -> String {
    let mut items = Vec::new();
    for p in registry() {
        let count = (p.build)().len();
        items.push(format!(
            "{{\"id\":\"{}\",\"domain\":\"{}\",\"calcs\":{},\"description\":\"{}\"}}",
            p.id, p.domain, count, p.description
        ));
    }
    format!("{{\"packs\":[{}]}}", items.join(","))
}

/// Run every act code in `pack` over `ops`, routing each through the ledger.
/// Returns the per-computation artifacts plus the dedup statistics.
pub fn run_pack(ledger: &mut ActLedger, pack: &Pack, ops: &[SdfOp]) -> std::io::Result<PackReport> {
    let codes = (pack.build)();
    let h0 = ledger.hits;
    let m0 = ledger.misses;
    let mut runs = Vec::with_capacity(codes.len());
    for code in &codes {
        let (json, from_cache) = ledger.run_cached(code.as_ref(), ops)?;
        runs.push((code.id().to_string(), json, from_cache));
    }
    Ok(PackReport {
        pack_id: pack.id.to_string(),
        runs,
        hits: ledger.hits - h0,
        misses: ledger.misses - m0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ball() -> Vec<SdfOp> {
        vec![SdfOp::Sphere { center: [0.0; 3], radius: 0.05 }]
    }

    #[test]
    fn catalog_is_compact_and_lists_all_packs() {
        let cat = catalog_json();
        assert!(cat.contains("rigid_body"));
        assert!(cat.contains("structural"));
        assert!(cat.contains("thermal"));
        // The catalogue the LLM reads must stay tiny — a few hundred bytes,
        // never the thousands of lines of math behind it.
        assert!(cat.len() < 1200, "catalogue too large: {} bytes", cat.len());
    }

    #[test]
    fn packs_expand_into_many_computations() {
        assert_eq!(rigid_body_pack().len(), 48);
        assert_eq!(structural_pack().len(), 24);
        assert_eq!(thermal_pack().len(), 28);
    }

    #[test]
    fn running_a_pack_then_rerunning_is_all_cache() {
        let dir = std::env::temp_dir().join("forge_pack_test.jsonl");
        let _ = std::fs::remove_file(&dir);
        let mut ledger = ActLedger::open(&dir).unwrap();
        let pack = find("rigid_body").unwrap();

        let first = run_pack(&mut ledger, &pack, &ball()).unwrap();
        assert_eq!(first.total(), 48);
        assert!(first.misses > 0, "cold run computes");

        let second = run_pack(&mut ledger, &pack, &ball()).unwrap();
        assert_eq!(second.hits, 48, "warm run is all cache");
        assert_eq!(second.misses, 0);
        let _ = std::fs::remove_file(&dir);
    }
}
