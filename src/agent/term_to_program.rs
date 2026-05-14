//! Ω-7.0.2.2 — Conversion Term → Program.
//!
//! Inverse complète de `meta::embed_program`. La structure produite par
//! `embed_program` est totalement déterministe :
//!
//! ```text
//!   App(App(App(App(App(App(STRUCT_PROGRAM, target), inputs), outputs),
//!       fuel), node_count), node_0) ... node_n
//! ```
//!
//! et chaque node est :
//!
//! ```text
//!   App(App(App(App(App(STRUCT_NODE, op), ty), a), b), imm)
//! ```
//!
//! Les feuilles sont toutes des `Sort(N)` avec N dans une plage tag
//! disjointe (cf. `kasm_embed.rs`). On peut donc réellement inverser
//! l'embedding byte-exact pour tout `Term` issu de `embed_program`.
//!
//! Pour tout `Term` qui n'est pas la forme exacte produite par
//! `embed_program`, on retourne `TermToProgramError::NotAnEmbedding`
//! avec un message décrivant la branche qui a échoué.

use crate::kasm::{Node, Op, Program, Target, Ty};
use crate::meta::Term;

// Les bases de tags sont privées dans kasm_embed ; on les redéclare ici
// (constantes de schéma de l'embedding, doctrine zéro-faux-couplage : si
// le schéma change côté embed, ces constantes doivent suivre).
const OP_TAG_BASE: u32 = 0x1000_0000;
const ARG_TAG_BASE: u32 = 0x2000_0000;
const IMM_TAG_BASE: u32 = 0x3000_0000;
const TY_TAG_BASE: u32 = 0x4000_0000;
const STRUCT_TAG_BASE: u32 = 0x5000_0000;
const TARGET_TAG_BASE: u32 = 0x6000_0000;

const STRUCT_PROGRAM: u32 = STRUCT_TAG_BASE;
const STRUCT_NODE: u32 = STRUCT_TAG_BASE + 1;

const HEADER_FIELDS: usize = 5; // target, inputs, outputs, fuel, node_count
const NODE_FIELDS: usize = 5; // op, ty, a, b, imm

#[derive(Debug)]
pub enum TermToProgramError {
    /// Le Term n'est pas une forme reconnue d'embedding KASM.
    NotAnEmbedding(String),
    /// L'embedding réfère à un opcode que term_to_program ne connaît pas.
    UnknownOpcodeEncoding,
    /// L'embedding réfère à un index hors du programme reconstruit.
    BadReference,
    /// Le Program reconstruit échoue à `Program::new` (validation KASM).
    InvalidProgram(crate::kasm::KasmError),
}

impl std::fmt::Display for TermToProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TermToProgramError::NotAnEmbedding(msg) => write!(f, "not an embedding: {msg}"),
            TermToProgramError::UnknownOpcodeEncoding => write!(f, "unknown opcode encoding"),
            TermToProgramError::BadReference => write!(f, "bad reference"),
            TermToProgramError::InvalidProgram(e) => write!(f, "invalid program: {e}"),
        }
    }
}

impl std::error::Error for TermToProgramError {}

/// Aplatit un `App` chaîné gauche-balancé en une racine et la liste de ses
/// arguments dans l'ordre d'application. Pour un terme qui n'est pas un
/// `App`, retourne `(term, [])`.
fn flatten_app_chain(term: &Term) -> (&Term, Vec<&Term>) {
    let mut args: Vec<&Term> = Vec::new();
    let mut cur = term;
    while let Term::App(f, x) = cur {
        args.push(x.as_ref());
        cur = f.as_ref();
    }
    args.reverse();
    (cur, args)
}

/// Extrait le niveau d'un `Term::Sort(N)` ou retourne une erreur.
fn expect_sort(term: &Term, ctx: &str) -> Result<u32, TermToProgramError> {
    match term {
        Term::Sort(n) => Ok(*n),
        _ => Err(TermToProgramError::NotAnEmbedding(format!(
            "{ctx}: attendu Sort(N), reçu {term:?}"
        ))),
    }
}

