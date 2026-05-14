//! HotProgram, HotPlan and structural-rule extraction.
//!
//! `hot_plan` walks a verified `Program` and decides whether the node can
//! short-circuit the interpreter via a structural rule (currently
//! `HashChain` and `AffineI64`). `execute_hot_plan` runs whichever plan
//! was selected — including the interpreter as fallback.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::Arc;

use crate::kasm::{self, Op, Program, Target, Ty};
use crate::Hash;

/// V7 γ.1: aligned 64 B. The hot path reads `hot.semantic_fingerprint`
/// (32 B), `hot.plan` (~24 B), and `hot.program` (16 B Arc) on every
/// dispatch — those three fields together fit in one cache line when
/// the struct is line-aligned. Without alignment a `HotProgram`
/// straddles two lines on every miss, adding ~5 ns of L1 fill.
#[repr(align(64))]
pub(super) struct HotProgram {
    pub(super) semantic_fingerprint: [u8; 32],
    pub(super) program: Arc<Program>,
    pub(super) plan: HotPlan,
    pub(super) explicit_memos: Arc<[MemoizedSubProgram]>,
    pub(super) jit: Mutex<Option<Arc<kasm::jit::JitKernel>>>,
    pub(super) jit_disabled: AtomicBool,
    pub(super) charged: u32,
    /// Phase 12.0 — analyse structurale calculée au load. Visibilité
    /// pour `dispatch_batch` afin qu'il classe les programmes par
    /// taille (Micro/Mini/Semi/Moyenne/Grande/Meta) et expose les
    /// opérations qui se répètent au sein du DAG. La consommation
    /// concrète (cache multi-échelle, partial eval) viendra en Phase
    /// 12.1+ — ce champ n'a aucun effet runtime aujourd'hui pour les
    /// programmes sans décomposition (silencieux par construction).
    pub(super) structure: StructuralAnalysis,
    /// Φ.ν.7g — AdaptiveInlineCache L0, 5-10 ns lock-free direct-mapped
    /// 64 slots × 64 octets (4 KB par programme, fit L1). S'auto-désactive
    /// si hit_rate < 5% après 100 probes (cf. `cache.rs::AdaptiveInlineCache`).
    /// Sur le workload reverse_synth (millions de candidats KASM évalués
    /// sur les MÊMES features_i64 d'une bougie), hit_rate attendu ≥ 50%
    /// → L0 reste actif et accélère 5-20x. Sur DNA k-mer args uniques,
    /// auto-désactivation après warmup → coût ≈ 0 vs la cascade existante.
    pub(super) inline_cache: super::cache::AdaptiveInlineCache,
}

#[derive(Clone)]
pub(super) struct MemoizedSubProgram {
    pub(super) semantic_fingerprint: [u8; 32],
    pub(super) program: Arc<Program>,
}

/// Phase 12.0 — classification de la taille d'un programme KASM, alignée
/// sur la doctrine §9 du CLAUDE.md (cascade lookup top-down sur 6
/// échelles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeClass {
    Micro,
    Mini,
    Semi,
    Moyenne,
    Grande,
    Meta,
}

impl SizeClass {
    pub fn classify(node_count: usize) -> Self {
        match node_count {
            0..=5 => Self::Micro,
            6..=30 => Self::Mini,
            31..=100 => Self::Semi,
            101..=500 => Self::Moyenne,
            501..=1000 => Self::Grande,
            _ => Self::Meta,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Micro => "Micro",
            Self::Mini => "Mini",
            Self::Semi => "Semi",
            Self::Moyenne => "Moyenne",
            Self::Grande => "Grande",
            Self::Meta => "Meta",
        }
    }
}

/// Phase 12.0 — résumé statique du programme. Calculé une fois au
/// chargement, lu par `dispatch_batch` pour télémétrie + futur
/// dispatch multi-échelle. **Aujourd'hui** : visibilité seule.
#[derive(Debug, Clone)]
pub(super) struct StructuralAnalysis {
    pub(super) size: SizeClass,
    pub(super) node_count: u32,
    /// Liste compacte (op, count) pour les ops apparaissant ≥ 2 fois.
    /// Trié par count décroissant. Vide pour les programmes sans
    /// répétition d'op (e.g. `Input → Hash64 → Output`, chaque op
    /// unique → analyse silencieuse).
    pub(super) recurring_ops: Vec<(Op, u32)>,
}

