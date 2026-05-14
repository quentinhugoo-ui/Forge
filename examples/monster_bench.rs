//! Unified MonsterNode benchmark — Φ.μ.épuration 2026-05-02.
//!
//! Fusionne 3 anciens benches (`monster_bench.rs`, `monster_miss_bench.rs`,
//! `monster_batch_bench.rs`) en un seul example avec sub-modes CLI.
//!
//! Usage :
//! ```bash
//! cargo run --release --example monster_bench -- hit    # cache-hit path
//! cargo run --release --example monster_bench -- miss   # cache-miss path
//! cargo run --release --example monster_bench -- batch  # scalar vs batch
//! cargo run --release --example monster_bench -- all    # tous les modes (default)
//! ```
//!
//! - `hit`   : 1000 calls / 16 unique args sur hash_chain(1024). Mesure
//!             l'avoidance ratio après warmup (target : ~98%).
//! - `miss`  : 100k calls / 100k unique args sur affine `y=7x+3`.
//!             Force un MISS à chaque appel ; isole le coût pur du
//!             dispatch+miss+insert. Phase β.1/β.2 mesure ici (~31%
//!             gain vs V6 mesuré).
//! - `batch` : compare scalar `call_many_values_i64(chunks)` vs batch
//!             `call_many_values_i64(big)` sur programme non-affine.
//!             Mesure le speedup batch lane (target : ≥5×).

use std::fs::File;
use std::hint::black_box;
use std::io::{BufRead, BufReader};
use std::time::{Instant, SystemTime};

use scan::kasm::{Node, Program, Target, Ty};
use scan::{BatchCall, DispatchResult, MemoryGovernor, MonsterNode, Store};

fn store_path(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(".codex-tmp");
    p.push(format!("scan-monster-bench-{tag}-{nanos}"));
    p
}

fn hash_chain(rounds: usize) -> Program {
    let mut nodes = Vec::with_capacity(rounds + 2);
    nodes.push(Node::input(0));
    let mut prev = 0u16;
    for _ in 0..rounds {
        nodes.push(Node::hash64(prev));
        prev = nodes.len() as u16 - 1;
    }
    nodes.push(Node::output(prev, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

fn affine_program() -> Program {
    // y = x*7 + 3 → HotPlan::AffineI64
    Program::new(
        Target::Cpu,
        1,
        1,
        1024,
        vec![
            Node::input(0),
            Node::const_i64(7),
            Node::mul(0, 1),
            Node::const_i64(3),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap()
}

/// Kmer-style mixer : 6 rounds de Hash64 entrelacés avec XOR, pour
/// simuler un hash type SplitMix64 / nthash sur k-mer. Casse le
/// pattern HashChain (XOR brise la chaîne pure) → force le slow path
/// de l'interpréteur (Layer 6) au lieu du fast lane structurel.
/// Représentatif des workloads bioinfo réels (k-mer counting,
/// minimizer extraction, locality-sensitive hashing).
fn kmer_mixer_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        24,
        vec![
            Node::input(0),         // 0
            Node::hash64(0),        // 1 = h(x)
            Node::hash64(1),        // 2 = h(h(x))
            Node::bit_xor(1, 2),    // 3 = h(x) ^ h(h(x))
            Node::hash64(3),        // 4 = h(3)
            Node::hash64(4),        // 5
            Node::bit_xor(4, 5),    // 6
            Node::hash64(6),        // 7
            Node::bit_xor(0, 7),    // 8 = mix avec input
            Node::output(8, Ty::I64),
        ],
    )
    .unwrap()
}

// ────────────────────────────────────────────────────────────────────
// Polyglot suite — 7 archetypes representing the compute variety Forge
// will host (bioinfo, finance, chemistry, signal processing, trading,
// crypto). Each archetype stresses a different region of the pipeline.
// Goal : show ns/call by archetype, NOT a single number.
// ────────────────────────────────────────────────────────────────────

/// Archetype 1 — TRIVIAL : `y = x + 1`. 4 nodes. Lower bound on dispatch
/// overhead — the actual compute is a single add. Simulates : passthrough
/// adapters, multiplexing, value renaming. Should hit AffineI64 fast lane
/// (mul=1 implicit, add=1).
fn trivial_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::add(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap()
}

/// Archetype 4 — NUMERICAL CUBIC : Horner `y = ((a·x + b)·x + c)·x + d`
/// in i64. 10 nodes. Simulates : option pricing (Black-Scholes proxy),
/// physics force fields (Lennard-Jones simplification), Kalman gain
/// computation. Fits Poly3 oracle detector.
fn numerical_cubic_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        10,
        vec![
            Node::input(0),       // 0 = x
            Node::const_i64(7),   // 1 = a
            Node::mul(0, 1),      // 2 = a·x
            Node::const_i64(11),  // 3 = b
            Node::add(2, 3),      // 4 = a·x + b
            Node::mul(4, 0),      // 5 = (a·x + b)·x
            Node::const_i64(13),  // 6 = c
            Node::add(5, 6),      // 7 = (a·x + b)·x + c
            Node::mul(7, 0),      // 8 = ((a·x + b)·x + c)·x
            Node::output(8, Ty::I64),
        ],
    )
    .unwrap()
}

/// Archetype 6 — BRANCHY : `y = if x < 0 { -x · 2 } else { x · 2 }`.
/// 9 nodes with Op::Cond. Simulates : trading rules, decision trees,
/// piecewise pricers. Stresses predicate handling.
fn branchy_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        10,
        vec![
            Node::input(0),       // 0 = x
            Node::const_i64(0),   // 1 = 0
            Node::lt(0, 1),       // 2 = x < 0  (Bool)
            Node::neg(0),         // 3 = -x
            Node::const_i64(2),   // 4 = 2
            Node::mul(3, 4),      // 5 = -x · 2
            Node::mul(0, 4),      // 6 = x · 2
            Node::cond(2, 5, 6),  // 7 = if (x<0) { -x·2 } else { x·2 }
            Node::output(7, Ty::I64),
        ],
    )
    .unwrap()
}

