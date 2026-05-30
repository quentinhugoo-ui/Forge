//! forge_drone_pipeline — the full Act-Code workflow on the spherical drone.
//!
//! Demonstrates the user's 3-step loop end-to-end :
//!   1. (LLM upstream) a natural-language drone request became drone v1.
//!   2. THIS binary decomposes the drone into independent material sub-parts,
//!      runs the inertia act code per part through the content-addressed
//!      ledger (KASM dedup), and recombines into whole-body mass properties.
//!   3. Emits drone_v1.physics.json + viability flags the LLM reads back to
//!      mutate the SDF.
//!
//! It also proves the dedup at two levels :
//!   - a second identical run is 100% ledger hits (0 recompute) ;
//!   - mutating ONE component (weight radius) re-runs only that part — every
//!     other sub-part is served from the ledger.
//!
//! Run : cargo run --example forge_drone_pipeline --release

use scan::act_codes::modal::ModalActCode;
use scan::act_codes::planner::{run_inertia_plan, PlanReport, SubPart};
use scan::act_codes::{ActCode, ActLedger, Artifact, SdfOp};

// Drone v1 optimum from forge_drone_design (score 0 on all constraints).
const CAGE_OUTER: f64 = 0.104878;
const CAGE_INNER: f64 = 0.102878;
const PROP_RADIUS: f64 = 0.028786;
const PROP_RING_R: f64 = 0.052812;
const PROP_RING_Z: f64 = -0.008119;
const WEIGHT_R_DEFAULT: f64 = 0.028228;
const WEIGHT_Z: f64 = -0.06965;
const RPI_HALF: f64 = 0.018233;
const CAM_R: f64 = 0.010065;
const CAM_X: f64 = 0.089813;
const WIFI_BASE_Z: f64 = 0.101878;
const WIFI_TIP_Z: f64 = 0.132779;

// Material densities (kg/m³).
const RHO_ABS: f64 = 1050.0;   // cage, propellers
const RHO_BRASS: f64 = 8500.0; // stabilising weight
const RHO_PCB: f64 = 1600.0;   // raspberry pi
const RHO_GLASS: f64 = 2500.0; // camera lens
const RHO_FOAM: f64 = 900.0;   // wifi antenna shell

/// Decompose the drone into independent, self-contained sub-parts. Each is a
/// standalone solid (no smin folding) so its inertia can be hashed and
/// computed in isolation, then recombined by the planner.
fn drone_subparts(weight_r: f64) -> Vec<SubPart> {
    let mut parts = Vec::new();

    // Cage — hollow shell.
    parts.push(SubPart {
        label: "cage".into(),
        density: RHO_ABS,
        ops: vec![
            SdfOp::Sphere { center: [0.0, 0.0, 0.0], radius: CAGE_OUTER },
            SdfOp::Sphere { center: [0.0, 0.0, 0.0], radius: CAGE_INNER },
            SdfOp::Diff,
        ],
    });

    // 4 propellers on the ring at 90°.
    for k in 0..4u32 {
        let theta = (k as f64) * std::f64::consts::FRAC_PI_2;
        let cx = PROP_RING_R * theta.cos();
        let cy = PROP_RING_R * theta.sin();
        let tx = -(theta.sin()) * PROP_RADIUS;
        let ty = theta.cos() * PROP_RADIUS;
        parts.push(SubPart {
            label: format!("prop{}", k),
            density: RHO_ABS,
            ops: vec![SdfOp::Capsule {
                a: [cx + tx, cy + ty, PROP_RING_Z],
                b: [cx - tx, cy - ty, PROP_RING_Z],
                radius: 0.0035,
            }],
        });
    }

    // Stabilising weight (brass) — the param the LLM mutates in the demo.
    parts.push(SubPart {
        label: "weight".into(),
        density: RHO_BRASS,
        ops: vec![SdfOp::Sphere { center: [0.0, 0.0, WEIGHT_Z], radius: weight_r }],
    });

    // Raspberry Pi.
    parts.push(SubPart {
        label: "rpi".into(),
        density: RHO_PCB,
        ops: vec![SdfOp::RoundedBox {
            center: [0.0, 0.0, 0.0],
            half_extents: [RPI_HALF, RPI_HALF, RPI_HALF * 0.4],
            corner_radius: 0.003,
        }],
    });

    // Camera lens.
    parts.push(SubPart {
        label: "camera".into(),
        density: RHO_GLASS,
        ops: vec![SdfOp::Sphere { center: [CAM_X, 0.0, 0.0], radius: CAM_R }],
    });

    // WiFi antenna.
    parts.push(SubPart {
        label: "wifi".into(),
        density: RHO_FOAM,
        ops: vec![SdfOp::Capsule {
            a: [0.0, 0.0, WIFI_BASE_Z],
            b: [0.0, 0.0, WIFI_TIP_Z],
            radius: 0.0025,
        }],
    });

    parts
}

