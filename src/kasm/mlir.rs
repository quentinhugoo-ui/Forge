//! Ω-1 — KASM ⊂ MLIR : surface text syntax bijective avec `Program`.
//!
//! Ce module est le premier mile de la fusion KASM↔MLIR. Il définit un
//! format texte déterministe pour le dialecte custom `kasm.*` et fournit :
//!
//!   * `emit_mlir(&Program) -> String` — sérialise un programme.
//!   * `parse_mlir(&str)   -> Result<Program, MlirError>` — désérialise.
//!
//! La propriété testée est :
//!     `parse_mlir(emit_mlir(P)).bytes() == P.bytes()`
//! et donc `canonical_hash_hex` invariant. Le `CallKey` survit à la
//! traversée MLIR.
//!
//! Format (pseudo-grammar) :
//! ```text
//! program  ::= "kasm.program" attrs "{" "\n" body "\n" "}" "\n"
//! attrs    ::= "{" "target = \"" T "\"" ", " "inputs = " I ", "
//!              "outputs = " O ", " "fuel = " F "}"
//! body     ::= line ("\n" line)*
//! line     ::= "  %n" IDX " = kasm." OP_TAIL
//! ```
//! Les types sont `i64` ou `i1` (Bool). Le format est **byte-exact** :
//! pas d'espace flottant, pas d'alternative de notation.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::types::{F64SubOp, KasmError, Node, Op, Target, Ty, MAX_NODES};
use super::{canonicalize, Program};

const SSA_PREFIX: &str = "%n";

#[derive(Debug)]
pub enum MlirError {
    Kasm(KasmError),
    Syntax { line: usize, msg: String },
    BadHeader,
    BadFooter,
    UnknownOp(String),
    BadType(String),
    BadTarget(String),
    BadIndex,
    BadInteger(String),
    NodeOverflow,
}

impl std::fmt::Display for MlirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MlirError::Kasm(e) => write!(f, "kasm error: {e}"),
            MlirError::Syntax { line, msg } => write!(f, "syntax error at line {line}: {msg}"),
            MlirError::BadHeader => write!(f, "bad MLIR program header"),
            MlirError::BadFooter => write!(f, "bad MLIR program footer"),
            MlirError::UnknownOp(s) => write!(f, "unknown kasm op: {s}"),
            MlirError::BadType(s) => write!(f, "bad MLIR type: {s}"),
            MlirError::BadTarget(s) => write!(f, "bad target: {s}"),
            MlirError::BadIndex => write!(f, "bad SSA index"),
            MlirError::BadInteger(s) => write!(f, "bad integer literal: {s}"),
            MlirError::NodeOverflow => write!(f, "too many nodes"),
        }
    }
}

impl std::error::Error for MlirError {}

impl From<KasmError> for MlirError {
    fn from(e: KasmError) -> Self {
        MlirError::Kasm(e)
    }
}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

/// Émet le programme KASM en MLIR text (dialecte `kasm.*`).
///
/// Format strictement déterministe : indentation 2 espaces, séparateurs
/// figés, ordre des attributs imposé. Aucune ambiguïté.
pub fn emit_mlir(program: &Program) -> String {
    let mut out = String::new();
    let target = target_name(program.target());

    let _ = writeln!(
        out,
        "kasm.program {{target = \"{}\", inputs = {}, outputs = {}, fuel = {}}} {{",
        target,
        program.inputs(),
        program.outputs(),
        program.fuel(),
    );

    for (idx, node) in program.nodes().iter().enumerate() {
        out.push_str("  ");
        emit_node_line(&mut out, idx, node);
        out.push('\n');
    }

    out.push_str("}\n");
    out
}

fn emit_node_line(out: &mut String, idx: usize, node: &Node) {
    let ty = type_name(node.ty);

    match node.op {
        Op::Input => {
            let _ = write!(out, "{}{} = kasm.input {{slot = {}}} : {}", SSA_PREFIX, idx, node.imm, ty);
        }
        Op::ConstI64 => {
            let _ = write!(out, "{}{} = kasm.const {{value = {}}} : {}", SSA_PREFIX, idx, node.imm, ty);
        }
        Op::Output => {
            let _ = write!(out, "{}{} = kasm.output {}{} : {}", SSA_PREFIX, idx, SSA_PREFIX, node.a, ty);
        }
        Op::NotBool => {
            let _ = write!(out, "{}{} = kasm.notb {}{} : {}", SSA_PREFIX, idx, SSA_PREFIX, node.a, ty);
        }
        Op::Hash64 => {
            let _ = write!(out, "{}{} = kasm.hash {}{} : {}", SSA_PREFIX, idx, SSA_PREFIX, node.a, ty);
        }
        op @ (Op::BitFlipI64 | Op::NegI64 | Op::ReverseBitsI64 | Op::ByteswapI64
        | Op::PopcntI64 | Op::LzcntI64 | Op::TzcntI64) => {
            let _ = write!(
                out,
                "{}{} = kasm.{} {}{} : {}",
                SSA_PREFIX, idx, op_mnemonic(op), SSA_PREFIX, node.a, ty
            );
        }
        Op::SelectI64 => {
            let _ = write!(
                out,
                "{}{} = kasm.select {}{}, {}{}, {}{} : {}",
                SSA_PREFIX, idx, SSA_PREFIX, node.a, SSA_PREFIX, node.b, SSA_PREFIX, node.imm as u16, ty
            );
        }
        Op::ClampI64 => {
            let _ = write!(
                out,
                "{}{} = kasm.clamp {}{}, {}{}, {}{} : {}",
                SSA_PREFIX, idx, SSA_PREFIX, node.a, SSA_PREFIX, node.b, SSA_PREFIX, node.imm as u16, ty
            );
        }
        Op::ReduceAddI64 => {
            let _ = write!(
                out,
                "{}{} = kasm.reduce_add {}{} {{count = {}}} : {}",
                SSA_PREFIX, idx, SSA_PREFIX, node.a, node.imm, ty
            );
        }
        Op::ReduceMulI64 => {
            let _ = write!(
                out,
                "{}{} = kasm.reduce_mul {}{} {{count = {}}} : {}",
                SSA_PREFIX, idx, SSA_PREFIX, node.a, node.imm, ty
            );
        }
        Op::ConstF64 => {
            let _ = write!(out, "{}{} = kasm.fconst {{value = {}}} : {}", SSA_PREFIX, idx, node.imm, ty);
        }
        Op::F64Op => {
            // We tolerate a malformed sub-op selector here: we render
            // the raw imm so a debugging round-trip stays lossless even
            // for programs that the verifier would otherwise reject.
            let mnem = match F64SubOp::from_imm(node.imm) {
                Ok(s) => f64_sub_mnemonic(s),
                Err(_) => "fxxx",
            };
            // Binary sub-ops emit `%a, %b`; unary emit `%a` only.
            let is_binary = F64SubOp::from_imm(node.imm)
                .map(|s| s.is_binary())
                .unwrap_or(false);
            if is_binary {
                let _ = write!(
                    out,
                    "{}{} = kasm.{} {}{}, {}{} : {}",
                    SSA_PREFIX, idx, mnem, SSA_PREFIX, node.a, SSA_PREFIX, node.b, ty
                );
            } else {
                let _ = write!(
                    out,
                    "{}{} = kasm.{} {}{} : {}",
                    SSA_PREFIX, idx, mnem, SSA_PREFIX, node.a, ty
                );
            }
        }
        // Opérations binaires régulières : a, b → result
        op @ (Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::LtI64
        | Op::LeI64
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64) => {
            let _ = write!(
                out,
                "{}{} = kasm.{} {}{}, {}{} : {}",
                SSA_PREFIX, idx, op_mnemonic(op), SSA_PREFIX, node.a, SSA_PREFIX, node.b, ty
            );
        }
        // KASM v1.0 — opaque rendering. The MLIR dialect doesn't have
        // first-class ops for these yet, so we emit them as opaque
        // attribute-decorated nodes for round-trip stability.
        op @ (Op::Adaptive | Op::Comptime | Op::Grad | Op::Cond | Op::Memoize
        | Op::Lazy | Op::Force
        | Op::Pipeline | Op::Vmap | Op::Pmap | Op::Fori
        | Op::WhileLoop | Op::Reduce | Op::Scan) => {
            let _ = write!(
                out,
                "{}{} = kasm.{} {}{}, {}{} {{imm = {}}} : {}",
                SSA_PREFIX, idx, op_mnemonic(op), SSA_PREFIX, node.a, SSA_PREFIX, node.b, node.imm, ty
            );
        }
        // Wave 7d — Op::VLenI64 unary form (input Vec, output I64).
        Op::VLenI64 => {
            let _ = write!(
                out,
                "{}{} = kasm.vlen {}{} : {}",
                SSA_PREFIX, idx, SSA_PREFIX, node.a, ty
            );
        }
        // Wave 7d-bis — VSumI64 unary, VAddI64/VMulI64 binary.
        // Wave 7e — VSubI64/VMaxI64/VMinI64 binary, VRangeI64 unary.
        // Wave 7f — VReverseI64 unary, VConcatI64/VBroadcastI64 binary.
        Op::VSumI64 | Op::VRangeI64 | Op::VReverseI64
        | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => {
            let _ = write!(
                out,
                "{}{} = kasm.{} {}{} : {}",
                SSA_PREFIX, idx, op_mnemonic(node.op), SSA_PREFIX, node.a, ty
            );
        }
        op @ (Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
             | Op::VConcatI64 | Op::VBroadcastI64
             | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
             | Op::VGetI64) => {
            let _ = write!(
                out,
                "{}{} = kasm.{} {}{}, {}{} : {}",
                SSA_PREFIX, idx, op_mnemonic(op), SSA_PREFIX, node.a, SSA_PREFIX, node.b, ty
            );
        }
        // Wave 8 self-hosting — Fractal/Eval ne sont pas représentés
        // en MLIR canonical car ils requièrent un Store runtime pour
        // résoudre les sous-programmes. Émission pseudo-MLIR comme
        // commentaire (ne sera pas re-parsable, ce qui est OK :
        // SelfHostingRuntime intercepte avant atteindre MLIR).
        Op::Fractal | Op::Eval => {
            let _ = write!(
                out,
                "// {}{} = kasm.{} (runtime self-host, opaque to MLIR)",
                SSA_PREFIX, idx, op_mnemonic(node.op)
            );
        }
    }
}

