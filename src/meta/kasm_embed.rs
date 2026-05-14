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
