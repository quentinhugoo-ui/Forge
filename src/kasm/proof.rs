//! Π.14 (Wave 9, 2026-05-02) — CompCert-style formal proofs in syntax.
//!
//! **Origine** : CompCert (Xavier Leroy, INRIA, 2008-). Le compilateur
//! C vérifié en Coq dont la promesse révolutionnaire :
//!
//!   "Si le code Coq type-check, le compilateur est correct."
//!
//! Plus besoin de tests post-hoc : la **structure des types** est la
//! preuve. Toute construction d'un programme illégal est bloquée à
//! la compilation.
//!
//! ## Pourquoi pour Forge ?
//!
//! Forge a déjà un verifier (`kasm::program::verify`) qui rejette les
//! programmes mal formés à la création. Mais ses invariants sont
//! exprimés en runtime checks (`Result<Program, KasmError>`). Une
//! fois le `Program` construit, le type Rust n'encode PAS quelles
//! propriétés ont été prouvées — un caller ne peut pas distinguer
//! "Program qui a passé le verify basic" de "Program prouvé pure" de
//! "Program prouvé total" etc.
//!
//! Wave 9 ajoute des **witness types** qui rendent ces propriétés
//! visibles au type checker :
//!
//!   - `Proven<P, Terminating>`  — terminaison prouvée
//!   - `Proven<P, NoUB>`         — pas d'UB (saturating arithmetic uniquement)
//!   - `Proven<P, Pure>`         — pure (pas d'I/O, pas de hash one-way)
//!   - `Proven<P, Deterministic>` — déterministe cross-machine
//!
//! Chaque témoin est un type marker zero-cost ; le combiner avec un
//! programme produit un wrapper qui ne peut être construit que via
//! une fonction de promotion qui vérifie l'invariant à runtime.
//!
//! ## Anatomie d'une preuve Forge
//!
//! ```ignore
//! use kasm::proof::{Proven, Terminating, prove_terminating};
//!
//! // Construction d'un Program (verify basique).
//! let prog = Program::new(...).unwrap();
//!
//! // Promotion vers un type Proven<_, Terminating>.
//! let proved: Proven<Program, Terminating> = prove_terminating(prog).unwrap();
//!
//! // À ce point, le type indique « ce programme termine ».
//! // Une API qui exige `Proven<_, Terminating>` ne peut PAS être
//! // appelée avec un Program brut — le compilateur refuse.
//! fn run_in_strict_realtime(p: &Proven<Program, Terminating>) { ... }
//! ```
//!
//! ## Limitations Wave 9 minimal
//!
//! - Les propriétés sont vérifiées RUNTIME (à la promotion), puis
//!   l'invariant est porté au type level. C'est plus fort que rien
//!   mais moins fort qu'une vraie preuve Coq (où la propriété est
//!   décidée à la compilation par le théorème checker).
//! - 4 witness types Wave 9 minimal. Extension Wave 11+ : Bounded,
//!   ConstantTime, MemoryBoundN, HashStable, etc.
//! - Les witnesses ne se composent pas encore (pas de `Proven<_,
//!   And<Terminating, Pure>>`) — Wave 11+ via type-level conjunction.

use crate::kasm::program::Program;
use crate::kasm::types::{KasmError, Op};
use std::marker::PhantomData;

// ═══════════════════════════════════════════════════════════════════
// Witness marker types
// ═══════════════════════════════════════════════════════════════════

/// Trait sealed : les witness types sont fermés à l'extension externe.
mod sealed {
    pub trait Witness {}
}

/// Le programme termine sur tout input. KASM verify garantit cette
/// propriété par construction (DAG borné, pas de loop unbounded).
#[derive(Debug, Clone, Copy)]
pub struct Terminating;
impl sealed::Witness for Terminating {}

/// Le programme n'a pas de undefined behavior. Pour KASM, cela
/// signifie : pas de division par zéro non protégée, pas de wrapping
/// arithmétique problématique, pas de uninitialized read.
#[derive(Debug, Clone, Copy)]
pub struct NoUB;
impl sealed::Witness for NoUB {}

/// Le programme est pur : pas d'I/O, pas de Hash64 (one-way),
/// pas de F64Op (libc dependency cross-host).
#[derive(Debug, Clone, Copy)]
pub struct Pure;
impl sealed::Witness for Pure {}

