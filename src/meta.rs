//! Ω-4 — L4 Méta + Preuves.
//!
//! Cap actuel : **Ω-4.0** — Calculus of Constructions minimal, content-addressed.
//!
//! Sous-ensemble Lean-like / CoC :
//!  * `Term` : Var (de Bruijn), Sort (universes), Lam, Pi, App.
//!  * Hash content-addressed sha256 domain-separated → identité unique
//!    par α-équivalence (les indices de de Bruijn rendent l'α-équivalence
//!    structurelle).
//!  * Beta reduction + normalisation avec fuel.
//!  * Type checker `infer(ctx, t) -> Type` et `check(ctx, t, expected)`.
//!  * Curry-Howard : un programme de type `Π (_: A). B` est une preuve de
//!    la proposition `A → B`.
//!
//! Reporté (sortis du scope V7 sous doctrine pure-Rust + std + sha2) :
//!  * Beta-réduction / type-checking dépendant / inductive / universe / tactic / bootstrap.
//!  * Réintroduits si un consommateur en prod les rappelle.


pub use kasm_embed::{embed_node, embed_program, meta_canonical_hash, meta_content_hash};
pub use term::Term;

pub mod kasm_embed {
//! Ω-4.1 — Encodage des programmes KASM-Int dans `Term`.
//!
//! Bridge structurel KASM ↔ Calculus of Constructions. Chaque `Program`
//! KASM produit un `Term` déterministe et content-addressable :
//!
//! ```text
//!   embed_program(p).hash()    ←  identité meta-namespace
//!   p.canonical_hash_hex()     ←  identité KASM-namespace
//! ```
//!
//! Les deux hashes vivent dans des namespaces distincts (sha256 mais
//! domain-separated dans `Term::hash`), mais sont **équivalents par
//! équivalence de programmes** : deux programmes byte-égaux produisent
//! le même `embed_program(p).hash()`, et deux programmes
//! kasm-canonical-équivalents produisent le même
//! `embed_program(p.canonical()).hash()`.
//!
//! ## Schéma d'encodage
//!
//! Chaque entité KASM est représentée par un `Sort(N)` avec N alloué
//! dans une plage disjointe :
//!
//! | Plage | Usage |
//! |---|---|
//! | `0x1000_0000 + op` | Tag d'opcode (0..27) |
//! | `0x2000_0000 + arg` | Index d'argument (0..4095) |
//! | `0x3000_0000 + (imm as u16)` | Valeur immediate (signed → unsigned) |
//! | `0x4000_0000 + ty` | Tag de type (I64=1, Bool=2) |
//! | `0x5000_0000 + tag` | Tags structurels (PROGRAM, NODE) |
//! | `0x6000_0000 + target` | Tag de target (Auto=0, Cpu=1, ...) |
//!
//! Un `Node` se sérialise en `App(App(App(App(App(NODE_TAG, op), ty), a), b), imm)`.
//! Un `Program` agrège header + nodes via `App` chaîné.
//!
//! ## Limites Ω-4.1
//!
//! * Le terme produit n'est **pas type-checkable** dans le strict CoC
//!   (`App(Sort, Sort)` n'a pas de Pi sur le LHS). C'est un encodage
//!   STRUCTUREL pour le hashing et la pattern-recognition, pas pour
//!   la simulation d'exécution.
//! * Connecter sémantiquement (i.e. `term β-réduit ≡ KASM exécute`) est
//!   l'enjeu Ω-4.1.x, qui demande soit des constantes/axiomes, soit des
//!   types inductifs (Ω-4.4).
//! * Seuls les programmes `kasm::Program` (i64) sont supportés. KASM-Tensor
//!   = Ω-4.1.2.

use crate::kasm::{Node, Op, Program, Target, Ty};

use super::term::Term;

// Tag bases (chacun a 16M slots de marge avant collision avec le suivant).
const OP_TAG_BASE: u32 = 0x1000_0000;
const ARG_TAG_BASE: u32 = 0x2000_0000;
const IMM_TAG_BASE: u32 = 0x3000_0000;
const TY_TAG_BASE: u32 = 0x4000_0000;
const STRUCT_TAG_BASE: u32 = 0x5000_0000;
const TARGET_TAG_BASE: u32 = 0x6000_0000;

const STRUCT_PROGRAM: u32 = STRUCT_TAG_BASE;
const STRUCT_NODE: u32 = STRUCT_TAG_BASE + 1;

fn op_tag(op: Op) -> Term {
    Term::sort(OP_TAG_BASE + op as u32)
}

fn arg_tag(a: u16) -> Term {
    Term::sort(ARG_TAG_BASE + a as u32)
}

fn imm_tag(imm: i16) -> Term {
    // Reinterpret signed → unsigned pour préserver l'identité bit-à-bit.
    Term::sort(IMM_TAG_BASE + (imm as u16) as u32)
}

fn ty_tag(ty: Ty) -> Term {
    Term::sort(TY_TAG_BASE + ty as u32)
}

fn target_tag(t: Target) -> Term {
    Term::sort(TARGET_TAG_BASE + t as u32)
}

fn header_count(n: u32) -> Term {
    // Petits entiers du header (inputs, outputs, fuel, node_count) en clair.
    Term::sort(n)
}

/// Encode un `Node` KASM en `Term`. Le Node a 5 champs (op, ty, a, b, imm),
/// chaîne de 5 `App` au-dessus du tag structurel.
pub fn embed_node(n: &Node) -> Term {
    let mut t = Term::sort(STRUCT_NODE);
    t = Term::app(t, op_tag(n.op));
    t = Term::app(t, ty_tag(n.ty));
    t = Term::app(t, arg_tag(n.a));
    t = Term::app(t, arg_tag(n.b));
    t = Term::app(t, imm_tag(n.imm));
    t
}

/// Encode un `Program` KASM en `Term`. Header (target, inputs, outputs,
/// fuel, node_count) puis chaque nœud chaîné via `App`. Déterministe.
pub fn embed_program(p: &Program) -> Term {
    let mut t = Term::sort(STRUCT_PROGRAM);
    t = Term::app(t, target_tag(p.target()));
    t = Term::app(t, header_count(p.inputs() as u32));
    t = Term::app(t, header_count(p.outputs() as u32));
    t = Term::app(t, header_count(p.fuel()));
    t = Term::app(t, header_count(p.nodes().len() as u32));
    for node in p.nodes() {
        t = Term::app(t, embed_node(node));
    }
    t
}

/// Hash content-addressed du programme dans le namespace meta.
/// Équivalent : deux programmes byte-égaux produisent le même hash.
pub fn meta_content_hash(p: &Program) -> [u8; 32] {
    embed_program(p).hash()
}

/// Hash canonique meta : applique `canonicalize` puis encode. C'est le
/// pendant meta-namespace de `Program::canonical_hash_hex()`. Deux
/// programmes kasm-canonical-équivalents produisent le même hash.
pub fn meta_canonical_hash(p: &Program) -> Result<[u8; 32], crate::kasm::KasmError> {
    let canon = p.canonical()?;
    Ok(meta_content_hash(&canon))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};