fn decode_target(term: &Term) -> Result<Target, TermToProgramError> {
    let n = expect_sort(term, "target_tag")?;
    if n < TARGET_TAG_BASE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "target tag hors plage : {n:#x}"
        )));
    }
    let raw = n - TARGET_TAG_BASE;
    match raw {
        0 => Ok(Target::Auto),
        1 => Ok(Target::Cpu),
        2 => Ok(Target::Kernel),
        3 => Ok(Target::Gpu),
        4 => Ok(Target::Qpu),
        _ => Err(TermToProgramError::NotAnEmbedding(format!(
            "target byte invalide : {raw}"
        ))),
    }
}

fn decode_op(term: &Term) -> Result<Op, TermToProgramError> {
    let n = expect_sort(term, "op_tag")?;
    if n < OP_TAG_BASE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "op tag hors plage : {n:#x}"
        )));
    }
    let raw = n - OP_TAG_BASE;
    if raw > u8::MAX as u32 {
        return Err(TermToProgramError::UnknownOpcodeEncoding);
    }
    op_from_byte(raw as u8).ok_or(TermToProgramError::UnknownOpcodeEncoding)
}

/// Reproduit la table `Op::from_byte` (privée côté kasm). Si KASM ajoute un
/// opcode, ce mapping doit suivre — sinon `term_to_program` retournera
/// `UnknownOpcodeEncoding` honnêtement plutôt que de fabriquer un opcode.
fn op_from_byte(b: u8) -> Option<Op> {
    Some(match b {
        0 => Op::Input,
        1 => Op::ConstI64,
        2 => Op::AddI64,
        3 => Op::MulI64,
        4 => Op::EqI64,
        5 => Op::Hash64,
        6 => Op::Output,
        7 => Op::SubI64,
        8 => Op::DivI64Checked,
        9 => Op::MinI64,
        10 => Op::MaxI64,
        11 => Op::SelectI64,
        12 => Op::AndBool,
        13 => Op::OrBool,
        14 => Op::NotBool,
        15 => Op::LtI64,
        16 => Op::LeI64,
        17 => Op::BitAndI64,
        18 => Op::BitOrI64,
        19 => Op::BitXorI64,
        20 => Op::ShlI64,
        21 => Op::ShrI64,
        22 => Op::SatAddI64,
        23 => Op::SatSubI64,
        24 => Op::ModI64Checked,
        25 => Op::ClampI64,
        26 => Op::ReduceAddI64,
        27 => Op::ReduceMulI64,
        28 => Op::BitFlipI64,
        29 => Op::NegI64,
        30 => Op::ReverseBitsI64,
        31 => Op::ByteswapI64,
        // Φ.0 — IEEE 754 layer.
        32 => Op::ConstF64,
        33 => Op::F64Op,
        // KASM v1.0 mutation (audit 2026-05-01) — méta-ops piquées à
        // JAX/Mojo/Julia/OCaml/APL. L'embedding meta a toujours su les
        // sérialiser (`op as u32`), mais le décodeur restait bloqué sur
        // v0.x — round-trip Program → Term → Program asymétrique. Tout
        // l'arc 32-45 est désormais couvert.
        34 => Op::Adaptive,
        35 => Op::Comptime,
        36 => Op::Grad,
        37 => Op::Cond,
        38 => Op::Memoize,
        39 => Op::Pipeline,
        40 => Op::Vmap,
        41 => Op::Pmap,
        42 => Op::Fori,
        43 => Op::WhileLoop,
        44 => Op::Reduce,
        45 => Op::Scan,
        46 => Op::VLenI64,
        47 => Op::VSumI64,
        48 => Op::VAddI64,
        49 => Op::VMulI64,
        50 => Op::VSubI64,
        51 => Op::VMaxI64,
        52 => Op::VMinI64,
        53 => Op::VRangeI64,
        54 => Op::VConcatI64,
        55 => Op::VReverseI64,
        56 => Op::VBroadcastI64,
        57 => Op::VEqI64,
        58 => Op::VAndI64,
        59 => Op::VOrI64,
        60 => Op::VXorI64,
        61 => Op::VAbsI64,
        62 => Op::VNegI64,
        63 => Op::VBitFlipI64,
        64 => Op::Fractal,  // Wave 8 self-hosting
        65 => Op::Eval,     // Wave 8 self-hosting
        66 => Op::VGetI64,
        67 => Op::PopcntI64,
        68 => Op::LzcntI64,
        69 => Op::TzcntI64,
        70 => Op::PextI64,
        71 => Op::PdepI64,
        72 => Op::Lazy,
        73 => Op::Force,
        _ => return None,
    })
}