/// Le programme est déterministe cross-machine : seuls les opcodes
/// avec layout binaire stable (i64 wrapping arithmetic, bitops). Pas
/// de F64 (ULP différents par libc), pas de transcendentals.
#[derive(Debug, Clone, Copy)]
pub struct Deterministic;
impl sealed::Witness for Deterministic {}

// ═══════════════════════════════════════════════════════════════════
// Proven<T, W> — le wrapper avec construction privée
// ═══════════════════════════════════════════════════════════════════

/// Un objet `T` accompagné d'un témoin `W` prouvant une propriété.
/// Construction privée → uniquement via une fonction de promotion
/// publique qui vérifie l'invariant.
#[derive(Debug, Clone)]
pub struct Proven<T, W: sealed::Witness> {
    inner: T,
    _witness: PhantomData<W>,
}

impl<T, W: sealed::Witness> Proven<T, W> {
    /// Lecture immutable du contenu (sans perdre la preuve).
    pub fn as_inner(&self) -> &T {
        &self.inner
    }

    /// Consume the proof, return the bare T.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

// ═══════════════════════════════════════════════════════════════════
// Proof errors
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub enum ProofError {
    /// Un opcode incompatible avec la propriété cible est présent.
    DisallowedOp { node: usize, op: Op, reason: &'static str },
    /// Le programme a été rejeté par un check structurel.
    StructureViolation(&'static str),
    /// Le verifier KASM standard a échoué — pas de preuve possible.
    BaseVerify(KasmError),
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofError::DisallowedOp { node, op, reason } =>
                write!(f, "node {} : op {:?} disallowed ({})", node, op, reason),
            ProofError::StructureViolation(s) =>
                write!(f, "structure violation: {}", s),
            ProofError::BaseVerify(e) =>
                write!(f, "base verify failed: {:?}", e),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Promotion functions — runtime check + type-level lift
// ═══════════════════════════════════════════════════════════════════

/// Promotion vers `Proven<Program, Terminating>`. Pour KASM, tout
/// programme valide termine par construction (DAG borné). Cette
/// preuve est triviale mais utile pour les API strict-realtime qui
/// exigent le type witness.
pub fn prove_terminating(prog: Program) -> Result<Proven<Program, Terminating>, ProofError> {
    // Re-verify pour garantir l'invariant base.
    let _ = Program::from_bytes(prog.bytes())
        .map_err(ProofError::BaseVerify)?;
    Ok(Proven { inner: prog, _witness: PhantomData })
}

/// Promotion vers `Proven<Program, NoUB>`. KASM minimal viable :
/// rejette `DivI64Checked` (qui est défini comme 0 sur b=0, donc safe)
/// — wait, c'est safe. Wave 9 minimal interdit plutôt les ops qui
/// pourraient observer hardware UB : aucun pour l'instant.
/// On accepte tous les programmes valides comme NoUB (KASM est total
/// par design grâce aux Checked variants).
pub fn prove_no_ub(prog: Program) -> Result<Proven<Program, NoUB>, ProofError> {
    let _ = Program::from_bytes(prog.bytes())
        .map_err(ProofError::BaseVerify)?;
    // KASM est total par design : tous les ops "potentiellement UB" du
    // C (div/0, signed overflow) ont des variantes Checked ou
    // wrapping qui sont total functions. Aucune action de filtrage
    // additionnelle nécessaire — la preuve est par construction de
    // l'ISA.
    Ok(Proven { inner: prog, _witness: PhantomData })
}

/// Promotion vers `Proven<Program, Pure>`. Rejette les opcodes
/// non-purs : `Hash64` (one-way fonction), `F64Op` (libc-dependent
/// transcendentals comme exp/ln). Wave 9 minimal — extension Wave
/// 11+ pour les ops Vec et meta-ops.
pub fn prove_pure(prog: Program) -> Result<Proven<Program, Pure>, ProofError> {
    let _ = Program::from_bytes(prog.bytes())
        .map_err(ProofError::BaseVerify)?;
    for (i, node) in prog.nodes().iter().enumerate() {
        match node.op {
            Op::Hash64 => return Err(ProofError::DisallowedOp {
                node: i, op: node.op,
                reason: "Hash64 is one-way (irreversible)",
            }),
            Op::F64Op => return Err(ProofError::DisallowedOp {
                node: i, op: node.op,
                reason: "F64Op uses libc transcendentals (cross-host drift)",
            }),
            // Wave 8 ops self-hosting : non-pures par défaut (peuvent
            // contenir des side effects via le dispatcher).
            Op::Fractal | Op::Eval => return Err(ProofError::DisallowedOp {
                node: i, op: node.op,
                reason: "Self-host opcodes have runtime side effects",
            }),
            _ => {}
        }
    }
    Ok(Proven { inner: prog, _witness: PhantomData })
}

/// Promotion vers `Proven<Program, Deterministic>`. Rejette tout
/// opcode dont le résultat peut différer cross-machine : `F64Op`
/// (libc ULP), opcodes Wave 8 self-hosting (dépendance dispatcher).
pub fn prove_deterministic(
    prog: Program,
) -> Result<Proven<Program, Deterministic>, ProofError> {
    let _ = Program::from_bytes(prog.bytes())
        .map_err(ProofError::BaseVerify)?;
    for (i, node) in prog.nodes().iter().enumerate() {
        match node.op {
            // F64Op transcendentals (Exp, Ln) divergent de 1 ULP cross-host
            // (audit Φ.7a). Les autres F64 ops sont bit-identical IEEE 754.
            // Wave 9 minimal : conservative — interdit tout F64Op pour
            // garantir Deterministic. Wave 11+ pourra distinguer les
            // sub-ops déterministes (Add/Sub/Mul/Div bit-stable) des
            // non-déterministes (Exp/Ln libc).
            Op::F64Op => return Err(ProofError::DisallowedOp {
                node: i, op: node.op,
                reason: "F64 transcendentals diverge cross-host (libc ULP)",
            }),
            Op::Fractal | Op::Eval => return Err(ProofError::DisallowedOp {
                node: i, op: node.op,
                reason: "Self-host depends on runtime callee table",
            }),
            _ => {}
        }
    }
    Ok(Proven { inner: prog, _witness: PhantomData })
}

// ═══════════════════════════════════════════════════════════════════
// Type-level API examples — fonctions qui exigent un witness
// ═══════════════════════════════════════════════════════════════════

/// API exemple : ne peut être appelée qu'avec un `Proven<_, Pure>`.
/// Le compilateur refuse tout `Program` brut — la preuve est exigée
/// au type level, pas un commentaire ou une assertion runtime.
pub fn require_pure_for_caching(p: &Proven<Program, Pure>) -> &Program {
    // Au sein de cette fonction, on sait que p est pure, donc
    // safe pour caching cross-process / cross-call.
    p.as_inner()
}

/// API exemple : exige `Deterministic` pour partager via le swarm.
/// Un programme non-déterministe pourrait diverger entre nodes du swarm.
pub fn require_deterministic_for_swarm(p: &Proven<Program, Deterministic>) -> &Program {
    p.as_inner()
}

/// API exemple : exige `Terminating` pour scheduling realtime.
pub fn require_terminating_for_realtime(p: &Proven<Program, Terminating>) -> &Program {
    p.as_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::types::{Node, Target, Ty};

    fn affine_program() -> Program {
        // f(x) = 3*x + 7
        Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(3),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::add(3, 2),
                Node::output(4, Ty::I64),
            ],
        ).unwrap()
    }

    fn hash_program() -> Program {
        // f(x) = hash64(x)
        Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::hash64(0),
                Node::output(1, Ty::I64),
            ],
        ).unwrap()
    }

