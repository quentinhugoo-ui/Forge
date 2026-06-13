//! forge_pack_run — the CLI the ActCode / Atlas dropdown calls to launch a pack.
//!
//! Usage :
//!   cargo run --example forge_pack_run --release -- --catalog
//!       → prints the token-cheap pack index (id, domain, calc count, desc).
//!         This is ALL the LLM ever reads before choosing a pack.
//!
//!   cargo run --example forge_pack_run --release -- <pack_id>
//!       → expands the pack into its (dozens of) computations and runs them
//!         over the drone geometry through the content-addressed ledger.
//!         Forge does the thousands of lines of math internally ; the caller
//!         spends zero tokens reading them.
//!
//! The ActCode server wraps this : `forge.run_pack(pack_id, geometry)` → report.

use scan::act_codes::pack::{catalog_json, find, run_pack};
use scan::act_codes::{ActLedger, SdfOp};

/// Drone cage hollow shell — a geometry meaningful to every domain pack
/// (it has mass for rigid-body, modes for structural, and is the thermal
/// enclosure). Matches the optimum from forge_drone_design.
fn drone_cage() -> Vec<SdfOp> {
    vec![
        SdfOp::Sphere { center: [0.0, 0.0, 0.0], radius: 0.104878 },
        SdfOp::Sphere { center: [0.0, 0.0, 0.0], radius: 0.102878 },
        SdfOp::Diff,
    ]
}

/// Solid inner medium (for the thermal enclosure — the air+electronics
/// volume the heat diffuses through).
fn drone_inner() -> Vec<SdfOp> {
    vec![SdfOp::Sphere { center: [0.0, 0.0, 0.0], radius: 0.102878 }]
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "--catalog" {
        // Token-cheap index — what the LLM reads to choose a pack.
        println!("{}", catalog_json());
        return Ok(());
    }

    let pack_id = &args[0];
    let pack = match find(pack_id) {
        Some(p) => p,
        None => {
            eprintln!("[forge_pack_run] unknown pack '{}'. Try --catalog.", pack_id);
            std::process::exit(2);
        }
    };

    // Thermal pack runs over the solid inner medium ; the others over the
    // cage shell. (A real ActCode call passes the geometry explicitly.)
    let ops = if pack.id == "thermal" { drone_inner() } else { drone_cage() };

    let ledger_path = std::path::PathBuf::from("examples/forge_drone_design.ledger.jsonl");
    let mut ledger = ActLedger::open(&ledger_path)?;

    let t0 = std::time::Instant::now();
    let report = run_pack(&mut ledger, &pack, &ops)?;
    let cold_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // Re-run to demonstrate the KASM dedup (warm = all cache).
    let t1 = std::time::Instant::now();
    let warm = run_pack(&mut ledger, &pack, &ops)?;
    let warm_ms = t1.elapsed().as_secs_f64() * 1000.0;

    println!("=== pack '{}' ({}) ===", pack.id, pack.domain);
    println!("  {}", pack.description);
    println!("  computations : {}", report.total());
    println!("  cold run     : {} hits, {} misses, {:.1} ms", report.hits, report.misses, cold_ms);
    println!("  warm re-run  : {} hits, {} misses, {:.1} ms (KASM dedup)", warm.hits, warm.misses, warm_ms);
    if cold_ms > 0.0 {
        println!("  speedup      : x{:.0} on re-run", cold_ms / warm_ms.max(0.001));
    }

    // Show a few representative artifacts so the operator sees the spread.
    println!("  sample artifacts:");
    for (id, json, _) in report.runs.iter().take(4) {
        let preview: String = json.chars().take(96).collect();
        println!("    [{}] {}", id, preview);
    }
    if report.runs.len() > 4 {
        println!("    ... and {} more", report.runs.len() - 4);
    }

    Ok(())
}
