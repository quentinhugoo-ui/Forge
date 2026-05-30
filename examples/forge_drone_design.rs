//! forge_drone_design — Computational engineering search for a spherical
//! drone whose propellers stay inside the protective cage.
//!
//! Doctrine : Forge crate fait le calcul (random-restart hill-climb sur
//! contraintes géométriques + masse), zéro round-trip LLM, output JSON
//! directement consommable par la scène INGEN Banger
//! (examples/forge_tauri_ui/ui/src/sections/banger/drone-scene.ts).
//!
//! Run :   cargo run --example forge_drone_design --release
//! Output : examples/forge_drone_design.out.json   (params + SDF ops)
//!
//! Components (each scored against viability constraints) :
//!   - spherical cage (hollow shell)             : outer/inner radius
//!   - 4 propellers (capsules tangent to inner)  : disc radius, ring Z
//!   - weight ball (passive stability)           : radius, bottom-anchored
//!   - Raspberry Pi (rounded box)                : edge length
//!   - camera (small sphere)                     : on equator, +X side
//!   - WiFi antenna (capsule)                    : top, +Z
//!
//! Each design lowers a scalar penalty over geometric clearance + mass
//! payload + protective-cage compliance. The winning record is
//! serialised as a drone-scene SdfOp[] for the WGSL INGEN raymarcher.

use std::f64::consts::PI;
use std::fs;
use std::path::PathBuf;

// ----- ASCII-only number formatting (Forge stdout discipline) ---------------

fn f(v: f64) -> String {
    // Six significant digits, no Unicode minus, JSON-friendly.
    if v.abs() < 1e-9 { return "0".into(); }
    let s = format!("{:.6}", v);
    // Trim trailing zeros except keep at least one digit after the dot.
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() { "0".into() } else { trimmed.into() }
}

// ----- Parametric drone design ----------------------------------------------

#[derive(Clone, Debug)]
struct Drone {
    cage_outer_r:   f64, // outer sphere radius (m)
    cage_inner_r:   f64, // inner sphere radius (m)  — cavity
    prop_radius:    f64, // propeller swept disc radius (m)
    prop_ring_r:    f64, // ring radius — distance from drone centre to each prop centre (m)
    prop_ring_z:    f64, // propeller disc Z coordinate (m), local frame
    prop_count:     u32, // always 4 here ; left as a field for future quads/hex
    weight_radius:  f64, // stabilising mass radius (m)
    rpi_edge:       f64, // raspberry pi rounded box side (m)
    cam_radius:     f64, // camera lens sphere (m)
    wifi_length:    f64, // wifi antenna capsule length (m)
}

#[derive(Clone, Debug)]
struct Score {
    total:          f64,
    cage_thickness: f64,
    prop_clearance: f64,
    prop_overlap:   f64,
    weight_seat:    f64,
    rpi_fit:        f64,
    cam_fit:        f64,
    wifi_fit:       f64,
    mass_payload:   f64,
}

impl Drone {
    /// Total mass (kg) under simple densities matching common hobby parts.
    fn mass_kg(&self) -> f64 {
        let cage_vol = (4.0/3.0) * PI * (self.cage_outer_r.powi(3) - self.cage_inner_r.powi(3));
        let weight_vol = (4.0/3.0) * PI * self.weight_radius.powi(3);
        let rpi_vol  = self.rpi_edge.powi(3) * 0.55; // PCB foam factor
        let cam_vol  = (4.0/3.0) * PI * self.cam_radius.powi(3);
        let wifi_vol = PI * (0.002_f64).powi(2) * self.wifi_length;
        // Densities (kg/m^3) : ABS cage 1050, brass weight 8500, PCB+chips
        // 1600, glass camera 2500, antenna foam-shell 900.
        cage_vol * 1050.0
            + weight_vol * 8500.0
            + rpi_vol * 1600.0
            + cam_vol * 2500.0
            + wifi_vol * 900.0
    }

    /// Hover thrust at 60% throttle on 4 props : approximation
    /// T = 0.5 * rho * A * (k * RPM * r)^2 per prop. We collapse the
    /// constants into a single empirical coefficient so the search has
    /// a single tunable "thrust budget" knob.
    fn hover_thrust_kg(&self) -> f64 {
        // 4 props of swept area pi*r^2, empirical 18 g/cm^2 at 60% throttle.
        let area_cm2 = PI * (self.prop_radius * 100.0).powi(2);
        let per_prop_g = area_cm2 * 18.0;
        4.0 * per_prop_g / 1000.0
    }