impl StructuralAnalysis {
    pub(super) fn analyze(program: &Program) -> Self {
        let nodes = program.nodes();
        let node_count = nodes.len() as u32;
        let size = SizeClass::classify(nodes.len());

        let mut histo: std::collections::HashMap<Op, u32> = std::collections::HashMap::new();
        for node in nodes {
            *histo.entry(node.op).or_insert(0) += 1;
        }
        let mut recurring: Vec<(Op, u32)> =
            histo.into_iter().filter(|(_, c)| *c >= 2).collect();
        recurring.sort_by(|a, b| b.1.cmp(&a.1));

        Self {
            size,
            node_count,
            recurring_ops: recurring,
        }
    }

    pub(super) fn is_decomposable(&self) -> bool {
        !self.recurring_ops.is_empty()
    }
}

#[derive(Clone)]
pub(super) enum HotPlan {
    Interpret,
    /// Pure-hash chain: `input(slot) → hash64 → hash64 → ... → output`.
    /// Faster than the JIT path because we run a tight native loop
    /// (`for _ in 0..rounds { value = hash_i64(value); }`) that beats
    /// the JIT's per-iteration dispatch overhead.
    ///
    /// **Removed in a V6.7 via-negativa attempt and immediately
    /// restored** when `efficiency_bench` showed throughput dropping
    /// from 444 K c/s to 84 K c/s (×5 slower) — proof by measurement
    /// that this plan earns its lines.
    HashChain { input_slot: u8, rounds: usize },
    AffineI64 { input_slot: u8, mul: i64, add: i64 },
    /// All-const program: outputs are computed once at load-time and
    /// emitted verbatim on every call. Folded into `HotPlan` so the
    /// dispatch is a single `match`, not a separate `Option` field.
    StaticOutput(Arc<[u8]>),
}

impl HotPlan {
    pub(super) fn is_rule(&self) -> bool {
        !matches!(self, Self::Interpret)
    }
}

pub(super) fn should_semantic_fingerprint(program: &Program) -> bool {
    // Wave 7c — semantic_fingerprint probes the program with 8-byte
    // i64 sample args (`semantic_sample_args`). Vec programs reject
    // those args via the per-slot length-prefixed parser, so skip
    // them and fall back to `exact_program_identity` (byte hash).
    let has_vec = program.nodes().iter().any(|node| node.ty == Ty::VecI64);
    !program.target().needs_external_backend()
        && program.nodes().len() <= 128
        && !program.nodes().iter().any(|node| node.op == Op::Hash64)
        && !has_vec
}

pub(super) fn should_simplify(program: &Program) -> bool {
    // Wave 7c — `simplified()` re-runs the optimizer which probes
    // with `semantic_sample_args` to verify byte-equivalence. Vec
    // programs can't survive that probe ; skip simplification.
    let has_vec = program.nodes().iter().any(|node| node.ty == Ty::VecI64);
    !program.target().needs_external_backend()
        && program.nodes().len() <= 256
        && !program.nodes().iter().any(|node| node.op == Op::Hash64)
        && !has_vec
}

pub(super) fn hot_plan(program: &Program) -> HotPlan {
    // All-const program → constant output, decided once at load-time.
    // Pre-empts every other layer because it's cheaper than a HashMap
    // lookup (just a refcounted Arc clone).
    if let Some(bytes) = program.static_output() {
        return HotPlan::StaticOutput(Arc::from(bytes.into_boxed_slice()));
    }

    if program.outputs() != 1 {
        return HotPlan::Interpret;
    }

    if let Some((input_slot, mul, add)) = affine_rule(program) {
        return HotPlan::AffineI64 { input_slot, mul, add };
    }

    let Some((_, output)) = program
        .nodes()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, node)| node.op == Op::Output)
    else {
        return HotPlan::Interpret;
    };
    if output.ty != Ty::I64 {
        return HotPlan::Interpret;
    }

    // Hash-chain detector: walk the output's source chain backward.
    // Every hop must be `hash64`, terminating on a non-negative
    // `Input`. A V6.7 via-negativa experiment removed this whole
    // detector; the resulting JIT-only path was 5× slower on
    // `efficiency_bench`. Lines kept, with measurement.
    let mut rounds = 0usize;
    let mut current = output.a as usize;
    loop {
        let Some(node) = program.nodes().get(current).copied() else {
            return HotPlan::Interpret;
        };
        match node.op {
            Op::Hash64 => {
                rounds += 1;
                current = node.a as usize;
            }
            Op::Input if node.imm >= 0 => {
                return HotPlan::HashChain { input_slot: node.imm as u8, rounds };
            }
            _ => return HotPlan::Interpret,
        }
    }
}