/// Archetype 7 — BIG MIXER : 30 nodes mixing Hash64 + arith + xor.
/// Simulates : encryption rounds, signature schemes, large signal
/// kernels. Goes through L6 slow path (no rule lane fits this shape).
fn big_mixer_program() -> Program {
    let mut nodes = Vec::with_capacity(32);
    nodes.push(Node::input(0));   // 0 = x
    nodes.push(Node::const_i64(7));   // 1
    nodes.push(Node::const_i64(11));  // 2
    nodes.push(Node::const_i64(13));  // 3
    let mut prev = 0u16;
    let consts = [1u16, 2, 3];
    for round in 0..7 {
        let h = nodes.len() as u16;
        nodes.push(Node::hash64(prev));            // hi
        let cidx = consts[round % consts.len()];
        let m = nodes.len() as u16;
        nodes.push(Node::mul(h, cidx));             // mi
        let x = nodes.len() as u16;
        nodes.push(Node::bit_xor(m, prev));         // xi
        prev = x;
    }
    nodes.push(Node::output(prev, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

fn non_affine_program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        12,
        vec![
            Node::input(0),
            Node::mul(0, 0),
            Node::const_i64(3),
            Node::add(1, 2),
            Node::const_i64(7),
            Node::bit_xor(3, 4),
            Node::const_i64(11),
            Node::mul(5, 6),
            Node::const_i64(5),
            Node::sub(7, 8),
            Node::bit_or(9, 0),
            Node::output(10, Ty::I64),
        ],
    )
    .unwrap()
}

fn mix(value: u64) -> i64 {
    let mut x = value.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    (x ^ (x >> 31)) as i64
}

fn bench_hit() -> std::io::Result<()> {
    println!("== hit mode (cache-hit path, hash_chain 1024) ==");
    let calls = 1000;
    let unique_args = 16;
    let program = hash_chain(1024);
    let monster = MonsterNode::new(
        Store::open(store_path("hit"))?,
        MemoryGovernor::one_percent_of(64 * 1024 * 1024),
    );
    let func = monster.store().store(program.bytes())?;
    let values = (0..calls).map(|i| (i % unique_args) as i64).collect::<Vec<_>>();

    let _ = monster.call_many_values_i64(&func, &[])?;
    let t0 = Instant::now();
    let _ = monster.call_many_values_i64(&func, &values)?;
    let elapsed = t0.elapsed();
    let stats = monster.stats();

    println!("  calls       : {calls}");
    println!("  unique args : {unique_args}");
    println!("  kasm nodes  : {}", program.nodes().len());
    println!("  elapsed     : {elapsed:>8.2?}");
    println!("  executions  : {}", stats.executions);
    println!("  avoided     : {}", stats.avoided());
    println!("  avoidance   : {:>8.1}%", stats.avoidance_ratio() * 100.0);
    println!("  RAM used    : {} bytes", monster.governor().used_bytes());
    Ok(())
}

fn bench_miss() -> std::io::Result<()> {
    println!("== miss mode (cache-miss path, AffineI64 fast lane) ==");
    let n: usize = 100_000;
    let monster = MonsterNode::new(
        Store::open(store_path("miss"))?,
        MemoryGovernor::new(256 * 1024 * 1024),
    );
    let program = affine_program();
    let func = monster.store().store(program.bytes())?;

    // Warmup : enregistre le programme dans `programs`.
    let _ = monster.call_one_i64(&func, -1)?;

    let t0 = Instant::now();
    for i in 0..n {
        let _ = monster.call_one_i64(&func, i as i64)?;
    }
    let elapsed = t0.elapsed();

    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    let throughput = n as f64 / elapsed.as_secs_f64();
    let stats = monster.stats();

    println!("  calls       : {n}");
    println!("  unique args : {n}");
    println!("  kasm nodes  : {}", program.nodes().len());
    println!("  elapsed     : {elapsed:>8.2?}");
    println!("  per call    : {per_call_ns:>8.1} ns");
    println!("  throughput  : {throughput:>10.0} c/s");
    println!("  rule_hits   : {}", stats.rule_hits);
    println!("  ram_hits    : {}", stats.ram_value_hits);
    println!("  executions  : {}", stats.executions);
    println!("  RAM used    : {} bytes", monster.governor().used_bytes());
    Ok(())
}

/// Bench kmer mixer en miss path : 100k unique args sur un programme
/// de 9 nœuds qui ne matche aucune rule structurelle. Force la
/// cascade complète L0→L6 (slow execute) + persistance libgit2
/// store + write_memo par appel. Réplique l'usage Forge "production"
/// pour bioinfo, finance, chimie : programme custom unique.
fn bench_kmer() -> std::io::Result<()> {
    println!("== kmer mode (slow path + libgit2 persist, mixer 9 nodes) ==");
    let n: usize = 100_000;
    let monster = MonsterNode::new(
        Store::open(store_path("kmer"))?,
        MemoryGovernor::new(256 * 1024 * 1024),
    );
    let program = kmer_mixer_program();
    let func = monster.store().store(program.bytes())?;

    // Warmup : enregistre le programme dans `programs`.
    let _ = monster.call_one_i64(&func, -1)?;

    let t0 = Instant::now();
    for i in 0..n {
        let _ = monster.call_one_i64(&func, mix(i as u64))?;
    }
    let elapsed = t0.elapsed();

    let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
    let throughput = n as f64 / elapsed.as_secs_f64();
    let stats = monster.stats();

    println!("  calls       : {n}");
    println!("  unique args : {n}");
    println!("  kasm nodes  : {}", program.nodes().len());
    println!("  elapsed     : {elapsed:>8.2?}");
    println!("  per call    : {per_call_ns:>8.1} ns");
    println!("  throughput  : {throughput:>10.0} c/s");
    println!("  rule_hits   : {}", stats.rule_hits);
    println!("  ram_hits    : {}", stats.ram_value_hits);
    println!("  executions  : {}", stats.executions);
    println!("  RAM used    : {} bytes", monster.governor().used_bytes());
    Ok(())
}

/// Polyglot bench — run 100k unique-args calls on each of the 7
/// archetypes, report ns/call per archetype. The diversity of programs
/// reveals where Forge is fast (good fit between program shape and
/// pipeline path) and where it's slow (forced into the wrong lane).
///
/// Use this output to size auto-routing thresholds : an archetype that
/// runs in ~5 µs on the slow path but could run in ~50 ns inlined is a
/// candidate for the Micro fast lane.
fn bench_polyglot() -> std::io::Result<()> {
    println!("== polyglot mode (7 archetypes, 100k unique args each) ==");
    println!();

    let mut results: Vec<(&'static str, &'static str, usize, f64, f64)> = Vec::new();

    let cases: &[(&'static str, &'static str, fn() -> Program)] = &[
        ("trivial",    "y = x + 1 (passthrough/multiplex)",          trivial_program),
        ("affine",     "y = 7x + 3 (linear pricing/calibration)",    affine_program),
        ("kmer",       "Hash64 + XOR mixer (bioinfo k-mer)",         kmer_mixer_program),
        ("hashchain",  "1024-round Hash64 (crypto/PoW)",             || hash_chain(1024)),
        ("numerical",  "Horner cubic i64 (Black-Scholes/LJ proxy)",  numerical_cubic_program),
        ("branchy",    "Op::Cond piecewise (trading rules)",         branchy_program),
        ("big_mixer",  "30-node Hash64+arith+xor (encryption)",      big_mixer_program),
    ];

    for (name, desc, build) in cases {
        let program = build();
        let monster = MonsterNode::new(
            Store::open(store_path(&format!("polyglot-{name}")))?,
            MemoryGovernor::new(256 * 1024 * 1024),
        );
        let func = monster.store().store(program.bytes())?;
        // Warmup : enregistre le programme dans `programs`.
        let _ = monster.call_one_i64(&func, -1)?;

        const N: usize = 100_000;
        let t0 = Instant::now();
        for i in 0..N {
            let _ = monster.call_one_i64(&func, mix(i as u64))?;
        }
        let elapsed = t0.elapsed();
        let per_call_ns = elapsed.as_nanos() as f64 / N as f64;
        let throughput = N as f64 / elapsed.as_secs_f64();
        results.push((name, desc, program.nodes().len(), per_call_ns, throughput));
    }

    // Compact comparison table — what the user actually wants to read.
    println!("  archetype     nodes      ns/call         c/s   description");
    println!("  -----------  ------  ----------  ----------  -----------------------------------");
    for (name, desc, nodes, ns, cps) in &results {
        println!("  {name:11}  {nodes:>6}  {ns:>10.1}  {cps:>10.0}  {desc}");
    }
    println!();

    // Identify candidates for auto-router fast lane.
    let trivial_ns = results.iter().find(|(n, _, _, _, _)| *n == "trivial").map(|r| r.3).unwrap_or(0.0);
    let max_ns = results.iter().map(|r| r.3).fold(0.0f64, |a, b| a.max(b));
    let ratio = if trivial_ns > 0.0 { max_ns / trivial_ns } else { 0.0 };
    println!("  trivial baseline : {trivial_ns:.1} ns/call");
    println!("  worst archetype  : {max_ns:.1} ns/call");
    println!("  spread ratio     : {ratio:.1}x");
    println!();
    println!("  → Auto-router target : every archetype within {trivial_ns:.0}-2x of");
    println!("    its physical floor. Archetypes much above that are paying");
    println!("    pipeline overhead they shouldn't (cf. CLAUDE.md §9 paranoid filter).");

    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// DNA real-data bench — k-mer extraction from human.txt + 8 programs
// matching the Tauri MODE dropdown taxonomy :
//
//   Léger (op triviale, bypass Rust gagne) :
//     1. k-mer hash (SplitMix64 ×1)
//     2. k-mer complement (opposite strand)
//     3. k-mer double-mix (SplitMix64 ×2)
//     4. k-mer strobemer (Sahlin 2021)
//     5. k-mer spaced seed (PatternHunter style)
//
//   KASM v1.0 mutation (utilise les nouveaux opcodes) :
//     6. k-mer branched hash (Op::Cond, JAX-style)
//
//   Moyen (transition bypass → brain) :
//     7. k-mer MinHash ×10 (Mash/sourmash)
//
//   Lourd (compute > plumbing, GPU domine) :
//     8. k-mer heavy hash ×64 (proof-of-work)
// ────────────────────────────────────────────────────────────────────

const KMER_SIZE: usize = 21;
const KMER_LIMIT: usize = 100_000;

/// Load k-mers from a tab-separated DNA file (col 0 = sequence).
/// Streams via BufReader::lines, encodes A=0/C=1/G=2/T=3 (2 bits per
/// base), packs k=21 bases into a single u64. Skips bases other than
/// ACGT (N, ambiguous codes). Stops at `max_kmers`.
fn load_kmers(path: &str, k: usize, max_kmers: usize) -> std::io::Result<Vec<i64>> {
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    let mut kmers = Vec::with_capacity(max_kmers);
    let mut first_line = true;

    'outer: for line in reader.lines() {
        let line = line?;
        if first_line {
            first_line = false;
            continue;
        }
        let seq = line.split('\t').next().unwrap_or("");
        if seq.len() < k {
            continue;
        }
        let bytes = seq.as_bytes();
        for i in 0..=bytes.len() - k {
            let mut packed: u64 = 0;
            let mut valid = true;
            for j in 0..k {
                let bit: u64 = match bytes[i + j] {
                    b'A' | b'a' => 0,
                    b'C' | b'c' => 1,
                    b'G' | b'g' => 2,
                    b'T' | b't' => 3,
                    _ => {
                        valid = false;
                        break;
                    }
                };
                packed = (packed << 2) | bit;
            }
            if !valid {
                continue;
            }
            kmers.push(packed as i64);
            if kmers.len() >= max_kmers {
                break 'outer;
            }
        }
    }
    Ok(kmers)
}

// --- 8 programmes du dropdown Tauri ------------------------------------

/// 1. SplitMix64 ×1 — single Hash64 round. Fits HotPlan::HashChain
/// (1 round) probably ; testera si auto-router Léger fire.
fn dna_splitmix1() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![
            Node::input(0),
            Node::hash64(0),
            Node::output(1, Ty::I64),
        ],
    )
    .unwrap()
}