    /// Score : 0 = perfect, larger = worse. Convex penalties on each
    /// constraint violation so the hill-climber has a well-shaped gradient.
    fn score(&self) -> Score {
        let safety = 0.005_f64; // 5 mm clearance everywhere
        // Cage must have a real wall (printable >= 2 mm).
        let cage_thickness = ((0.002 - (self.cage_outer_r - self.cage_inner_r)).max(0.0)).powi(2) * 1e6;

        // Propeller disc must fit inside the cage interior with safety
        // margin. Farthest point of the spinning disc = ring_r + prop_r
        // (centre on the ring, swept out by prop_r).
        let prop_far = self.prop_ring_r + self.prop_radius;
        let prop_clearance = ((prop_far - (self.cage_inner_r - safety)).max(0.0)).powi(2) * 1e6;

        // 4 props at 90 deg on a ring of radius ring_r. Chord between
        // adjacent prop centres = ring_r * sqrt(2). Must clear by
        // 2 * prop_radius + safety so the discs never touch.
        let center_dist = self.prop_ring_r * (2.0_f64).sqrt();
        let need = 2.0 * self.prop_radius + safety;
        let prop_overlap = ((need - center_dist).max(0.0)).powi(2) * 1e6;

        // Weight must sit at the bottom of the cavity for passive stability
        // — its center at z = -(cage_inner_r - weight_radius - safety).
        let weight_seat_target = -(self.cage_inner_r - self.weight_radius - safety);
        let weight_seat = (weight_seat_target.abs() - (self.cage_inner_r - safety)).max(0.0).powi(2) * 1e6;

        // RPi sits at the geometric center, must fit inside the cavity
        // diagonally : half_diag = rpi_edge * sqrt(3) / 2 <= cage_inner_r - safety.
        let rpi_half_diag = self.rpi_edge * (3.0_f64).sqrt() * 0.5;
        let rpi_fit = ((rpi_half_diag - (self.cage_inner_r - safety)).max(0.0)).powi(2) * 1e6;

        // Camera lens on +X equator, just inside the cage wall.
        let cam_fit = ((self.cam_radius - (self.cage_inner_r - safety)).max(0.0)).powi(2) * 1e6;

        // WiFi antenna sticks up +Z from the cage top. Its base must be
        // attached to the cage wall ; length is unconstrained but capped
        // for sanity at 0.05 m.
        let wifi_fit = ((self.wifi_length - 0.05).max(0.0)).powi(2) * 1e6
            + ((0.01 - self.wifi_length).max(0.0)).powi(2) * 1e6;

        // Hover thrust must exceed total mass with 1.4x safety factor.
        let thrust_kg = self.hover_thrust_kg();
        let m = self.mass_kg();
        let mass_payload = ((m * 1.4 - thrust_kg).max(0.0)).powi(2) * 1e4;

        let total = cage_thickness + prop_clearance + prop_overlap + weight_seat + rpi_fit + cam_fit + wifi_fit + mass_payload;
        Score { total, cage_thickness, prop_clearance, prop_overlap, weight_seat, rpi_fit, cam_fit, wifi_fit, mass_payload }
    }
}