pub(super) fn execute_hot_plan(hot: &HotProgram, args: &[u8]) -> io::Result<Vec<u8>> {
    match &hot.plan {
        HotPlan::Interpret => execute_with_jit(hot, args),
        HotPlan::HashChain { input_slot, rounds } => {
            let start = *input_slot as usize * 8;
            let bytes = args
                .get(start..start + 8)
                .ok_or_else(|| io::Error::other("bad KASM input length for hot hash chain"))?;
            let mut value = i64::from_le_bytes(bytes.try_into().unwrap());
            for _ in 0..*rounds {
                value = kasm::hash_i64(value);
            }
            Ok(value.to_le_bytes().to_vec())
        }
        HotPlan::AffineI64 { input_slot, mul, add } => {
            let start = *input_slot as usize * 8;
            let bytes = args
                .get(start..start + 8)
                .ok_or_else(|| io::Error::other("bad KASM input length for affine rule"))?;
            let value = i64::from_le_bytes(bytes.try_into().unwrap());
            Ok(value.wrapping_mul(*mul).wrapping_add(*add).to_le_bytes().to_vec())
        }
        HotPlan::StaticOutput(bytes) => Ok(bytes.to_vec()),
    }
}

pub(super) fn execute_hot_batch_i64(hot: &HotProgram, values: &[i64]) -> io::Result<Option<Vec<i64>>> {
    if hot.program.inputs() != 1 || hot.program.outputs() != 1 || hot.program.output_types() != vec![Ty::I64] {
        return Ok(None);
    }

    match &hot.plan {
        HotPlan::AffineI64 { input_slot: 0, mul, add } => {
            let mut out = Vec::with_capacity(values.len());
            for value in values {
                out.push(value.wrapping_mul(*mul).wrapping_add(*add));
            }
            Ok(Some(out))
        }
        HotPlan::HashChain { input_slot: 0, rounds } => {
            let mut out = Vec::with_capacity(values.len());
            for value in values {
                let mut value = *value;
                for _ in 0..*rounds {
                    value = kasm::hash_i64(value);
                }
                out.push(value);
            }
            Ok(Some(out))
        }
        HotPlan::StaticOutput(bytes) => {
            // Single i64 output by construction (output_types() check
            // above already passed; for non-i64 we'd be in the early
            // None return). Decode once, replicate per call.
            if bytes.len() < 8 {
                return Ok(None);
            }
            let constant = i64::from_le_bytes(bytes[..8].try_into().unwrap());
            Ok(Some(vec![constant; values.len()]))
        }
        HotPlan::Interpret => {
            // Try JIT first — fastest for vanilla v0/v0.2 programs.
            if let Some(out) = execute_jit_batch_i64(hot, values)? {
                return Ok(Some(out));
            }
            // JIT rejected (KASM v1.0 meta-ops like Op::Cond, Op::Memoize,
            // Op::Fractal, etc — see kasm/jit.rs:114). Without this
            // fallback, `call_many_values_i64` would tumble onto the
            // dedup + per-call dispatch_call path which costs ~3 µs/call
            // (full cascade overhead). For 100k branched k-mer calls,
            // that's ~300 ms vs ~10 ms via tight interpreter loop.
            //
            // Mesuré sur DNA bench (cb583ac) : `branched` (Op::Cond)
            // restait à 3050 ns/call en batch alors que tous les autres
            // Léger descendaient à 3-37 ns. C'était la fall-back
            // scalar via dispatch_call.
            //
            // The interpreter `kasm::execute` already handles all KASM
            // v1.0 opcodes correctly — it's the JIT codegen that bails.
            // So we just call it in a tight loop without the dispatch
            // cascade overhead.
            execute_interpret_batch_i64(hot, values)
        }
        _ => Ok(None),
    }
}