    fn affine_program() -> Program {
        // f(x) = 3 * x + 1
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

    fn alt_affine_program() -> Program {
        // f(x) = 5 * x + 7 — différent
        Program::new(
            Target::Cpu,
            1,
            1,
            16,
            vec![
                Node::input(0),
                Node::const_i64(5),
                Node::mul(0, 1),
                Node::const_i64(7),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn embed_is_deterministic() {
        let p = affine_program();
        let t1 = embed_program(&p);
        let t2 = embed_program(&p);
        assert_eq!(t1, t2);
        assert_eq!(t1.hash(), t2.hash());
    }

    #[test]
    fn embed_distinguishes_different_programs() {
        let p1 = affine_program();
        let p2 = alt_affine_program();
        let t1 = embed_program(&p1);
        let t2 = embed_program(&p2);
        assert_ne!(t1, t2);
        assert_ne!(t1.hash(), t2.hash());
    }

    #[test]
    fn embed_distinguishes_op_change() {
        // Mêmes args, op différente.
        let add_node = Node::add(0, 1);
        let mul_node = Node::mul(0, 1);
        assert_ne!(embed_node(&add_node), embed_node(&mul_node));
    }

    #[test]
    fn embed_distinguishes_arg_change() {
        let n1 = Node::add(0, 1);
        let n2 = Node::add(0, 2);
        assert_ne!(embed_node(&n1), embed_node(&n2));
    }

    #[test]
    fn embed_distinguishes_imm_signs() {
        // imm = -1 vs imm = +1 : la cast u16 préserve la distinction.
        let n_pos = Node::const_i64(1);
        let n_neg = Node::const_i64(-1);
        assert_ne!(embed_node(&n_pos), embed_node(&n_neg));
        assert_ne!(embed_node(&n_pos).hash(), embed_node(&n_neg).hash());
    }

    #[test]
    fn embed_distinguishes_target() {
        let p_cpu = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        )
        .unwrap();
        let p_gpu = Program::new(
            Target::Gpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        )
        .unwrap();
        assert_ne!(embed_program(&p_cpu), embed_program(&p_gpu));
    }

    #[test]
    fn embed_distinguishes_ty_change() {
        let n_i64 = Node::output(0, Ty::I64);
        let n_bool = Node::output(0, Ty::Bool);
        assert_ne!(embed_node(&n_i64), embed_node(&n_bool));
    }

    #[test]
    fn meta_content_hash_matches_program_byte_equivalence() {
        // Deux programmes byte-égaux → même meta_content_hash.
        let p1 = affine_program();
        let p2 = affine_program();
        assert_eq!(p1.bytes(), p2.bytes());
        assert_eq!(meta_content_hash(&p1), meta_content_hash(&p2));
    }

    #[test]
    fn meta_content_hash_diverges_on_byte_change() {
        let p1 = affine_program();
        let p2 = alt_affine_program();
        assert_ne!(p1.bytes(), p2.bytes());
        assert_ne!(meta_content_hash(&p1), meta_content_hash(&p2));
    }

    #[test]
    fn meta_canonical_hash_bridges_with_kasm_canonical() {
        // Deux programmes qui canonicalisent au même résultat doivent
        // produire la même meta_canonical_hash. On utilise des programmes
        // identiques pour tester la base ; tester l'équivalence sous CSE
        // demanderait des programmes distincts mais sémantiquement égaux.
        let p1 = affine_program();
        let p2 = affine_program();
        let h1 = meta_canonical_hash(&p1).unwrap();
        let h2 = meta_canonical_hash(&p2).unwrap();
        assert_eq!(h1, h2);

        // Et la KASM canonical hash est aussi cohérente.
        assert_eq!(p1.canonical_hash_hex().unwrap(), p2.canonical_hash_hex().unwrap());
    }

    #[test]
    fn meta_canonical_hash_diverges_when_kasm_canonical_diverges() {
        let p1 = affine_program();
        let p2 = alt_affine_program();
        let h1 = meta_canonical_hash(&p1).unwrap();
        let h2 = meta_canonical_hash(&p2).unwrap();
        assert_ne!(h1, h2);

        // Les canonical_hash_hex KASM divergent aussi.
        assert_ne!(p1.canonical_hash_hex().unwrap(), p2.canonical_hash_hex().unwrap());
    }

    #[test]
    fn embed_handles_all_28_opcodes() {
        // Construit un programme exerçant tous les opcodes ; vérifie
        // simplement qu'embed ne panique pas et produit un hash unique.
        let nodes = vec![
            Node::input(0),                  // Input
            Node::input(1),                  // Input
            Node::const_i64(3),              // ConstI64
            Node::add(0, 1),                 // AddI64
            Node::sub(0, 1),                 // SubI64
            Node::mul(3, 2),                 // MulI64
            Node::div_checked(5, 2),         // DivI64Checked
            Node::min(3, 4),                 // MinI64
            Node::max(3, 4),                 // MaxI64
            Node::eq(0, 1),                  // EqI64 → Bool
            Node::lt(0, 1),                  // LtI64
            Node::le(0, 1),                  // LeI64
            Node::and(9, 10),                // AndBool
            Node::or(9, 10),                 // OrBool
            Node::not(9),                    // NotBool
            Node::select_i64(9, 0, 1),       // SelectI64
            Node::bit_and(0, 1),             // BitAndI64
            Node::bit_or(0, 1),              // BitOrI64
            Node::bit_xor(0, 1),             // BitXorI64
            Node::shl(0, 2),                 // ShlI64
            Node::shr(0, 2),                 // ShrI64
            Node::sat_add(0, 1),             // SatAddI64
            Node::sat_sub(0, 1),             // SatSubI64
            Node::mod_checked(0, 2),         // ModI64Checked
            Node::clamp(0, 4, 3),            // ClampI64
            Node::reduce_add(0, 3),          // ReduceAddI64
            Node::reduce_mul(0, 3),          // ReduceMulI64
            Node::hash64(25),                // Hash64
            Node::output(27, Ty::I64),       // Output
        ];
        let p = Program::new(Target::Auto, 2, 1, 64, nodes).unwrap();
        let h = meta_content_hash(&p);
        // Hash non-zéro (tag PROGRAM + 28 nodes injectés).
        assert_ne!(h, [0u8; 32]);
    }

    #[test]
    fn embed_inputs_outputs_fuel_in_header() {
        let p1 = Program::new(
            Target::Cpu, 1, 1, 16,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        )
        .unwrap();
        let p2 = Program::new(
            Target::Cpu, 1, 1, 32, // fuel différent
            vec![Node::input(0), Node::output(0, Ty::I64)],
        )
        .unwrap();
        assert_ne!(meta_content_hash(&p1), meta_content_hash(&p2));
    }
}

}

pub mod mlir_bridge {
//! Ω-4.6 — Bridge `meta::Term` ↔ MLIR text dialect `meta`.
//!
//! Le pendant strict, pour le L4 méta (Calculus of Constructions), du codec
//! `kasm` ⊂ MLIR (cf. [`crate::kasm::mlir`]). Définit un format texte
//! déterministe pour le dialecte custom `meta.*` et fournit :
//!
//!   * [`emit_meta_mlir`] — sérialise un `Term`.
//!   * [`parse_meta_mlir`] — désérialise vers un `Term`.
//!
//! La propriété centrale (testée) :
//! ```text
//!     parse_meta_mlir(emit_meta_mlir(t)) == t   (byte-for-byte sur l'AST)
//!     hash invariant                            (Term::hash() préservé)
//! ```
//!
//! Format (pseudo-grammar) :
//! ```text
//! module   ::= "meta.term" "{" "\n" body "\n" "}" "\n"
//! body     ::= line ("\n" line)* "\n" root
//! line     ::= "  %" IDX " = meta." OP_TAIL
//! OP_TAIL  ::= "var {index = " N "}"
//!            | "sort {level = " N "}"
//!            | "lam %" T_IDX ", %" B_IDX
//!            | "pi %" T_IDX ", %" B_IDX
//!            | "app %" F_IDX ", %" X_IDX
//! root     ::= "  meta.root %" IDX
//! ```
//!
//! L'ordre des lignes est un **tri topologique post-ordre** déterministe :
//! les sous-termes apparaissent toujours avant leurs parents. Cela donne
//! au format la même propriété que la forme SSA de MLIR, sans ambiguïté
//! ni espace flottant.
//!
//! ## Pourquoi un dialecte distinct du `kasm.dialect` (via negativa)
//!
//! `kasm` opère sur i64/i1, opcodes pré-définis, DAG borné. `meta` opère
//! sur termes du Calculus of Constructions : universes, binders, types
//! dépendants. Réutiliser `kasm` reviendrait à confondre les domaines
//! sémantiques — interdit par doctrine.
//!
//! ## Pourquoi pas de raccourci syntaxique
//!
//! L'AST `Term` est minimal (5 constructeurs). Sa forme MLIR doit lui
//! coller 1:1. Pas de Pi/Lam shorthand "→", pas d'inférence d'index :
//! tout est explicite. C'est ce qui garantit le roundtrip byte-exact.

use std::collections::HashMap;
use std::fmt::Write as _;

use super::term::Term;

const SSA_PREFIX: &str = "%";

#[derive(Debug, PartialEq, Eq)]
pub enum MlirBridgeError {
    Syntax(String),
    UnknownOp(String),
    BadIndex(String),
    BadInteger(String),
    BadHeader,
    BadFooter,
    MissingRoot,
    DuplicateRoot,
    UnresolvedRef(usize),
    EmptyModule,
}

impl std::fmt::Display for MlirBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MlirBridgeError::Syntax(s) => write!(f, "syntax error: {s}"),
            MlirBridgeError::UnknownOp(s) => write!(f, "unknown meta op: {s}"),
            MlirBridgeError::BadIndex(s) => write!(f, "bad SSA index: {s}"),
            MlirBridgeError::BadInteger(s) => write!(f, "bad integer literal: {s}"),
            MlirBridgeError::BadHeader => write!(f, "bad meta.term header"),
            MlirBridgeError::BadFooter => write!(f, "bad meta.term footer"),
            MlirBridgeError::MissingRoot => write!(f, "missing meta.root line"),
            MlirBridgeError::DuplicateRoot => write!(f, "duplicate meta.root line"),
            MlirBridgeError::UnresolvedRef(i) => write!(f, "unresolved SSA reference %{i}"),
            MlirBridgeError::EmptyModule => write!(f, "empty meta.term module (no root)"),
        }
    }
}