// ----- Forge-style random-restart hill-climb --------------------------------
//
// Each restart samples a Drone uniformly in plausible hobby-drone ranges,
// then walks small Gaussian perturbations as long as score improves. This
// matches the doctrine's "minimum middlemen" — no external optimiser dep,
// just a deterministic seed + scalar penalty driving the search.

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed | 1) }
    fn next(&mut self) -> u64 {
        // SplitMix64 — small, predictable, good enough for parameter search.
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn u01(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 { lo + (hi - lo) * self.u01() }
    /// Box-Muller normal — pair generation, we discard the second sample.
    fn normal(&mut self, sigma: f64) -> f64 {
        let u1 = self.u01().max(1e-9);
        let u2 = self.u01();
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos() * sigma
    }
}

fn sample_drone(rng: &mut Rng) -> Drone {
    let outer = rng.range(0.06, 0.14);
    let wall  = rng.range(0.003, 0.012);
    Drone {
        cage_outer_r:   outer,
        cage_inner_r:   outer - wall,
        prop_radius:    rng.range(0.012, 0.030),
        prop_ring_r:    rng.range(0.020, 0.080),
        prop_ring_z:    rng.range(-0.02, 0.02),
        prop_count:     4,
        weight_radius:  rng.range(0.005, 0.018),
        rpi_edge:       rng.range(0.030, 0.060),
        cam_radius:     rng.range(0.004, 0.010),
        wifi_length:    rng.range(0.012, 0.040),
    }
}

fn perturb(d: &Drone, rng: &mut Rng, sigma: f64) -> Drone {
    let mut next = d.clone();
    next.cage_outer_r  = (d.cage_outer_r  + rng.normal(sigma * 0.040)).max(0.04);
    next.cage_inner_r  = (d.cage_inner_r  + rng.normal(sigma * 0.040)).max(0.03);
    if next.cage_inner_r >= next.cage_outer_r {
        next.cage_inner_r = next.cage_outer_r - 0.002;
    }
    next.prop_radius   = (d.prop_radius   + rng.normal(sigma * 0.008)).max(0.005);
    next.prop_ring_r   = (d.prop_ring_r   + rng.normal(sigma * 0.010)).max(0.010);
    next.prop_ring_z   =  d.prop_ring_z   + rng.normal(sigma * 0.010);
    next.weight_radius = (d.weight_radius + rng.normal(sigma * 0.005)).max(0.003);
    next.rpi_edge      = (d.rpi_edge      + rng.normal(sigma * 0.010)).max(0.015);
    next.cam_radius    = (d.cam_radius    + rng.normal(sigma * 0.002)).max(0.003);
    next.wifi_length   = (d.wifi_length   + rng.normal(sigma * 0.005)).max(0.010);
    next
}

fn hill_climb(seed: u64, steps: u32) -> (Drone, Score) {
    let mut rng = Rng::new(seed);
    let mut best = sample_drone(&mut rng);
    let mut best_score = best.score();
    let mut sigma = 1.0;
    for i in 0..steps {
        let cand = perturb(&best, &mut rng, sigma);
        let cs = cand.score();
        if cs.total < best_score.total {
            best = cand;
            best_score = cs;
        }
        // Cool the perturbation amplitude — finer search as we converge.
        if i % 64 == 63 { sigma *= 0.85; }
    }
    (best, best_score)
}

// ----- SDF op serialisation matches scenes.ts OP_* ----------------------------
//
// We emit a JSON document whose `sceneOps` array is verbatim consumable by
// the JS side : the banger surface calls __forgeBangerSetScene(scene.ops).
// Op identifiers and field names mirror scenes.ts::SdfOp exactly.

fn drone_scene_ops(d: &Drone) -> String {
    let mut out = String::from("[\n");
    let push = |out: &mut String, line: String| {
        out.push_str("    ");
        out.push_str(&line);
        out.push_str(",\n");
    };

    // 1. Cage : outer sphere minus inner sphere = hollow shell.
    push(&mut out, format!(
        r#"{{"op":"sphere","center":[0,0,0],"radius":{}}}"#,
        f(d.cage_outer_r),
    ));
    push(&mut out, format!(
        r#"{{"op":"sphere","center":[0,0,0],"radius":{}}}"#,
        f(d.cage_inner_r),
    ));
    push(&mut out, r#"{"op":"diff"}"#.into());

    // 2. Four propellers (capsules) at 90 deg around prop_ring_z. Each
    //    capsule lies tangentially : centre on the ring of radius
    //    prop_ring_r, endpoints offset by ±prop_radius along the tangent.
    for k in 0..d.prop_count {
        let theta = (k as f64) * std::f64::consts::FRAC_PI_2;
        let cx = d.prop_ring_r * theta.cos();
        let cy = d.prop_ring_r * theta.sin();
        let tx = -(theta.sin()) * d.prop_radius;
        let ty =   theta.cos()  * d.prop_radius;
        let ax = cx + tx; let ay = cy + ty; let az = d.prop_ring_z;
        let bx = cx - tx; let by = cy - ty; let bz = d.prop_ring_z;
        push(&mut out, format!(
            r#"{{"op":"capsule","a":[{},{},{}],"b":[{},{},{}],"radius":0.0035}}"#,
            f(ax), f(ay), f(az), f(bx), f(by), f(bz),
        ));
        push(&mut out, r#"{"op":"smin","k":0.004}"#.into());
    }

    // 3. Stabilising weight — bottom of cavity.
    let weight_z = -(d.cage_inner_r - d.weight_radius - 0.005);
    push(&mut out, format!(
        r#"{{"op":"sphere","center":[0,0,{}],"radius":{}}}"#,
        f(weight_z), f(d.weight_radius),
    ));
    push(&mut out, r#"{"op":"smin","k":0.005}"#.into());

    // 4. Raspberry Pi rounded box at the geometric centre.
    let half = d.rpi_edge * 0.5;
    push(&mut out, format!(
        r#"{{"op":"roundedBox","center":[0,0,0],"halfExtents":[{},{},{}],"cornerRadius":0.003}}"#,
        f(half), f(half), f(half * 0.4),
    ));
    push(&mut out, r#"{"op":"smin","k":0.004}"#.into());

    // 5. Camera lens, +X equator, flush with the inner wall.
    let cam_x = d.cage_inner_r - d.cam_radius - 0.003;
    push(&mut out, format!(
        r#"{{"op":"sphere","center":[{},0,0],"radius":{}}}"#,
        f(cam_x), f(d.cam_radius),
    ));
    push(&mut out, r#"{"op":"smin","k":0.003}"#.into());

    // 6. WiFi antenna sticking up +Z from the cage top.
    let wifi_base_z = d.cage_outer_r - 0.003;
    let wifi_tip_z  = wifi_base_z + d.wifi_length;
    push(&mut out, format!(
        r#"{{"op":"capsule","a":[0,0,{}],"b":[0,0,{}],"radius":0.0025}}"#,
        f(wifi_base_z), f(wifi_tip_z),
    ));
    push(&mut out, r#"{"op":"smin","k":0.004}"#.into());

    if out.ends_with(",\n") { out.truncate(out.len() - 2); out.push('\n'); }
    out.push_str("  ]");
    out
}

fn main() {
    // Multi-restart : 8 seeds × 4 096 steps each. Total < 50 ms cold run.
    let restarts: u32 = 8;
    let steps:    u32 = 4096;
    let mut best: Option<(Drone, Score)> = None;
    for r in 0..restarts {
        let seed = 0xF06E_5F09_0000_0001_u64.wrapping_add((r as u64) * 0x9E37_79B9);
        let (d, s) = hill_climb(seed, steps);
        let take = match &best { None => true, Some((_, bs)) => s.total < bs.total };
        if take { best = Some((d, s)); }
    }
    let (drone, score) = best.expect("at least one restart");

    let report = format!(
        r#"{{
  "tool":"forge_drone_design",
  "seed_restarts":{},
  "steps_per_restart":{},
  "score":{{
    "total":{},
    "cage_thickness":{},
    "prop_clearance":{},
    "prop_overlap":{},
    "weight_seat":{},
    "rpi_fit":{},
    "cam_fit":{},
    "wifi_fit":{},
    "mass_payload":{}
  }},
  "physics":{{
    "mass_kg":{},
    "hover_thrust_kg":{}
  }},
  "params":{{
    "cage_outer_r":{},
    "cage_inner_r":{},
    "prop_radius":{},
    "prop_ring_r":{},
    "prop_ring_z":{},
    "prop_count":{},
    "weight_radius":{},
    "rpi_edge":{},
    "cam_radius":{},
    "wifi_length":{}
  }},
  "sceneOps":{}
}}
"#,
        restarts, steps,
        f(score.total), f(score.cage_thickness), f(score.prop_clearance), f(score.prop_overlap),
        f(score.weight_seat), f(score.rpi_fit), f(score.cam_fit), f(score.wifi_fit), f(score.mass_payload),
        f(drone.mass_kg()), f(drone.hover_thrust_kg()),
        f(drone.cage_outer_r), f(drone.cage_inner_r), f(drone.prop_radius),
        f(drone.prop_ring_r), f(drone.prop_ring_z),
        drone.prop_count,
        f(drone.weight_radius), f(drone.rpi_edge), f(drone.cam_radius), f(drone.wifi_length),
        drone_scene_ops(&drone),
    );

    let path = PathBuf::from("examples/forge_drone_design.out.json");
    fs::write(&path, &report).expect("write drone design output");
    println!("{}", report);
    eprintln!("[forge_drone_design] wrote {} ({} bytes)", path.display(), report.len());
}