/// Viability flags derived from the consolidated inertia. These are what the
/// LLM reads to decide whether (and how) to mutate the SDF.
fn viability_flags(rep: &PlanReport) -> Vec<String> {
    let mut flags = Vec::new();

    // Passive stability : the COM should sit BELOW the geometric centre
    // (z=0) so the cage behaves like a pendulum and self-rights. Margin in
    // millimetres.
    let com_z_mm = rep.global_com[2] * 1000.0;
    if rep.global_com[2] < -0.005 {
        flags.push(format!(
            "PASS passive-stability : COM {:.1} mm below centre (pendulum self-rights)",
            -com_z_mm
        ));
    } else {
        flags.push(format!(
            "WARN passive-stability : COM only {:.1} mm below centre — add weight or lower it",
            -com_z_mm
        ));
    }

    // Hover thrust (4 props, 18 g/cm² at 60% throttle — same model as the
    // designer) must beat 1.4× total mass.
    let area_cm2 = std::f64::consts::PI * (PROP_RADIUS * 100.0).powi(2);
    let hover_kg = 4.0 * area_cm2 * 18.0 / 1000.0;
    let need = rep.total_mass * 1.4;
    if hover_kg >= need {
        flags.push(format!(
            "PASS thrust : hover {:.3} kg >= 1.4x mass {:.3} kg (margin x{:.2})",
            hover_kg, need, hover_kg / rep.total_mass
        ));
    } else {
        flags.push(format!(
            "FAIL thrust : hover {:.3} kg < 1.4x mass {:.3} kg — lighten or bigger props",
            hover_kg, need
        ));
    }

    // Yaw-vs-pitch agility ratio : Izz / Ixx. A ratio near 1 is sluggish in
    // yaw ; drones want Izz < Ixx for snappy heading changes.
    let ixx = rep.global_tensor[0];
    let izz = rep.global_tensor[2];
    if ixx > 0.0 {
        let ratio = izz / ixx;
        flags.push(format!(
            "INFO agility : Izz/Ixx = {:.3} ({})",
            ratio,
            if ratio < 1.1 { "yaw-agile" } else { "yaw-sluggish — narrow the prop ring" }
        ));
    }

    flags
}

fn print_report(tag: &str, rep: &PlanReport) {
    println!("--- {} ---", tag);
    println!("  total mass    : {:.4} kg", rep.total_mass);
    println!(
        "  global COM    : [{:.4}, {:.4}, {:.4}] m",
        rep.global_com[0], rep.global_com[1], rep.global_com[2]
    );
    println!(
        "  inertia (kg.m2): Ixx={:.6} Iyy={:.6} Izz={:.6}",
        rep.global_tensor[0], rep.global_tensor[1], rep.global_tensor[2]
    );
    println!("  ledger        : {} hits, {} misses", rep.hits, rep.misses);
    for p in &rep.parts {
        println!(
            "    {:<8} m={:.5} kg  com.z={:+.4}  {}",
            p.label, p.mass, p.com[2],
            if p.from_cache { "[cache]" } else { "[compute]" }
        );
    }
}