fn decode_ty(term: &Term) -> Result<Ty, TermToProgramError> {
    let n = expect_sort(term, "ty_tag")?;
    if n < TY_TAG_BASE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "ty tag hors plage : {n:#x}"
        )));
    }
    let raw = n - TY_TAG_BASE;
    match raw {
        1 => Ok(Ty::I64),
        2 => Ok(Ty::Bool),
        // Φ.0 — IEEE 754 layer (storage-polymorphic over Value::I64
        // bits). Audit 2026-05-01 : embedding writes `ty as u32` so it
        // already serialises F64, but the decoder rejected it. Symmetry
        // restored.
        3 => Ok(Ty::F64),
        // Wave 1 (Phase Ω.10) — VecI64 scaffolding. Decoded for round-
        // trip parity ; runtime use still gates on `KasmError::
        // VecNotSupportedYet`.
        4 => Ok(Ty::VecI64),
        _ => Err(TermToProgramError::NotAnEmbedding(format!(
            "ty byte invalide : {raw}"
        ))),
    }
}

fn decode_arg(term: &Term) -> Result<u16, TermToProgramError> {
    let n = expect_sort(term, "arg_tag")?;
    if n < ARG_TAG_BASE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "arg tag hors plage : {n:#x}"
        )));
    }
    let raw = n - ARG_TAG_BASE;
    if raw > u16::MAX as u32 {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "arg index hors plage u16 : {raw}"
        )));
    }
    Ok(raw as u16)
}

fn decode_imm(term: &Term) -> Result<i16, TermToProgramError> {
    let n = expect_sort(term, "imm_tag")?;
    if n < IMM_TAG_BASE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "imm tag hors plage : {n:#x}"
        )));
    }
    let raw = n - IMM_TAG_BASE;
    if raw > u16::MAX as u32 {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "imm hors plage u16 : {raw}"
        )));
    }
    // L'embed côté forward réinterprète signed → unsigned bit-à-bit.
    // L'inverse réinterprète unsigned → signed.
    Ok(raw as u16 as i16)
}

fn decode_header_count(term: &Term, ctx: &str) -> Result<u32, TermToProgramError> {
    // Header counts sont encodés en `Sort(n)` "en clair", donc n < base
    // de tag la plus basse (OP_TAG_BASE = 0x1000_0000).
    let n = expect_sort(term, ctx)?;
    if n >= OP_TAG_BASE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "{ctx}: valeur header en plage de tag réservée : {n:#x}"
        )));
    }
    Ok(n)
}

fn decode_node(term: &Term) -> Result<Node, TermToProgramError> {
    let (root, args) = flatten_app_chain(term);
    let root_n = expect_sort(root, "node root")?;
    if root_n != STRUCT_NODE {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "node root tag attendu STRUCT_NODE ({STRUCT_NODE:#x}), reçu {root_n:#x}"
        )));
    }
    if args.len() != NODE_FIELDS {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "node attend {NODE_FIELDS} champs, reçu {}",
            args.len()
        )));
    }
    let op = decode_op(args[0])?;
    let ty = decode_ty(args[1])?;
    let a = decode_arg(args[2])?;
    let b = decode_arg(args[3])?;
    let imm = decode_imm(args[4])?;
    Ok(Node { op, ty, a, b, imm })
}