impl std::error::Error for MlirBridgeError {}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

/// Émet un `Term` dans la forme MLIR text du dialecte `meta`.
///
/// Format strictement déterministe : indentation 2 espaces, tri topologique
/// post-ordre (les feuilles d'abord). Aucune ambiguïté, aucun espace
/// flottant.
pub fn emit_meta_mlir(t: &Term) -> String {
    let mut out = String::new();
    out.push_str("meta.term {\n");

    let mut emitter = Emitter { out: &mut out, next_idx: 0 };
    let root_idx = emitter.emit_node(t);

    let _ = writeln!(out, "  meta.root {SSA_PREFIX}{root_idx}");
    out.push_str("}\n");
    out
}

struct Emitter<'a> {
    out: &'a mut String,
    next_idx: usize,
}

impl<'a> Emitter<'a> {
    fn emit_node(&mut self, t: &Term) -> usize {
        match t {
            Term::Var(i) => {
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.var {{index = {i}}}",
                );
                idx
            }
            Term::Sort(n) => {
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.sort {{level = {n}}}",
                );
                idx
            }
            Term::Lam { ty, body } => {
                let ty_idx = self.emit_node(ty);
                let body_idx = self.emit_node(body);
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.lam {SSA_PREFIX}{ty_idx}, {SSA_PREFIX}{body_idx}",
                );
                idx
            }
            Term::Pi { ty, body } => {
                let ty_idx = self.emit_node(ty);
                let body_idx = self.emit_node(body);
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.pi {SSA_PREFIX}{ty_idx}, {SSA_PREFIX}{body_idx}",
                );
                idx
            }
            Term::App(f, x) => {
                let f_idx = self.emit_node(f);
                let x_idx = self.emit_node(x);
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.app {SSA_PREFIX}{f_idx}, {SSA_PREFIX}{x_idx}",
                );
                idx
            }
        }
    }

    fn alloc_idx(&mut self) -> usize {
        let i = self.next_idx;
        self.next_idx += 1;
        i
    }
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