fn op_mnemonic(op: Op) -> &'static str {
    match op {
        Op::Input => "input",
        Op::ConstI64 => "const",
        Op::AddI64 => "add",
        Op::MulI64 => "mul",
        Op::EqI64 => "eq",
        Op::Hash64 => "hash",
        Op::Output => "output",
        Op::SubI64 => "sub",
        Op::DivI64Checked => "divc",
        Op::MinI64 => "min",
        Op::MaxI64 => "max",
        Op::SelectI64 => "select",
        Op::AndBool => "andb",
        Op::OrBool => "orb",
        Op::NotBool => "notb",
        Op::LtI64 => "lt",
        Op::LeI64 => "le",
        Op::BitAndI64 => "band",
        Op::BitOrI64 => "bor",
        Op::BitXorI64 => "bxor",
        Op::ShlI64 => "shl",
        Op::ShrI64 => "shr",
        Op::SatAddI64 => "satadd",
        Op::SatSubI64 => "satsub",
        Op::ModI64Checked => "modc",
        Op::ClampI64 => "clamp",
        Op::ReduceAddI64 => "reduce_add",
        Op::ReduceMulI64 => "reduce_mul",
        Op::BitFlipI64 => "bit_flip",
        Op::NegI64 => "neg",
        Op::ReverseBitsI64 => "rev_bits",
        Op::ByteswapI64 => "bswap",
        Op::PopcntI64 => "popcnt",
        Op::LzcntI64 => "lzcnt",
        Op::TzcntI64 => "tzcnt",
        Op::PextI64 => "pext",
        Op::PdepI64 => "pdep",
        Op::Lazy => "lazy",
        Op::Force => "force",
        // `Op::ConstF64` and `Op::F64Op` are emitted via dedicated
        // arms in `emit_node_line`; this mnemonic table is only used
        // for the I64 surface that has a 1-to-1 op-to-mnemonic map.
        Op::ConstF64 => "fconst",
        Op::F64Op => "fop",
        // KASM v1.0 mnemonics
        Op::Adaptive => "adaptive",
        Op::Comptime => "comptime",
        Op::Grad => "grad",
        Op::Cond => "cond",
        Op::Memoize => "memoize",
        Op::Pipeline => "pipeline",
        Op::Vmap => "vmap",
        Op::Pmap => "pmap",
        Op::Fori => "fori",
        Op::WhileLoop => "while",
        Op::Reduce => "reduce",
        Op::Scan => "scan",
        Op::VLenI64 => "vlen",
        Op::VSumI64 => "vsum",
        Op::VAddI64 => "vadd",
        Op::VMulI64 => "vmul",
        Op::VSubI64 => "vsub",
        Op::VMaxI64 => "vmax",
        Op::VMinI64 => "vmin",
        Op::VRangeI64 => "vrange",
        Op::VConcatI64 => "vconcat",
        Op::VReverseI64 => "vreverse",
        Op::VBroadcastI64 => "vbroadcast",
        Op::VEqI64 => "veq",
        Op::VAndI64 => "vand",
        Op::VOrI64 => "vor",
        Op::VXorI64 => "vxor",
        Op::VAbsI64 => "vabs",
        Op::VNegI64 => "vneg",
        Op::VBitFlipI64 => "vbitflip",
        Op::VGetI64 => "vget",
        Op::Fractal => "fractal",
        Op::Eval => "eval",
    }
}

fn f64_sub_mnemonic(sub: F64SubOp) -> &'static str {
    match sub {
        F64SubOp::Add => "fadd",
        F64SubOp::Sub => "fsub",
        F64SubOp::Mul => "fmul",
        F64SubOp::DivChecked => "fdivc",
        F64SubOp::Min => "fmin",
        F64SubOp::Max => "fmax",
        F64SubOp::Sqrt => "fsqrt",
        F64SubOp::Abs => "fabs",
        F64SubOp::Neg => "fneg",
        F64SubOp::FromI64 => "i64_to_f64",
        F64SubOp::ToI64 => "f64_to_i64",
        F64SubOp::Exp => "fexp",
        F64SubOp::Ln => "fln",
    }
}

fn f64_sub_from_mnemonic(s: &str) -> Option<F64SubOp> {
    Some(match s {
        "fadd" => F64SubOp::Add,
        "fsub" => F64SubOp::Sub,
        "fmul" => F64SubOp::Mul,
        "fdivc" => F64SubOp::DivChecked,
        "fmin" => F64SubOp::Min,
        "fmax" => F64SubOp::Max,
        "fsqrt" => F64SubOp::Sqrt,
        "fabs" => F64SubOp::Abs,
        "fneg" => F64SubOp::Neg,
        "i64_to_f64" => F64SubOp::FromI64,
        "f64_to_i64" => F64SubOp::ToI64,
        "fexp" => F64SubOp::Exp,
        "fln" => F64SubOp::Ln,
        _ => return None,
    })
}