/// Reconstruit un `Program` depuis un `Term` issu de `meta::embed_program`.
///
/// Pour tout programme `p` valide, garantit le roundtrip byte-exact :
/// `term_to_program(embed_program(p))?.bytes() == p.bytes()`.
pub fn term_to_program(term: &Term) -> Result<Program, TermToProgramError> {
    let (root, args) = flatten_app_chain(term);
    let root_n = expect_sort(root, "program root")?;
    if root_n != STRUCT_PROGRAM {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "root tag attendu STRUCT_PROGRAM ({STRUCT_PROGRAM:#x}), reçu {root_n:#x}"
        )));
    }
    if args.len() < HEADER_FIELDS {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "program attend au moins {HEADER_FIELDS} champs header, reçu {}",
            args.len()
        )));
    }
    let target = decode_target(args[0])?;
    let inputs_u32 = decode_header_count(args[1], "inputs")?;
    let outputs_u32 = decode_header_count(args[2], "outputs")?;
    let fuel = decode_header_count(args[3], "fuel")?;
    let node_count_u32 = decode_header_count(args[4], "node_count")?;

    if inputs_u32 > u8::MAX as u32 {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "inputs hors plage u8 : {inputs_u32}"
        )));
    }
    if outputs_u32 > u8::MAX as u32 {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "outputs hors plage u8 : {outputs_u32}"
        )));
    }
    let inputs = inputs_u32 as u8;
    let outputs = outputs_u32 as u8;

    let node_terms = &args[HEADER_FIELDS..];
    if node_terms.len() as u32 != node_count_u32 {
        return Err(TermToProgramError::NotAnEmbedding(format!(
            "node_count header = {node_count_u32}, mais {} nodes encodés",
            node_terms.len()
        )));
    }

    let mut nodes = Vec::with_capacity(node_terms.len());
    for nt in node_terms {
        nodes.push(decode_node(nt)?);
    }

    Program::new(target, inputs, outputs, fuel, nodes)
        .map_err(TermToProgramError::InvalidProgram)
}

