//! Atlas Cartographie — phase A1 : feasibility test.
//!
//! **Question critique** : l'espace KASM ≤ 4 nœuds est-il sémantiquement
//! compressible avec un ratio ≥ 1000:1 ?
//!
//! - Ratio ≥ 1000:1 → continue → A2 Atlas v0
//! - Ratio < 100:1  → rearchitecture Atlas
//!
//! Méthode :
//!   1. Énumère exhaustivement tous les programmes valides depth ≤4
//!      (input + 1..=4 expr nodes + output) sur un sous-set d'ops
//!      arithmétique i64 + bits.
//!   2. Pour chaque programme valide : `semantic_fingerprint()` → 32 B.
//!   3. Groupe par fingerprint, mesure le ratio (programmes / classes).
//!
//! USAGE :
//!   cargo run --release --example atlas_a1 [-- max_depth]
//!
//! Default max_depth=3 (~40K programs, <2s). max_depth=4 → ~5M, ~5 min.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use scan::kasm::{execute, semantic_fingerprint, Node, Op, Program, Target, Ty};
use scan::Atlas;

/// Φ.μ.7.13 (A2.2) — F64 binary subops. 6 ops totales (Add/Sub/Mul/Div/Min/Max).
/// Skip Exp/Ln (transcendental, non-déterministe libc).
#[derive(Clone, Copy)]
enum F64Bin {
    Add, Sub, Mul, Div, Min, Max,
}
impl F64Bin {
    fn make(self, a: u16, b: u16) -> Node {
        match self {
            Self::Add => Node::f64_add(a, b),
            Self::Sub => Node::f64_sub(a, b),
            Self::Mul => Node::f64_mul(a, b),
            Self::Div => Node::f64_div(a, b),
            Self::Min => Node::f64_min(a, b),
            Self::Max => Node::f64_max(a, b),
        }
    }
}
const F64_BIN_OPS: &[F64Bin] = &[
    F64Bin::Add, F64Bin::Sub, F64Bin::Mul, F64Bin::Div, F64Bin::Min, F64Bin::Max,
];

/// F64 unary subops (3 ops : Sqrt/Abs/Neg). Bijectifs sur leur domaine,
/// total functions via kill-switch KASM (NaN → 0).
#[derive(Clone, Copy)]
enum F64Unary { Sqrt, Abs, Neg }
impl F64Unary {
    fn make(self, a: u16) -> Node {
        match self {
            Self::Sqrt => Node::f64_sqrt(a),
            Self::Abs => Node::f64_abs(a),
            Self::Neg => Node::f64_neg(a),
        }
    }
}
const F64_UNARY_OPS: &[F64Unary] = &[F64Unary::Sqrt, F64Unary::Abs, F64Unary::Neg];

/// F64 constantes explorées (i16 range cast en f64).
const F64_CONSTS: &[i16] = &[-10, -5, -1, 0, 1, 2, 5, 10, 100];

/// Inputs canoniques utilisés par lab_runner (`build_diverse_inputs`).
/// L'atlas pré-calcule outputs sur ces inputs pour permettre un lookup
/// O(1) au runtime.
const LAB_CANONICAL_INPUTS: [i64; 12] = [
    -7, -1, 1, 11, -100, 100, -987, 987, -12345, -50000, 12345, 50000,
];

/// Sous-set d'ops binaires i64 → i64 utiles à l'arithmétique de base.
///
/// **Φ.μ.7.12 — Décision** : op-set expansion testée (13 binary +
/// 2 unary + 17 const → 335K classes, ratio 1136:1, milestone A1 ≥1000:1
/// franchi) mais reverted. Mesure lab_runner -- 10000 :
///   V1 10 ops 131K classes : 1109 iter/sec, total exact 9549
///   V1 étendu 335K classes : 687 iter/sec, total exact 9554
/// Le gain coverage (+3-5 pts wall_random_kasm) est absorbé par la
/// perte throughput (-40%). Total_exact équivalent. Op-set actuel
/// reste optimal jusqu'à ce que phase A3 (depth ≤6) soit atteinte.
const BIN_OPS: &[Op] = &[
    Op::AddI64,
    Op::SubI64,
    Op::MulI64,
    Op::MinI64,
    Op::MaxI64,
    Op::BitAndI64,
    Op::BitOrI64,
    Op::BitXorI64,
    Op::ShlI64,
    Op::ShrI64,
];