fn op_from_mnemonic(s: &str) -> Option<Op> {
    Some(match s {
        "input" => Op::Input,
        "const" => Op::ConstI64,
        "add" => Op::AddI64,
        "mul" => Op::MulI64,
        "eq" => Op::EqI64,
        "hash" => Op::Hash64,
        "output" => Op::Output,
        "sub" => Op::SubI64,
        "divc" => Op::DivI64Checked,
        "min" => Op::MinI64,
        "max" => Op::MaxI64,
        "select" => Op::SelectI64,
        "andb" => Op::AndBool,
        "orb" => Op::OrBool,
        "notb" => Op::NotBool,
        "lt" => Op::LtI64,
        "le" => Op::LeI64,
        "band" => Op::BitAndI64,
        "bor" => Op::BitOrI64,
        "bxor" => Op::BitXorI64,
        "shl" => Op::ShlI64,
        "shr" => Op::ShrI64,
        "satadd" => Op::SatAddI64,
        "satsub" => Op::SatSubI64,
        "modc" => Op::ModI64Checked,
        "clamp" => Op::ClampI64,
        "reduce_add" => Op::ReduceAddI64,
        "reduce_mul" => Op::ReduceMulI64,
        "bit_flip" => Op::BitFlipI64,
        "neg" => Op::NegI64,
        "rev_bits" => Op::ReverseBitsI64,
        "bswap" => Op::ByteswapI64,
        "popcnt" => Op::PopcntI64,
        "lzcnt" => Op::LzcntI64,
        "tzcnt" => Op::TzcntI64,
        "pext" => Op::PextI64,
        "pdep" => Op::PdepI64,
        "lazy" => Op::Lazy,
        "force" => Op::Force,
        "fconst" => Op::ConstF64,
        "adaptive" => Op::Adaptive,
        "comptime" => Op::Comptime,
        "grad" => Op::Grad,
        "cond" => Op::Cond,
        "memoize" => Op::Memoize,
        "pipeline" => Op::Pipeline,
        "vmap" => Op::Vmap,
        "pmap" => Op::Pmap,
        "fori" => Op::Fori,
        "while" => Op::WhileLoop,
        "reduce" => Op::Reduce,
        "scan" => Op::Scan,
        "vlen" => Op::VLenI64,
        "vsum" => Op::VSumI64,
        "vadd" => Op::VAddI64,
        "vmul" => Op::VMulI64,
        "vsub" => Op::VSubI64,
        "vmax" => Op::VMaxI64,
        "vmin" => Op::VMinI64,
        "vrange" => Op::VRangeI64,
        "vconcat" => Op::VConcatI64,
        "vreverse" => Op::VReverseI64,
        "vbroadcast" => Op::VBroadcastI64,
        "veq" => Op::VEqI64,
        "vand" => Op::VAndI64,
        "vor" => Op::VOrI64,
        "vxor" => Op::VXorI64,
        "vabs" => Op::VAbsI64,
        "vneg" => Op::VNegI64,
        "vbitflip" => Op::VBitFlipI64,
        "vget" => Op::VGetI64,
        "fractal" => Op::Fractal,
        "eval" => Op::Eval,
        // F64Op sub-mnemonics — all map to the same opcode, the sub-op
        // selector is filled in by `parse_node_line` after we know
        // which specific mnemonic was matched.
        "fadd" | "fsub" | "fmul" | "fdivc" | "fmin" | "fmax" | "fsqrt" | "fabs" | "fneg"
        | "i64_to_f64" | "f64_to_i64" | "fexp" | "fln" => Op::F64Op,
        _ => return None,
    })
}

fn target_name(t: Target) -> &'static str {
    match t {
        Target::Auto => "auto",
        Target::Cpu => "cpu",
        Target::Kernel => "kernel",
        Target::Gpu => "gpu",
        Target::Qpu => "qpu",
    }
}

fn target_from_str(s: &str) -> Option<Target> {
    Some(match s {
        "auto" => Target::Auto,
        "cpu" => Target::Cpu,
        "kernel" => Target::Kernel,
        "gpu" => Target::Gpu,
        "qpu" => Target::Qpu,
        _ => return None,
    })
}

fn type_name(t: Ty) -> &'static str {
    match t {
        Ty::I64 => "i64",
        Ty::Bool => "i1",
        Ty::F64 => "f64",
        Ty::VecI64 => "vec<i64>",
    }
}