/// Parse une forme MLIR text émise par [`emit_meta_mlir`] et reconstruit
/// le `Term` d'origine.
pub fn parse_meta_mlir(text: &str) -> Result<Term, MlirBridgeError> {
    let mut lines = text.lines();

    // Header : doit être "meta.term {".
    let header = lines
        .find(|l| !l.trim().is_empty())
        .ok_or(MlirBridgeError::EmptyModule)?;
    if header.trim() != "meta.term {" {
        return Err(MlirBridgeError::BadHeader);
    }

    let mut bindings: HashMap<usize, Term> = HashMap::new();
    let mut next_expected_idx: usize = 0;
    let mut root: Option<Term> = None;
    let mut footer_seen = false;

    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "}" {
            footer_seen = true;
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("meta.root ") {
            if root.is_some() {
                return Err(MlirBridgeError::DuplicateRoot);
            }
            let idx = parse_ssa(rest)?;
            let term = bindings
                .get(&idx)
                .ok_or(MlirBridgeError::UnresolvedRef(idx))?
                .clone();
            root = Some(term);
            continue;
        }

        // Forme : "%N = meta.OP <body>"
        let (lhs, rhs) = trimmed
            .split_once('=')
            .ok_or_else(|| MlirBridgeError::Syntax(format!("expected `=` in `{trimmed}`")))?;
        let idx = parse_ssa(lhs.trim())?;
        if idx != next_expected_idx {
            return Err(MlirBridgeError::Syntax(format!(
                "expected SSA index {next_expected_idx}, got {idx}",
            )));
        }
        next_expected_idx += 1;

        let rhs = rhs.trim();
        let (op_token, body) = match rhs.split_once(' ') {
            Some((op, body)) => (op, body.trim()),
            None => (rhs, ""),
        };
        let op_mnem = op_token
            .strip_prefix("meta.")
            .ok_or_else(|| MlirBridgeError::UnknownOp(op_token.to_string()))?;

        let term = match op_mnem {
            "var" => {
                let n = parse_attr_u32(body, "index")?;
                Term::var(n)
            }
            "sort" => {
                let n = parse_attr_u32(body, "level")?;
                Term::sort(n)
            }
            "lam" | "pi" | "app" => {
                let (a, b) = parse_two_ssa(body)?;
                let lhs_term = bindings
                    .get(&a)
                    .ok_or(MlirBridgeError::UnresolvedRef(a))?
                    .clone();
                let rhs_term = bindings
                    .get(&b)
                    .ok_or(MlirBridgeError::UnresolvedRef(b))?
                    .clone();
                match op_mnem {
                    "lam" => Term::lam(lhs_term, rhs_term),
                    "pi" => Term::pi(lhs_term, rhs_term),
                    "app" => Term::app(lhs_term, rhs_term),
                    _ => unreachable!(),
                }
            }
            _ => return Err(MlirBridgeError::UnknownOp(op_mnem.to_string())),
        };

        bindings.insert(idx, term);
    }

    if !footer_seen {
        return Err(MlirBridgeError::BadFooter);
    }

    root.ok_or(MlirBridgeError::MissingRoot)
}