fn main() -> std::io::Result<()> {
    // Persistent ledger so dedup survives across process runs too.
    let ledger_path = std::path::PathBuf::from("examples/forge_drone_design.ledger.jsonl");
    let mut ledger = ActLedger::open(&ledger_path)?;
    let samples = 1_500_000u32;
    let seed = 0xD0E_u64;

    // --- Step 2 : decompose + run the inertia battery -----------------------
    let parts_v1 = drone_subparts(WEIGHT_R_DEFAULT);
    let cold = run_inertia_plan(&mut ledger, &parts_v1, samples, seed)?;
    print_report("drone v1 (cold or warm depending on prior runs)", &cold);

    // --- Dedup proof : identical re-run is all hits -------------------------
    let warm = run_inertia_plan(&mut ledger, &parts_v1, samples, seed)?;
    print_report("drone v1 re-run (KASM dedup)", &warm);
    assert_eq!(warm.misses, 0, "identical re-run must be 100% ledger hits");

    // --- LLM mutation : bump the weight radius +1 mm ------------------------
    // Only the `weight` sub-part hash changes → only it recomputes ; cage,
    // props, rpi, camera, wifi are served from the ledger untouched.
    let parts_v2 = drone_subparts(WEIGHT_R_DEFAULT + 0.001);
    let mutated = run_inertia_plan(&mut ledger, &parts_v2, samples, seed)?;
    print_report("drone v2 (weight +1mm) — partial recompute", &mutated);
    assert_eq!(
        mutated.misses, 1,
        "mutating one component must recompute exactly one sub-part"
    );

    // --- modal × hélices : the flutter cross-check --------------------------
    // Run the modal act code on the CAGE (the resonating structural shell).
    // ABS wave speed c = sqrt(E/rho) ~ sqrt(2.3e9/1050) ~ 1480 m/s.
    let cage_ops = drone_subparts(WEIGHT_R_DEFAULT)[0].ops.clone();
    let modal_code = ModalActCode::with(64, 6, 1480.0, 60);
    let (modal_json, modal_cached) = ledger.run_cached(&modal_code, &cage_ops)?;
    let cage_modes = match modal_code.run(&cage_ops) {
        Artifact::Scalars { values, .. } => values,
        _ => vec![],
    };
    // Blade-pass band : 2-blade props at the hover RPM range 8k-12k.
    // f_blade = RPM/60 * blades. We flag any cage mode within +/-15% of any
    // blade-pass frequency the rotor will sweep through on spin-up.
    let blade_count = 2.0;
    let rpm_lo = 8000.0;
    let rpm_hi = 12000.0;
    let fb_lo = rpm_lo / 60.0 * blade_count;
    let fb_hi = rpm_hi / 60.0 * blade_count;
    println!("\n=== modal x props (flutter) ===");
    println!(
        "  cage modes (Hz): [{}]  {}",
        cage_modes.iter().map(|f| format!("{:.0}", f)).collect::<Vec<_>>().join(", "),
        if modal_cached { "[cache]" } else { "[compute]" }
    );
    println!("  blade-pass band : {:.0}-{:.0} Hz (2 blades, 8k-12k RPM)", fb_lo, fb_hi);
    let mut flutter_hit = None;
    for &f in &cage_modes {
        if f >= fb_lo * 0.85 && f <= fb_hi * 1.15 {
            flutter_hit = Some(f);
            break;
        }
    }
    let flutter_flag = match flutter_hit {
        Some(f) => format!("FAIL flutter : cage mode {:.0} Hz inside blade-pass band — stiffen cage or change RPM", f),
        None => "PASS flutter : no cage mode in the blade-pass band".to_string(),
    };
    println!("  {}", flutter_flag);
    let _ = modal_json;

    // --- Step 3 : viability flags for the LLM -------------------------------
    println!("\n=== viability flags (drone v2) ===");
    let mut flags = viability_flags(&mutated);
    flags.push(flutter_flag);
    for f in &flags {
        println!("  {}", f);
    }

    // Consolidated physics JSON the LLM reads back to rewrite the SDF.
    let flags_json: Vec<String> = flags.iter().map(|f| format!("\"{}\"", f.replace('"', "'"))).collect();
    let report_json = format!(
        "{{\n  \"tool\":\"forge_drone_pipeline\",\n  \"drone\":\"v2-weight+1mm\",\n  \"physics\":{},\n  \"flags\":[{}]\n}}\n",
        mutated.to_json(),
        flags_json.join(",")
    );
    let out_path = std::path::PathBuf::from("examples/forge_drone_pipeline.out.json");
    std::fs::write(&out_path, &report_json)?;

    println!(
        "\n[forge_drone_pipeline] ledger now holds {} entries → {}",
        ledger.entry_count(),
        ledger_path.display()
    );
    println!("[forge_drone_pipeline] wrote {}", out_path.display());
    Ok(())
}