/// Tight-loop interpreter batch lane for Interpret programs the JIT
/// can't compile (KASM v1.0 meta-ops). Skips the dispatch_call cascade
/// entirely — calls `kasm::execute` directly per value. Suitable for
/// 1-input → 1-output i64 programs (the gate is checked by caller via
/// `execute_hot_batch_i64`'s precondition).
fn execute_interpret_batch_i64(hot: &HotProgram, values: &[i64]) -> io::Result<Option<Vec<i64>>> {
    if hot.program.target().needs_external_backend() {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let arg_bytes = value.to_le_bytes();
        let result_bytes = match kasm::execute(&hot.program, &arg_bytes) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if result_bytes.len() != 8 {
            return Ok(None);
        }
        out.push(i64::from_le_bytes(result_bytes.try_into().unwrap()));
    }
    Ok(Some(out))
}

fn execute_with_jit(hot: &HotProgram, args: &[u8]) -> io::Result<Vec<u8>> {
    if !hot.jit_disabled.load(Ordering::Relaxed) {
        let kernel = {
            let mut slot = hot.jit.lock().unwrap();
            if slot.is_none() {
                match kasm::jit::compile(&hot.program) {
                    Ok(kernel) => {
                        *slot = Some(Arc::new(kernel));
                    }
                    Err(_) => {
                        hot.jit_disabled.store(true, Ordering::Relaxed);
                    }
                }
            }
            slot.clone()
        };
        if let Some(kernel) = kernel {
            if let Ok(bytes) = kernel.execute(args) {
                return Ok(bytes);
            }
        }
    }

    kasm::execute(&hot.program, args).map_err(|err| io::Error::other(format!("kasm: {err}")))
}

fn execute_jit_batch_i64(hot: &HotProgram, values: &[i64]) -> io::Result<Option<Vec<i64>>> {
    if hot.jit_disabled.load(Ordering::Relaxed) {
        return Ok(None);
    }

    let kernel = {
        let mut slot = hot.jit.lock().unwrap();
        if slot.is_none() {
            match kasm::jit::compile(&hot.program) {
                Ok(kernel) => {
                    *slot = Some(Arc::new(kernel));
                }
                Err(_) => {
                    hot.jit_disabled.store(true, Ordering::Relaxed);
                    return Ok(None);
                }
            }
        }
        slot.clone()
    };

    let Some(kernel) = kernel else {
        return Ok(None);
    };
    let mut out = vec![0i64; values.len()];
    match kernel.execute_batch_i64(values, &mut out) {
        Ok(()) => Ok(Some(out)),
        Err(_) => Ok(None),
    }
}

/// Vrai si ce programme atteindra un fast path CPU à 75-300 ns/call :
///   - HotPlan::AffineI64 (auto-router v0)
///   - HotPlan::HashChain (auto-router v1)
///   - HotPlan::StaticOutput (Layer 0)
///   - HotPlan::Interpret avec ≤ 64 nodes + 1 input i64 + 1 output i64
///     (auto-router v2 stack interp)
///
/// Utilisé par `dispatch_batch` pour bypass entirely la cascade
/// dispatch_impl (3 µs/call d'overhead) sur les programmes Léger,
/// et par `gpunode::eval_batch` pour skip le GPU.
pub(super) fn is_cpu_auto_routable(hot: &HotProgram) -> bool {
    match &hot.plan {
        HotPlan::AffineI64 { .. } | HotPlan::HashChain { .. } | HotPlan::StaticOutput(_) => true,
        HotPlan::Interpret => {
            hot.program.nodes().len() <= 64
                && hot.program.inputs() == 1
                && hot.program.outputs() == 1
                && hot.program.output_types() == vec![Ty::I64]
                && hot.explicit_memos.is_empty()
                && !hot.program.target().needs_external_backend()
        }
    }
}

pub(super) fn oracle_eligible(hot: &HotProgram) -> bool {
    matches!(hot.plan, HotPlan::Interpret)
        && hot.program.inputs() == 1
        && hot.program.outputs() == 1
        && hot.program.output_types() == vec![Ty::I64]
}