/// Constantes immédiates explorées (i16 range pour `ConstI64::imm`).
/// Couvre négatif, zéro, petits, puissances de 2, masques.
const CONSTS: &[i16] = &[-1, 0, 1, 2, 3, 7, 8, 15, 16, 32, 64];

#[derive(Default)]
struct Stats {
    enumerated: u64,
    valid: u64,
    fingerprints: HashMap<[u8; 32], u64>,
    sample_per_class: HashMap<[u8; 32], (u32, u32)>, // (depth, output_index) — pour info
    /// Φ.μ.7.8 — programme canonique le plus PETIT par classe
    /// sémantique + outputs sur LAB_CANONICAL_INPUTS pour index V1.
    /// Storage : (canonical_bytes_size, canonical_bytes, canonical_outputs).
    smallest_per_class: HashMap<[u8; 32], (usize, Vec<u8>, Vec<i64>)>,
}

fn make_node(op: Op, a: u16, b: u16) -> Node {
    match op {
        Op::AddI64 => Node::add(a, b),
        Op::SubI64 => Node::sub(a, b),
        Op::MulI64 => Node::mul(a, b),
        Op::MinI64 => Node::min(a, b),
        Op::MaxI64 => Node::max(a, b),
        Op::BitAndI64 => Node::bit_and(a, b),
        Op::BitOrI64 => Node::bit_or(a, b),
        Op::BitXorI64 => Node::bit_xor(a, b),
        Op::ShlI64 => Node::shl(a, b),
        Op::ShrI64 => Node::shr(a, b),
        _ => unreachable!("not in BIN_OPS"),
    }
}