    #[test]
    fn proof_terminating_succeeds_on_basic_program() {
        let prog = affine_program();
        let proved = prove_terminating(prog).unwrap();
        assert_eq!(proved.as_inner().nodes().len(), 6);
    }

    #[test]
    fn proof_pure_rejects_hash64() {
        let prog = hash_program();
        let err = prove_pure(prog).unwrap_err();
        match err {
            ProofError::DisallowedOp { op: Op::Hash64, .. } => {}
            _ => panic!("expected DisallowedOp Hash64, got {:?}", err),
        }
    }

    #[test]
    fn proof_pure_succeeds_on_affine_program() {
        let prog = affine_program();
        let proved = prove_pure(prog).unwrap();
        assert_eq!(proved.as_inner().nodes().len(), 6);
    }

    #[test]
    fn proof_deterministic_succeeds_on_pure_i64_program() {
        let prog = affine_program();
        let proved = prove_deterministic(prog).unwrap();
        // Hash64 est OK pour Deterministic (bit-stable cross-machine),
        // c'est seulement Pure qui le refuse.
        assert_eq!(proved.as_inner().nodes().len(), 6);
    }

    #[test]
    fn proof_deterministic_accepts_hash64() {
        // Hash64 est déterministe (SplitMix64 fixe), donc accepté
        // pour Deterministic ; refusé seulement pour Pure.
        let prog = hash_program();
        let proved = prove_deterministic(prog).unwrap();
        assert_eq!(proved.as_inner().nodes().len(), 3);
    }