pub(super) fn exact_program_identity(program: &Program, canonical_hash: &Hash) -> [u8; 32] {
    let mut identity = [0u8; 32];
    identity[..20].copy_from_slice(canonical_hash.as_bytes());
    identity[30] = 0xff;
    identity[31] = program.target() as u8;
    identity
}

pub(super) fn reject_external_target(program: &Program) -> io::Result<()> {
    if program.target().needs_external_backend() {
        return Err(io::Error::other(match program.target() {
            Target::Qpu => "QPU target has no local classical fallback",
            _ => "external target has no local fallback",
        }));
    }
    Ok(())
}

fn affine_rule(program: &Program) -> Option<(u8, i64, i64)> {
    #[derive(Clone, Copy)]
    struct Affine {
        input_slot: Option<u8>,
        mul: i64,
        add: i64,
    }

    fn add_affine(a: Affine, b: Affine) -> Option<Affine> {
        match (a.input_slot, b.input_slot) {
            (None, None) => Some(Affine { input_slot: None, mul: 0, add: a.add.wrapping_add(b.add) }),
            (Some(slot), None) | (None, Some(slot)) => Some(Affine {
                input_slot: Some(slot),
                mul: a.mul.wrapping_add(b.mul),
                add: a.add.wrapping_add(b.add),
            }),
            (Some(left), Some(right)) if left == right => Some(Affine {
                input_slot: Some(left),
                mul: a.mul.wrapping_add(b.mul),
                add: a.add.wrapping_add(b.add),
            }),
            _ => None,
        }
    }

    fn sub_affine(a: Affine, b: Affine) -> Option<Affine> {
        match (a.input_slot, b.input_slot) {
            (None, None) => Some(Affine { input_slot: None, mul: 0, add: a.add.wrapping_sub(b.add) }),
            (Some(slot), None) => Some(Affine {
                input_slot: Some(slot),
                mul: a.mul,
                add: a.add.wrapping_sub(b.add),
            }),
            (None, Some(slot)) => Some(Affine {
                input_slot: Some(slot),
                mul: b.mul.wrapping_neg(),
                add: a.add.wrapping_sub(b.add),
            }),
            (Some(left), Some(right)) if left == right => Some(Affine {
                input_slot: Some(left),
                mul: a.mul.wrapping_sub(b.mul),
                add: a.add.wrapping_sub(b.add),
            }),
            _ => None,
        }
    }

    fn mul_affine(a: Affine, b: Affine) -> Option<Affine> {
        match (a.input_slot, b.input_slot) {
            (None, None) => Some(Affine { input_slot: None, mul: 0, add: a.add.wrapping_mul(b.add) }),
            (Some(slot), None) => Some(Affine {
                input_slot: Some(slot),
                mul: a.mul.wrapping_mul(b.add),
                add: a.add.wrapping_mul(b.add),
            }),
            (None, Some(slot)) => Some(Affine {
                input_slot: Some(slot),
                mul: b.mul.wrapping_mul(a.add),
                add: b.add.wrapping_mul(a.add),
            }),
            _ => None,
        }
    }

    let output_index = program
        .nodes()
        .iter()
        .position(|node| node.op == Op::Output && node.ty == Ty::I64)?;

    let mut values = Vec::with_capacity(program.nodes().len());
    for node in program.nodes().iter().copied() {
        let value = match node.op {
            Op::Input if node.imm >= 0 => Affine { input_slot: Some(node.imm as u8), mul: 1, add: 0 },
            Op::ConstI64 => Affine { input_slot: None, mul: 0, add: node.imm as i64 },
            Op::AddI64 => add_affine(values[node.a as usize], values[node.b as usize])?,
            Op::SubI64 => sub_affine(values[node.a as usize], values[node.b as usize])?,
            Op::MulI64 => mul_affine(values[node.a as usize], values[node.b as usize])?,
            Op::Output if node.ty == Ty::I64 => values[node.a as usize],
            _ => return None,
        };
        values.push(value);
    }

    let output = values.get(output_index).copied()?;
    Some((output.input_slot?, output.mul, output.add))
}