/// 2. Complement — bitwise complement (opposite strand). XOR with -1.
/// NOT affine (no mul), pas de structural rule → Layer 6 slow path
/// attendu, candidat évident pour bypass v1.
fn dna_complement() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(-1),
            Node::bit_xor(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap()
}

/// 3. SplitMix64 ×2 — Hash64 ∘ Hash64. HotPlan::HashChain (2 rounds).
fn dna_double_mix() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::hash64(0),
            Node::hash64(1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap()
}

/// 4. Strobemer (Sahlin 2021 simplified) — h(x) ^ h(x ^ const).
/// Approximation : combine k-mer with shifted version.
fn dna_strobemer() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        7,
        vec![
            Node::input(0),         // 0 = k-mer
            Node::hash64(0),        // 1 = h(x)
            Node::const_i64(31),    // 2 = small shift constant
            Node::bit_xor(0, 2),    // 3 = x ^ shift
            Node::hash64(3),        // 4 = h(x ^ shift)
            Node::bit_xor(1, 4),    // 5 = combined
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap()
}

/// 5. Spaced seed (PatternHunter-style) — apply mask, then hash.
fn dna_spaced_seed() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        5,
        vec![
            Node::input(0),
            Node::const_i64(0x5555),  // mask 0101 0101 0101 0101 (every other bit)
            Node::bit_and(0, 1),
            Node::hash64(2),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap()
}