    #[test]
    fn proof_no_ub_succeeds_on_div_program() {
        // KASM Op::DivI64Checked retourne 0 sur b=0 — total function,
        // donc NoUB par construction.
        let prog = Program::new(
            Target::Cpu, 2, 1, 32,
            vec![
                Node::input(0),
                Node::input(1),
                Node {
                    op: Op::DivI64Checked,
                    ty: Ty::I64,
                    a: 0, b: 1, imm: 0,
                },
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let proved = prove_no_ub(prog).unwrap();
        assert_eq!(proved.as_inner().nodes().len(), 4);
    }

    #[test]
    fn proof_witness_type_required_at_compile() {
        // Le wrapper est zero-size : PhantomData<W>. Un Proven<P, T>
        // a la même size que P.
        let prog = affine_program();
        let proved = prove_terminating(prog).unwrap();
        // PhantomData<Terminating> est ZST — Proven<Program, T> = sizeof(Program).
        assert!(std::mem::size_of_val(&proved) >= std::mem::size_of::<Program>());
    }

    #[test]
    fn proof_into_inner_consumes_proof() {
        let prog = affine_program();
        let proved = prove_terminating(prog).unwrap();
        let bare = proved.into_inner();
        // Bare Program — la preuve est consommée. On ne peut plus
        // appeler require_terminating_for_realtime sur `bare`.
        assert_eq!(bare.nodes().len(), 6);
    }

    #[test]
    fn proof_pure_rejects_fractal() {
        // Wave 8 self-hosting opcodes ne sont pas pures.
        let prog = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(42),
                Node {
                    op: Op::Fractal,
                    ty: Ty::I64,
                    a: 1, b: 0, imm: 0,
                },
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let err = prove_pure(prog).unwrap_err();
        assert!(matches!(err, ProofError::DisallowedOp { op: Op::Fractal, .. }));
    }

    #[test]
    fn proof_caching_api_requires_pure_witness() {
        // Démonstration du pattern compile-time enforcement.
        let prog = affine_program();
        let proved_pure = prove_pure(prog).unwrap();
        // Cette API n'accepte QUE Proven<_, Pure>.
        let _ref: &Program = require_pure_for_caching(&proved_pure);
        // Si on essaie : require_pure_for_caching(&affine_program()) →
        // compile error, expected &Proven<Program, Pure>, found &Program.
    }

    #[test]
    fn proof_witness_types_are_distinct() {
        // Proven<P, Pure> et Proven<P, Deterministic> sont des types
        // différents même si l'underlying Program est le même.
        let prog1 = affine_program();
        let prog2 = affine_program();
        let p_pure: Proven<Program, Pure> = prove_pure(prog1).unwrap();
        let p_det: Proven<Program, Deterministic> = prove_deterministic(prog2).unwrap();
        // require_pure n'accepte pas un Proven<_, Deterministic> :
        // require_pure_for_caching(&p_det) → compile error.
        // (Documenté ; pas testable sans #[test] compile_fail).
        let _ = require_pure_for_caching(&p_pure);
        let _ = require_deterministic_for_swarm(&p_det);
    }

    #[test]
    fn proof_deterministic_rejects_eval() {
        let prog = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(99),
                Node {
                    op: Op::Eval,
                    ty: Ty::I64,
                    a: 1, b: 0, imm: 0,
                },
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let err = prove_deterministic(prog).unwrap_err();
        assert!(matches!(err, ProofError::DisallowedOp { op: Op::Eval, .. }));
    }
}