/// Roundtrip helper : pour un programme `p`, vérifie que
/// `term_to_program(embed_program(p))` retourne un programme byte-exact.
pub fn roundtrip_via_term(p: &Program) -> Result<Program, TermToProgramError> {
    let t = crate::meta::embed_program(p);
    term_to_program(&t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};
    use crate::meta::embed_program;

    fn affine_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
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

    fn alt_program() -> Program {
        // f(x) = 5 * x + (-7) — exerce target Gpu et imm négatif.
        Program::new(
            Target::Gpu,
            1,
            1,
            16,
            vec![
                Node::input(0),
                Node::const_i64(5),
                Node::mul(0, 1),
                Node::const_i64(-7),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn roundtrip_affine_program_byte_exact() {
        let p = affine_program();
        let t = embed_program(&p);
        let p2 = term_to_program(&t).expect("doit décoder l'embedding");
        assert_eq!(p.bytes(), p2.bytes(), "roundtrip doit être byte-exact");
    }

    #[test]
    fn roundtrip_handles_negative_imm_and_gpu_target() {
        let p = alt_program();
        let p2 = roundtrip_via_term(&p).expect("doit décoder");
        assert_eq!(p.bytes(), p2.bytes());
        assert_eq!(p.target(), p2.target());
    }

    #[test]
    fn roundtrip_via_term_helper_works() {
        let p = affine_program();
        let p2 = roundtrip_via_term(&p).unwrap();
        assert_eq!(p.bytes(), p2.bytes());
    }

    #[test]
    fn arbitrary_term_rejected_with_clear_error() {
        // Un Term qui n'est pas un embedding (ex. λ. Var(0)) doit échouer
        // avec NotAnEmbedding, pas un panic ni un programme bidon.
        let t = Term::lam(Term::sort(0), Term::var(0));
        let err = term_to_program(&t).expect_err("attendu erreur");
        match err {
            TermToProgramError::NotAnEmbedding(msg) => {
                assert!(msg.contains("program root"), "msg = {msg}");
            }
            other => panic!("attendu NotAnEmbedding, reçu {other:?}"),
        }
    }

    #[test]
    fn malformed_node_root_rejected() {
        // Construit manuellement un terme avec STRUCT_PROGRAM en tête mais
        // un node mal-formé (root != STRUCT_NODE).
        // Header complet : target=Cpu, inputs=1, outputs=1, fuel=4, count=1.
        let mut t = Term::sort(STRUCT_PROGRAM);
        t = Term::app(t, Term::sort(TARGET_TAG_BASE + 1));
        t = Term::app(t, Term::sort(1));
        t = Term::app(t, Term::sort(1));
        t = Term::app(t, Term::sort(4));
        t = Term::app(t, Term::sort(1));
        // Faux node : root = sort(0) au lieu de STRUCT_NODE.
        let bad_node = Term::sort(0);
        t = Term::app(t, bad_node);
        let err = term_to_program(&t).expect_err("attendu erreur sur node mal-formé");
        match err {
            TermToProgramError::NotAnEmbedding(msg) => {
                assert!(msg.contains("node root"), "msg = {msg}");
            }
            other => panic!("attendu NotAnEmbedding, reçu {other:?}"),
        }
    }

    #[test]
    fn roundtrip_program_with_all_28_opcodes() {
        let nodes = vec![
            Node::input(0),
            Node::input(1),
            Node::const_i64(3),
            Node::add(0, 1),
            Node::sub(0, 1),
            Node::mul(3, 2),
            Node::div_checked(5, 2),
            Node::min(3, 4),
            Node::max(3, 4),
            Node::eq(0, 1),
            Node::lt(0, 1),
            Node::le(0, 1),
            Node::and(9, 10),
            Node::or(9, 10),
            Node::not(9),
            Node::select_i64(9, 0, 1),
            Node::bit_and(0, 1),
            Node::bit_or(0, 1),
            Node::bit_xor(0, 1),
            Node::shl(0, 2),
            Node::shr(0, 2),
            Node::sat_add(0, 1),
            Node::sat_sub(0, 1),
            Node::mod_checked(0, 2),
            Node::clamp(0, 4, 3),
            Node::reduce_add(0, 3),
            Node::reduce_mul(0, 3),
            Node::hash64(25),
            Node::output(27, Ty::I64),
        ];
        let p = Program::new(Target::Auto, 2, 1, 64, nodes).unwrap();
        let p2 = roundtrip_via_term(&p).expect("roundtrip 28 opcodes");
        assert_eq!(p.bytes(), p2.bytes(), "roundtrip byte-exact sur tous les opcodes");
    }

    /// Audit 2026-05-01 — gap closed : the embed/decode round-trip
    /// previously rejected programs using opcodes ≥ 32 because
    /// `op_from_byte` only handled v0.x (0-31). embed_program was
    /// already symmetric (it casts `op as u32`), so a Program touching
    /// ConstF64 / F64Op / Op::Cond / Op::Comptime / etc. would survive
    /// the meta hash but fail to round-trip. This regression test
    /// exercises the full v1.0 surface so a future shrinkage is caught.
    #[test]
    fn roundtrip_program_with_v1_opcodes_and_f64_type() {
        // Program structure (10 nodes) :
        //   0: input(0) : I64
        //   1: const_f64(2)             — ConstF64 (op=32, ty=F64)
        //   2: comptime(0)              — wrapper, tests Op::Comptime (35)
        //   3: adaptive(2, family=1)    — wrapper, Op::Adaptive (34)
        //   4: memoize(3)               — wrapper, Op::Memoize (38)
        //   5: const_i64(0)             — for Cond's else slot
        //   6: eq(0, 5)                 — Bool predicate
        //   7: cond(6, 4, 5)            — Op::Cond (37) ; imm = else_ref
        //   8: pipeline(4, 5)           — Op::Pipeline (39)
        //   9: lazy(7)                  — Op::Lazy (72)
        //  10: force(9)                 — Op::Force (73)
        //  11: output(10, I64)
        let nodes = vec![
            Node::input(0),
            Node::const_f64(2),
            Node::comptime(0),
            Node::adaptive(2, 1),
            Node::memoize(3),
            Node::const_i64(0),
            Node::eq(0, 5),
            Node::cond(6, 4, 5),
            Node::pipeline(4, 5),
            Node::lazy(7),
            Node::force(9),
            Node::output(10, Ty::I64),
        ];
        let p = Program::new(Target::Cpu, 1, 1, 32, nodes).unwrap();
        let p2 = roundtrip_via_term(&p).expect("v1.0 opcodes must round-trip");
        assert_eq!(
            p.bytes(),
            p2.bytes(),
            "v1.0 round-trip byte-exact (audit 2026-05-01 gap closed)"
        );
    }
}