/// Énumère récursivement les programmes :
///   nodes[0] = Input(0)
///   nodes[1..=depth] = expr nodes
///   nodes[depth+1] = Output(...)
///
/// Pour chaque arrangement, essaie tous les indices de sortie possibles
/// (Input + chaque expr node).
///
/// Φ.μ.7.7 : version PARALLÈLE via `std::thread::scope`. Chaque thread
/// pioche un sous-ensemble des "premier nodes" possibles et énumère
/// le sous-arbre complet à partir de là. Pas de Mutex sur la HashMap —
/// chaque thread maintient son `Stats` local, merge à la fin.
///
/// Φ.μ.7.13 (A2.2) : `with_f64` active la cartographie multi-numeric.
/// Chaque prefix track son type (Vec<Ty>) et seules les combinaisons
/// type-valides sont énumérées (skip wasted programs avant Program::new).
fn enumerate_depth(depth: u32, with_f64: bool, stats: &mut Stats) {
    if depth == 0 {
        return;
    }

    // Pré-calcule tous les "premier nodes" possibles. Un premier node
    // utilise uniquement l'input (slot 0, type I64) comme ref.
    //   - ConstI64 : CONSTS.len() variants → I64
    //   - BinI64Op : BIN_OPS.len() × (a=0, b=0) → I64
    //   - ConstF64 (with_f64) : F64_CONSTS.len() → F64
    //   - F64FromI64 (with_f64) : 1 → F64
    let mut first_nodes: Vec<(Node, Ty)> = Vec::new();
    for &c in CONSTS {
        first_nodes.push((Node::const_i64(c), Ty::I64));
    }
    for &op in BIN_OPS {
        first_nodes.push((make_node(op, 0, 0), Ty::I64));
    }
    if with_f64 {
        for &c in F64_CONSTS {
            first_nodes.push((Node::const_f64(c), Ty::F64));
        }
        // FromI64 sur l'input (seul I64 disponible au premier slot)
        first_nodes.push((Node::f64_from_i64(0), Ty::F64));
    }

    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(first_nodes.len());

    let progress = AtomicU64::new(0);
    let total_first = first_nodes.len() as u64;

    let chunk_size = (first_nodes.len() + n_threads - 1) / n_threads;
    let chunks: Vec<Vec<(Node, Ty)>> = first_nodes
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    let chunk_stats: Vec<Stats> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk in chunks {
            let progress = &progress;
            handles.push(s.spawn(move || {
                let mut local = Stats::default();
                for (first_node, first_ty) in chunk {
                    let mut prefix_nodes = vec![Node::input(0), first_node];
                    let mut prefix_types = vec![Ty::I64, first_ty];
                    if depth >= 2 {
                        enumerate_inner(
                            depth - 1,
                            &mut prefix_nodes,
                            &mut prefix_types,
                            with_f64,
                            &mut local,
                        );
                    } else {
                        finalize(prefix_nodes.clone(), &prefix_types, &mut local);
                    }
                    let done = progress.fetch_add(1, Ordering::Relaxed) + 1;
                    if total_first > 4 && (done % (total_first / 4).max(1) == 0) {
                        eprint!(".");
                    }
                }
                local
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    // Merge des chunks vers le `stats` final.
    for chunk in chunk_stats {
        stats.enumerated += chunk.enumerated;
        stats.valid += chunk.valid;
        for (k, v) in chunk.fingerprints {
            *stats.fingerprints.entry(k).or_default() += v;
        }
        for (k, v) in chunk.sample_per_class {
            stats.sample_per_class.entry(k).or_insert(v);
        }
        // Merge smallest_per_class : on garde le plus petit programme parmi les chunks.
        for (k, (size, bytes, outputs)) in chunk.smallest_per_class {
            stats
                .smallest_per_class
                .entry(k)
                .and_modify(|existing| {
                    if size < existing.0 {
                        *existing = (size, bytes.clone(), outputs.clone());
                    }
                })
                .or_insert((size, bytes, outputs));
        }
    }
}

/// Calcule les outputs d'un programme canonique sur les inputs lab.
/// Si une exécution échoue (extrêmement rare car canonical est valide),
/// retourne 0 pour ce slot.
fn compute_canonical_outputs(prog: &Program) -> Vec<i64> {
    LAB_CANONICAL_INPUTS
        .iter()
        .map(|&x| {
            execute(prog, &x.to_le_bytes())
                .ok()
                .and_then(|bytes| bytes.get(..8).and_then(|c| c.try_into().ok()).map(i64::from_le_bytes))
                .unwrap_or(0)
        })
        .collect()
}

/// Helper : finalise un prefix donné en essayant chaque node comme output.
/// Φ.μ.7.13 : `types[i]` donne le type du node `i` (utilisé pour
/// déterminer le bon Ty pour Output).
fn finalize(prefix: Vec<Node>, types: &[Ty], stats: &mut Stats) {
    let n = prefix.len() as u16;
    for out_idx in 0..n {
        stats.enumerated += 1;
        let out_ty = types[out_idx as usize];
        let mut nodes = prefix.clone();
        nodes.push(Node::output(out_idx, out_ty));
        let nodes_len = nodes.len() as u32;
        let prog = match Program::new(Target::Cpu, 1, 1, nodes_len.max(1024), nodes) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let fp = match semantic_fingerprint(&prog) {
            Ok(f) => f,
            Err(_) => continue,
        };
        stats.valid += 1;
        *stats.fingerprints.entry(fp).or_default() += 1;
        stats
            .sample_per_class
            .entry(fp)
            .or_insert((prefix.len() as u32, out_idx as u32));
        if let Ok(canonical) = prog.canonical() {
            let bytes = canonical.bytes().to_vec();
            let size = bytes.len();
            let outputs = compute_canonical_outputs(&canonical);
            stats
                .smallest_per_class
                .entry(fp)
                .and_modify(|existing| {
                    if size < existing.0 {
                        *existing = (size, bytes.clone(), outputs.clone());
                    }
                })
                .or_insert((size, bytes, outputs));
        }
    }
}

fn enumerate_inner(
    remaining: u32,
    prefix: &mut Vec<Node>,
    types: &mut Vec<Ty>,
    with_f64: bool,
    stats: &mut Stats,
) {
    let n = prefix.len() as u16;

    if remaining == 0 {
        finalize(prefix.clone(), types, stats);
        return;
    }

    // ConstI64 (output Ty::I64)
    for &c in CONSTS {
        prefix.push(Node::const_i64(c));
        types.push(Ty::I64);
        enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
        prefix.pop();
        types.pop();
    }

    // BinaryI64 — a, b ∈ I64 prev nodes
    for &op in BIN_OPS {
        for a in 0..n {
            if types[a as usize] != Ty::I64 { continue; }
            for b in 0..n {
                if types[b as usize] != Ty::I64 { continue; }
                prefix.push(make_node(op, a, b));
                types.push(Ty::I64);
                enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
                prefix.pop();
                types.pop();
            }
        }
    }

    if !with_f64 {
        return;
    }

    // ConstF64 (output Ty::F64)
    for &c in F64_CONSTS {
        prefix.push(Node::const_f64(c));
        types.push(Ty::F64);
        enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
        prefix.pop();
        types.pop();
    }

    // F64 binary — a, b ∈ F64 prev
    for &op in F64_BIN_OPS {
        for a in 0..n {
            if types[a as usize] != Ty::F64 { continue; }
            for b in 0..n {
                if types[b as usize] != Ty::F64 { continue; }
                prefix.push(op.make(a, b));
                types.push(Ty::F64);
                enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
                prefix.pop();
                types.pop();
            }
        }
    }

    // F64 unary — a ∈ F64 prev
    for &op in F64_UNARY_OPS {
        for a in 0..n {
            if types[a as usize] != Ty::F64 { continue; }
            prefix.push(op.make(a));
            types.push(Ty::F64);
            enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
            prefix.pop();
            types.pop();
        }
    }

    // F64FromI64 — a ∈ I64 prev → F64
    for a in 0..n {
        if types[a as usize] != Ty::I64 { continue; }
        prefix.push(Node::f64_from_i64(a));
        types.push(Ty::F64);
        enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
        prefix.pop();
        types.pop();
    }

    // F64ToI64 — a ∈ F64 prev → I64
    for a in 0..n {
        if types[a as usize] != Ty::F64 { continue; }
        prefix.push(Node::f64_to_i64(a));
        types.push(Ty::I64);
        enumerate_inner(remaining - 1, prefix, types, with_f64, stats);
        prefix.pop();
        types.pop();
    }
}

fn main() {
    // Φ.μ.7.8 — args parsing : `[max_depth] [--build PATH] [--with-f64]`
    let args: Vec<String> = std::env::args().collect();
    let mut max_depth: u32 = 3;
    let mut build_path: Option<PathBuf> = None;
    let mut with_f64 = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--build" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --build needs a path argument");
                    std::process::exit(2);
                }
                build_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--with-f64" => {
                with_f64 = true;
                i += 1;
            }
            other => {
                if let Ok(d) = other.parse::<u32>() {
                    max_depth = d;
                }
                i += 1;
            }
        }
    }

    println!("=== Atlas Cartographie A1 — feasibility test ===");
    println!("  max expr nodes  : {max_depth}");
    println!("  binary ops i64  : {}", BIN_OPS.len());
    println!("  constants i64   : {} ({:?})", CONSTS.len(), CONSTS);
    if with_f64 {
        println!("  --with-f64      : F64 ops + constantes ACTIVÉS (A2.2 multi-numeric)");
        println!("  binary ops f64  : {}", F64_BIN_OPS.len());
        println!("  unary ops f64   : {}", F64_UNARY_OPS.len());
        println!("  constants f64   : {} ({:?})", F64_CONSTS.len(), F64_CONSTS);
        println!("  + 2 conversions : F64FromI64, F64ToI64");
    }
    if let Some(p) = &build_path {
        println!("  build target    : {}", p.display());
    }
    println!();

    let mut prev_classes = 0u64;
    let mut prev_valid = 0u64;

    for depth in 1..=max_depth {
        let t0 = Instant::now();
        let mut stats = Stats::default();
        enumerate_depth(depth, with_f64, &mut stats);
        let elapsed = t0.elapsed();

        let classes = stats.fingerprints.len() as u64;
        let ratio = if classes > 0 {
            stats.valid as f64 / classes as f64
        } else {
            0.0
        };
        let new_classes = classes.saturating_sub(prev_classes);
        let valid_growth = stats.valid as f64 / prev_valid.max(1) as f64;

        println!(
            "  depth = {depth} | enumerated = {:>10} | valid = {:>10} | classes = {:>10} | ratio = {:>8.1}:1 | new_classes = {:>10} | grow = {:>5.2}× | {:.2}s",
            stats.enumerated,
            stats.valid,
            classes,
            ratio,
            new_classes,
            valid_growth,
            elapsed.as_secs_f64()
        );

        prev_classes = classes;
        prev_valid = stats.valid;

        if depth == max_depth {
            // Top-N classes (most populated → most candidates of redundancy)
            let mut top: Vec<(&[u8; 32], &u64)> = stats.fingerprints.iter().collect();
            top.sort_by(|a, b| b.1.cmp(a.1));
            println!();
            println!("  --- top 10 most populated classes (depth={depth}) ---");
            for (fp, count) in top.iter().take(10) {
                let hex: String = fp[..8].iter().map(|b| format!("{:02x}", b)).collect();
                println!("    fp={hex}...  {} programs collapse to it", count);
            }

            println!();
            println!("=== Verdict A1 ===");
            if ratio >= 1000.0 {
                println!("  ratio = {:.1}:1 — ✅ ≥ 1000:1 — GO pour A2 (Atlas v0 énumération + tri externe + dédup)", ratio);
            } else if ratio >= 100.0 {
                println!("  ratio = {:.1}:1 — ⚠ entre 100 et 1000 — proceed avec prudence, A2 + monitoring", ratio);
            } else {
                println!("  ratio = {:.1}:1 — ❌ < 100:1 — STOP, rearchitecture nécessaire avant A2", ratio);
            }

            // Φ.μ.7.11 — Atlas v1 build : entries indexées par outputs canoniques.
            if let Some(path) = &build_path {
                println!();
                println!("=== Atlas v1 build (Φ.μ.7.11 indexed) ===");
                let entries: Vec<(Vec<u8>, Vec<i64>, Vec<u8>)> = stats
                    .smallest_per_class
                    .iter()
                    .map(|(fp, (_size, bytes, outputs))| {
                        (fp.to_vec(), outputs.clone(), bytes.clone())
                    })
                    .collect();
                let outputs_size = LAB_CANONICAL_INPUTS.len() * 8;
                let total_size: usize = entries
                    .iter()
                    .map(|(fp, outs, p)| fp.len() + outs.len() * 8 + 2 + p.len())
                    .sum();
                let header = 8 + 4 + 4 + LAB_CANONICAL_INPUTS.len() * 8;
                println!("  classes         : {}", entries.len());
                println!("  canonical inputs: {} (lab inputs)", LAB_CANONICAL_INPUTS.len());
                println!("  outputs/entry   : {} bytes", outputs_size);
                println!(
                    "  on-disk size    : {} bytes ({:.1} MB)",
                    header + total_size,
                    (header + total_size) as f64 / 1_048_576.0
                );
                println!("  writing to      : {}", path.display());
                match Atlas::write(path, &LAB_CANONICAL_INPUTS, entries) {
                    Ok(()) => {
                        let bytes_written = std::fs::metadata(path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        println!("  ✅ atlas v1 écrit : {} bytes confirmés", bytes_written);
                    }
                    Err(e) => println!("  ❌ écriture atlas échouée : {e}"),
                }
            }
        }
    }
}