fn parse_ssa(s: &str) -> Result<usize, MlirBridgeError> {
    let s = s.trim();
    let n = s
        .strip_prefix(SSA_PREFIX)
        .ok_or_else(|| MlirBridgeError::BadIndex(s.to_string()))?;
    n.parse::<usize>()
        .map_err(|_| MlirBridgeError::BadIndex(s.to_string()))
}

fn parse_two_ssa(s: &str) -> Result<(usize, usize), MlirBridgeError> {
    let mut parts = s.split(',');
    let a = parts
        .next()
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected two SSA in `{s}`")))?;
    let b = parts
        .next()
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected two SSA in `{s}`")))?;
    if parts.next().is_some() {
        return Err(MlirBridgeError::Syntax(format!(
            "too many operands in `{s}`",
        )));
    }
    Ok((parse_ssa(a)?, parse_ssa(b)?))
}

fn parse_attr_u32(body: &str, key: &str) -> Result<u32, MlirBridgeError> {
    let body = body.trim();
    let lbrace = body
        .find('{')
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected `{{` in `{body}`")))?;
    let rbrace = body
        .rfind('}')
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected `}}` in `{body}`")))?;
    if rbrace <= lbrace {
        return Err(MlirBridgeError::Syntax(format!(
            "malformed attr block in `{body}`",
        )));
    }
    let inner = &body[lbrace + 1..rbrace];
    let (k, v) = inner.split_once('=').ok_or_else(|| {
        MlirBridgeError::Syntax(format!("expected `key = value` in `{inner}`"))
    })?;
    if k.trim() != key {
        return Err(MlirBridgeError::Syntax(format!(
            "expected attr `{key}`, got `{}`",
            k.trim()
        )));
    }
    let v = v.trim();
    v.parse::<u32>()
        .map_err(|_| MlirBridgeError::BadInteger(v.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::embed_program;

    fn assert_roundtrip(t: &Term, label: &str) {
        let text = emit_meta_mlir(t);
        let t2 = parse_meta_mlir(&text).unwrap_or_else(|e| {
            panic!("[{label}] parse_meta_mlir failed: {e}\nMLIR:\n{text}");
        });
        assert_eq!(*t, t2, "[{label}] structural mismatch");
        assert_eq!(t.hash(), t2.hash(), "[{label}] hash mismatch");
    }

    // -------- 1. roundtrip_var --------
    #[test]
    fn roundtrip_var() {
        let t = Term::var(5);
        let text = emit_meta_mlir(&t);
        let parsed = parse_meta_mlir(&text).unwrap();
        assert_eq!(parsed, t);
        // Forme attendue, byte-exact.
        assert_eq!(
            text,
            "meta.term {\n  %0 = meta.var {index = 5}\n  meta.root %0\n}\n"
        );
    }

    // -------- 2. roundtrip_sort --------
    #[test]
    fn roundtrip_sort() {
        let t = Term::sort(7);
        let text = emit_meta_mlir(&t);
        let parsed = parse_meta_mlir(&text).unwrap();
        assert_eq!(parsed, t);
        assert_eq!(
            text,
            "meta.term {\n  %0 = meta.sort {level = 7}\n  meta.root %0\n}\n"
        );
    }

    // -------- 3. roundtrip_lam --------
    #[test]
    fn roundtrip_lam() {
        // λ x: Sort(0). x  ≡ identity at Type
        let t = Term::lam(Term::sort(0), Term::var(0));
        assert_roundtrip(&t, "lam_id");
    }

    // -------- 4. roundtrip_pi --------
    #[test]
    fn roundtrip_pi() {
        // Π x: Sort(0). x
        let t = Term::pi(Term::sort(0), Term::var(0));
        assert_roundtrip(&t, "pi_id");
    }

    // -------- 5. roundtrip_app --------
    #[test]
    fn roundtrip_app() {
        let t = Term::app(Term::var(0), Term::var(1));
        assert_roundtrip(&t, "app_var0_var1");
    }

    // -------- 6. roundtrip_polymorphic_identity --------
    #[test]
    fn roundtrip_polymorphic_identity() {
        // λ A: Sort(1). λ x: A. x
        let inner = Term::lam(Term::var(0), Term::var(0));
        let t = Term::lam(Term::sort(1), inner);
        assert_roundtrip(&t, "poly_id");
    }

    // -------- 7. roundtrip_nested_apps_and_binders --------
    #[test]
    fn roundtrip_nested_apps_and_binders() {
        // λ x: Sort(0). λ y: Sort(1). (Π z: Sort(2). App(App(x, y), App(z, x)))
        // Profondeur ≥ 5.
        let depth5 = Term::lam(
            Term::sort(0),
            Term::lam(
                Term::sort(1),
                Term::pi(
                    Term::sort(2),
                    Term::app(
                        Term::app(Term::var(2), Term::var(1)),
                        Term::app(Term::var(0), Term::var(2)),
                    ),
                ),
            ),
        );
        assert_roundtrip(&depth5, "depth5");
    }

    // -------- 8. emit_is_deterministic --------
    #[test]
    fn emit_is_deterministic() {
        let t = Term::lam(
            Term::sort(0),
            Term::app(Term::var(0), Term::lam(Term::sort(1), Term::var(0))),
        );
        let a = emit_meta_mlir(&t);
        let b = emit_meta_mlir(&t);
        assert_eq!(a, b, "emit non-deterministic");
    }

    // -------- 9. parse_rejects_garbage --------
    #[test]
    fn parse_rejects_garbage() {
        let bad = "this is not valid MLIR text at all\n";
        assert!(parse_meta_mlir(bad).is_err());

        let bad2 = "meta.term {\n  garbage line\n}\n";
        assert!(parse_meta_mlir(bad2).is_err());

        let bad3 = "meta.term {\n  %0 = meta.var {index = 5}\n";
        // Missing footer.
        assert!(matches!(parse_meta_mlir(bad3), Err(MlirBridgeError::BadFooter)));

        let bad_root = "meta.term {\n  %0 = meta.var {index = 5}\n}\n";
        // Missing meta.root line.
        assert!(matches!(parse_meta_mlir(bad_root), Err(MlirBridgeError::MissingRoot)));
    }

    // -------- 10. parse_rejects_unknown_op --------
    #[test]
    fn parse_rejects_unknown_op() {
        let bad = "meta.term {\n  %0 = meta.zorglub {index = 0}\n  meta.root %0\n}\n";
        let result = parse_meta_mlir(bad);
        assert!(matches!(result, Err(MlirBridgeError::UnknownOp(_))));

        // Mauvais préfixe (pas meta.).
        let bad2 = "meta.term {\n  %0 = kasm.input {slot = 0} : i64\n  meta.root %0\n}\n";
        let r2 = parse_meta_mlir(bad2);
        assert!(matches!(r2, Err(MlirBridgeError::UnknownOp(_))));
    }

    // -------- 11. roundtrip_preserves_hash --------
    #[test]
    fn roundtrip_preserves_hash() {
        // Échantillon varié — chaque term doit conserver son hash content-addressed.
        let cases = vec![
            ("var0", Term::var(0)),
            ("var_max", Term::var(u32::MAX)),
            ("sort0", Term::sort(0)),
            ("sort_max", Term::sort(u32::MAX)),
            ("id", Term::lam(Term::sort(0), Term::var(0))),
            ("pi_arrow", Term::pi(Term::sort(0), Term::sort(0))),
            (
                "self_app",
                Term::lam(Term::ty(), Term::app(Term::var(0), Term::var(0))),
            ),
        ];
        for (label, t) in cases {
            let text = emit_meta_mlir(&t);
            let t2 = parse_meta_mlir(&text)
                .unwrap_or_else(|e| panic!("[{label}] parse failed: {e}\n{text}"));
            assert_eq!(t.hash(), t2.hash(), "[{label}] hash drift after roundtrip");
        }
    }

    // -------- 13. roundtrip_on_kasm_embedded_program --------
    #[test]
    fn roundtrip_on_kasm_embedded_program() {
        use crate::kasm::{Node, Program, Target, Ty};

        // Petit programme jouet f(x) = 3*x + 1.
        let nodes = vec![
            Node::input(0),
            Node::const_i64(3),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ];
        let program = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        let term = embed_program(&program);
        assert_roundtrip(&term, "kasm_embed_affine");
    }

    // -------- bonus : structurels supplémentaires --------

    #[test]
    fn topological_order_is_postorder() {
        // λ x: Sort(0). x → ordre attendu : %0 = sort, %1 = var, %2 = lam.
        let t = Term::lam(Term::sort(0), Term::var(0));
        let text = emit_meta_mlir(&t);
        let expected = "meta.term {\n  \
            %0 = meta.sort {level = 0}\n  \
            %1 = meta.var {index = 0}\n  \
            %2 = meta.lam %0, %1\n  \
            meta.root %2\n\
            }\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn alpha_equivalent_terms_share_emission() {
        // En de Bruijn, l'α-équivalence est structurelle. Deux constructions
        // de "λ x: Type. x" donnent la même chaîne MLIR.
        let a = Term::lam(Term::ty(), Term::var(0));
        let b = Term::lam(Term::sort(0), Term::var(0));
        assert_eq!(emit_meta_mlir(&a), emit_meta_mlir(&b));
    }

    #[test]
    fn rejects_duplicate_root() {
        let bad = "meta.term {\n  \
            %0 = meta.var {index = 0}\n  \
            meta.root %0\n  \
            meta.root %0\n\
            }\n";
        assert!(matches!(
            parse_meta_mlir(bad),
            Err(MlirBridgeError::DuplicateRoot)
        ));
    }

    #[test]
    fn rejects_unresolved_ssa_ref() {
        let bad = "meta.term {\n  \
            %0 = meta.lam %7, %8\n  \
            meta.root %0\n\
            }\n";
        assert!(matches!(
            parse_meta_mlir(bad),
            Err(MlirBridgeError::UnresolvedRef(_))
        ));
    }

    #[test]
    fn rejects_out_of_order_indices() {
        // Format exige ordre croissant strict 0, 1, 2, ...
        let bad = "meta.term {\n  \
            %5 = meta.var {index = 0}\n  \
            meta.root %5\n\
            }\n";
        assert!(parse_meta_mlir(bad).is_err());
    }

    #[test]
    fn parse_strips_trailing_whitespace_only() {
        // Les lignes vides intercalaires sont tolérées (cohérent avec kasm/mlir).
        let text = "meta.term {\n\n  %0 = meta.var {index = 3}\n\n  meta.root %0\n}\n";
        let t = parse_meta_mlir(text).unwrap();
        assert_eq!(t, Term::var(3));
    }

    #[test]
    fn deep_self_app_roundtrip() {
        // (λ x: Type. x x) (λ x: Type. x x) = Ω. On le construit mais on ne
        // normalise pas — on roundtripe juste sa structure.
        let dup = Term::lam(Term::ty(), Term::app(Term::var(0), Term::var(0)));
        let omega = Term::app(dup.clone(), dup);
        assert_roundtrip(&omega, "omega_self_app");
    }

    #[test]
    fn arrow_type_roundtrip() {
        // a → b non dépendant : Π a. (b lift 0 1).
        let a = Term::sort(0);
        let b = Term::sort(1);
        let arrow = Term::arrow(a, b);
        assert_roundtrip(&arrow, "arrow_type");
    }

}

}

pub mod term {
//! Term : AST du calcul des constructions minimal, content-addressed.

use sha2::{Digest, Sha256};

const TERM_HASH_DOMAIN: &[u8] = b"SCAN-OMEGA-META-TERM-V1";

const TAG_VAR: u8 = 1;
const TAG_SORT: u8 = 2;
const TAG_LAM: u8 = 3;
const TAG_PI: u8 = 4;
const TAG_APP: u8 = 5;

/// Term du Calculus of Constructions minimal.
///
/// **Représentation** : indices de de Bruijn — `Var(0)` désigne le binder
/// le plus interne, `Var(1)` le binder qui le contient, etc. Cette
/// représentation rend l'α-équivalence **structurelle** : deux termes
/// α-équivalents ont la même structure et donc le même hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Term {
    /// Variable de de Bruijn. `Var(0)` = binder le plus interne.
    Var(u32),
    /// Universe : `Sort(0)` = `Type`, `Sort(1)` = `Type1`, etc.
    Sort(u32),
    /// Lambda abstraction : `λ (_: ty). body`. Le body référence
    /// le paramètre via `Var(0)`.
    Lam { ty: Box<Term>, body: Box<Term> },
    /// Pi (type fonction dépendant) : `Π (_: ty). body`. Si `body`
    /// n'utilise pas `Var(0)`, c'est un type `ty → body` ordinaire.
    Pi { ty: Box<Term>, body: Box<Term> },
    /// Application : `f x`.
    App(Box<Term>, Box<Term>),
}

impl Term {
    pub fn var(i: u32) -> Self {
        Self::Var(i)
    }
    pub fn sort(level: u32) -> Self {
        Self::Sort(level)
    }
    pub fn ty() -> Self {
        Self::Sort(0)
    }
    pub fn lam(ty: Term, body: Term) -> Self {
        Self::Lam { ty: Box::new(ty), body: Box::new(body) }
    }
    pub fn pi(ty: Term, body: Term) -> Self {
        Self::Pi { ty: Box::new(ty), body: Box::new(body) }
    }
    pub fn app(f: Term, x: Term) -> Self {
        Self::App(Box::new(f), Box::new(x))
    }

    /// Type fonction non-dépendant : `a → b`. Le body est lifté de 1 pour
    /// que ses variables libres ignorent le binder Pi.
    pub fn arrow(a: Term, b: Term) -> Self {
        let b_lifted = b.lift(0, 1);
        Self::pi(a, b_lifted)
    }

    /// Lift toutes les variables libres ≥ `cutoff` par `delta`. Utilisé
    /// pour faire passer un terme sous un nouveau binder.
    pub fn lift(&self, cutoff: u32, delta: u32) -> Term {
        match self {
            Term::Var(i) => {
                if *i >= cutoff {
                    Term::Var(i + delta)
                } else {
                    Term::Var(*i)
                }
            }
            Term::Sort(n) => Term::Sort(*n),
            Term::Lam { ty, body } => Term::Lam {
                ty: Box::new(ty.lift(cutoff, delta)),
                body: Box::new(body.lift(cutoff + 1, delta)),
            },
            Term::Pi { ty, body } => Term::Pi {
                ty: Box::new(ty.lift(cutoff, delta)),
                body: Box::new(body.lift(cutoff + 1, delta)),
            },
            Term::App(f, x) => Term::App(
                Box::new(f.lift(cutoff, delta)),
                Box::new(x.lift(cutoff, delta)),
            ),
        }
    }

    /// Substitue `Var(target)` par `replacement`. Décrémente les variables
    /// supérieures (un binder a été éliminé). Lifte automatiquement le
    /// `replacement` selon la profondeur courante.
    pub fn subst(&self, target: u32, replacement: &Term) -> Term {
        match self {
            Term::Var(i) => {
                if *i == target {
                    replacement.lift(0, target)
                } else if *i > target {
                    Term::Var(i - 1)
                } else {
                    Term::Var(*i)
                }
            }
            Term::Sort(n) => Term::Sort(*n),
            Term::Lam { ty, body } => Term::Lam {
                ty: Box::new(ty.subst(target, replacement)),
                body: Box::new(body.subst(target + 1, replacement)),
            },
            Term::Pi { ty, body } => Term::Pi {
                ty: Box::new(ty.subst(target, replacement)),
                body: Box::new(body.subst(target + 1, replacement)),
            },
            Term::App(f, x) => Term::App(
                Box::new(f.subst(target, replacement)),
                Box::new(x.subst(target, replacement)),
            ),
        }
    }

    /// Hash content-addressed (sha256 domain-separated).
    ///
    /// Propriété : deux termes α-équivalents (i.e. structurellement
    /// identiques en de Bruijn) ont le même hash.
    pub fn hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(TERM_HASH_DOMAIN);
        self.write_canonical(&mut h);
        let result = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    fn write_canonical(&self, h: &mut Sha256) {
        match self {
            Term::Var(i) => {
                h.update([TAG_VAR]);
                h.update(i.to_le_bytes());
            }
            Term::Sort(n) => {
                h.update([TAG_SORT]);
                h.update(n.to_le_bytes());
            }
            Term::Lam { ty, body } => {
                h.update([TAG_LAM]);
                ty.write_canonical(h);
                body.write_canonical(h);
            }
            Term::Pi { ty, body } => {
                h.update([TAG_PI]);
                ty.write_canonical(h);
                body.write_canonical(h);
            }
            Term::App(f, x) => {
                h.update([TAG_APP]);
                f.write_canonical(h);
                x.write_canonical(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_same_term_same() {
        let a = Term::lam(Term::ty(), Term::var(0));
        let b = Term::lam(Term::ty(), Term::var(0));
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn hash_distinguishes_constructors() {
        let v = Term::var(0).hash();
        let s = Term::sort(0).hash();
        let l = Term::lam(Term::ty(), Term::var(0)).hash();
        let p = Term::pi(Term::ty(), Term::var(0)).hash();
        let app = Term::app(Term::var(0), Term::var(0)).hash();
        let unique: std::collections::BTreeSet<_> = [v, s, l, p, app].into_iter().collect();
        assert_eq!(unique.len(), 5, "tags doivent distinguer les constructeurs");
    }

    #[test]
    fn hash_distinguishes_indices() {
        assert_ne!(Term::var(0).hash(), Term::var(1).hash());
        assert_ne!(Term::sort(0).hash(), Term::sort(1).hash());
    }

    #[test]
    fn hash_uses_domain_separation() {
        // Sha256 brut sur les bytes des tags ne doit PAS coïncider.
        let mut raw = Sha256::new();
        raw.update([TAG_VAR]);
        raw.update(0u32.to_le_bytes());
        let raw_result: [u8; 32] = raw.finalize().into();
        assert_ne!(Term::var(0).hash(), raw_result);
    }

    #[test]
    fn lift_var_above_cutoff_increases() {
        let t = Term::var(2);
        assert_eq!(t.lift(1, 3), Term::var(5));
    }

    #[test]
    fn lift_var_below_cutoff_unchanged() {
        let t = Term::var(0);
        assert_eq!(t.lift(1, 3), Term::var(0));
    }

    #[test]
    fn lift_traverses_binders() {
        // λ. Var(1) — le 1 référence un binder externe. Sous le λ on est à
        // profondeur 1, donc cutoff=0+1=1 dans le body. Var(1) ≥ 1 → lift.
        let t = Term::lam(Term::ty(), Term::var(1));
        let lifted = t.lift(0, 1);
        assert_eq!(lifted, Term::lam(Term::ty(), Term::var(2)));
    }

    #[test]
    fn subst_replaces_target_var() {
        // Var(0)[0 := y] = y (lifté de 0)
        let t = Term::var(0);
        let r = Term::sort(7);
        assert_eq!(t.subst(0, &r), Term::sort(7));
    }

    #[test]
    fn subst_decrements_higher_vars() {
        // Var(2)[0 := y] = Var(1) (le binder à 0 est consommé, les supérieurs descendent)
        let t = Term::var(2);
        let r = Term::sort(0);
        assert_eq!(t.subst(0, &r), Term::var(1));
    }

    #[test]
    fn subst_lifts_replacement_under_binders() {
        // (λ. Var(1))[0 := Var(0)] :
        //   sous le λ, target=1. On hit Var(1), on remplace par Var(0).lift(0, 1) = Var(1).
        let body = Term::lam(Term::ty(), Term::var(1));
        let r = Term::var(0);
        let result = body.subst(0, &r);
        assert_eq!(result, Term::lam(Term::ty(), Term::var(1)));
    }
}

}