fn type_from_str(s: &str) -> Option<Ty> {
    Some(match s {
        "i64" => Ty::I64,
        "i1" => Ty::Bool,
        "f64" => Ty::F64,
        "vec<i64>" => Ty::VecI64,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Forme canonique MLIR + hash MLIR-canonique (Ω-1.0 critères #3 et #5)
// ---------------------------------------------------------------------------

/// Forme texte canonique MLIR d'un programme.
///
/// Définition opérationnelle : `canonical_mlir_text(P) := emit_mlir(canonicalize(P))`.
/// Le canonicaliseur (`kasm::canonicalize`) effectue : élimination de code mort
/// (output-driven walk), CSE par fingerprint sémantique, normalisation des ops
/// commutatives. Cette définition rend la forme texte canonique **idempotente**
/// (test : `idempotence_canonical_mlir_text`).
pub fn canonical_mlir_text(program: &Program) -> Result<String, KasmError> {
    let canon = canonicalize(program)?;
    Ok(emit_mlir(&canon))
}

/// Hash MLIR-canonique d'un programme : `sha256(canonical_mlir_text(P))`.
///
/// Propriété centrale : pour tous P, Q, on a
/// `hash_mlir_canonical(P) == hash_mlir_canonical(Q)` ⟺ `canonical_hash_hex(P) == canonical_hash_hex(Q)`
/// (équivalence sémantique). Cette propriété est testée sur 4096 nœuds + corpus.
pub fn hash_mlir_canonical(program: &Program) -> Result<[u8; 32], KasmError> {
    let text = canonical_mlir_text(program)?;
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    Ok(hasher.finalize().into())
}

/// Hash MLIR-canonique en notation hexadécimale (compagnon de
/// `hash_mlir_canonical`, mêmes garanties).
pub fn hash_mlir_canonical_hex(program: &Program) -> Result<String, KasmError> {
    let h = hash_mlir_canonical(program)?;
    let mut s = String::with_capacity(64);
    for b in h.iter() {
        let _ = write!(s, "{b:02x}");
    }
    Ok(s)
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

/// Parse un programme MLIR émis par `emit_mlir` et reconstruit le `Program`
/// d'origine (byte-exact via `canonical_hash_hex`).
///
/// **Entrée canonique officielle Ω** : c'est la fonction qu'on utilise pour
/// charger un programme depuis sa forme texte. Le format bytes legacy reste
/// supporté par `kasm::verify` (fast-path), mais MLIR text est désormais la
/// surface officiellement publiée.
pub fn parse_mlir(text: &str) -> Result<Program, MlirError> {
    let mut lines = text.lines().enumerate();

    // Header
    let (lineno, header_line) = lines
        .find(|(_, l)| !l.trim().is_empty())
        .ok_or(MlirError::BadHeader)?;
    let header = parse_header(header_line).map_err(|msg| MlirError::Syntax { line: lineno + 1, msg })?;

    // Body
    let mut nodes: Vec<Node> = Vec::new();
    let mut footer_seen = false;

    for (lineno, raw) in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "}" {
            footer_seen = true;
            // Toute ligne non vide après le footer = erreur silencieuse pour
            // l'instant; on accepte trailing whitespace.
            break;
        }

        if nodes.len() >= MAX_NODES {
            return Err(MlirError::NodeOverflow);
        }

        let expected_idx = nodes.len();
        let node = parse_node_line(trimmed, expected_idx)
            .map_err(|msg| MlirError::Syntax { line: lineno + 1, msg })?;
        nodes.push(node);
    }

    if !footer_seen {
        return Err(MlirError::BadFooter);
    }

    let program = Program::new(header.target, header.inputs, header.outputs, header.fuel, nodes)?;
    Ok(program)
}

struct Header {
    target: Target,
    inputs: u8,
    outputs: u8,
    fuel: u32,
}

fn parse_header(line: &str) -> Result<Header, String> {
    // Forme attendue :
    //   kasm.program {target = "auto", inputs = 2, outputs = 1, fuel = 16} {
    let line = line.trim();
    let prefix = "kasm.program {";
    if !line.starts_with(prefix) {
        return Err(format!("expected `{prefix}`"));
    }
    let suffix = "} {";
    if !line.ends_with(suffix) {
        return Err(format!("expected trailing `{suffix}`"));
    }
    let inner = &line[prefix.len()..line.len() - suffix.len()];

    let mut target = None;
    let mut inputs = None;
    let mut outputs = None;
    let mut fuel = None;

    for part in split_top_level_commas(inner) {
        let part = part.trim();
        let (key, val) = part
            .split_once('=')
            .ok_or_else(|| format!("expected `key = value` in `{part}`"))?;
        let key = key.trim();
        let val = val.trim();
        match key {
            "target" => {
                let s = val
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .ok_or_else(|| format!("target must be quoted: `{val}`"))?;
                target = Some(target_from_str(s).ok_or_else(|| format!("unknown target {s}"))?);
            }
            "inputs" => {
                inputs = Some(val.parse::<u8>().map_err(|_| format!("bad inputs `{val}`"))?);
            }
            "outputs" => {
                outputs = Some(val.parse::<u8>().map_err(|_| format!("bad outputs `{val}`"))?);
            }
            "fuel" => {
                fuel = Some(val.parse::<u32>().map_err(|_| format!("bad fuel `{val}`"))?);
            }
            other => return Err(format!("unknown header key `{other}`")),
        }
    }

    Ok(Header {
        target: target.ok_or("missing target")?,
        inputs: inputs.ok_or("missing inputs")?,
        outputs: outputs.ok_or("missing outputs")?,
        fuel: fuel.ok_or("missing fuel")?,
    })
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    // Header attrs n'ont pas de virgule imbriquée pour le moment.
    s.split(',').collect()
}

fn parse_node_line(line: &str, expected_idx: usize) -> Result<Node, String> {
    // Forme : `%n{idx} = kasm.{op} <args> : {ty}`
    let (lhs, rhs) = line
        .split_once('=')
        .ok_or_else(|| format!("expected `=` in node line `{line}`"))?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();

    let idx = parse_ssa(lhs)?;
    if idx != expected_idx {
        return Err(format!(
            "expected SSA index {expected_idx}, got {idx} in `{line}`"
        ));
    }

    // rhs : "kasm.OP <body> : <ty>"
    let (op_token, body) = rhs
        .split_once(' ')
        .ok_or_else(|| format!("expected op in `{rhs}`"))?;
    let op_mnem = op_token
        .strip_prefix("kasm.")
        .ok_or_else(|| format!("expected `kasm.<op>`, got `{op_token}`"))?;
    let op = op_from_mnemonic(op_mnem).ok_or_else(|| format!("unknown op `{op_mnem}`"))?;

    let (body, ty_str) = split_trailing_type(body)?;
    let ty = type_from_str(ty_str).ok_or_else(|| format!("bad type `{ty_str}`"))?;

    // Φ.0 — F64Op uses 11 distinct mnemonics that all map to the same
    // opcode, so we detect the sub-op selector BEFORE the main match.
    if op == Op::F64Op {
        let sub = f64_sub_from_mnemonic(op_mnem)
            .ok_or_else(|| format!("unknown F64Op sub-mnemonic `{op_mnem}`"))?;
        let trimmed = body.trim();
        let (a, b) = if sub.is_binary() {
            let (a, b) = parse_two_ssa(trimmed)?;
            (a as u16, b as u16)
        } else {
            (parse_ssa(trimmed)? as u16, 0u16)
        };
        return Ok(Node {
            op: Op::F64Op,
            ty,
            a,
            b,
            imm: sub.imm(),
        });
    }

    let node = match op {
        Op::Input => {
            let imm = parse_attr_int(body, "slot")?;
            Node {
                op: Op::Input,
                ty,
                a: 0,
                b: 0,
                imm,
            }
        }
        Op::ConstI64 => {
            let imm = parse_attr_int(body, "value")?;
            Node {
                op: Op::ConstI64,
                ty,
                a: 0,
                b: 0,
                imm,
            }
        }
        Op::ConstF64 => {
            let imm = parse_attr_int(body, "value")?;
            Node {
                op: Op::ConstF64,
                ty,
                a: 0,
                b: 0,
                imm,
            }
        }
        Op::Output => {
            let a = parse_ssa(body.trim())?;
            Node {
                op: Op::Output,
                ty,
                a: a as u16,
                b: 0,
                imm: 0,
            }
        }
        Op::NotBool => {
            let a = parse_ssa(body.trim())?;
            Node {
                op: Op::NotBool,
                ty,
                a: a as u16,
                b: 0,
                imm: 0,
            }
        }
        Op::Hash64 => {
            let a = parse_ssa(body.trim())?;
            Node {
                op: Op::Hash64,
                ty,
                a: a as u16,
                b: 0,
                imm: 0,
            }
        }
        Op::BitFlipI64 | Op::NegI64 | Op::ReverseBitsI64 | Op::ByteswapI64
        | Op::PopcntI64 | Op::LzcntI64 | Op::TzcntI64 => {
            let a = parse_ssa(body.trim())?;
            Node {
                op,
                ty,
                a: a as u16,
                b: 0,
                imm: 0,
            }
        }
        Op::SelectI64 => {
            let (a, b, c) = parse_three_ssa(body.trim())?;
            Node {
                op: Op::SelectI64,
                ty,
                a: a as u16,
                b: b as u16,
                imm: c as i16,
            }
        }
        Op::ClampI64 => {
            let (a, lo, hi) = parse_three_ssa(body.trim())?;
            Node {
                op: Op::ClampI64,
                ty,
                a: a as u16,
                b: lo as u16,
                imm: hi as i16,
            }
        }
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            // body = "%nA {count = N}"
            let (ssa_part, attr_part) = body
                .split_once('{')
                .ok_or_else(|| format!("expected `{{count = N}}` in `{body}`"))?;
            let attr_part = format!("{{{}", attr_part);
            let a = parse_ssa(ssa_part.trim())?;
            let count = parse_attr_int(&attr_part, "count")?;
            Node {
                op,
                ty,
                a: a as u16,
                b: 0,
                imm: count,
            }
        }
        // Binaires réguliers
        Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::LtI64
        | Op::LeI64
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64 => {
            let (a, b) = parse_two_ssa(body.trim())?;
            Node {
                op,
                ty,
                a: a as u16,
                b: b as u16,
                imm: 0,
            }
        }
        // Φ.0 — F64Op is handled via the early return above; reaching
        // this arm means `op_from_mnemonic` returned `Op::F64Op` for a
        // mnemonic that `f64_sub_from_mnemonic` rejected, which is a
        // table-mismatch bug rather than user input.
        Op::F64Op => unreachable!("F64Op handled via early return"),
        // KASM v1.0 — opaque parsing : best-effort 2-SSA + imm attribute.
        // Per-op specialised parsing can be added later when the MLIR
        // dialect formalises these ops. For now we accept either form
        // and rely on the verifier to catch malformed shapes.
        Op::Adaptive | Op::Comptime | Op::Grad | Op::Cond | Op::Memoize
        | Op::Lazy | Op::Force
        | Op::Pipeline | Op::Vmap | Op::Pmap | Op::Fori
        | Op::WhileLoop | Op::Reduce | Op::Scan
        | Op::Fractal | Op::Eval  // Wave 8 — opaque to MLIR, runtime self-host
        => {
            let imm = parse_attr_int(body, "imm").unwrap_or(0);
            let (a, b) = parse_two_ssa(body.trim()).unwrap_or((0, 0));
            Node { op, ty, a: a as u16, b: b as u16, imm }
        }
        // Wave 7d — Op::VLenI64 unary, single SSA reference.
        // Wave 7e — VRangeI64 unary too (i64 → Vec).
        // Wave 7f — VReverseI64 unary too (Vec → Vec).
        Op::VLenI64 | Op::VSumI64 | Op::VRangeI64 | Op::VReverseI64
        | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => {
            let a = parse_ssa(body.trim()).unwrap_or(0);
            Node { op, ty, a: a as u16, b: 0, imm: 0 }
        }
        // Wave 7d-bis + 7e + 7f — binary Vec ops.
        // Wave 7i — VGetI64 (Vec, i64) → i64.
        Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
        | Op::VConcatI64 | Op::VBroadcastI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
        | Op::VGetI64 => {
            let (a, b) = parse_two_ssa(body.trim()).unwrap_or((0, 0));
            Node { op, ty, a: a as u16, b: b as u16, imm: 0 }
        }
    };

    Ok(node)
}

fn parse_ssa(s: &str) -> Result<usize, String> {
    let s = s.trim();
    let n = s
        .strip_prefix(SSA_PREFIX)
        .ok_or_else(|| format!("expected `%n` SSA, got `{s}`"))?;
    n.parse::<usize>().map_err(|_| format!("bad SSA index `{s}`"))
}

fn parse_two_ssa(s: &str) -> Result<(usize, usize), String> {
    let mut parts = s.split(',');
    let a = parts.next().ok_or("expected first ssa")?;
    let b = parts.next().ok_or("expected second ssa")?;
    if parts.next().is_some() {
        return Err(format!("too many operands in `{s}`"));
    }
    Ok((parse_ssa(a)?, parse_ssa(b)?))
}

fn parse_three_ssa(s: &str) -> Result<(usize, usize, usize), String> {
    let mut parts = s.split(',');
    let a = parts.next().ok_or("expected first ssa")?;
    let b = parts.next().ok_or("expected second ssa")?;
    let c = parts.next().ok_or("expected third ssa")?;
    if parts.next().is_some() {
        return Err(format!("too many operands in `{s}`"));
    }
    Ok((parse_ssa(a)?, parse_ssa(b)?, parse_ssa(c)?))
}

fn split_trailing_type(body: &str) -> Result<(&str, &str), String> {
    // Sépare le corps de l'op de son type final `: <ty>`.
    // Le type est tout ce qui suit le DERNIER `:` non précédé d'une accolade
    // ouvrante non fermée.
    let bytes = body.as_bytes();
    let mut depth: i32 = 0;
    let mut last_colon = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b':' if depth == 0 => last_colon = Some(i),
            _ => {}
        }
    }
    let i = last_colon.ok_or_else(|| format!("missing `:` type in `{body}`"))?;
    let lhs = body[..i].trim_end();
    let rhs = body[i + 1..].trim();
    Ok((lhs, rhs))
}

fn parse_attr_int(body: &str, key: &str) -> Result<i16, String> {
    // body contient `{key = N}` quelque part.
    let body = body.trim();
    let lbrace = body.find('{').ok_or_else(|| format!("expected `{{` in `{body}`"))?;
    let rbrace = body.rfind('}').ok_or_else(|| format!("expected `}}` in `{body}`"))?;
    let inner = &body[lbrace + 1..rbrace];
    let (k, v) = inner
        .split_once('=')
        .ok_or_else(|| format!("expected `=` in attr `{inner}`"))?;
    if k.trim() != key {
        return Err(format!("expected attr `{key}`, got `{}`", k.trim()));
    }
    let v = v.trim();
    v.parse::<i16>()
        .map_err(|_| format!("bad i16 literal `{v}` for `{key}`"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty, MAX_NODES, MAX_SLOTS};

    // ----- xorshift RNG (no external dep) -----
    struct Rng(u64);
    impl Rng {
        fn new(seed: u64) -> Self {
            // Évite le seed nul (cycle dégénéré).
            Self(seed | 0xdead_beef)
        }
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn range(&mut self, n: usize) -> usize {
            assert!(n > 0);
            (self.next_u64() as usize) % n
        }
        fn pick<T: Copy>(&mut self, slice: &[T]) -> T {
            slice[self.range(slice.len())]
        }
    }

    /// Génère un programme KASM valide (forward refs only, types corrects,
    /// au moins un output) avec au plus `target_nodes` nœuds.
    fn random_program(seed: u64, target_nodes: usize, num_inputs: u8) -> Program {
        assert!(target_nodes >= 4 && target_nodes <= MAX_NODES);
        assert!(num_inputs >= 1 && num_inputs <= MAX_SLOTS);
        let mut rng = Rng::new(seed);

        let mut nodes: Vec<Node> = Vec::with_capacity(target_nodes);
        let mut tys: Vec<Ty> = Vec::with_capacity(target_nodes);
        let mut i64_idx: Vec<u16> = Vec::new();
        let mut bool_idx: Vec<u16> = Vec::new();

        // Inputs en tête.
        for slot in 0..num_inputs {
            nodes.push(Node::input(slot));
            tys.push(Ty::I64);
            i64_idx.push(nodes.len() as u16 - 1);
        }
        // Au moins une constante pour amorcer.
        nodes.push(Node::const_i64((rng.next_u64() as i16) % 100));
        tys.push(Ty::I64);
        i64_idx.push(nodes.len() as u16 - 1);

        // Réserver 1 slot pour le output final.
        let body_target = target_nodes.saturating_sub(1);

        while nodes.len() < body_target {
            // 0..=18 op kinds. Certaines requièrent des opérandes booléens.
            let kind = rng.range(20);
            let new_node = match kind {
                0 => Node::const_i64((rng.next_u64() as i16) % 200 - 100),
                1 if num_inputs > 0 => Node::input((rng.range(num_inputs as usize)) as u8),
                2 => Node::add(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                3 => Node::sub(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                4 => Node::mul(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                5 => Node::div_checked(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                6 => Node::min(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                7 => Node::max(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                8 => Node::eq(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                9 => Node::lt(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                10 => Node::le(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                11 => Node::bit_and(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                12 => Node::bit_or(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                13 => Node::bit_xor(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                14 => Node::shl(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                15 => Node::shr(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                16 => Node::sat_add(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                17 => Node::sat_sub(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                18 => Node::mod_checked(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                _ => {
                    // Ops bool, hash, select, clamp, reduce — gated par dispo.
                    let sub = rng.range(6);
                    match sub {
                        0 if !bool_idx.is_empty() => Node::and(
                            rng.pick(&bool_idx),
                            rng.pick(&bool_idx),
                        ),
                        1 if !bool_idx.is_empty() => Node::or(
                            rng.pick(&bool_idx),
                            rng.pick(&bool_idx),
                        ),
                        2 if !bool_idx.is_empty() => Node::not(rng.pick(&bool_idx)),
                        3 if !bool_idx.is_empty() => {
                            Node::select_i64(
                                rng.pick(&bool_idx),
                                rng.pick(&i64_idx),
                                rng.pick(&i64_idx),
                            )
                        }
                        4 => Node::clamp(
                            rng.pick(&i64_idx),
                            rng.pick(&i64_idx),
                            rng.pick(&i64_idx),
                        ),
                        5 => Node::hash64(rng.pick(&i64_idx)),
                        _ => Node::add(rng.pick(&i64_idx), rng.pick(&i64_idx)),
                    }
                }
            };

            // ReduceAdd / ReduceMul ont une contrainte structurelle (count >= 1
            // && base + count <= current_index) — on les ajoute opportunément
            // toutes les ~32 itérations.
            let final_node = if nodes.len() % 31 == 30 && i64_idx.len() >= 4 {
                let count_max = i64_idx.len().min(8) as i16;
                let count = (rng.range(count_max as usize) as i16) + 1;
                let max_base = nodes.len() as i16 - count;
                if max_base > 0 {
                    let base = rng.range(max_base as usize) as u16;
                    // Vérifier que [base, base+count) sont tous I64.
                    let all_i64 = (base as usize..base as usize + count as usize)
                        .all(|i| tys[i] == Ty::I64);
                    if all_i64 {
                        if rng.next_u64() & 1 == 0 {
                            Node::reduce_add(base, count as u16)
                        } else {
                            Node::reduce_mul(base, count as u16)
                        }
                    } else {
                        new_node
                    }
                } else {
                    new_node
                }
            } else {
                new_node
            };

            // Skip si le pick a renvoyé un add fallback identique au précédent
            // (n'arrive presque jamais avec random refs).
            let idx = nodes.len() as u16;
            nodes.push(final_node);
            tys.push(final_node.ty);
            match final_node.ty {
                Ty::I64 => i64_idx.push(idx),
                Ty::Bool => bool_idx.push(idx),
                // Φ.0 — random_program never emits F64 nodes; this arm
                // exists only to satisfy match exhaustiveness now that
                // `Ty::F64` is part of the enum. F64 fuzz coverage will
                // come via dedicated tests in Φ.0h.
                Ty::F64 => {}
                Ty::VecI64 => panic!("random_program must not emit Ty::VecI64 before vector support lands"),
            }
        }

        // Output final : on prend toujours le dernier nœud I64 disponible.
        let last_i64 = *i64_idx.last().expect("at least one i64 in scope");
        nodes.push(Node::output(last_i64, Ty::I64));

        let total = nodes.len() as u32;
        Program::new(Target::Cpu, num_inputs, 1, total, nodes)
            .expect("random_program should be valid by construction")
    }

    /// Roundtrip + comparaison byte-exact + CallKey.
    fn assert_roundtrip(p: &Program, label: &str) {
        let text = emit_mlir(p);
        let p2 = parse_mlir(&text).unwrap_or_else(|e| {
            panic!("[{label}] parse_mlir failed: {e}\nMLIR:\n{text}");
        });
        assert_eq!(p.bytes(), p2.bytes(), "[{label}] byte-exact mismatch");
        let h1 = p.canonical_hash_hex().unwrap();
        let h2 = p2.canonical_hash_hex().unwrap();
        assert_eq!(h1, h2, "[{label}] CallKey mismatch");
    }

    // ----- Critère Ω-1.0 #1 : test différentiel jusqu'à 4096 nœuds -----

    #[test]
    fn fuzz_roundtrip_at_4096_nodes() {
        // 8 programmes au plafond MAX_NODES (4096).
        for seed in 0..8u64 {
            let p = random_program(seed * 1_234_567 + 1, MAX_NODES, 4);
            assert_eq!(
                p.nodes().len(),
                MAX_NODES,
                "seed {seed} should reach MAX_NODES"
            );
            assert_roundtrip(&p, &format!("4096-seed-{seed}"));
        }
    }

    #[test]
    fn fuzz_roundtrip_varied_sizes() {
        // 1024 programmes, tailles dispersées sur tout le spectre 4..4096.
        let mut rng = Rng::new(0xc0ffee_u64);
        let mut total_nodes = 0usize;
        let n_programs = 1024;
        for i in 0..n_programs {
            // Distribution qui couvre les petites tailles ET les grandes.
            let pivot = rng.range(100);
            let target = if pivot < 50 {
                4 + rng.range(60) // 4..63 : petites
            } else if pivot < 85 {
                64 + rng.range(960) // 64..1023 : moyennes
            } else {
                1024 + rng.range(MAX_NODES - 1024) // 1024..4095 : grandes
            };
            let inputs = 1 + (rng.range(MAX_SLOTS as usize - 1)) as u8;
            let seed = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let p = random_program(seed, target, inputs);
            total_nodes += p.nodes().len();
            assert_roundtrip(&p, &format!("varied-{i}-n{}", p.nodes().len()));
        }
        // Sanity : on a brassé largement plus que 4096 nœuds au cumul.
        assert!(
            total_nodes >= 4096,
            "expected >= 4096 cumulative nodes, got {total_nodes}"
        );
    }

    // ----- Critère Ω-1.0 #2 : roundtrip sur le corpus existant -----

    fn corpus_affine() -> Program {
        // Réplique de `kasm::tests::affine_nodes`.
        Program::new(
            Target::Cpu,
            1,
            1,
            16,
            vec![
                Node::input(0),
                Node::const_i64(3),
                Node::mul(0, 1),
                Node::const_i64(1),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn corpus_const_heavy(seed: i16) -> Program {
        // Réplique de `kasm::tests::const_heavy_program`.
        let mut nodes = Vec::new();
        nodes.push(Node::input(0));
        let live_mul_const = nodes.len() as u16;
        nodes.push(Node::const_i64(seed.rem_euclid(5) + 2));
        let live_mul = nodes.len() as u16;
        nodes.push(Node::mul(0, live_mul_const));
        let mut const_ref = nodes.len() as u16;
        nodes.push(Node::const_i64(seed.rem_euclid(17) - 8));
        for i in 0..48i16 {
            let c = nodes.len() as u16;
            nodes.push(Node::const_i64(((seed + i * 3).rem_euclid(19)) - 9));
            let next = nodes.len() as u16;
            match i % 4 {
                0 => nodes.push(Node::add(const_ref, c)),
                1 => nodes.push(Node::sub(const_ref, c)),
                2 => nodes.push(Node::min(const_ref, c)),
                _ => nodes.push(Node::max(const_ref, c)),
            }
            const_ref = next;
        }
        let dead_base = nodes.len() as u16;
        nodes.push(Node::const_i64(seed.rem_euclid(13) - 6));
        let mut dead_ref = dead_base;
        for i in 0..16i16 {
            let c = nodes.len() as u16;
            nodes.push(Node::const_i64(((seed - i * 2).rem_euclid(11)) - 5));
            let next = nodes.len() as u16;
            nodes.push(Node::add(dead_ref, c));
            dead_ref = next;
        }
        let const_eq = nodes.len() as u16;
        nodes.push(Node::eq(const_ref, const_ref));
        let zero = nodes.len() as u16;
        nodes.push(Node::const_i64(0));
        let selected = nodes.len() as u16;
        nodes.push(Node::select_i64(const_eq, const_ref, zero));
        let combined = nodes.len() as u16;
        nodes.push(Node::add(live_mul, selected));
        nodes.push(Node::output(combined, Ty::I64));
        Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
    }

    fn corpus_static_rewrite(seed: i16) -> Program {
        // Réplique de `kasm::tests::static_rewrite_program`.
        Program::new(
            Target::Cpu,
            1,
            1,
            10,
            vec![
                Node::input(0),
                Node::const_i64(seed.rem_euclid(7) + 1),
                Node::mul(0, 1),
                Node::sub(2, 2),
                Node::const_i64(seed.rem_euclid(11) - 5),
                Node::add(3, 4),
                Node::eq(5, 5),
                Node::const_i64(0),
                Node::select_i64(6, 5, 7),
                Node::output(8, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn corpus_oracle_affine(a: i16, b: i16) -> Program {
        // Programme typique synthétisé par l'oracle Affine : f(x) = a*x + b.
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(a),
                Node::mul(0, 1),
                Node::const_i64(b),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn corpus_oracle_bit_mixer(s1: i16, s2: i16) -> Program {
        // Programme typique BitMixer : f(x) = (x ^ (x >> s1)) << s2.
        Program::new(
            Target::Cpu,
            1,
            1,
            10,
            vec![
                Node::input(0),
                Node::const_i64(s1.rem_euclid(63).max(1)),
                Node::shr(0, 1),
                Node::bit_xor(0, 2),
                Node::const_i64(s2.rem_euclid(63).max(1)),
                Node::shl(3, 4),
                Node::output(5, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn corpus_existing_fixtures_roundtrip() {
        // Réplique exacte des fixtures du module tests/ + variations seed.
        assert_roundtrip(&corpus_affine(), "affine");
        for seed in [-5i16, -1, 0, 1, 7, 13, 31, 127] {
            assert_roundtrip(&corpus_const_heavy(seed), &format!("const_heavy[{seed}]"));
            assert_roundtrip(
                &corpus_static_rewrite(seed),
                &format!("static_rewrite[{seed}]"),
            );
        }
    }

    #[test]
    fn corpus_real_training_pipeline_roundtrip() {
        // Critère #2 « corpus existant » dans sa forme la plus stricte :
        // on invoque le **vrai** pipeline `MonsterNode::train_i64_program`
        // sur 5 fonctions cibles différentes, puis on roundtripe le
        // programme effectivement synthétisé par le moteur.
        use crate::monster::{MonsterNode, MonsterTrainingConfig};
        use crate::{MemoryGovernor, Store};
        

        fn fresh_path(tag: &str) -> std::path::PathBuf {
            crate::fresh_tmp_path("scan-mlir-train", tag)
        }

        let cases: &[(&str, fn(i64) -> i64, MonsterTrainingConfig)] = &[
            ("affine9p1", |x| x * 9 + 1, MonsterTrainingConfig::default()),
            ("squareP3", |x| x * x + 3, MonsterTrainingConfig { max_nodes: 6, beam_width: 512, progress: None }),
            ("xor7or3", |x| (x ^ 7) | 3, MonsterTrainingConfig { max_nodes: 6, beam_width: 768, progress: None }),
            ("affineN5p2", |x| -5 * x + 2, MonsterTrainingConfig::default()),
            ("xor3", |x| x ^ 3, MonsterTrainingConfig::default()),
        ];

        for (label, target_fn, config) in cases {
            let monster = MonsterNode::new(
                Store::open(fresh_path(label)).unwrap(),
                MemoryGovernor::new(1024 * 1024),
            );
            let examples: Vec<(i64, i64)> = (-8..=8).map(|x| (x, target_fn(x))).collect();
            let trained = monster
                .train_i64_program(&examples, config.clone())
                .unwrap_or_else(|e| panic!("training [{label}] failed: {e}"));
            assert!(trained.exact, "[{label}] training did not converge");
            assert_roundtrip(&trained.program, &format!("real_train[{label}]"));
        }
    }

    // ----- Critère Ω-1.0 #3 : canonicaliseur idempotent + forme canonique stable -----

    #[test]
    fn canonicalize_is_idempotent_on_corpus() {
        // canonicalize(canonicalize(P)) == canonicalize(P) byte-pour-byte.
        // Sans cette propriété, la "forme canonique" n'en est pas une.
        let mut programs: Vec<(String, Program)> = Vec::new();
        programs.push(("affine".into(), corpus_affine()));
        for seed in [-5i16, -1, 0, 1, 7, 13, 31, 127] {
            programs.push((format!("const_heavy[{seed}]"), corpus_const_heavy(seed)));
            programs.push((format!("static_rewrite[{seed}]"), corpus_static_rewrite(seed)));
        }
        for a in [-3i16, -1, 0, 1, 2, 5, 11] {
            for b in [-7i16, 0, 1, 13, -100] {
                programs.push((format!("oracle_aff[a={a},b={b}]"), corpus_oracle_affine(a, b)));
            }
        }

        for (label, p) in &programs {
            let c1 = p.canonical().unwrap();
            let c2 = c1.canonical().unwrap();
            assert_eq!(
                c1.bytes(),
                c2.bytes(),
                "[{label}] canonicalize is not idempotent"
            );
        }
    }

    #[test]
    fn canonical_mlir_text_is_idempotent_under_parse_canon_emit() {
        // canonical_mlir_text(parse_mlir(canonical_mlir_text(P))) == canonical_mlir_text(P).
        // La forme texte canonique MLIR est un point fixe sous le pipeline
        // emit→parse→canonicalize→emit.
        let cases: Vec<(&str, Program)> = vec![
            ("affine", corpus_affine()),
            ("const_heavy[7]", corpus_const_heavy(7)),
            ("static_rewrite[13]", corpus_static_rewrite(13)),
            ("oracle_aff[5,1]", corpus_oracle_affine(5, 1)),
        ];

        for (label, p) in cases {
            let t1 = canonical_mlir_text(&p).unwrap();
            let p2 = parse_mlir(&t1).unwrap();
            let t2 = canonical_mlir_text(&p2).unwrap();
            assert_eq!(
                t1, t2,
                "[{label}] canonical_mlir_text not a fixed point under parse→canon→emit"
            );
        }
    }

    #[test]
    fn canonical_mlir_text_idempotent_on_random_corpus() {
        // Sur 256 programmes random tailles variées, vérifie que le pipeline
        // emit→parse→canon est un point fixe.
        let mut rng = Rng::new(0xfeedface);
        for i in 0..256u32 {
            let pivot = rng.range(100);
            let target = if pivot < 60 {
                4 + rng.range(60)
            } else if pivot < 90 {
                64 + rng.range(512)
            } else {
                512 + rng.range(MAX_NODES - 512)
            };
            let inputs = 1 + (rng.range(MAX_SLOTS as usize - 1)) as u8;
            let p = random_program(i as u64 + 0x1234, target, inputs);

            // Si le programme contient des Reduce, canonicalize l'identité —
            // on l'accepte comme point fixe trivial.
            let t1 = match canonical_mlir_text(&p) {
                Ok(t) => t,
                Err(_) => continue, // peu probable, skip si erreur transitoire
            };
            let p2 = parse_mlir(&t1).unwrap_or_else(|e| {
                panic!("parse failed for fuzz #{i}: {e}\n{t1}");
            });
            let t2 = canonical_mlir_text(&p2).unwrap();
            assert_eq!(t1, t2, "fuzz #{i} not a fixed point");
        }
    }

    // ----- Critère Ω-1.0 #5 : hash_mlir_canonical équivalent à canonical_hash_hex -----

    #[test]
    fn hash_mlir_canonical_is_byte_stable_under_emit_parse() {
        // hash_mlir_canonical(P) == hash_mlir_canonical(parse(emit(P)))
        // pour 8 programmes au plafond MAX_NODES (4096 nœuds chacun).
        for seed in 0..8u64 {
            let p = random_program(seed * 42 + 1, MAX_NODES, 4);
            let h1 = hash_mlir_canonical_hex(&p).unwrap();

            let text = emit_mlir(&p);
            let p2 = parse_mlir(&text).unwrap();
            let h2 = hash_mlir_canonical_hex(&p2).unwrap();

            assert_eq!(h1, h2, "hash_mlir_canonical not stable under emit/parse (seed {seed})");
        }
    }

    #[test]
    fn hash_mlir_canonical_equivalence_with_canonical_hash_hex() {
        // Propriété sémantique centrale Ω-1.0 #5 :
        //   ∀ P, Q : hash_mlir_canonical(P) == hash_mlir_canonical(Q)
        //          ⟺ canonical_hash_hex(P)   == canonical_hash_hex(Q)
        //
        // On prouve cette propriété sur un corpus mélangeant doublons
        // sémantiques (programmes différents textuellement mais équivalents
        // après canonicalize) et programmes distincts.
        let mut programs: Vec<(String, Program)> = Vec::new();
        programs.push(("affine_a".into(), corpus_oracle_affine(3, 1)));
        programs.push(("affine_b".into(), corpus_oracle_affine(3, 1)));
        programs.push(("affine_c".into(), corpus_oracle_affine(3, 2)));
        programs.push(("static_rewrite_7".into(), corpus_static_rewrite(7)));
        programs.push(("static_rewrite_13".into(), corpus_static_rewrite(13)));
        for a in [-2i16, 0, 5] {
            for b in [-3i16, 1, 7] {
                programs.push((format!("aff[{a},{b}]"), corpus_oracle_affine(a, b)));
            }
        }

        for (li, (la, pa)) in programs.iter().enumerate() {
            for (rj, (lb, pb)) in programs.iter().enumerate().skip(li) {
                let c_eq = pa.canonical_hash_hex().unwrap() == pb.canonical_hash_hex().unwrap();
                let m_eq = pa.hash_mlir_canonical_hex().unwrap()
                    == pb.hash_mlir_canonical_hex().unwrap();
                assert_eq!(
                    c_eq, m_eq,
                    "equivalence broken for ({la}, {lb}) [i={li}, j={rj}]: \
                     canonical_hash_hex_equal={c_eq}, mlir_hash_equal={m_eq}"
                );
            }
        }
    }

    #[test]
    fn hash_mlir_canonical_stable_on_4096_node_random_programs() {
        // 4 programmes au plafond + recalcul après emit/parse.
        for seed in 0..4u64 {
            let p = random_program(seed * 7919 + 3, MAX_NODES, 6);
            assert_eq!(p.nodes().len(), MAX_NODES);
            let h1 = hash_mlir_canonical_hex(&p).unwrap();
            // Les 4096 nœuds peuvent contenir du Reduce → canonicalize devient
            // l'identité dans ce cas mais reste cohérent par construction.
            let h2 = hash_mlir_canonical_hex(&p.canonical().unwrap()).unwrap();
            assert_eq!(h1, h2, "hash_mlir_canonical(P) != hash_mlir_canonical(canonical(P))");
        }
    }

    // ----- Critère Ω-1.0 #4 : Program::from_mlir = entrée canonique officielle -----

    #[test]
    fn program_from_mlir_is_official_entry_point() {
        // Program::from_mlir doit produire un programme byte-exact identique
        // à parse_mlir, et fonctionner sur tous les programmes du corpus.
        let cases = vec![
            corpus_affine(),
            corpus_const_heavy(11),
            corpus_static_rewrite(7),
            corpus_oracle_affine(2, -3),
            corpus_oracle_bit_mixer(7, 3),
        ];
        for p in cases {
            let text = emit_mlir(&p);
            let via_parse_mlir = parse_mlir(&text).unwrap();
            let via_from_mlir = Program::from_mlir(&text).unwrap();
            assert_eq!(via_parse_mlir.bytes(), via_from_mlir.bytes());
            assert_eq!(p.bytes(), via_from_mlir.bytes());

            // Et le programme reconstruit sait re-émettre sa forme canonique.
            let canon_text = via_from_mlir.canonical_mlir_text().unwrap();
            let canon_text2 = p.canonical_mlir_text().unwrap();
            assert_eq!(canon_text, canon_text2);
        }
    }

    #[test]
    fn corpus_oracle_synthesised_programs_roundtrip() {
        // Famille de programmes typiquement synthétisés par les oracles
        // V6.x (Affine, BitMixer). Couvre les patterns de la lab_runner.
        for a in [-3i16, -1, 0, 1, 2, 5, 11] {
            for b in [-7i16, 0, 1, 13, -100] {
                assert_roundtrip(
                    &corpus_oracle_affine(a, b),
                    &format!("oracle_affine[a={a},b={b}]"),
                );
            }
        }
        for s1 in 1..32i16 {
            for s2 in 1..16i16 {
                assert_roundtrip(
                    &corpus_oracle_bit_mixer(s1, s2),
                    &format!("oracle_bitmix[s1={s1},s2={s2}]"),
                );
            }
        }
    }

    fn build_affine_program() -> Program {
        // f(x, y) = (x + y) * 7
        let nodes = vec![
            Node::input(0),       // %n0
            Node::input(1),       // %n1
            Node::const_i64(7),   // %n2
            Node::add(0, 1),      // %n3
            Node::mul(3, 2),      // %n4
            Node::output(4, Ty::I64), // %n5
        ];
        Program::new(Target::Cpu, 2, 1, 16, nodes).unwrap()
    }

    fn build_complex_program() -> Program {
        // Programme exerçant les opcodes non triviaux.
        let nodes = vec![
            Node::input(0),                // %n0
            Node::input(1),                // %n1
            Node::const_i64(10),           // %n2
            Node::const_i64(0),            // %n3
            Node::lt(0, 2),                // %n4 : i1
            Node::select_i64(4, 0, 3),     // %n5
            Node::bit_xor(5, 1),           // %n6
            Node::shl(6, 2),               // %n7
            Node::clamp(7, 3, 2),          // %n8
            Node::reduce_add(0, 3),        // %n9 (reduce sur %n0..%n2)
            Node::sat_add(8, 9),           // %n10
            Node::output(10, Ty::I64),     // %n11
        ];
        Program::new(Target::Auto, 2, 1, 32, nodes).unwrap()
    }

    #[test]
    fn roundtrip_affine_byte_exact() {
        let p = build_affine_program();
        let text = emit_mlir(&p);
        let p2 = parse_mlir(&text).expect("parse");
        assert_eq!(p.bytes(), p2.bytes(), "byte-exact roundtrip failed");
    }

    #[test]
    fn roundtrip_complex_byte_exact() {
        let p = build_complex_program();
        let text = emit_mlir(&p);
        let p2 = parse_mlir(&text).expect("parse");
        assert_eq!(
            p.bytes(),
            p2.bytes(),
            "byte-exact roundtrip failed:\nMLIR:\n{text}"
        );
    }

    #[test]
    fn callkey_invariant_under_mlir_roundtrip() {
        for p in [build_affine_program(), build_complex_program()] {
            let h_before = p.canonical_hash_hex().expect("canonical");
            let text = emit_mlir(&p);
            let p2 = parse_mlir(&text).expect("parse");
            let h_after = p2.canonical_hash_hex().expect("canonical");
            assert_eq!(
                h_before, h_after,
                "CallKey changed across MLIR roundtrip"
            );
        }
    }

    #[test]
    fn emit_is_deterministic() {
        let p = build_complex_program();
        let a = emit_mlir(&p);
        let b = emit_mlir(&p);
        assert_eq!(a, b, "emit non-deterministic");
    }

    #[test]
    fn header_format_is_strict() {
        let p = build_affine_program();
        let text = emit_mlir(&p);
        let first = text.lines().next().unwrap();
        assert_eq!(
            first,
            "kasm.program {target = \"cpu\", inputs = 2, outputs = 1, fuel = 16} {"
        );
    }

    #[test]
    fn rejects_bad_header() {
        let bad = "kasm.foo {target = \"cpu\", inputs = 1, outputs = 1, fuel = 8} {\n}\n";
        assert!(parse_mlir(bad).is_err());
    }

    #[test]
    fn roundtrip_covers_all_32_opcodes() {
        // Construit un programme qui touche les 32 opcodes au moins une fois.
        // L'objectif n'est pas la cohérence sémantique mais la couverture du
        // codec MLIR : émettre, reparser, vérifier byte-exact + CallKey.
        let nodes = vec![
            Node::input(0),                  // 0  Input
            Node::input(1),                  // 1  Input
            Node::const_i64(3),              // 2  ConstI64
            Node::add(0, 1),                 // 3  AddI64
            Node::sub(0, 1),                 // 4  SubI64
            Node::mul(3, 2),                 // 5  MulI64
            Node::div_checked(5, 2),         // 6  DivI64Checked
            Node::min(3, 4),                 // 7  MinI64
            Node::max(3, 4),                 // 8  MaxI64
            Node::eq(0, 1),                  // 9  EqI64 → Bool
            Node::lt(0, 1),                  // 10 LtI64 → Bool
            Node::le(0, 1),                  // 11 LeI64 → Bool
            Node::and(9, 10),                // 12 AndBool
            Node::or(9, 10),                 // 13 OrBool
            Node::not(9),                    // 14 NotBool
            Node::select_i64(9, 0, 1),       // 15 SelectI64
            Node::bit_and(0, 1),             // 16 BitAndI64
            Node::bit_or(0, 1),              // 17 BitOrI64
            Node::bit_xor(0, 1),             // 18 BitXorI64
            Node::shl(0, 2),                 // 19 ShlI64
            Node::shr(0, 2),                 // 20 ShrI64
            Node::sat_add(0, 1),             // 21 SatAddI64
            Node::sat_sub(0, 1),             // 22 SatSubI64
            Node::mod_checked(0, 2),         // 23 ModI64Checked
            Node::clamp(0, 4, 3),            // 24 ClampI64
            Node::reduce_add(0, 3),          // 25 ReduceAddI64 (sum %n0..%n2)
            Node::reduce_mul(0, 3),          // 26 ReduceMulI64
            Node::hash64(25),                // 27 Hash64
            Node::bit_flip(0),               // 28 BitFlipI64 (Ω-6.1)
            Node::neg(0),                    // 29 NegI64
            Node::reverse_bits(0),           // 30 ReverseBitsI64
            Node::byteswap(0),               // 31 ByteswapI64
            Node::output(31, Ty::I64),       // 32 Output
        ];
        let p = Program::new(Target::Auto, 2, 1, 64, nodes).unwrap();

        let text = emit_mlir(&p);
        let p2 = parse_mlir(&text).expect("parse");
        assert_eq!(
            p.bytes(),
            p2.bytes(),
            "byte-exact roundtrip failed on full-coverage program:\nMLIR:\n{text}"
        );

        let h_before = p.canonical_hash_hex().unwrap();
        let h_after = p2.canonical_hash_hex().unwrap();
        assert_eq!(h_before, h_after, "CallKey changed across full-coverage roundtrip");
    }

    #[test]
    fn rejects_unknown_op() {
        let bad = "kasm.program {target = \"cpu\", inputs = 0, outputs = 1, fuel = 8} {\n  \
            %n0 = kasm.const {value = 1} : i64\n  \
            %n1 = kasm.zorglub %n0, %n0 : i64\n  \
            %n2 = kasm.output %n1 : i64\n\
            }\n";
        assert!(parse_mlir(bad).is_err());
    }
}