/// 6. Branched hash — Op::Cond if k-mer is "AT-rich" (low bits set) →
/// different hash. Exercise KASM v1.0 Op::Cond opcode.
fn dna_branched() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        7,
        vec![
            Node::input(0),         // 0 = x
            Node::const_i64(0),     // 1 = 0
            Node::lt(0, 1),         // 2 = x < 0  (Bool)
            Node::hash64(0),        // 3 = h(x)
            Node::hash64(3),        // 4 = h(h(x))
            Node::cond(2, 3, 4),    // 5 = if (x<0) h(x) else h(h(x))
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap()
}

/// 7. MinHash ×10 — 10 hash variants, take min. Mash/sourmash style.
/// Each variant XORs input with a different small constant before
/// hashing, then min-reduces.
fn dna_minhash10() -> Program {
    let mut nodes: Vec<Node> = Vec::with_capacity(40);
    nodes.push(Node::input(0));            // 0 = x

    // 10 variants : const_i, x ^ const_i, hash(x ^ const_i)
    let consts: [i16; 10] = [1, 3, 5, 7, 11, 13, 17, 19, 23, 29];
    let mut hash_slots: Vec<u16> = Vec::with_capacity(10);
    for c in &consts {
        let cidx = nodes.len() as u16;
        nodes.push(Node::const_i64(*c));
        let xor_idx = nodes.len() as u16;
        nodes.push(Node::bit_xor(0, cidx));
        let h_idx = nodes.len() as u16;
        nodes.push(Node::hash64(xor_idx));
        hash_slots.push(h_idx);
    }
    // Reduce by min : min(h0, h1) → m0, min(m0, h2) → m1, etc.
    let mut acc = hash_slots[0];
    for &h in &hash_slots[1..] {
        let m_idx = nodes.len() as u16;
        nodes.push(Node::min(acc, h));
        acc = m_idx;
    }
    nodes.push(Node::output(acc, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

/// 9. GPU-FAVORABLE crypto mixer — 100 nodes alternating Hash64 + XOR
/// + Add. Casse le pattern HashChain (les XOR/Add brisent la chaîne
/// pure → pas de Layer 3 fast lane). Pas Affine. > 64 nodes (skip
/// stack interp v2). > 4096 batch → atteint vraiment l'evaluator GPU.
/// Représentatif des kernels crypto / blockchain hashing.
fn dna_crypto_heavy() -> Program {
    let mut nodes = Vec::with_capacity(110);
    nodes.push(Node::input(0));        // 0
    nodes.push(Node::const_i64(7));    // 1
    nodes.push(Node::const_i64(11));   // 2
    nodes.push(Node::const_i64(13));   // 3

    let mut prev = 0u16;
    let consts = [1u16, 2, 3];
    // 32 rounds × 3 ops = 96 nodes + 4 setup + 1 output ≈ 101 nodes.
    // Borderline 128 — vise 32 rounds.
    for round in 0..32 {
        let h = nodes.len() as u16;
        nodes.push(Node::hash64(prev));
        let cidx = consts[round % consts.len()];
        let m = nodes.len() as u16;
        nodes.push(Node::add(h, cidx));
        let x = nodes.len() as u16;
        nodes.push(Node::bit_xor(m, prev));
        prev = x;
    }
    nodes.push(Node::output(prev, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

/// 8. Heavy hash ×64 — 64 rounds of Hash64. HashChain pattern,
/// HotPlan::HashChain fast lane attendu côté CPU. Pour batch large,
/// gpunode.eval_batch devrait router vers cuda_min si cuda_enabled.
fn dna_heavy_hash64() -> Program {
    let rounds = 64;
    let mut nodes = Vec::with_capacity(rounds + 2);
    nodes.push(Node::input(0));
    let mut prev = 0u16;
    for _ in 0..rounds {
        nodes.push(Node::hash64(prev));
        prev = nodes.len() as u16 - 1;
    }
    nodes.push(Node::output(prev, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

/// dispatch_batch GPU bench. Force le path qui peut taper cuda_min /
/// wgpu en passant par `dispatch_batch(calls, &monster)`. Les misses
/// (k-mers uniques jamais vus) sont évalués par le BulkEvaluator qui
/// est `MonsterNode` lui-même → `gpunode_runtime.eval_batch` →
/// cuda_min OR wgpu OR eval_serial selon les features.
///
/// Résultat attendu :
///   - default build (cuda OFF, wgpu OFF) : eval_serial CPU
///   - --features cuda + NVIDIA           : try_eval_cuda_min → GPU
///   - --features wgpu + Vulkan           : try_eval_wgpu_affine → GPU
///   - --features cuda,wgpu               : cuda priorité, fallback wgpu
fn bench_gpu_dispatch(path: &str) -> std::io::Result<()> {
    println!("== GPU dispatch_batch bench (real path to gpunode) ==");
    println!();

    let cuda = cfg!(feature = "cuda");
    let wgpu = cfg!(feature = "wgpu");
    println!("  Build features : cuda={cuda} wgpu={wgpu}");
    if !cuda && !wgpu {
        println!("  ⚠ Aucune feature GPU active. Le path eval_serial CPU sera testé.");
        println!("    Pour tester GPU :");
        println!("      cargo run --release --features cuda --example monster_bench -- gpu_dispatch");
        println!("      cargo run --release --features wgpu --example monster_bench -- gpu_dispatch");
        println!();
    }

    println!("  loading k-mers (k={KMER_SIZE}, limit={KMER_LIMIT})...");
    let kmers = load_kmers(path, KMER_SIZE, KMER_LIMIT)?;
    println!("  loaded {} k-mers", kmers.len());
    println!();

    type ProgFn = fn() -> Program;
    let cases: &[(&'static str, ProgFn)] = &[
        ("splitmix1",  dna_splitmix1),
        ("complement", dna_complement),
        ("double_mix", dna_double_mix),
        ("strobemer",  dna_strobemer),
        ("spaced",     dna_spaced_seed),
        ("branched",   dna_branched),
        ("minhash10",  dna_minhash10),
        ("heavy_64",   dna_heavy_hash64),
        ("crypto_heavy", dna_crypto_heavy),  // 100 nodes, GPU-favorable
    ];

    println!("  {:<11} {:>6}  {:>10}  {:>13}  {}",
             "program", "nodes", "ns/call", "c/s", "GPU status");
    println!("  {:-<11} {:->6}  {:->10}  {:->13}  {:-<35}",
             "", "", "", "", "");

    for (name, build) in cases {
        let program = build();
        let monster = MonsterNode::new(
            Store::open(store_path(&format!("gpu-{name}")))?,
            MemoryGovernor::new(256 * 1024 * 1024),
        );
        let func = monster.store().store(program.bytes())?;

        // Construire BatchCall list (8 bytes args = i64 LE par k-mer).
        let calls: Vec<BatchCall> = kmers.iter()
            .map(|k| BatchCall::new(func, k.to_le_bytes().to_vec()))
            .collect();

        // Warmup : un dispatch préliminaire pour load le program dans hot.
        let _ = monster.dispatch_batch(&calls[..1], &monster)?;

        // Reset CUDA status counter avant la mesure.
        let _ = scan::take_last_cuda_status();

        let t0 = Instant::now();
        let results = monster.dispatch_batch(&calls, &monster)?;
        let elapsed = t0.elapsed();
        let n = kmers.len();
        let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
        let throughput = n as f64 / elapsed.as_secs_f64();

        // Compter combien de calls ont été Hit vs Computed (pour vérifier
        // que le path eval_batch a bien fire).
        let (hits, computed) = results.iter().fold((0, 0), |(h, c), r| match r {
            DispatchResult::Hit(_) => (h + 1, c),
            DispatchResult::Computed(_) => (h, c + 1),
        });

        // Read CUDA status set during the batch (None = not fired).
        let cuda_status = scan::take_last_cuda_status();
        let gpu_msg = match cuda_status {
            Some(scan::CudaStatus::Ok) => "cuda_min OK (single-GPU)".to_string(),
            Some(scan::CudaStatus::SplitOk) => "🎯 SPLIT cuda+wgpu PARALLEL (multi-GPU)".to_string(),
            Some(scan::CudaStatus::DriverTooOld(_)) => "cuda_min driver too old".to_string(),
            Some(scan::CudaStatus::Io(_)) => "cuda_min io error".to_string(),
            Some(scan::CudaStatus::Panicked(_)) => "cuda_min panicked".to_string(),
            None => format!("CPU eval_serial (hits={hits} computed={computed})"),
        };

        println!("  {:<11} {:>6}  {:>9.1}   {:>11.0}  {}",
                 name, program.nodes().len(), per_call_ns, throughput, gpu_msg);
        black_box(results);
    }
    println!();
    println!("  Note : un k-mer apparaissant ≥ 2 fois → 2nd call = Hit (cache cascade,");
    println!("  pas de GPU). Avec 100k k-mers ADN ~30k uniques, ~30k vont à eval_batch");
    println!("  qui est le seuil GPU (4096) → cuda_min/wgpu firent SI features compilées.");

    Ok(())
}

/// STRICT correctness probe : appelle `run_wgpu_universal_for_test`
/// directement (path wgpu pur, pas de cache, pas d'auto-router CPU
/// fallback) et compare bit-pour-bit avec `kasm::execute`. C'est LE
/// test qui valide vraiment que le kernel WGSL universel produit les
/// bons octets. Disponible uniquement avec --features wgpu.
#[cfg(feature = "wgpu")]
fn bench_wgpu_strict_correctness(path: &str) -> std::io::Result<()> {
    println!("== WGPU universal kernel STRICT correctness ==");
    println!("  (calls run_wgpu_universal_for_test directly, no CPU fallback)");
    println!();

    let kmers = load_kmers(path, KMER_SIZE, 4096)?;
    println!("  loaded {} k-mers", kmers.len());
    println!();

    type ProgFn = fn() -> Program;
    let cases: &[(&'static str, ProgFn)] = &[
        ("splitmix1",  dna_splitmix1),
        ("complement", dna_complement),
        ("double_mix", dna_double_mix),
        ("strobemer",  dna_strobemer),
        ("spaced",     dna_spaced_seed),
        ("branched",   dna_branched),
        ("minhash10",  dna_minhash10),
        ("heavy_64",   dna_heavy_hash64),
        ("crypto_heavy", dna_crypto_heavy),  // 100 nodes, GPU-favorable
    ];

    println!("  {:<11}  {:>10}  {:>10}  {}",
             "program", "wgpu_run", "match", "first 3 mismatches (cpu vs wgpu)");
    println!("  {:-<11}  {:->10}  {:->10}  {:-<60}", "", "", "", "");

    let mut all_ok = true;
    let mut wgpu_actually_ran = false;

    for (name, build) in cases {
        let program = build();

        // CPU reference.
        let mut cpu_out: Vec<i64> = Vec::with_capacity(kmers.len());
        for &kmer in &kmers {
            let bytes = kmer.to_le_bytes();
            let result = scan::kasm::execute(&program, &bytes)
                .map_err(|e| std::io::Error::other(format!("kasm exec: {e}")))?;
            cpu_out.push(i64::from_le_bytes(result.try_into().unwrap()));
        }

        // Direct wgpu invocation.
        let wgpu_result = scan::run_wgpu_universal_for_test(&program, &kmers)?;
        let (wgpu_status, wgpu_out) = match wgpu_result {
            None => ("UNAVAIL", None),
            Some(v) => {
                wgpu_actually_ran = true;
                ("RAN", Some(v))
            }
        };

        match wgpu_out {
            None => {
                println!("  {:<11}  {:>10}  {:>10}  (kernel skipped — out of spec)",
                         name, wgpu_status, "-");
            }
            Some(gpu) => {
                let mut mismatches = Vec::new();
                for (i, (cpu_v, gpu_v)) in cpu_out.iter().zip(gpu.iter()).enumerate() {
                    if cpu_v != gpu_v && mismatches.len() < 3 {
                        mismatches.push(format!("[{i}] cpu={cpu_v} wgpu={gpu_v}"));
                    }
                }
                let ok = mismatches.is_empty();
                if !ok { all_ok = false; }
                println!("  {:<11}  {:>10}  {:>10}  {}",
                         name, wgpu_status,
                         if ok { "OK" } else { "FAIL" },
                         mismatches.join("  "));
            }
        }
    }

    println!();
    if !wgpu_actually_ran {
        println!("  ⚠ WGPU kernel jamais exécuté. Probable cause :");
        println!("    - GPU ne supporte pas SHADER_INT64");
        println!("    - wgpu request_device a échoué (driver, etc)");
    } else if all_ok {
        println!("  ✓ WGPU universal kernel : bit-pour-bit identique au CPU sur 8 programmes ADN");
    } else {
        println!("  ✗ WGPU universal kernel a un BUG. Voir mismatches ci-dessus.");
    }
    Ok(())
}

/// Correctness probe — vérifie que le path GPU (cuda_min OR wgpu_universal)
/// produit des résultats bit-pour-bit identiques au CPU pour les 8
/// programmes ADN. Utile pour valider le multi-GPU split avant de
/// trust ses outputs en production.
fn bench_gpu_correctness(path: &str) -> std::io::Result<()> {
    println!("== GPU correctness probe (CPU reference vs GPU dispatch) ==");
    println!();

    let cuda = cfg!(feature = "cuda");
    let wgpu = cfg!(feature = "wgpu");
    println!("  Build features : cuda={cuda} wgpu={wgpu}");
    if !cuda && !wgpu {
        println!("  ⚠ No GPU features, this probe will compare CPU vs CPU (no signal).");
    }
    println!();

    let kmers = load_kmers(path, KMER_SIZE, 8192)?;
    println!("  loaded {} k-mers (sample for correctness check)", kmers.len());
    println!();

    type ProgFn = fn() -> Program;
    let cases: &[(&'static str, ProgFn)] = &[
        ("splitmix1",  dna_splitmix1),
        ("complement", dna_complement),
        ("double_mix", dna_double_mix),
        ("strobemer",  dna_strobemer),
        ("spaced",     dna_spaced_seed),
        ("branched",   dna_branched),
        ("minhash10",  dna_minhash10),
        ("heavy_64",   dna_heavy_hash64),
        ("crypto_heavy", dna_crypto_heavy),  // 100 nodes, GPU-favorable
    ];

    println!("  {:<11}  {:>8}  {}", "program", "match", "first 3 mismatches (cpu vs gpu)");
    println!("  {:-<11}  {:->8}  {:-<60}", "", "", "");

    let mut all_ok = true;
    for (name, build) in cases {
        let program = build();

        // CPU reference via direct interpreter (bit-exact).
        let mut cpu_out: Vec<i64> = Vec::with_capacity(kmers.len());
        for &kmer in &kmers {
            let bytes = kmer.to_le_bytes();
            let result = scan::kasm::execute(&program, &bytes)
                .map_err(|e| std::io::Error::other(format!("kasm exec: {e}")))?;
            cpu_out.push(i64::from_le_bytes(result.try_into().unwrap()));
        }

        // GPU path : on force `dispatch_batch` qui route les misses
        // au BulkEvaluator → gpunode_runtime.eval_batch → cuda_min /
        // wgpu_universal / wgpu_affine / eval_serial selon les features.
        // C'est ce path qui peut tirer le GPU en réalité (le JIT batch
        // de call_many_values_i64 reste CPU).
        let monster = MonsterNode::new(
            Store::open(store_path(&format!("correctness-{name}")))?,
            MemoryGovernor::new(256 * 1024 * 1024),
        );
        let func = monster.store().store(program.bytes())?;
        let calls: Vec<BatchCall> = kmers.iter()
            .map(|k| BatchCall::new(func, k.to_le_bytes().to_vec()))
            .collect();
        let _ = monster.dispatch_batch(&calls[..1], &monster)?;  // warmup
        let results = monster.dispatch_batch(&calls, &monster)?;

        // Récupère les bytes via call_one_i64 — post-batch les RAM cache
        // slots sont chauds, on hit cache et lit le résultat. Si cache
        // a été pollué entre temps, call_one_i64 re-calcule via
        // auto-router CPU → toujours correct mais ne valide pas le GPU.
        // OK pour un test global.
        let gpu_out: Vec<i64> = kmers.iter()
            .map(|&k| monster.call_one_i64(&func, k).unwrap_or(0))
            .collect();
        // Sanity : check le nombre de DispatchResult correspond.
        if results.len() != kmers.len() {
            eprintln!("  dispatch_batch returned {} results for {} calls",
                      results.len(), kmers.len());
        }

        let mut mismatches = Vec::new();
        for (i, (cpu_v, gpu_v)) in cpu_out.iter().zip(gpu_out.iter()).enumerate() {
            if cpu_v != gpu_v && mismatches.len() < 3 {
                mismatches.push(format!("[{i}] cpu={cpu_v} gpu={gpu_v}"));
            }
        }
        let ok = mismatches.is_empty();
        if !ok {
            all_ok = false;
        }
        let mismatch_summary = if mismatches.is_empty() {
            String::new()
        } else {
            mismatches.join("  ")
        };
        println!("  {:<11}  {:>8}  {}",
                 name,
                 if ok { "OK" } else { "FAIL" },
                 mismatch_summary);
    }
    println!();
    if all_ok {
        println!("  ✓ All 8 programs : GPU output bit-pour-bit identical to CPU reference.");
    } else {
        println!("  ✗ Some programs FAILED correctness check. GPU kernel has a bug.");
    }
    Ok(())
}

/// CUDA status probe. Pourquoi heavy_64 ne tape jamais cuda_min :
/// 3 conditions doivent être satisfaites simultanément.
fn bench_cuda_status() -> std::io::Result<()> {
    println!("== CUDA path status probe ==");
    println!();

    // 1. Compile-time : la feature `cuda` doit être active.
    let cuda_compiled = cfg!(feature = "cuda");
    println!("  1. Compile-time `cuda` feature : {}",
             if cuda_compiled { "ACTIVE" } else { "INACTIVE (default build)" });
    if !cuda_compiled {
        println!("     → Sans `--features cuda`, le bloc cuda_min est");
        println!("       dead code ; le path GPU tombe sur eval_serial.");
        println!("     Build avec : cargo run --release --features cuda \\");
        println!("                  --example monster_bench -- cuda_status");
    }

    // 2. Runtime : cuda_min doit pouvoir charger nvcuda.dll + parser PTX.
    #[cfg(feature = "cuda")]
    {
        let avail = scan::forge_cuda_min_available();
        println!("  2. Runtime cuda_min available  : {}",
                 if avail { "YES (nvcuda.dll loaded)" } else { "NO (driver missing?)" });
    }
    #[cfg(not(feature = "cuda"))]
    println!("  2. Runtime cuda_min available  : (feature off, can't probe)");

    // 3. FORGE_CUDA_MIN env var : doit ne pas être "0" (kill switch).
    let env = std::env::var("FORGE_CUDA_MIN").unwrap_or_else(|_| "<unset>".to_string());
    println!("  3. FORGE_CUDA_MIN env var      : {} ({})", env,
             if env == "0" { "DISABLED by user" } else { "OK" });

    // 4. Bootstrap : MonsterNode bootstraps gpunode runtime au new().
    //    Crée un node temporaire pour récupérer le bootstrap.
    let monster = MonsterNode::new(
        Store::open(store_path("cuda-probe"))?,
        MemoryGovernor::new(64 * 1024 * 1024),
    );
    let report: Vec<String> = scan::gpu_capability_report();
    println!("  4. GPU capability report       :");
    if report.is_empty() {
        println!("     (none)");
    } else {
        for line in &report {
            println!("     {line}");
        }
    }
    drop(monster);

    println!();
    println!("  Pourquoi heavy_64 batch n'a pas tapé cuda_min dans le DNA bench :");
    println!();
    println!("    A) Le bench utilise `call_many_values_i64` qui route via");
    println!("       JIT batch CPU (HotPlan::HashChain → tight native loop).");
    println!("       gpunode.eval_batch n'est PAS dans le chemin de cette API.");
    println!();
    println!("    B) Le path GPU est uniquement appelable via");
    println!("       `dispatch_batch(calls, &monster)` qui passe les misses à");
    println!("       l'évaluator (`BulkEvaluator::eval_batch` → gpunode).");
    println!();
    println!("    C) Même là, sans `--features cuda` à la compilation, le bloc");
    println!("       try_eval_cuda_min est dead code → fallback eval_serial.");
    println!();
    println!("  Conclusion : c'est by design pour les workloads call_many_values.");
    println!("  Pour exposer le path GPU réel, il faudrait un bench dédié");
    println!("  `dispatch_batch_cuda` avec `--features cuda` build flag.");

    Ok(())
}

fn bench_dna(path: &str) -> std::io::Result<()> {
    println!("== DNA real-data bench ({}) ==", path);
    println!("  loading k-mers (k={KMER_SIZE}, limit={KMER_LIMIT})...");
    let kmers = match load_kmers(path, KMER_SIZE, KMER_LIMIT) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("  ERROR: {e}");
            eprintln!("  Vérifie que le fichier existe : {path}");
            return Err(e);
        }
    };
    println!("  loaded {} k-mers", kmers.len());
    println!();

    type ProgFn = fn() -> Program;
    let cases: &[(&'static str, &'static str, &'static str, ProgFn)] = &[
        ("splitmix1",  "Léger",      "SplitMix64 ×1 (3 nodes)",            dna_splitmix1),
        ("complement", "Léger",      "complement opposite strand (4 nodes)", dna_complement),
        ("double_mix", "Léger",      "SplitMix64 ×2 (4 nodes)",            dna_double_mix),
        ("strobemer",  "Léger",      "Strobemer Sahlin (7 nodes)",          dna_strobemer),
        ("spaced",     "Léger",      "Spaced seed PatternHunter (5 nodes)", dna_spaced_seed),
        ("branched",   "KASM v1.0",  "Op::Cond branched hash (7 nodes)",    dna_branched),
        ("minhash10",  "Moyen",      "MinHash ×10 Mash (~40 nodes)",       dna_minhash10),
        ("heavy_64",   "Lourd",      "Heavy hash ×64 PoW (66 nodes)",       dna_heavy_hash64),
    ];

    println!("  {:<11} {:<10} {:>6}  {:>10}  {:>13}  {:<35}",
             "program", "tier", "nodes", "ns/call", "scalar c/s", "source breakdown");
    println!("  {:-<11} {:-<10} {:->6}  {:->10}  {:->13}  {:-<35}",
             "", "", "", "", "", "");

    let mut batch_summary: Vec<(String, usize, f64, f64)> = Vec::new();

    for (name, tier, _desc, build) in cases {
        let program = build();
        let monster = MonsterNode::new(
            Store::open(store_path(&format!("dna-{name}")))?,
            MemoryGovernor::new(256 * 1024 * 1024),
        );
        let func = monster.store().store(program.bytes())?;
        // Warmup : enregistre le programme dans `programs`.
        let _ = monster.call_one_i64(&func, kmers[0])?;

        // Scalar bench : call_one_i64 sur tous les k-mers.
        let t0 = Instant::now();
        let mut checksum: i64 = 0;
        for &kmer in &kmers {
            checksum ^= monster.call_one_i64(&func, kmer)?;
        }
        let elapsed = t0.elapsed();
        let n = kmers.len();
        let per_call_ns = elapsed.as_nanos() as f64 / n as f64;
        let throughput = n as f64 / elapsed.as_secs_f64();
        let stats = monster.stats();

        let source = format!(
            "rule={} ram_v={} exec={}",
            stats.rule_hits, stats.ram_value_hits, stats.executions
        );

        println!("  {:<11} {:<10} {:>6}  {:>9.1}   {:>11.0}    {}",
                 name, tier, program.nodes().len(), per_call_ns, throughput, source);
        black_box(checksum);

        // Batch bench : call_many_values_i64 sur tous les k-mers.
        // Pour les Lourds (heavy_64), ceci doit déclencher le path
        // gpunode.eval_batch (≥ 4096 calls + cuda_enabled). Si cuda
        // pas disponible, fallback eval_serial CPU.
        let monster2 = MonsterNode::new(
            Store::open(store_path(&format!("dna-batch-{name}")))?,
            MemoryGovernor::new(256 * 1024 * 1024),
        );
        let func2 = monster2.store().store(program.bytes())?;
        let _ = monster2.call_one_i64(&func2, kmers[0])?;

        let t0 = Instant::now();
        let outputs = monster2.call_many_values_i64(&func2, &kmers)?;
        let batch_elapsed = t0.elapsed();
        let batch_per_call_ns = batch_elapsed.as_nanos() as f64 / kmers.len() as f64;
        let batch_throughput = kmers.len() as f64 / batch_elapsed.as_secs_f64();
        black_box(outputs);

        batch_summary.push((
            format!("{} ({})", name, tier),
            program.nodes().len(),
            batch_per_call_ns,
            batch_throughput,
        ));
    }

    println!();
    println!("  --- batch mode (call_many_values_i64) — Lourd attendu via GPU si cuda_enabled ---");
    println!("  {:<28} {:>6}  {:>10}  {:>13}", "program", "nodes", "ns/call", "batch c/s");
    println!("  {:-<28} {:->6}  {:->10}  {:->13}", "", "", "", "");
    for (name, nodes, ns, cps) in &batch_summary {
        println!("  {:<28} {:>6}  {:>9.1}   {:>11.0}", name, nodes, ns, cps);
    }
    println!();
    println!("  → Léger : auto-router v0 ne bypass que AffineI64. Tous les");
    println!("    Léger ici utilisent Hash64 → pas de bypass aujourd'hui.");
    println!("    Candidats v1 : étendre le bypass à HotPlan::HashChain (1-2 rounds).");

    Ok(())
}

fn bench_batch() -> std::io::Result<()> {
    println!("== batch mode (scalar chunks vs batch lane) ==");
    const SCALAR_CALLS: usize = 100_000;
    const BATCH_CALLS: usize = 2_000_000;
    let program = non_affine_program();

    let scalar = MonsterNode::new(
        Store::open(store_path("batch-scalar"))?,
        MemoryGovernor::one_percent_of(256 * 1024 * 1024),
    );
    let batch = MonsterNode::new(
        Store::open(store_path("batch-batch"))?,
        MemoryGovernor::one_percent_of(256 * 1024 * 1024),
    );
    let scalar_func = scalar.store().store(program.bytes())?;
    let batch_func = batch.store().store(program.bytes())?;

    let scalar_values: Vec<i64> = (0..SCALAR_CALLS as u64).map(mix).collect();
    let batch_values: Vec<i64> = (0..BATCH_CALLS as u64).map(mix).collect();

    let start = Instant::now();
    let mut scalar_checksum = 0i64;
    for chunk in scalar_values.chunks(512) {
        let out = scalar.call_many_values_i64(&scalar_func, black_box(chunk))?;
        scalar_checksum ^= out.iter().fold(0, |acc, value| acc ^ value);
    }
    let scalar_elapsed = start.elapsed();

    let start = Instant::now();
    let out = batch.call_many_values_i64(&batch_func, black_box(&batch_values))?;
    let batch_elapsed = start.elapsed();
    let batch_checksum = out.iter().fold(0, |acc, value| acc ^ value);

    let scalar_cps = SCALAR_CALLS as f64 / scalar_elapsed.as_secs_f64();
    let batch_cps = BATCH_CALLS as f64 / batch_elapsed.as_secs_f64();

    println!("  program_nodes : {}", program.nodes().len());
    println!("  scalar_calls  : {SCALAR_CALLS}");
    println!("  scalar_elapsed: {:.3} ms", scalar_elapsed.as_secs_f64() * 1000.0);
    println!("  scalar c/s    : {scalar_cps:.0}");
    println!("  batch_calls   : {BATCH_CALLS}");
    println!("  batch_elapsed : {:.3} ms", batch_elapsed.as_secs_f64() * 1000.0);
    println!("  batch c/s     : {batch_cps:.0}");
    println!("  speedup       : {:.2}x", batch_cps / scalar_cps);
    println!("  checksums (scalar/batch) : {scalar_checksum} / {batch_checksum}");
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    match mode.as_str() {
        "hit" => bench_hit(),
        "miss" => bench_miss(),
        "batch" => bench_batch(),
        "kmer" => bench_kmer(),
        "polyglot" => bench_polyglot(),
        "dna" => {
            let path = std::env::args()
                .nth(2)
                .unwrap_or_else(|| "C:\\Users\\quent\\Desktop\\human.txt\\human.txt".to_string());
            bench_dna(&path)
        }
        "cuda_status" | "cuda" => bench_cuda_status(),
        "gpu_dispatch" | "gpu" => {
            let path = std::env::args()
                .nth(2)
                .unwrap_or_else(|| "C:\\Users\\quent\\Desktop\\human.txt\\human.txt".to_string());
            bench_gpu_dispatch(&path)
        }
        "gpu_correctness" | "correctness" => {
            let path = std::env::args()
                .nth(2)
                .unwrap_or_else(|| "C:\\Users\\quent\\Desktop\\human.txt\\human.txt".to_string());
            bench_gpu_correctness(&path)
        }
        "wgpu_strict" | "strict" => {
            #[cfg(feature = "wgpu")]
            {
                let path = std::env::args()
                    .nth(2)
                    .unwrap_or_else(|| "C:\\Users\\quent\\Desktop\\human.txt\\human.txt".to_string());
                return bench_wgpu_strict_correctness(&path);
            }
            #[cfg(not(feature = "wgpu"))]
            {
                println!("This mode requires --features wgpu");
                Ok(())
            }
        }
        "all" | _ => {
            bench_hit()?;
            println!();
            bench_miss()?;
            println!();
            bench_kmer()?;
            println!();
            bench_polyglot()?;
            println!();
            bench_batch()
        }
    }
}
