//! Ω-7 — La Dissolution du Tokenizer.
//!
//! Cap actuel : **Ω-7.0** — agent symbolique non-LLM qui propose des
//! réécritures KASM **sans aucun token de langue naturelle dans son IO**.
//!
//! ## Doctrine
//!
//! L'agent perçoit un `kasm::Program` (bytes content-addressed) et
//! produit des candidats `Program` (bytes content-addressed). Sa
//! représentation interne utilise :
//!
//!  * Les opcodes KASM (`Op` enum, 28 variants).
//!  * Les indices de nœuds (`u16`).
//!  * Optionnellement, les termes du calcul des constructions Ω-4 (`Term`).
//!
//! Aucun `&str`, `String`, ou autre encodage texte humain n'entre dans
//! le raisonnement. Les seules `&str` sont les noms internes des règles
//! pour debug/logging — jamais consommés par la logique de transformation.
//!
//! ## Critère Ω-7
//!
//! > L'agent produit une réécriture KASM valide sans avoir vu un token
//! > de langue naturelle.
//!
//! Implémenté par : `SymbolicAgent::propose_rewrites(&Program) -> Vec<Program>`.
//! Chaque candidat retourné est :
//!  * Un `kasm::Program` valide (passe `verify`).
//!  * Sémantiquement équivalent à l'input sur les ops déterministes.
//!  * Strictement préférable (plus petit OU coût Landauer plus bas).
//!
//! ## Limites assumées
//!
//! - 3 règles algébriques minimales (add_zero, mul_one, const_fold).
//!   Étendre = Ω-7.0.x.
//! - Pas d'apprentissage, pas de neural net. C'est un agent *symbolique*,
//!   première brique avant l'agent **appris** Ω-7.1+.
//! - Pas de connexion au corpus Linux/Lean mathlib (Ω-7.x).


pub use symbolic::{RankedCandidate, SymbolicAgent};
pub use term_pattern::{match_pattern, Bindings, HoleId, TermPattern};

pub mod symbolic {
//! Ω-7.0 — Agent symbolique : pattern-match sur AST KASM, propose des
//! réécritures équivalentes mais préférables.
//!
//! Entrée : `kasm::Program` (bytes content-addressed).
//! Sortie : `Vec<Program>` (bytes content-addressed).
//! IO interne : `Op`, `u16`, `i16`, `i64`. Aucun `String` n'entre dans la
//! logique de transformation — le critère Ω-7 est satisfait par typage.

use crate::kasm::{Node, Op, Program};
use crate::landauer::{program_cost, ProgramCost};

/// Candidat de réécriture, ranké par taille puis coût Landauer.
#[derive(Debug, Clone)]
pub struct RankedCandidate {
    pub program: Program,
    pub size: usize,
    pub landauer: ProgramCost,
    /// Score lexicographique : (size, total_bits_erased) — lower = better.
    pub score: (usize, u64),
}

/// Agent symbolique. Applique des règles algébriques pour générer
/// des candidats. Aucun apprentissage — la logique est entièrement
/// déterministe et content-addressed.
#[derive(Debug, Default)]
pub struct SymbolicAgent;

impl SymbolicAgent {
    pub fn new() -> Self {
        Self
    }

    /// Propose des réécritures équivalentes du programme. Chaque candidat
    /// est **canonicalisé** avant scoring (élimine dead code, applique CSE).
    /// Retourne uniquement les candidats **strictement préférables** au
    /// programme original lui-même canonicalisé.
    pub fn propose_rewrites(&self, program: &Program) -> Vec<RankedCandidate> {
        let mut raw_candidates: Vec<Program> = Vec::new();

        // Règles Ω-7.0 (first mile)
        if let Some(p) = rule_add_zero(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_mul_one(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_const_fold(program) {
            raw_candidates.push(p);
        }
        // Règles Ω-7.0.1 (extension first mile)
        if let Some(p) = rule_sub_x_x_zero(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_bit_xor_x_x_zero(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_bit_and_x_x(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_bit_or_x_x(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_associativity_add(program) {
            raw_candidates.push(p);
        }
        if let Some(p) = rule_distributivity_left(program) {
            raw_candidates.push(p);
        }

        // Score de référence = forme canonique du programme original.
        let original_score = score_canonical(program);

        // Pour chaque candidat brut : canonicalize (élimine dead code,
        // CSE, etc.), puis score. Filtre les non-améliorations.
        let mut ranked: Vec<RankedCandidate> = raw_candidates
            .into_iter()
            .filter_map(|raw| {
                let canon = raw.canonical().ok()?;
                let cost = program_cost(&canon);
                let score = (canon.nodes().len(), cost.total_bits_erased);
                Some(RankedCandidate {
                    size: canon.nodes().len(),
                    landauer: cost,
                    score,
                    program: canon,
                })
            })
            .filter(|c| c.score < original_score)
            .collect();

        ranked.sort_by_key(|c| c.score);
        ranked
    }
}

/// Score d'un programme via sa forme canonique : (n_nodes, bits_erased).
/// Si canonicalize échoue (rare — programmes avec Reduce ops sont
/// retournés tels quels), on score sur le brut.
fn score_canonical(p: &Program) -> (usize, u64) {
    let canon = p.canonical().unwrap_or_else(|_| p.clone());
    let cost = program_cost(&canon);
    (canon.nodes().len(), cost.total_bits_erased)
}

// ---------------------------------------------------------------------------
// Règles
// ---------------------------------------------------------------------------

/// `Add(x, 0)` ou `Add(0, x)` → `x`. Plus généralement : élimine toute
/// AddI64 dont une opérande est une ConstI64(0).
fn rule_add_zero(p: &Program) -> Option<Program> {
    rule_unit_elimination(p, Op::AddI64, 0)
}

/// `Mul(x, 1)` ou `Mul(1, x)` → `x`.
fn rule_mul_one(p: &Program) -> Option<Program> {
    rule_unit_elimination(p, Op::MulI64, 1)
}

/// Helper générique : trouve une op `target_op` dont une opérande est
/// `ConstI64(unit_value)`, remplace la référence à l'op par la référence
/// à l'autre opérande.
fn rule_unit_elimination(p: &Program, target_op: Op, unit_value: i16) -> Option<Program> {
    let nodes = p.nodes();

    // Cherche le premier candidat (premier nœud target_op avec un opérande const(unit)).
    let (target_idx, replacement_idx) = find_unit_elim_match(nodes, target_op, unit_value)?;

    // Construit un nouveau programme où toutes les références à target_idx
    // sont remplacées par replacement_idx, et le nœud target_idx est laissé
    // en place (canonicalize l'éliminera comme dead code).
    rebuild_with_substitution(p, target_idx, replacement_idx)
}

fn find_unit_elim_match(nodes: &[Node], target_op: Op, unit_value: i16) -> Option<(u16, u16)> {
    for (i, node) in nodes.iter().enumerate() {
        if node.op != target_op {
            continue;
        }
        // node.a et node.b sont les indices des opérandes.
        let a_node = nodes.get(node.a as usize)?;
        let b_node = nodes.get(node.b as usize)?;
        if a_node.op == Op::ConstI64 && a_node.imm == unit_value {
            return Some((i as u16, node.b));
        }
        if b_node.op == Op::ConstI64 && b_node.imm == unit_value {
            return Some((i as u16, node.a));
        }
    }
    None
}

/// `Add(c1, c2)` ou `Mul(c1, c2)` avec c1, c2 ConstI64 → ConstI64(folded).
/// Retourne None si folded déborde i16 (KASM ConstI64 limité à i16).
fn rule_const_fold(p: &Program) -> Option<Program> {
    let nodes = p.nodes();

    for (i, node) in nodes.iter().enumerate() {
        let target_idx = i as u16;
        let folded_imm = match node.op {
            Op::AddI64 => fold_pair(nodes, node.a, node.b, |a, b| a.checked_add(b))?,
            Op::MulI64 => fold_pair(nodes, node.a, node.b, |a, b| a.checked_mul(b))?,
            _ => continue,
        };

        // Ajoute un nouveau nœud ConstI64(folded) APRÈS l'op originale et
        // remplace les références au target_idx par ce nouvel index.
        let new_const = Node::const_i64(folded_imm);
        let new_const_idx = (nodes.len() + 1) as u16; // sera positionné en fin
        let _ = new_const_idx;

        // Stratégie plus simple : on reconstruit en plaçant le const à la
        // position target_idx (en remplacement de l'op fold-able), et on
        // ajuste tout le reste.
        return rebuild_replacing_node(p, target_idx, new_const);
    }
    None
}

fn fold_pair(
    nodes: &[Node],
    a: u16,
    b: u16,
    op: impl FnOnce(i16, i16) -> Option<i16>,
) -> Option<i16> {
    let a_node = nodes.get(a as usize)?;
    let b_node = nodes.get(b as usize)?;
    if a_node.op != Op::ConstI64 || b_node.op != Op::ConstI64 {
        return None;
    }
    op(a_node.imm, b_node.imm)
}

// ---------------------------------------------------------------------------
// Règles Ω-7.0.1
// ---------------------------------------------------------------------------

/// `Sub(x, x) → 0`. Quand l'op a les mêmes deux opérandes, le résultat
/// est constant zéro.
fn rule_sub_x_x_zero(p: &Program) -> Option<Program> {
    rule_xx_to_const(p, Op::SubI64, 0)
}

/// `BitXor(x, x) → 0`.
fn rule_bit_xor_x_x_zero(p: &Program) -> Option<Program> {
    rule_xx_to_const(p, Op::BitXorI64, 0)
}

/// `BitAnd(x, x) → x`. Idempotence bitwise.
fn rule_bit_and_x_x(p: &Program) -> Option<Program> {
    rule_xx_to_x(p, Op::BitAndI64)
}

/// `BitOr(x, x) → x`. Idempotence bitwise.
fn rule_bit_or_x_x(p: &Program) -> Option<Program> {
    rule_xx_to_x(p, Op::BitOrI64)
}

/// Helper : trouve une op `target_op` dont les deux opérandes sont identiques
/// et remplace le nœud par `Const(replacement)`.
fn rule_xx_to_const(p: &Program, target_op: Op, replacement: i16) -> Option<Program> {
    let nodes = p.nodes();
    for (i, node) in nodes.iter().enumerate() {
        if node.op != target_op {
            continue;
        }
        if node.a == node.b {
            return rebuild_replacing_node(p, i as u16, Node::const_i64(replacement));
        }
    }
    None
}

/// Helper : trouve une op binaire idempotente (`x op x = x`) et remplace le
/// résultat par x lui-même via substitution.
fn rule_xx_to_x(p: &Program, target_op: Op) -> Option<Program> {
    let nodes = p.nodes();
    for (i, node) in nodes.iter().enumerate() {
        if node.op != target_op {
            continue;
        }
        if node.a == node.b {
            return rebuild_with_substitution(p, i as u16, node.a);
        }
    }
    None
}

/// `Add(Add(a, b), c) → Add(a, Add(b, c))`. Réassociation à droite ; même
/// sémantique sous wrapping i64 (associatif). Utile pour le CSE en aval qui
/// peut détecter des sous-arbres communs sous une forme.
///
/// Bail si le programme contient des Reduce* (range invariant fragile sous
/// re-indexation).
fn rule_associativity_add(p: &Program) -> Option<Program> {
    if has_reduce_op(p) {
        return None;
    }
    let nodes = p.nodes();

    let mut outer_idx: Option<usize> = None;
    for (i, node) in nodes.iter().enumerate() {
        if node.op != Op::AddI64 {
            continue;
        }
        let inner = nodes.get(node.a as usize)?;
        if inner.op == Op::AddI64 {
            outer_idx = Some(i);
            break;
        }
    }
    let outer_idx = outer_idx?;
    let outer = nodes[outer_idx];
    let inner = nodes[outer.a as usize];

    let a = inner.a;
    let b = inner.b;
    let c = outer.b;

    rebuild_with_inserted_assoc(p, outer_idx, a, b, c)
}

/// Re-indexation pour associativity: insère `Add(b, c)` puis `Add(a, ...)`
/// à la place du `Add(Add(a, b), c)` original.
fn rebuild_with_inserted_assoc(
    p: &Program,
    target_idx: usize,
    a: u16,
    b: u16,
    c: u16,
) -> Option<Program> {
    let nodes = p.nodes();
    let mut new_nodes: Vec<Node> = Vec::with_capacity(nodes.len() + 1);
    let mut remap: Vec<u16> = vec![0u16; nodes.len()];

    for (i, node) in nodes.iter().enumerate() {
        if i == target_idx {
            let a_r = remap[a as usize];
            let b_r = remap[b as usize];
            let c_r = remap[c as usize];
            let inner_idx = new_nodes.len() as u16;
            new_nodes.push(Node::add(b_r, c_r));
            let outer_idx_new = new_nodes.len() as u16;
            new_nodes.push(Node::add(a_r, inner_idx));
            remap[i] = outer_idx_new;
        } else {
            let mut n = *node;
            if (n.a as usize) < remap.len() {
                n.a = remap[n.a as usize];
            }
            if (n.b as usize) < remap.len() {
                n.b = remap[n.b as usize];
            }
            if matches!(n.op, Op::SelectI64 | Op::ClampI64 | Op::Cond) {
                // Op::Cond uses imm as else_slot (3rd ref) just like
                // SelectI64/ClampI64 — audit 2026-05-01.
                let third = n.imm as u16;
                if (third as usize) < remap.len() {
                    n.imm = remap[third as usize] as i16;
                }
            }
            let new_idx = new_nodes.len() as u16;
            new_nodes.push(n);
            remap[i] = new_idx;
        }
    }

    Program::new(p.target(), p.inputs(), p.outputs(), new_nodes.len() as u32, new_nodes).ok()
}

/// `Mul(a, Add(b, c)) → Add(Mul(a, b), Mul(a, c))`. Distributivité gauche.
/// Cette règle augmente le compte de nœuds dans le cas général ; elle est
/// proposée pour permettre au filtre de score de la garder UNIQUEMENT quand
/// la canonicalisation post-distribution réduit (e.g. `b == 0` ou `c == 0`).
///
/// Bail si reduce ops (re-indexation hostile).
fn rule_distributivity_left(p: &Program) -> Option<Program> {
    if has_reduce_op(p) {
        return None;
    }
    let nodes = p.nodes();

    for (i, node) in nodes.iter().enumerate() {
        if node.op != Op::MulI64 {
            continue;
        }
        // Mul(a, Add(b, c)) — second operand est un Add.
        let rhs = nodes.get(node.b as usize)?;
        if rhs.op != Op::AddI64 {
            continue;
        }
        let a = node.a;
        let b = rhs.a;
        let c = rhs.b;

        // Build new program. Insert: Mul(a, b), Mul(a, c), Add(...) en
        // remplacement du Mul original. Donc: 2 nouveaux nœuds AVANT i, et i
        // devient un Add(mul_ab, mul_ac).
        let candidate = rebuild_with_inserted_distrib(p, i, a, b, c);
        return candidate;
    }
    None
}

fn has_reduce_op(p: &Program) -> bool {
    p.nodes()
        .iter()
        .any(|n| matches!(n.op, Op::ReduceAddI64 | Op::ReduceMulI64))
}

/// Re-indexation pour distributivité : Mul(a, Add(b,c)) à `target_idx`
/// devient deux Muls + un Add (3 nœuds → +2 par rapport à l'original).
fn rebuild_with_inserted_distrib(
    p: &Program,
    target_idx: usize,
    a: u16,
    b: u16,
    c: u16,
) -> Option<Program> {
    let nodes = p.nodes();
    let mut new_nodes: Vec<Node> = Vec::with_capacity(nodes.len() + 2);
    let mut remap: Vec<u16> = vec![0u16; nodes.len()];

    for (i, node) in nodes.iter().enumerate() {
        if i == target_idx {
            let a_r = remap[a as usize];
            let b_r = remap[b as usize];
            let c_r = remap[c as usize];
            // Insert Mul(a, b)
            let mul_ab = new_nodes.len() as u16;
            new_nodes.push(Node::mul(a_r, b_r));
            // Insert Mul(a, c)
            let mul_ac = new_nodes.len() as u16;
            new_nodes.push(Node::mul(a_r, c_r));
            // Insert Add(mul_ab, mul_ac)
            let add_idx = new_nodes.len() as u16;
            new_nodes.push(Node::add(mul_ab, mul_ac));
            remap[i] = add_idx;
        } else {
            let mut n = *node;
            if (n.a as usize) < remap.len() {
                n.a = remap[n.a as usize];
            }
            if (n.b as usize) < remap.len() {
                n.b = remap[n.b as usize];
            }
            if matches!(n.op, Op::SelectI64 | Op::ClampI64 | Op::Cond) {
                // Op::Cond uses imm as else_slot (3rd ref) just like
                // SelectI64/ClampI64 — audit 2026-05-01.
                let third = n.imm as u16;
                if (third as usize) < remap.len() {
                    n.imm = remap[third as usize] as i16;
                }
            }
            let new_idx = new_nodes.len() as u16;
            new_nodes.push(n);
            remap[i] = new_idx;
        }
    }

    Program::new(p.target(), p.inputs(), p.outputs(), new_nodes.len() as u32, new_nodes).ok()
}

// ---------------------------------------------------------------------------
// Rebuild helpers
// ---------------------------------------------------------------------------

/// Reconstruit le programme avec toutes les références à `from_idx`
/// remplacées par `to_idx`. Le nœud à `from_idx` est conservé (deviendra
/// dead code, sera éliminé par `canonicalize`).
fn rebuild_with_substitution(
    p: &Program,
    from_idx: u16,
    to_idx: u16,
) -> Option<Program> {
    let mut new_nodes = Vec::with_capacity(p.nodes().len());
    for node in p.nodes() {
        let mut n = *node;
        if n.a == from_idx {
            n.a = to_idx;
        }
        if n.b == from_idx {
            n.b = to_idx;
        }
        // Pour SelectI64, ClampI64 et Op::Cond (audit 2026-05-01),
        // le 3e argument est dans imm. On le substitue aussi si on
        // l'utilise comme index (signed).
        if matches!(n.op, Op::SelectI64 | Op::ClampI64 | Op::Cond) {
            let third = n.imm as u16;
            if third == from_idx {
                n.imm = to_idx as i16;
            }
        }
        new_nodes.push(n);
    }
    Program::new(p.target(), p.inputs(), p.outputs(), p.fuel(), new_nodes).ok()
}

/// Remplace le nœud à `idx` par `new_node`, garde tout le reste, et
/// préserve les références.
fn rebuild_replacing_node(p: &Program, idx: u16, new_node: Node) -> Option<Program> {
    let mut new_nodes: Vec<Node> = p.nodes().to_vec();
    if (idx as usize) >= new_nodes.len() {
        return None;
    }
    new_nodes[idx as usize] = new_node;
    Program::new(p.target(), p.inputs(), p.outputs(), p.fuel(), new_nodes).ok()
}

// ---------------------------------------------------------------------------
// Pont Omega-7.0.3 -- conversion en RewriteV2 pour le verifier_v2.
// ---------------------------------------------------------------------------

/// Convertit les candidats de reecriture en `RewriteV2::ProgramSubstitution`.
/// Le hash `from` est calcule sur les bytes de l'input via `Hash::for_blob`
/// (meme convention que le store libgit2). Chaque candidat devient une
/// rewrite distincte.
///
/// Pont Omega-7.0.3 : permet d'envoyer les sorties de l'agent symbolique au
/// verifier Godel-machine v2 sans toucher au Codex original.
pub fn candidates_as_rewrites_v2(
    input: &Program,
    candidates: &[RankedCandidate],
) -> Vec<crate::godel::verifier_v2::RewriteV2> {
    use crate::godel::verifier_v2::RewriteV2;
    let from_hash = crate::Hash::for_blob(input.bytes());
    candidates
        .iter()
        .map(|c| RewriteV2::ProgramSubstitution {
            from: from_hash,
            to: crate::Hash::for_blob(c.program.bytes()),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Target, Ty};

    fn add_zero_program() -> Program {
        // f(x) = x + 0 — devrait être réduit à f(x) = x
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),       // %0
                Node::const_i64(0),   // %1
                Node::add(0, 1),      // %2 = %0 + %1
                Node::output(2, Ty::I64), // %3 = output(%2)
            ],
        )
        .unwrap()
    }

    fn mul_one_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn const_fold_add_program() -> Program {
        // 3 + 4 = 7 — devrait être réduit à un const direct
        Program::new(
            Target::Cpu,
            0,
            1,
            8,
            vec![
                Node::const_i64(3),
                Node::const_i64(4),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn no_rewrite_program() -> Program {
        // Programme déjà optimal : f(x, y) = x + y
        Program::new(
            Target::Cpu,
            2,
            1,
            8,
            vec![
                Node::input(0),
                Node::input(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn agent_finds_add_zero_elimination() {
        let agent = SymbolicAgent::new();
        let p = add_zero_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(
            !candidates.is_empty(),
            "agent doit proposer au moins une réécriture pour Add(x, 0)"
        );
        // Le meilleur candidat doit être strictement plus petit ou avec
        // moins de bits effacés.
        let best = &candidates[0];
        assert!(
            best.score < score_canonical(&p),
            "best candidate must improve on original"
        );
    }

    #[test]
    fn agent_finds_mul_one_elimination() {
        let agent = SymbolicAgent::new();
        let p = mul_one_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(
            !candidates.is_empty(),
            "agent doit proposer une réécriture pour Mul(x, 1)"
        );
    }

    #[test]
    fn agent_const_folds_add() {
        let agent = SymbolicAgent::new();
        let p = const_fold_add_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(
            !candidates.is_empty(),
            "agent doit constant-fold Add(3, 4)"
        );
        // Le candidat retourné a remplacé l'Add par un Const(7).
        let best = &candidates[0];
        let canon = best.program.canonical().unwrap();
        // Après canonicalize, le programme devrait juste être Const(7) + Output.
        // Au minimum : strictement moins de Lossy ops.
        let cost_canon = program_cost(&canon);
        let cost_orig = program_cost(&p);
        assert!(
            cost_canon.total_bits_erased < cost_orig.total_bits_erased,
            "after const-fold + canonicalize, Landauer cost must drop"
        );
    }

    #[test]
    fn agent_returns_empty_for_optimal_program() {
        let agent = SymbolicAgent::new();
        let p = no_rewrite_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(
            candidates.is_empty(),
            "agent ne doit rien proposer sur un programme déjà optimal"
        );
    }

    #[test]
    fn agent_output_is_valid_kasm_program() {
        // Critère Ω-7 : le candidat retourné est un Program valide
        // (passe verify). Comme on construit via Program::new, c'est
        // garanti par construction, mais on le vérifie explicitement.
        let agent = SymbolicAgent::new();
        let p = add_zero_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());
        for c in &candidates {
            // Round-trip via les bytes : si verify accepte, c'est valide.
            let bytes = c.program.bytes().to_vec();
            let reverified = Program::from_bytes(&bytes);
            assert!(reverified.is_ok(), "candidate must be a valid KASM Program");
        }
    }

    #[test]
    fn agent_output_is_executable_and_equivalent() {
        // Critère central Ω-7 : l'output est non seulement valide mais
        // sémantiquement équivalent à l'input sur les inputs testés.
        use crate::kasm::execute;

        let agent = SymbolicAgent::new();
        let p = add_zero_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());

        let best = &candidates[0];
        for x in [-5i64, 0, 1, 7, 100] {
            let inputs = x.to_le_bytes().to_vec();
            let original_out = execute(&p, &inputs).unwrap();
            let candidate_out = execute(&best.program, &inputs).unwrap();
            assert_eq!(
                original_out, candidate_out,
                "candidate must compute the same output for x={x}"
            );
        }
    }

    #[test]
    fn agent_produces_no_natural_language_in_output() {
        // Critère Ω-7 byte-stable : l'output est uniquement des bytes
        // KASM (pas un &str, pas un encodage texte). On vérifie via le
        // type de retour : Vec<RankedCandidate> où program: Program.
        let agent = SymbolicAgent::new();
        let p = add_zero_program();
        let candidates = agent.propose_rewrites(&p);
        // Le hash content-addressed est un fingerprint cryptographique
        // de l'AST KASM, pas un encoding texte.
        for c in &candidates {
            let _hash = c.program.canonical_hash_hex().unwrap();
            // _hash est un string hex mais c'est une projection du hash bytes,
            // pas une représentation linguistique. La donnée canonique est
            // c.program.bytes() — pure binary.
            let bytes = c.program.bytes();
            assert!(!bytes.is_empty());
        }
    }

    #[test]
    fn ranked_candidates_are_sorted_by_score() {
        // Construire un programme où plusieurs règles s'appliquent pour
        // tester le tri. Mais nos règles renvoient typiquement un
        // candidat par type d'optimisation. Testons sur un programme avec
        // Add(x, 0) ET Mul(y, 1).
        let p = Program::new(
            Target::Cpu,
            2,
            1,
            16,
            vec![
                Node::input(0),       // %0 = x
                Node::input(1),       // %1 = y
                Node::const_i64(0),   // %2 = 0
                Node::const_i64(1),   // %3 = 1
                Node::add(0, 2),      // %4 = x + 0
                Node::mul(1, 3),      // %5 = y * 1
                Node::add(4, 5),      // %6 = (x+0) + (y*1)
                Node::output(6, Ty::I64),
            ],
        )
        .unwrap();
        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        // Au moins 2 candidats (un par règle qui matche).
        assert!(candidates.len() >= 2);
        // Triés par score croissant.
        for i in 1..candidates.len() {
            assert!(candidates[i].score >= candidates[i - 1].score);
        }
    }

    #[test]
    fn agent_works_with_meta_term_embedding() {
        // Cross-cap Ω-7 ⊗ Ω-4.1 : l'output de l'agent s'embed dans
        // meta::Term. C'est le pont vers les preuves formelles.
        use crate::meta::embed_program;

        let agent = SymbolicAgent::new();
        let p = add_zero_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());

        for c in &candidates {
            let term = embed_program(&c.program);
            let h = term.hash();
            assert_ne!(h, [0u8; 32], "embedded term must have non-zero hash");
        }
    }

    // -----------------------------------------------------------------------
    // Tests Ω-7.0.1 — règles supplémentaires
    // -----------------------------------------------------------------------

    fn sub_x_x_program() -> Program {
        // f(x) = x - x → 0
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::sub(0, 0),
                Node::output(1, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn xor_x_x_program() -> Program {
        // f(x) = x ^ x → 0
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::bit_xor(0, 0),
                Node::output(1, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn and_x_x_program() -> Program {
        // f(x) = x & x → x
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::bit_and(0, 0),
                Node::output(1, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn or_x_x_program() -> Program {
        // f(x) = x | x → x
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::bit_or(0, 0),
                Node::output(1, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn assoc_add_program() -> Program {
        // f(a, b, c) = (a + b) + c — devrait pouvoir être réécrit a + (b + c)
        Program::new(
            Target::Cpu,
            3,
            1,
            8,
            vec![
                Node::input(0),     // %0 = a
                Node::input(1),     // %1 = b
                Node::input(2),     // %2 = c
                Node::add(0, 1),    // %3 = a + b
                Node::add(3, 2),    // %4 = (a + b) + c
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn distrib_with_zero_program() -> Program {
        // f(a, b) = a * (b + 0) — distrib donne a*b + a*0, et a*0 = 0,
        // puis canonicalize donne a*b. Strictement plus petit qu'avant.
        // Wait : a * (b + 0) canonicalise déjà en a * b via add_zero rule.
        // Pour rendre distrib visible, on évite l'add_zero sur le const.
        // Sub case : a * (b + c) où c est const(7). Distrib donne a*b + a*7.
        // Pas plus petit dans ce cas. Mais on teste juste que la règle ne
        // crashe pas et est sémantiquement correcte.
        Program::new(
            Target::Cpu,
            2,
            1,
            10,
            vec![
                Node::input(0),     // %0 = a
                Node::input(1),     // %1 = b
                Node::const_i64(7), // %2 = 7
                Node::add(1, 2),    // %3 = b + 7
                Node::mul(0, 3),    // %4 = a * (b + 7)
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn agent_finds_sub_x_x_zero() {
        let agent = SymbolicAgent::new();
        let p = sub_x_x_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty(), "agent doit trouver Sub(x,x)→0");
        // Sémantique préservée : pour tout x, output = 0.
        let best = &candidates[0];
        for x in [-100i64, 0, 1, 42, 1_000_000] {
            let bytes = x.to_le_bytes().to_vec();
            let out = crate::kasm::execute(&best.program, &bytes).unwrap();
            assert_eq!(i64::from_le_bytes(out[..8].try_into().unwrap()), 0);
        }
    }

    #[test]
    fn agent_finds_bit_xor_x_x_zero() {
        let agent = SymbolicAgent::new();
        let p = xor_x_x_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty(), "agent doit trouver Xor(x,x)→0");
        let best = &candidates[0];
        for x in [0i64, 1, -1, 0xff, 0xdeadbeef_i64] {
            let bytes = x.to_le_bytes().to_vec();
            let out = crate::kasm::execute(&best.program, &bytes).unwrap();
            assert_eq!(i64::from_le_bytes(out[..8].try_into().unwrap()), 0);
        }
    }

    #[test]
    fn agent_finds_bit_and_x_x_idempotent() {
        let agent = SymbolicAgent::new();
        let p = and_x_x_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty(), "agent doit trouver And(x,x)→x");
        let best = &candidates[0];
        for x in [0i64, 1, -1, 0xff, 42] {
            let bytes = x.to_le_bytes().to_vec();
            let out = crate::kasm::execute(&best.program, &bytes).unwrap();
            assert_eq!(i64::from_le_bytes(out[..8].try_into().unwrap()), x);
        }
    }

    #[test]
    fn agent_finds_bit_or_x_x_idempotent() {
        let agent = SymbolicAgent::new();
        let p = or_x_x_program();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty(), "agent doit trouver Or(x,x)→x");
        let best = &candidates[0];
        for x in [0i64, 1, -1, 0xff, 42] {
            let bytes = x.to_le_bytes().to_vec();
            let out = crate::kasm::execute(&best.program, &bytes).unwrap();
            assert_eq!(i64::from_le_bytes(out[..8].try_into().unwrap()), x);
        }
    }

    #[test]
    fn agent_associativity_add_preserves_semantics() {
        // L'associativité ne réduit pas la taille, donc le filtre score
        // peut la rejeter. Ce qui compte : si elle est PROPOSÉE, alors le
        // résultat doit être sémantiquement équivalent.
        let p = assoc_add_program();
        // Construit le candidat directement via la règle, sans le filtre.
        let candidate = rule_associativity_add(&p).expect("assoc rule must fire");
        // Sémantique : (a + b) + c == a + (b + c) sous wrapping.
        for (a, b, c) in [(1i64, 2, 3), (-5, 7, 11), (i64::MAX, 1, -2)] {
            let inputs: Vec<u8> = [a, b, c]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let out_orig = crate::kasm::execute(&p, &inputs).unwrap();
            let out_cand = crate::kasm::execute(&candidate, &inputs).unwrap();
            assert_eq!(out_orig, out_cand, "associativity must preserve semantics");
        }
    }

    #[test]
    fn agent_distributivity_left_preserves_semantics() {
        // distrib seul augmente la taille — vérifions juste sémantique.
        let p = distrib_with_zero_program();
        let candidate = rule_distributivity_left(&p)
            .expect("distrib rule must fire on Mul(a, Add(b, c))");
        for (a, b) in [(2i64, 3), (-5, 7), (10, -3), (0, 100)] {
            let inputs: Vec<u8> = [a, b].iter().flat_map(|v| v.to_le_bytes()).collect();
            let out_orig = crate::kasm::execute(&p, &inputs).unwrap();
            let out_cand = crate::kasm::execute(&candidate, &inputs).unwrap();
            assert_eq!(out_orig, out_cand, "distributivity must preserve semantics");
        }
    }

    #[test]
    fn rules_skip_when_program_has_reduce_op() {
        // Les règles assoc/distrib bail sur les programmes contenant Reduce*
        // (re-indexation casserait les ranges). On le vérifie explicitement.
        let p = Program::new(
            Target::Cpu,
            0,
            1,
            10,
            vec![
                Node::const_i64(1),
                Node::const_i64(2),
                Node::const_i64(3),
                Node::reduce_add(0, 3),
                Node::output(3, Ty::I64),
            ],
        )
        .unwrap();
        assert!(rule_associativity_add(&p).is_none());
        assert!(rule_distributivity_left(&p).is_none());
    }

    #[test]
    fn agent_handles_multiple_rules_simultaneously() {
        // Programme qui matche plusieurs règles à la fois : Sub(x, x) et
        // BitAnd(y, y). L'agent doit proposer >= 2 candidats.
        let p = Program::new(
            Target::Cpu,
            2,
            1,
            10,
            vec![
                Node::input(0),     // %0 = x
                Node::input(1),     // %1 = y
                Node::sub(0, 0),    // %2 = x - x = 0
                Node::bit_and(1, 1), // %3 = y & y = y
                Node::add(2, 3),    // %4 = 0 + y = y
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap();
        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        assert!(candidates.len() >= 2, "expected >= 2 candidates, got {}", candidates.len());
    }

    /// Audit 2026-05-01 — gap closed : agent rebuild paths used to
    /// remap the imm-as-3rd-ref slot only for `SelectI64 | ClampI64`,
    /// silently dropping `Op::Cond`'s `else_slot`. A program containing
    /// Op::Cond would survive the rebuild with a stale reference,
    /// producing a malformed Program (or worse, a Program that quietly
    /// computes the wrong result). This regression test executes a
    /// Cond-bearing program through the agent's `propose_rewrites`
    /// pipeline and verifies the candidate is still byte-valid and
    /// semantically equivalent on probe inputs.
    #[test]
    fn op_cond_third_ref_survives_agent_rebuild() {
        use crate::kasm::execute;
        // Program (8 nodes) : f(x) = if x == 0 { 7 } else { 11 } + (x * 1)
        // The `Mul(x, 1)` triggers the rule_mul_by_one rewrite, which
        // forces a rebuild via rebuild_with_substitution. If Op::Cond's
        // imm wasn't remapped during that rebuild, the resulting
        // Program would reference a wrong node.
        let p = Program::new(
            Target::Cpu,
            1,
            1,
            16,
            vec![
                Node::input(0),         // %0 : x
                Node::const_i64(0),     // %1 : 0
                Node::const_i64(7),     // %2 : 7  (cond then-branch)
                Node::const_i64(11),    // %3 : 11 (cond else-branch — IN IMM)
                Node::eq(0, 1),         // %4 : x == 0  (Bool)
                Node::cond(4, 2, 3),    // %5 : 3rd ref (else=3) lives in imm
                Node::const_i64(1),     // %6 : 1  (mul-by-one trigger)
                Node::mul(0, 6),        // %7 : x * 1  (will be rewritten)
                Node::add(5, 7),        // %8 : cond_value + (x * 1)
                Node::output(8, Ty::I64),
            ],
        )
        .unwrap();

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        assert!(
            !candidates.is_empty(),
            "agent should propose at least the mul-by-one rewrite"
        );

        for c in &candidates {
            // Bytes must still parse as a valid KASM Program.
            let bytes = c.program.bytes().to_vec();
            assert!(
                Program::from_bytes(&bytes).is_ok(),
                "candidate must be byte-valid after rebuild touching Op::Cond"
            );
            // Semantic equivalence on probe inputs : if Cond's third
            // ref was remapped to the wrong node, results would diverge.
            for x in [-3i64, -1, 0, 1, 5] {
                let original = execute(&p, &x.to_le_bytes()).unwrap();
                let rewritten = execute(&c.program, &x.to_le_bytes()).unwrap();
                assert_eq!(
                    original, rewritten,
                    "Op::Cond third-ref remap regression for x={x}: \
                     original={original:?} rewritten={rewritten:?}"
                );
            }
        }
    }
}

}

pub mod term_pattern {
//! Ω-7.0.2 — Pattern-match sur meta::Term.
//!
//! Permet d'exprimer des règles de réécriture sous forme de motifs
//! structurels sur l'AST CoC (`meta::Term`) plutôt que sur `kasm::Node`.
//!
//! Avantages :
//!  - Variables anonymes (`Hole`) qui matchent n'importe quel sous-terme.
//!  - Composition naturelle : un `Hole` peut apparaître plusieurs fois
//!    et doit alors lier la même valeur (consistance d'unification).
//!  - Pont vers les preuves Ω-4 : chaque pattern peut, à terme, être
//!    annoté d'une preuve d'équivalence sémantique.
//!
//! ## Périmètre du first mile
//!
//! `meta::Term` a **5 variantes** : `Var`, `Sort`, `Lam`, `Pi`, `App`.
//! `TermPattern` couvre les 5, plus la variante `Hole` (variable
//! d'unification). Les variantes liantes (`Lam`, `Pi`) sont matchées
//! structurellement (sans α-conversion supplémentaire) ce qui est
//! suffisant car `Term` utilise déjà des indices de de Bruijn — donc
//! l'α-équivalence est *structurelle*.
//!
//! ## Reportés
//!
//! * Ω-7.0.2.1 — pattern de second ordre (un `Hole` qui matche un
//!   contexte avec trous internes, pour exprimer des règles type
//!   `f x → g x` indépendamment de `x`).
//! * Ω-7.0.2.2 — conversion `Term → Program` (inverse de
//!   `embed_program`) pour transformer un match en réécriture KASM.

use std::collections::BTreeMap;

use crate::meta::Term;

/// Identifiant numérique d'un trou dans un pattern (variable d'unification).
pub type HoleId = u32;

/// Pattern structurel sur `Term`. Mirroir des variantes `Term` plus la
/// variante `Hole` qui matche n'importe quoi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermPattern {
    /// Trou : matche n'importe quel sous-terme. Si le même `HoleId`
    /// apparaît plusieurs fois dans le pattern, le matcher exige que
    /// les sous-termes matchés soient égaux (consistance d'unification).
    Hole(HoleId),
    /// Matche `Term::Var(i)` exactement.
    Var(u32),
    /// Matche `Term::Sort(level)` exactement.
    Sort(u32),
    /// Matche `Term::Lam { ty, body }`.
    Lam { ty: Box<TermPattern>, body: Box<TermPattern> },
    /// Matche `Term::Pi { ty, body }`.
    Pi { ty: Box<TermPattern>, body: Box<TermPattern> },
    /// Matche `Term::App(f, x)`.
    App(Box<TermPattern>, Box<TermPattern>),
}

impl TermPattern {
    /// Constructeur ergonomique pour `Hole`.
    pub fn hole(id: HoleId) -> Self {
        TermPattern::Hole(id)
    }
    /// Constructeur ergonomique pour `Var`.
    pub fn var(i: u32) -> Self {
        TermPattern::Var(i)
    }
    /// Constructeur ergonomique pour `Sort`.
    pub fn sort(level: u32) -> Self {
        TermPattern::Sort(level)
    }
    /// Constructeur ergonomique pour `Lam`.
    pub fn lam(ty: TermPattern, body: TermPattern) -> Self {
        TermPattern::Lam { ty: Box::new(ty), body: Box::new(body) }
    }
    /// Constructeur ergonomique pour `Pi`.
    pub fn pi(ty: TermPattern, body: TermPattern) -> Self {
        TermPattern::Pi { ty: Box::new(ty), body: Box::new(body) }
    }
    /// Constructeur ergonomique pour `App`.
    pub fn app(f: TermPattern, x: TermPattern) -> Self {
        TermPattern::App(Box::new(f), Box::new(x))
    }
}

/// Bindings : map d'un `HoleId` vers le `Term` qu'il a unifié.
pub type Bindings = BTreeMap<HoleId, Term>;

/// Tente de matcher `pattern` contre `term`. Retourne `Some(bindings)`
/// si le pattern matche, `None` sinon. La cohérence d'unification est
/// vérifiée : si le même `HoleId` est utilisé plusieurs fois, les
/// bindings doivent concorder.
pub fn match_pattern(pattern: &TermPattern, term: &Term) -> Option<Bindings> {
    let mut bindings = Bindings::new();
    if match_recursive(pattern, term, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn match_recursive(
    pattern: &TermPattern,
    term: &Term,
    bindings: &mut Bindings,
) -> bool {
    match pattern {
        TermPattern::Hole(id) => {
            if let Some(existing) = bindings.get(id) {
                existing == term
            } else {
                bindings.insert(*id, term.clone());
                true
            }
        }
        TermPattern::Var(i) => matches!(term, Term::Var(j) if i == j),
        TermPattern::Sort(level) => matches!(term, Term::Sort(l) if level == l),
        TermPattern::Lam { ty: p_ty, body: p_body } => match term {
            Term::Lam { ty: t_ty, body: t_body } => {
                match_recursive(p_ty, t_ty, bindings)
                    && match_recursive(p_body, t_body, bindings)
            }
            _ => false,
        },
        TermPattern::Pi { ty: p_ty, body: p_body } => match term {
            Term::Pi { ty: t_ty, body: t_body } => {
                match_recursive(p_ty, t_ty, bindings)
                    && match_recursive(p_body, t_body, bindings)
            }
            _ => false,
        },
        TermPattern::App(p_f, p_x) => match term {
            Term::App(t_f, t_x) => {
                match_recursive(p_f, t_f, bindings)
                    && match_recursive(p_x, t_x, bindings)
            }
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// Ω-7.0.2.1 — Patterns de second ordre
// ---------------------------------------------------------------------------

/// Construit une variante "HoleApp" qui matche `Hole(id)` appliqué à des
/// arguments. Exemple : pour matcher `f(x, y)` quel que soit `f`, on utilise
/// `TermPattern::hole_app(0, vec![pat_x, pat_y])`.
///
/// Implémenté en réécrivant en chaîne d'`App` : `App(App(f, x), y)` etc.
impl TermPattern {
    pub fn hole_app(id: HoleId, args: Vec<TermPattern>) -> TermPattern {
        let mut acc = TermPattern::Hole(id);
        for arg in args {
            acc = TermPattern::App(Box::new(acc), Box::new(arg));
        }
        acc
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    #[test]
    fn hole_matches_any_term() {
        let p = affine_program();
        let term = embed_program(&p);
        let pattern = TermPattern::hole(0);
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "Hole doit matcher n'importe quel terme");
        let bindings = m.unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.get(&0), Some(&term));
    }

    #[test]
    fn var_pattern_matches_var_exact() {
        let term = Term::var(3);
        assert!(match_pattern(&TermPattern::var(3), &term).is_some());
        assert!(match_pattern(&TermPattern::var(4), &term).is_none());
        // Var pattern ne matche pas une autre variante.
        assert!(match_pattern(&TermPattern::var(3), &Term::sort(3)).is_none());
    }

    #[test]
    fn sort_pattern_matches_sort_exact() {
        let term = Term::sort(0);
        assert!(match_pattern(&TermPattern::sort(0), &term).is_some());
        assert!(match_pattern(&TermPattern::sort(1), &term).is_none());
    }

    #[test]
    fn hole_with_same_id_must_bind_consistently() {
        // Pattern : App(Hole(0), Hole(0)).
        // Sur App(Var(1), Var(1)) → match (les deux holes lient Var(1)).
        // Sur App(Var(1), Var(2)) → fail (incohérence).
        let pattern = TermPattern::app(TermPattern::hole(0), TermPattern::hole(0));

        let consistent = Term::app(Term::var(1), Term::var(1));
        let m = match_pattern(&pattern, &consistent);
        assert!(m.is_some(), "Hole(0) lie deux fois Var(1) — doit matcher");
        let b = m.unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b.get(&0), Some(&Term::var(1)));

        let inconsistent = Term::app(Term::var(1), Term::var(2));
        assert!(
            match_pattern(&pattern, &inconsistent).is_none(),
            "Hole(0) lie Var(1) puis Var(2) — incohérence, doit fail"
        );
    }

    #[test]
    fn distinct_hole_ids_bind_independently() {
        let pattern = TermPattern::app(TermPattern::hole(0), TermPattern::hole(1));
        let term = Term::app(Term::var(7), Term::sort(2));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some());
        let b = m.unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b.get(&0), Some(&Term::var(7)));
        assert_eq!(b.get(&1), Some(&Term::sort(2)));
    }

    #[test]
    fn lam_pattern_matches_lam_recursively() {
        // λ(_: Type). Var(0) — pattern avec un trou pour le body.
        let pattern = TermPattern::lam(TermPattern::sort(0), TermPattern::hole(9));
        let term = Term::lam(Term::sort(0), Term::var(0));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some());
        let b = m.unwrap();
        assert_eq!(b.get(&9), Some(&Term::var(0)));

        // Un Pi ne doit pas matcher un Lam pattern.
        let pi_term = Term::pi(Term::sort(0), Term::var(0));
        assert!(match_pattern(&pattern, &pi_term).is_none());
    }

    #[test]
    fn pi_pattern_distinct_from_lam() {
        let pattern = TermPattern::pi(TermPattern::hole(0), TermPattern::hole(1));
        let pi_term = Term::pi(Term::sort(0), Term::var(0));
        assert!(match_pattern(&pattern, &pi_term).is_some());

        let lam_term = Term::lam(Term::sort(0), Term::var(0));
        assert!(match_pattern(&pattern, &lam_term).is_none());
    }

    #[test]
    fn deep_nested_pattern_matches() {
        // Pattern : App(App(Hole(0), Hole(1)), Hole(2)) — un App à 3 niveaux.
        let pattern = TermPattern::app(
            TermPattern::app(TermPattern::hole(0), TermPattern::hole(1)),
            TermPattern::hole(2),
        );
        let inner = Term::app(Term::sort(100), Term::var(0));
        let term = Term::app(inner.clone(), Term::sort(200));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some());
        let b = m.unwrap();
        assert_eq!(b.get(&0), Some(&Term::sort(100)));
        assert_eq!(b.get(&1), Some(&Term::var(0)));
        assert_eq!(b.get(&2), Some(&Term::sort(200)));
    }

    #[test]
    fn pattern_match_on_embedded_program_finds_substructure() {
        // L'embedding d'un programme produit un Term en App-chain :
        //   App(App(...App(Sort(STRUCT_PROGRAM), tgt)...), node_n).
        // La racine est donc un App. On vérifie qu'un pattern App générique
        // matche, et qu'on peut extraire le LHS et le RHS via deux Holes.
        let p = affine_program();
        let term = embed_program(&p);

        let pattern = TermPattern::app(TermPattern::hole(0), TermPattern::hole(1));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "embedded program a forme App(_,_) à la racine");
        let b = m.unwrap();
        assert_eq!(b.len(), 2);

        // Le RHS de la racine est l'embedding du dernier node (Output).
        // On vérifie au moins que le Hole(1) est bien lié à un sous-terme
        // de l'embedding.
        let rhs = b.get(&1).expect("Hole(1) doit être lié");
        // Sanity : le hash du Hole(1) doit différer de celui du term complet.
        assert_ne!(rhs.hash(), term.hash());
    }

    #[test]
    fn no_match_fails_cleanly() {
        // Pattern Sort(5) sur un App → None, pas de panic.
        let pattern = TermPattern::sort(5);
        let term = Term::app(Term::var(0), Term::var(1));
        assert!(match_pattern(&pattern, &term).is_none());
    }

    // -----------------------------------------------------------------
    // Ω-7.0.2.1 — patterns de second ordre (hole_app)
    // -----------------------------------------------------------------

    #[test]
    fn hole_app_matches_function_application() {
        // hole_app(0, [Hole(1)]) matche App(f, x) et lie 0 → f, 1 → x.
        let pattern = TermPattern::hole_app(0, vec![TermPattern::hole(1)]);
        let f = Term::var(42);
        let x = Term::sort(7);
        let term = Term::app(f.clone(), x.clone());
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "hole_app à un argument doit matcher App(f, x)");
        let b = m.unwrap();
        assert_eq!(b.get(&0), Some(&f));
        assert_eq!(b.get(&1), Some(&x));
    }

    #[test]
    fn hole_app_two_args_matches_curried_app() {
        // hole_app(0, [Hole(1), Hole(2)]) matche App(App(f, x), y).
        let pattern = TermPattern::hole_app(
            0,
            vec![TermPattern::hole(1), TermPattern::hole(2)],
        );
        let f = Term::var(1);
        let x = Term::var(2);
        let y = Term::var(3);
        let term = Term::app(Term::app(f.clone(), x.clone()), y.clone());
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "hole_app à deux arguments doit matcher App-curried");
        let b = m.unwrap();
        assert_eq!(b.get(&0), Some(&f));
        assert_eq!(b.get(&1), Some(&x));
        assert_eq!(b.get(&2), Some(&y));
    }

    #[test]
    fn hole_app_fails_on_non_app() {
        // hole_app(0, [Hole(1)]) ne doit pas matcher Var(_) ou Sort(_).
        let pattern = TermPattern::hole_app(0, vec![TermPattern::hole(1)]);
        assert!(match_pattern(&pattern, &Term::var(5)).is_none());
        assert!(match_pattern(&pattern, &Term::sort(0)).is_none());
        // Cas zéro arg : hole_app(0, []) = Hole(0), matche tout.
        let zero_args = TermPattern::hole_app(0, vec![]);
        assert!(match_pattern(&zero_args, &Term::var(5)).is_some());
    }
}

}

pub mod term_to_program {
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

}

pub mod corpus {
//! Ω-7.2 first-mile-honnête — Corpus SCAN-natif de programmes KASM.
//!
//! ## Doctrine
//!
//! La promesse Ω-7.2 historique ("corpus MLIR : Linux kernel + mathlib +
//! top-100k Rust") nécessite Polygeist + Lean4 + rustc-MLIR backend.
//! Doctrinairement impossible (3 toolchains externes).
//!
//! Réinterprétation honnête : générer un corpus DÉTERMINISTE de N programmes
//! KASM via fuzz, avec leurs émissions MLIR text et leurs embeddings Term.
//! Le corpus est SCAN-natif (pas un import externe), reproductible (seed
//! fixe), et utilisable par BanditAgent pour entraînement.
//!
//! Ce N'EST PAS le corpus Linux/mathlib. Document pour ce qu'il est : un
//! corpus first-mile auto-généré, pierre angulaire pour Ω-7.2.x étendu.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::kasm::{Node, Program, Target, Ty, MAX_NODES};

/// Une entrée du corpus : un Program + son MLIR-text + son embedding Term.
#[derive(Debug, Clone)]
pub struct CorpusEntry {
    pub program: Program,
    pub mlir_text: String,
    pub term_hash: [u8; 32],
}

/// Corpus = vec d'entrées + meta. Reproductible via le seed.
#[derive(Debug, Clone)]
pub struct Corpus {
    pub seed: u64,
    pub entries: Vec<CorpusEntry>,
}

impl Corpus {
    /// Génère un corpus de `n` programmes à partir de `seed`. Pour chaque
    /// programme : random_program → emit_mlir → embed_program.hash.
    pub fn generate(seed: u64, n: usize) -> Self {
        let mut rng = XorshiftRng::new(seed);
        let mut entries = Vec::with_capacity(n);
        for i in 0..n {
            // Taille cible pseudo-uniforme dans [4, 64].
            let target_size = 4 + (rng.next_u64() as usize) % 60;
            let n_inputs = 1 + (rng.next_u64() as usize) % 4;
            let entry_seed = seed
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .wrapping_add(i as u64);
            let p = generate_random_program(entry_seed, target_size, n_inputs as u8);
            let mlir = p.canonical_mlir_text().unwrap_or_default();
            let term = crate::meta::embed_program(&p);
            let term_hash = term.hash();
            entries.push(CorpusEntry { program: p, mlir_text: mlir, term_hash });
        }
        Self { seed, entries }
    }

    /// Nombre d'entrées.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// xorshift 64-bit (no external dep).
struct XorshiftRng(u64);
impl XorshiftRng {
    fn new(seed: u64) -> Self {
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
}

/// Génère un Program KASM aléatoire valide. Forward refs only, types corrects.
fn generate_random_program(seed: u64, target_size: usize, n_inputs: u8) -> Program {
    let mut rng = XorshiftRng::new(seed);
    let target = target_size.clamp(4, MAX_NODES);
    let inputs = n_inputs.clamp(1, 16);

    let mut nodes: Vec<Node> = Vec::with_capacity(target);
    let mut i64_idx: Vec<u16> = Vec::new();

    for slot in 0..inputs {
        nodes.push(Node::input(slot));
        i64_idx.push(nodes.len() as u16 - 1);
    }
    nodes.push(Node::const_i64((rng.next_u64() as i16) % 100));
    i64_idx.push(nodes.len() as u16 - 1);

    while nodes.len() < target.saturating_sub(1) {
        let kind = rng.next_u64() % 9;
        let n = match kind {
            0 => Node::const_i64((rng.next_u64() as i16) % 200 - 100),
            1 => Node::add(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            2 => Node::sub(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            3 => Node::mul(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            4 => Node::min(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            5 => Node::max(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            6 => Node::bit_and(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            7 => Node::bit_or(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            _ => Node::bit_xor(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
        };
        let idx = nodes.len() as u16;
        nodes.push(n);
        // Tous les ops sélectionnés produisent I64.
        i64_idx.push(idx);
    }
    let last = *i64_idx.last().expect("at least 1 i64");
    nodes.push(Node::output(last, Ty::I64));
    let total = nodes.len() as u32;
    Program::new(Target::Cpu, inputs, 1, total, nodes).expect("valid by construction")
}

fn pick(slice: &[u16], rng: &mut XorshiftRng) -> u16 {
    slice[(rng.next_u64() as usize) % slice.len()]
}

// =============================================================================
// η — Lifting de fonctions système (kraken-η.0)
// =============================================================================
//
// Pipe la sortie d'un disassembleur local (dumpbin Windows, objdump Linux,
// otool macOS — Intel syntax requis) et lifte les fonctions arithmétiques
// pures straight-line en programmes KASM. Lazy detection, no-op si l'outil
// est absent. Allow-list stricte d'opcodes : mov, add, sub, imul, and, or,
// xor, shl, shr, neg, push/pop (no-op stack frame), ret. Toute fonction
// avec branche, syscall, mémoire indirecte ou registre xmm/ymm est skippée
// entière. Le lift est SSA : chaque GPR mappe à un index Node KASM courant.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisasmTool {
    Dumpbin,
    Objdump,
    Otool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Reg {
    Rax, Rbx, Rcx, Rdx, Rsi, Rdi, Rbp, Rsp,
    R8, R9, R10, R11, R12, R13, R14, R15,
}

impl Reg {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('%').to_ascii_lowercase();
        Some(match s.as_str() {
            "rax" => Reg::Rax, "rbx" => Reg::Rbx, "rcx" => Reg::Rcx, "rdx" => Reg::Rdx,
            "rsi" => Reg::Rsi, "rdi" => Reg::Rdi, "rbp" => Reg::Rbp, "rsp" => Reg::Rsp,
            "r8" => Reg::R8, "r9" => Reg::R9, "r10" => Reg::R10, "r11" => Reg::R11,
            "r12" => Reg::R12, "r13" => Reg::R13, "r14" => Reg::R14, "r15" => Reg::R15,
            _ => return None,
        })
    }

    fn is_stack(self) -> bool {
        matches!(self, Reg::Rsp | Reg::Rbp)
    }
}

#[derive(Debug, Clone, Copy)]
enum Operand {
    Reg(Reg),
    Imm(i64),
}

impl Operand {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(r) = Reg::parse(s) {
            return Some(Operand::Reg(r));
        }
        // Immédiat décimal ou hex (avec/sans préfixe 0x, suffixe h).
        let raw = s.trim_start_matches('$').trim_start_matches('#');
        let (body, radix) = if let Some(stripped) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
            (stripped, 16)
        } else if let Some(stripped) = raw.strip_suffix('h').or_else(|| raw.strip_suffix('H')) {
            (stripped, 16)
        } else {
            (raw, 10)
        };
        // Gestion du signe pour décimal.
        if radix == 10 {
            if let Ok(v) = body.parse::<i64>() {
                return Some(Operand::Imm(v));
            }
        } else if let Ok(v) = i64::from_str_radix(body.trim_start_matches('-'), radix) {
            let signed = if body.starts_with('-') { -(v as i64) } else { v };
            return Some(Operand::Imm(signed));
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mnem {
    Mov, Add, Sub, Imul, And, Or, Xor, Shl, Shr, Neg, Push, Pop, Ret,
}

impl Mnem {
    fn parse(s: &str) -> Option<Self> {
        // Intel syntax requis (objdump invoqué avec -M intel ; dumpbin Intel
        // par défaut). Les suffixes AT&T (movq, addl) ne sont pas tolérés
        // pour éviter les ambiguïtés type `imul`/`imul`+suffixe.
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "mov" => Mnem::Mov,
            "add" => Mnem::Add,
            "sub" => Mnem::Sub,
            "imul" => Mnem::Imul,
            "and" => Mnem::And,
            "or" => Mnem::Or,
            "xor" => Mnem::Xor,
            "shl" | "sal" => Mnem::Shl,
            "shr" => Mnem::Shr,
            "neg" => Mnem::Neg,
            "push" => Mnem::Push,
            "pop" => Mnem::Pop,
            "ret" => Mnem::Ret,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
struct Instr {
    mnem: Mnem,
    dst: Option<Operand>,
    src: Option<Operand>,
}

#[derive(Debug, Clone)]
struct RawFunction {
    instrs: Vec<Instr>,
    /// `true` si une ligne non-parsable ou un opcode hors allow-list a été
    /// rencontré dans cette fonction → fonction entière à skip.
    poisoned: bool,
}

/// Detection lazy de l'outil de désassemblage local. Cache le résultat dans
/// un OnceLock pour éviter les `Command::new` répétés.
pub fn detect_disasm_tool() -> Option<DisasmTool> {
    static CACHED: OnceLock<Option<DisasmTool>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        // Ordre de préférence : outil natif de la plateforme courante d'abord.
        if cfg!(windows) {
            if probe_tool("dumpbin", &["/?"]) {
                return Some(DisasmTool::Dumpbin);
            }
            if probe_tool("objdump", &["--version"]) {
                return Some(DisasmTool::Objdump);
            }
        } else if cfg!(target_os = "macos") {
            if probe_tool("otool", &["--version"]) {
                return Some(DisasmTool::Otool);
            }
            if probe_tool("objdump", &["--version"]) {
                return Some(DisasmTool::Objdump);
            }
        } else if probe_tool("objdump", &["--version"]) {
            return Some(DisasmTool::Objdump);
        }
        None
    })
}

fn probe_tool(name: &str, args: &[&str]) -> bool {
    Command::new(name)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_disasm(tool: DisasmTool, path: &Path) -> Option<String> {
    let output = match tool {
        DisasmTool::Dumpbin => Command::new("dumpbin")
            .arg("/DISASM:NOBYTES")
            .arg(path)
            .output()
            .ok()?,
        DisasmTool::Objdump => Command::new("objdump")
            .args(["-d", "-M", "intel", "--no-show-raw-insn"])
            .arg(path)
            .output()
            .ok()?,
        DisasmTool::Otool => Command::new("otool")
            .args(["-tv"])
            .arg(path)
            .output()
            .ok()?,
    };
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Convention d'appel : ordre des registres argument selon la plateforme
/// déduite depuis l'outil. Windows MS x64 : RCX, RDX, R8, R9. SysV (Linux/
/// macOS) : RDI, RSI, RDX, RCX, R8, R9.
fn call_conv_args(tool: DisasmTool) -> &'static [Reg] {
    match tool {
        DisasmTool::Dumpbin => &[Reg::Rcx, Reg::Rdx, Reg::R8, Reg::R9],
        DisasmTool::Objdump | DisasmTool::Otool => {
            &[Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9]
        }
    }
}

/// Découpe la sortie en fonctions. Une fonction commence à un label de la
/// forme `<name>:` (objdump), `name:` seul sur sa ligne (dumpbin) ou
/// `_name:` (otool). Les lignes d'instructions sont toutes les lignes après
/// le header jusqu'au prochain label ou EOF.
fn parse_disasm_output(text: &str, tool: DisasmTool) -> Vec<RawFunction> {
    let mut out: Vec<RawFunction> = Vec::new();
    let mut current: Option<RawFunction> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_function_label(trimmed, tool) {
            if let Some(f) = current.take() {
                if !f.instrs.is_empty() && !f.poisoned {
                    out.push(f);
                }
            }
            current = Some(RawFunction { instrs: Vec::new(), poisoned: false });
            continue;
        }
        let Some(func) = current.as_mut() else { continue };
        if func.poisoned {
            continue;
        }
        if let Some(instr_line) = strip_address_prefix(trimmed) {
            match parse_instr_line(instr_line) {
                ParseLine::Instr(i) => func.instrs.push(i),
                ParseLine::Skip => {}
                ParseLine::Poison => func.poisoned = true,
            }
        }
    }
    if let Some(f) = current {
        if !f.instrs.is_empty() && !f.poisoned {
            out.push(f);
        }
    }
    out
}

fn is_function_label(line: &str, _tool: DisasmTool) -> bool {
    // objdump : "0000000000401000 <my_func>:"
    if let Some(open) = line.find('<') {
        if let Some(close) = line[open..].find('>') {
            let name = &line[open + 1..open + close];
            if line.trim_end().ends_with(':') && !name.is_empty() {
                return true;
            }
        }
    }
    // dumpbin : "my_func:" en début de ligne
    // otool : "_my_func:"
    if line.ends_with(':') && !line.contains(' ') {
        let name = line.trim_end_matches(':');
        if !name.is_empty() && name.chars().next().map(is_label_start).unwrap_or(false) {
            // Filtrer les labels d'adresses purs ("00401000:") qu'on ne veut pas.
            if !name.chars().all(|c| c.is_ascii_hexdigit()) {
                return true;
            }
        }
    }
    false
}

fn is_label_start(c: char) -> bool {
    c == '_' || c == '.' || c == '$' || c.is_ascii_alphabetic()
}

/// Strip un préfixe d'adresse type `00401000:` ou `0000000180001000:`.
fn strip_address_prefix(line: &str) -> Option<&str> {
    let mut chars = line.char_indices();
    // Skip hex digits.
    let mut last_hex = 0;
    let mut saw_hex = false;
    for (i, c) in chars.by_ref() {
        if c.is_ascii_hexdigit() {
            last_hex = i + c.len_utf8();
            saw_hex = true;
        } else {
            if saw_hex && c == ':' {
                let rest = &line[last_hex + 1..];
                return Some(rest.trim());
            }
            // Pas de préfixe d'adresse : retourner la ligne telle quelle.
            return Some(line);
        }
    }
    if saw_hex {
        Some(&line[last_hex..])
    } else {
        Some(line)
    }
}

enum ParseLine {
    Instr(Instr),
    Skip,
    Poison,
}

fn parse_instr_line(line: &str) -> ParseLine {
    // Strip commentaire éventuel après `;` ou `#`.
    let line = match line.find(|c| c == ';' || c == '#') {
        Some(i) => &line[..i],
        None => line,
    };
    let line = line.trim();
    if line.is_empty() {
        return ParseLine::Skip;
    }
    // Tokenizer : mnemonic = premier token, le reste = operandes séparées par `,`.
    let mut parts = line.splitn(2, char::is_whitespace);
    let mnem_str = parts.next().unwrap_or("");
    let ops_str = parts.next().unwrap_or("").trim();
    let Some(mnem) = Mnem::parse(mnem_str) else {
        return ParseLine::Poison;
    };
    if mnem == Mnem::Ret {
        return ParseLine::Instr(Instr { mnem, dst: None, src: None });
    }
    if matches!(mnem, Mnem::Push | Mnem::Pop | Mnem::Neg) {
        let Some(dst) = Operand::parse(ops_str) else {
            return ParseLine::Poison;
        };
        return ParseLine::Instr(Instr { mnem, dst: Some(dst), src: None });
    }
    // Deux opérandes attendus.
    let mut ops = ops_str.splitn(2, ',');
    let dst_s = ops.next().unwrap_or("").trim();
    let src_s = ops.next().unwrap_or("").trim();
    if dst_s.is_empty() || src_s.is_empty() {
        return ParseLine::Poison;
    }
    // Référence mémoire `[...]` non supportée → poison.
    if dst_s.contains('[') || src_s.contains('[') {
        return ParseLine::Poison;
    }
    let Some(dst) = Operand::parse(dst_s) else {
        return ParseLine::Poison;
    };
    let Some(src) = Operand::parse(src_s) else {
        return ParseLine::Poison;
    };
    ParseLine::Instr(Instr { mnem, dst: Some(dst), src: Some(src) })
}

/// Lifte une fonction straight-line en Program KASM. Renvoie None si elle
/// dépasse les contraintes (taille, opcodes, capacités KASM).
fn lift_function(func: &RawFunction, args_order: &[Reg]) -> Option<Program> {
    if func.instrs.is_empty() || func.instrs.len() > 50 {
        return None;
    }
    if !matches!(func.instrs.last()?.mnem, Mnem::Ret) {
        return None;
    }

    // Phase 1 : déterminer les registres argument lus avant écriture (= inputs).
    let mut written: [bool; 16] = [false; 16];
    let mut read_first: [bool; 16] = [false; 16];
    for instr in &func.instrs {
        let (reads, writes) = instr_reg_uses(instr);
        for r in reads {
            let idx = reg_index(r);
            if !written[idx] {
                read_first[idx] = true;
            }
        }
        for r in writes {
            written[reg_index(r)] = true;
        }
    }
    // Sélectionne les regs argument utilisés, dans l'ordre de la convention.
    let inputs: Vec<Reg> = args_order
        .iter()
        .copied()
        .filter(|r| read_first[reg_index(*r)])
        .collect();
    if inputs.is_empty() {
        return None;
    }

    // Phase 2 : émission SSA.
    let mut nodes: Vec<Node> = Vec::with_capacity(func.instrs.len() * 2 + 4);
    let mut reg_idx: [Option<u16>; 16] = [None; 16];
    for (slot, &reg) in inputs.iter().enumerate() {
        nodes.push(Node::input(slot as u8));
        reg_idx[reg_index(reg)] = Some((nodes.len() - 1) as u16);
    }

    for instr in &func.instrs {
        if !lift_instr(instr, &mut nodes, &mut reg_idx) {
            return None;
        }
        if nodes.len() > MAX_NODES.saturating_sub(2) {
            return None;
        }
    }

    let rax_final = reg_idx[reg_index(Reg::Rax)]?;
    nodes.push(Node::output(rax_final, Ty::I64));
    let total = nodes.len() as u32;
    Program::new(Target::Cpu, inputs.len() as u8, 1, total, nodes).ok()
}

fn reg_index(r: Reg) -> usize {
    r as usize
}

fn instr_reg_uses(instr: &Instr) -> (Vec<Reg>, Vec<Reg>) {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    match instr.mnem {
        Mnem::Ret => {}
        Mnem::Push => {
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
            }
        }
        Mnem::Pop => {
            if let Some(Operand::Reg(r)) = instr.dst {
                writes.push(r);
            }
        }
        Mnem::Neg => {
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
                writes.push(r);
            }
        }
        Mnem::Mov => {
            if let Some(Operand::Reg(r)) = instr.dst {
                writes.push(r);
            }
            if let Some(Operand::Reg(r)) = instr.src {
                reads.push(r);
            }
        }
        Mnem::Xor => {
            if let (Some(Operand::Reg(d)), Some(Operand::Reg(s))) = (instr.dst, instr.src) {
                if d == s {
                    writes.push(d);
                    return (reads, writes);
                }
            }
            // Fallthrough → comme Add.
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
                writes.push(r);
            }
            if let Some(Operand::Reg(r)) = instr.src {
                reads.push(r);
            }
        }
        Mnem::Add | Mnem::Sub | Mnem::Imul | Mnem::And | Mnem::Or | Mnem::Shl | Mnem::Shr => {
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
                writes.push(r);
            }
            if let Some(Operand::Reg(r)) = instr.src {
                reads.push(r);
            }
        }
    }
    (reads, writes)
}

/// Effectue le lift d'une instruction. Renvoie false si elle ne peut pas
/// être liftée (constante hors i16, opcode mémoire, etc.).
fn lift_instr(instr: &Instr, nodes: &mut Vec<Node>, reg_idx: &mut [Option<u16>; 16]) -> bool {
    match instr.mnem {
        Mnem::Ret => true,
        Mnem::Push => true, // no-op pour SSA arithmétique
        Mnem::Pop => {
            // Sur RBP/RSP, no-op. Sinon, le tracking devient incertain (la
            // valeur poppée vient de la pile, qu'on ne modélise pas) → fail.
            if let Some(Operand::Reg(r)) = instr.dst {
                if r.is_stack() {
                    return true;
                }
                // Désync : invalide le tracking de ce registre.
                reg_idx[reg_index(r)] = None;
                return true;
            }
            false
        }
        Mnem::Neg => {
            let Some(Operand::Reg(r)) = instr.dst else { return false };
            if r.is_stack() {
                return true;
            }
            let Some(src_idx) = reg_idx[reg_index(r)] else { return false };
            nodes.push(Node::neg(src_idx));
            reg_idx[reg_index(r)] = Some((nodes.len() - 1) as u16);
            true
        }
        Mnem::Mov => {
            let (Some(Operand::Reg(dst)), Some(src)) = (instr.dst, instr.src) else { return false };
            if dst.is_stack() {
                return true;
            }
            match src {
                Operand::Reg(s) => {
                    if s.is_stack() {
                        // mov r, rsp/rbp : on ne modélise pas la pile → invalide r.
                        reg_idx[reg_index(dst)] = None;
                        return true;
                    }
                    let Some(idx) = reg_idx[reg_index(s)] else { return false };
                    reg_idx[reg_index(dst)] = Some(idx);
                    true
                }
                Operand::Imm(v) => {
                    let Some(idx) = push_const(nodes, v) else { return false };
                    reg_idx[reg_index(dst)] = Some(idx);
                    true
                }
            }
        }
        Mnem::Xor => {
            let (Some(Operand::Reg(dst)), Some(src)) = (instr.dst, instr.src) else { return false };
            if dst.is_stack() {
                return true;
            }
            // xor r, r → const_i64(0)
            if let Operand::Reg(s) = src {
                if s == dst {
                    let Some(idx) = push_const(nodes, 0) else { return false };
                    reg_idx[reg_index(dst)] = Some(idx);
                    return true;
                }
            }
            lift_binop(dst, src, nodes, reg_idx, Node::bit_xor)
        }
        Mnem::Add => bin(instr, nodes, reg_idx, Node::add),
        Mnem::Sub => bin(instr, nodes, reg_idx, Node::sub),
        Mnem::Imul => bin(instr, nodes, reg_idx, Node::mul),
        Mnem::And => bin(instr, nodes, reg_idx, Node::bit_and),
        Mnem::Or => bin(instr, nodes, reg_idx, Node::bit_or),
        Mnem::Shl => bin(instr, nodes, reg_idx, Node::shl),
        Mnem::Shr => bin(instr, nodes, reg_idx, Node::shr),
    }
}

fn bin(
    instr: &Instr,
    nodes: &mut Vec<Node>,
    reg_idx: &mut [Option<u16>; 16],
    op: fn(u16, u16) -> Node,
) -> bool {
    let (Some(Operand::Reg(dst)), Some(src)) = (instr.dst, instr.src) else { return false };
    if dst.is_stack() {
        return true;
    }
    lift_binop(dst, src, nodes, reg_idx, op)
}

fn lift_binop(
    dst: Reg,
    src: Operand,
    nodes: &mut Vec<Node>,
    reg_idx: &mut [Option<u16>; 16],
    op: fn(u16, u16) -> Node,
) -> bool {
    let Some(a_idx) = reg_idx[reg_index(dst)] else { return false };
    let b_idx = match src {
        Operand::Reg(s) => {
            if s.is_stack() {
                return false;
            }
            let Some(i) = reg_idx[reg_index(s)] else { return false };
            i
        }
        Operand::Imm(v) => match push_const(nodes, v) {
            Some(i) => i,
            None => return false,
        },
    };
    nodes.push(op(a_idx, b_idx));
    reg_idx[reg_index(dst)] = Some((nodes.len() - 1) as u16);
    true
}

fn push_const(nodes: &mut Vec<Node>, v: i64) -> Option<u16> {
    if v < i16::MIN as i64 || v > i16::MAX as i64 {
        return None;
    }
    nodes.push(Node::const_i64(v as i16));
    Some((nodes.len() - 1) as u16)
}

impl Corpus {
    /// Lifte les fonctions arithmétiques pures d'un binaire système via
    /// l'outil de désassemblage local. Renvoie un Vec vide (sans erreur) si
    /// l'outil est absent ou si la commande échoue. Conservateur : seules
    /// les fonctions straight-line, sans branches/syscalls/mémoire/SSE,
    /// sont liftées. Cible Ω-7.2 corpus système sans Polygeist.
    pub fn lift_system_binary(path: &Path) -> Vec<CorpusEntry> {
        let Some(tool) = detect_disasm_tool() else { return Vec::new() };
        let Some(text) = run_disasm(tool, path) else { return Vec::new() };
        Self::lift_disasm_text(&text, tool)
    }

    /// Variante testable sans dépendance sur un outil installé : prend le
    /// texte de désassemblage en entrée et retourne les fonctions liftées.
    pub fn lift_disasm_text(text: &str, tool: DisasmTool) -> Vec<CorpusEntry> {
        let raws = parse_disasm_output(text, tool);
        let args = call_conv_args(tool);
        raws.iter()
            .filter_map(|raw| {
                let p = lift_function(raw, args)?;
                let mlir = p.canonical_mlir_text().unwrap_or_default();
                let term_hash = crate::meta::embed_program(&p).hash();
                Some(CorpusEntry { program: p, mlir_text: mlir, term_hash })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_generate_n_entries() {
        let c = Corpus::generate(42, 16);
        assert_eq!(c.len(), 16);
        assert_eq!(c.seed, 42);
    }

    #[test]
    fn corpus_is_deterministic_under_same_seed() {
        let c1 = Corpus::generate(42, 8);
        let c2 = Corpus::generate(42, 8);
        for (e1, e2) in c1.entries.iter().zip(c2.entries.iter()) {
            assert_eq!(e1.program.bytes(), e2.program.bytes());
            assert_eq!(e1.mlir_text, e2.mlir_text);
            assert_eq!(e1.term_hash, e2.term_hash);
        }
    }

    #[test]
    fn corpus_distinct_seeds_produce_distinct_entries() {
        let c1 = Corpus::generate(1, 4);
        let c2 = Corpus::generate(2, 4);
        // Au moins une entrée doit différer.
        let mut all_match = true;
        for (e1, e2) in c1.entries.iter().zip(c2.entries.iter()) {
            if e1.program.bytes() != e2.program.bytes() {
                all_match = false;
                break;
            }
        }
        assert!(!all_match, "seeds différents doivent diverger");
    }

    #[test]
    fn each_corpus_entry_has_valid_program() {
        let c = Corpus::generate(123, 8);
        for entry in &c.entries {
            // Programme doit passer round-trip.
            let p2 = Program::from_bytes(entry.program.bytes()).unwrap();
            assert_eq!(entry.program.bytes(), p2.bytes());
        }
    }

    #[test]
    fn each_corpus_entry_has_non_empty_mlir() {
        let c = Corpus::generate(123, 4);
        for entry in &c.entries {
            assert!(!entry.mlir_text.is_empty());
            assert!(entry.mlir_text.starts_with("kasm.program"));
        }
    }

    #[test]
    fn each_corpus_entry_has_nonzero_term_hash() {
        let c = Corpus::generate(123, 4);
        for entry in &c.entries {
            assert_ne!(entry.term_hash, [0u8; 32]);
        }
    }

    #[test]
    fn corpus_term_hashes_distinguish_distinct_programs() {
        let c = Corpus::generate(456, 8);
        // Au moins 2 entrées doivent avoir des hashes différents.
        let mut hashes: std::collections::BTreeSet<_> = std::collections::BTreeSet::new();
        for entry in &c.entries {
            hashes.insert(entry.term_hash);
        }
        assert!(hashes.len() >= 2, "corpus doit contenir programmes diversifiés");
    }

    // =========================================================================
    // η : tests du lifter binaire système
    // =========================================================================

    /// Évalue un programme KASM avec inputs concrets et retourne son output.
    /// Utilisé pour vérifier que le lift préserve la sémantique x86 → KASM.
    fn run_one_output(program: &Program, inputs: &[i64]) -> Option<i64> {
        use crate::kasm::execute;
        let mut buf = Vec::new();
        for v in inputs {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        let result = execute(program, &buf).ok()?;
        if result.len() != 8 {
            return None;
        }
        Some(i64::from_le_bytes(result[..8].try_into().ok()?))
    }

    #[test]
    fn lift_dumpbin_simple_add_preserves_semantics() {
        // Convention MS x64 : RCX = arg0, RDX = arg1, RAX = retour.
        let text = "\
add_two:
0000000180001000: mov         rax,rcx
0000000180001003: add         rax,rdx
0000000180001006: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 1, "doit lifter exactement add_two");
        let p = &entries[0].program;
        assert_eq!(p.inputs(), 2);
        assert_eq!(p.outputs(), 1);
        assert_eq!(run_one_output(p, &[5, 3]), Some(8));
        assert_eq!(run_one_output(p, &[100, -42]), Some(58));
    }

    #[test]
    fn lift_objdump_simple_mul_preserves_semantics() {
        // Convention SysV : RDI = arg0, RSI = arg1, RAX = retour.
        let text = "\
0000000000401000 <mul_two>:
  401000:       mov    rax, rdi
  401003:       imul   rax, rsi
  401006:       ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Objdump);
        assert_eq!(entries.len(), 1);
        let p = &entries[0].program;
        assert_eq!(p.inputs(), 2);
        assert_eq!(run_one_output(p, &[6, 7]), Some(42));
        assert_eq!(run_one_output(p, &[-3, 5]), Some(-15));
    }

    #[test]
    fn lift_xor_zero_idiom_then_add() {
        // xor rax,rax → 0 ; add rax, rcx → rcx ; ret. Output = arg0.
        let text = "\
identity:
0000000180001000: xor         rax,rax
0000000180001003: add         rax,rcx
0000000180001006: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 1);
        assert_eq!(run_one_output(&entries[0].program, &[42]), Some(42));
        assert_eq!(run_one_output(&entries[0].program, &[-7]), Some(-7));
    }

    #[test]
    fn lift_handles_push_pop_prologue() {
        // Prologue/épilogue MSVC standard, intercalé avec un add.
        let text = "\
framed_add:
0000000180001000: push        rbp
0000000180001001: mov         rbp,rsp
0000000180001004: mov         rax,rcx
0000000180001007: add         rax,rdx
000000018000100a: pop         rbp
000000018000100b: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 1, "le prologue/épilogue doit être ignoré");
        assert_eq!(run_one_output(&entries[0].program, &[10, 20]), Some(30));
    }

    #[test]
    fn lift_skips_branch_instructions() {
        let text = "\
branchy:
0000000180001000: mov         rax,rcx
0000000180001003: jmp         0000000180001100h
0000000180001008: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction avec jmp doit être skipée");
    }

    #[test]
    fn lift_skips_call_instructions() {
        let text = "\
callable:
0000000180001000: mov         rax,rcx
0000000180001003: call        0000000180002000h
0000000180001008: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty());
    }

    #[test]
    fn lift_skips_xmm_registers() {
        let text = "\
sse_func:
0000000180001000: movaps      xmm0,xmm1
0000000180001003: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction avec xmm doit être skipée");
    }

    #[test]
    fn lift_skips_memory_references() {
        let text = "\
mem_func:
0000000180001000: mov         rax,qword ptr [rcx+8]
0000000180001004: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction avec mémoire doit être skipée");
    }

    #[test]
    fn lift_skips_imm_too_large_for_i16() {
        // 1 000 000 ne fit pas en i16 : on ne peut pas représenter la const → skip.
        let text = "\
big_imm:
0000000180001000: mov         rax,1000000
0000000180001005: add         rax,rcx
0000000180001008: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "constante > i16 doit échouer le lift");
    }

    #[test]
    fn lift_returns_empty_when_no_function_label() {
        let text = "mov rax,rcx\nadd rax,rdx\nret\n";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty());
    }

    #[test]
    fn lift_handles_multiple_functions_in_one_input() {
        let text = "\
add_two:
0000000180001000: mov         rax,rcx
0000000180001003: add         rax,rdx
0000000180001006: ret
sub_two:
0000000180002000: mov         rax,rcx
0000000180002003: sub         rax,rdx
0000000180002006: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 2);
        // Les deux entrées doivent avoir des term_hash différents (sémantiques distinctes).
        assert_ne!(entries[0].term_hash, entries[1].term_hash);
    }

    #[test]
    fn lift_function_caps_at_50_instructions() {
        // 51 movs + ret = 52 instructions → skip.
        let mut text = String::from("huge:\n");
        for i in 0..51 {
            text.push_str(&format!("0000000180001{i:03}: mov         rax,rcx\n"));
        }
        text.push_str("00000001800010ff: ret\n");
        let entries = Corpus::lift_disasm_text(&text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "> 50 instructions doit être rejeté");
    }

    #[test]
    fn lift_system_binary_returns_empty_on_bogus_path() {
        // Path bidon : soit l'outil est absent → vec vide ; soit il est
        // présent mais retourne une erreur sur le path → vec vide. Aucun panic.
        let entries = Corpus::lift_system_binary(Path::new("Z:/__nonexistent__/__forge_eta_test__"));
        assert!(entries.is_empty());
    }

    #[test]
    fn lift_skips_function_without_ret() {
        let text = "\
no_ret:
0000000180001000: mov         rax,rcx
0000000180001003: add         rax,rdx
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction sans ret doit être skipée");
    }

    #[test]
    fn lift_handles_objdump_imm_hex_format() {
        // objdump utilise typiquement 0x... pour les immédiats.
        let text = "\
0000000000401000 <add_seven>:
  401000:       mov    rax, rdi
  401003:       add    rax, 0x7
  401006:       ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Objdump);
        assert_eq!(entries.len(), 1);
        assert_eq!(run_one_output(&entries[0].program, &[10]), Some(17));
    }
}

}

pub mod extract {
//! Ω-2 — L'Extraction Universelle.
//!
//! Cap actuel : **Ω-2.0** — Tracer-based extraction de fonctions Rust pures
//! et bornées vers des `kasm::Program` content-addressed.
//!
//! Φ.μ.7 : `macros.rs` et `term_extract.rs` repliés ici (3 fichiers → 1).
//! Le sous-dossier conserve `tracer.rs` + `tensor_tracer.rs` (les vrais
//! moteurs d'extraction).
//!
//! ## Le détournement (via negativa)
//!
//! La promesse Ω-2 originale visait Mojo. On a abandonné Mojo (toolchain
//! lourde, doctrine indépendance). À la place, **Rust** devient le langage
//! hôte d'où on extrait. Cohérent : tout SCAN est en Rust.
//!
//! Pas de proc-macro (qui demanderait un crate séparé + frontend lourd).
//! Pas de parser de strings (pas de `syn` en deps). À la place, on utilise
//! le **type system Rust pour CONTRAINDRE l'extraction** :
//!
//! 1. On expose un type `Tracer` qui implémente `std::ops::Add/Sub/Mul/...`.
//! 2. Toute fonction `Fn(Vec<Tracer>) -> Tracer` est automatiquement
//!    extractable.
//! 3. Toute fonction qui essaie d'appeler une op non-supportée (I/O,
//!    allocation, boucle dynamique, etc.) ne **compile pas** — Rust refuse
//!    avant même qu'on tente l'extraction.
//!
//! C'est l'inverse du pattern usuel "détecter la pureté à l'inférence" :
//! on **rend la pureté inférable au compilateur Rust** en restreignant le
//! type. La sécurité émerge du type-system, pas d'une analyse statique.

mod tracer {
//! Tracer + builder thread-local pour Ω-2.0.
//!
//! Le `Tracer` est un index opaque dans un builder de programme KASM.
//! Toutes les opérations sur Tracers s'enregistrent comme des nœuds
//! KASM via le builder thread-local. Quand l'extraction se termine,
//! le builder est consommé pour produire un `Program` final.

use std::cell::RefCell;
use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Shl, Shr, Sub};

use crate::kasm::{KasmError, Node, Program, Target, Ty, MAX_NODES};

#[derive(Debug)]
pub enum ExtractError {
    /// Tentative d'extraction nichée (pas supporté en Ω-2.0).
    NestedExtraction,
    /// Builder absent au moment de l'op (Tracer utilisé hors extract()).
    NoActiveBuilder,
    /// Plus de 4096 nœuds (limite KASM).
    TooManyNodes,
    /// Plus de 16 inputs ou outputs.
    TooManySlots,
    /// Const i64 hors plage i16 (limitation KASM ConstI64 imm).
    ConstOutOfRange(i64),
    /// Builder inconsistant (état corrompu).
    BuilderCorrupted,
    /// Erreur KASM lors de la finalisation.
    Kasm(KasmError),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::NestedExtraction => write!(f, "nested extraction not supported"),
            ExtractError::NoActiveBuilder => write!(f, "no active extraction builder"),
            ExtractError::TooManyNodes => write!(f, "more than {} nodes", MAX_NODES),
            ExtractError::TooManySlots => write!(f, "too many input/output slots"),
            ExtractError::ConstOutOfRange(v) => {
                write!(f, "const {v} does not fit in i16")
            }
            ExtractError::BuilderCorrupted => write!(f, "builder state corrupted"),
            ExtractError::Kasm(e) => write!(f, "kasm: {e}"),
        }
    }
}

impl std::error::Error for ExtractError {}

impl From<KasmError> for ExtractError {
    fn from(e: KasmError) -> Self {
        ExtractError::Kasm(e)
    }
}

// ---------------------------------------------------------------------------
// Builder thread-local
// ---------------------------------------------------------------------------

struct Builder {
    nodes: Vec<Node>,
    inputs: u8,
}

impl Builder {
    fn new(inputs: u8) -> Self {
        let mut nodes = Vec::with_capacity(64);
        for slot in 0..inputs {
            nodes.push(Node::input(slot));
        }
        Self { nodes, inputs }
    }

    fn push(&mut self, node: Node) -> Result<u16, ExtractError> {
        if self.nodes.len() >= MAX_NODES {
            return Err(ExtractError::TooManyNodes);
        }
        let idx = self.nodes.len() as u16;
        self.nodes.push(node);
        Ok(idx)
    }
}

thread_local! {
    static BUILDER: RefCell<Option<Builder>> = const { RefCell::new(None) };
}

fn with_builder<R>(f: impl FnOnce(&mut Builder) -> Result<R, ExtractError>) -> R {
    BUILDER.with(|b| {
        let mut slot = b.borrow_mut();
        let builder = slot.as_mut().expect(
            "Tracer used outside of extract() — extraction context inactive",
        );
        f(builder).expect("tracer op failed inside extract()")
    })
}

// ---------------------------------------------------------------------------
// Tracer
// ---------------------------------------------------------------------------

/// Handle opaque vers un nœud KASM dans le builder thread-local courant.
///
/// `Tracer` n'est utilisable QU'À L'INTÉRIEUR d'un appel à `extract()`.
/// Toute opération std::ops::* enregistre un nœud dans le builder courant.
#[derive(Clone, Copy, Debug)]
pub struct Tracer {
    node_idx: u16,
}

impl Tracer {
    /// Construit un Tracer pour un slot d'entrée du programme.
    /// Réservé à l'usage interne d'`extract()` ; les utilisateurs reçoivent
    /// les inputs via le `Vec<Tracer>` passé à leur closure.
    fn from_input_slot(slot: u8) -> Self {
        // Les inputs sont les `inputs` premiers nodes du builder, donc
        // node_idx = slot pour les inputs (Builder::new les empile dans
        // l'ordre des slots).
        Self { node_idx: slot as u16 }
    }

    /// Construit un Tracer constant à partir d'une valeur i16.
    /// Pour les valeurs hors i16, voir Ω-2.0.x (encodage par bit_xor / shifts).
    pub fn const_i16(value: i16) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::const_i64(value))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Helper : construit un Tracer constant depuis i64. Erreur compile-time
    /// impossible (i64 -> i16 cast), mais panique à runtime si hors plage.
    pub fn const_(value: i64) -> Self {
        let v = i16::try_from(value).expect("Tracer::const_ value out of i16 range");
        Self::const_i16(v)
    }

    /// Min(self, other) — élémentaire dans KASM.
    pub fn min(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::min(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Max(self, other).
    pub fn max(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::max(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Division checkée (a / b, ou 0 si b == 0).
    pub fn div_checked(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::div_checked(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Modulo Euclidien (a mod b, ou 0 si b == 0).
    pub fn mod_checked(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::mod_checked(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Saturating add.
    pub fn sat_add(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::sat_add(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Saturating sub.
    pub fn sat_sub(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::sat_sub(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Cross-type : i64 → bool (Ω-2.0.1) -----

    /// `self == other` → `BoolTracer`.
    pub fn eq(self, other: Self) -> BoolTracer {
        with_builder(|b| {
            let idx = b.push(Node::eq(self.node_idx, other.node_idx))?;
            Ok(BoolTracer { node_idx: idx })
        })
    }

    /// `self < other` (signed) → `BoolTracer`.
    pub fn lt(self, other: Self) -> BoolTracer {
        with_builder(|b| {
            let idx = b.push(Node::lt(self.node_idx, other.node_idx))?;
            Ok(BoolTracer { node_idx: idx })
        })
    }

    /// `self <= other` (signed) → `BoolTracer`.
    pub fn le(self, other: Self) -> BoolTracer {
        with_builder(|b| {
            let idx = b.push(Node::le(self.node_idx, other.node_idx))?;
            Ok(BoolTracer { node_idx: idx })
        })
    }

    // ----- Clamp ternaire (Ω-2.0.3) -----

    /// `clamp(self, lo, hi)` — KASM Clamp ternaire.
    pub fn clamp(self, lo: Self, hi: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::clamp(self.node_idx, lo.node_idx, hi.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Hash64 (Ω-2.0.4) -----

    /// Hash 64-bit du Tracer (KASM Hash64).
    pub fn hash64(self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::hash64(self.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Reduce ops (Ω-2.0.2) -----

    /// Réduction par addition d'un slice de Tracers contigus.
    ///
    /// `items` doit être non-vide ET les Tracers doivent être contigus
    /// dans le builder (i.e. `items[i].node_idx == items[0].node_idx + i`
    /// pour tout `i`). C'est une contrainte structurelle de KASM
    /// `ReduceAddI64` qui référence un intervalle `[base, base + count)`.
    ///
    /// # Panique
    ///
    /// - Si `items` est vide.
    /// - Si les Tracers ne sont pas contigus.
    /// - Si `items.len() > i16::MAX` (limite KASM count).
    pub fn reduce_add(items: &[Tracer]) -> Tracer {
        let (base, count) = check_contiguous(items, "reduce_add");
        with_builder(|b| {
            let idx = b.push(Node::reduce_add(base, count))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Réduction par multiplication d'un slice de Tracers contigus.
    /// Mêmes contraintes que `reduce_add`.
    pub fn reduce_mul(items: &[Tracer]) -> Tracer {
        let (base, count) = check_contiguous(items, "reduce_mul");
        with_builder(|b| {
            let idx = b.push(Node::reduce_mul(base, count))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Constantes wide (Ω-2.0.6) -----

    /// Construit un Tracer pour n'importe quelle valeur i64.
    ///
    /// Pour les valeurs dans `i16::MIN..=i16::MAX`, c'est un seul nœud
    /// Const. Sinon, on décompose en 4 chunks de 16 bits + shifts/ors —
    /// jusqu'à 15 nœuds dans le pire cas. Le coût est documenté dans le
    /// doc Ω-2.
    ///
    /// Astuce de masquage : pour zeroer les bits au-delà des 16 bas d'un
    /// nombre `x`, on calcule `(x << 48) >> 48` (logical right shift KASM).
    pub fn const_i64_wide(value: i64) -> Self {
        if let Ok(small) = i16::try_from(value) {
            return Self::const_i16(small);
        }

        let bits = value as u64;
        let chunk = |shift: u32| -> u16 { ((bits >> shift) & 0xFFFF) as u16 };

        // Construit les 4 chunks. Pour les chunks bas (qu'on va shifter et
        // masquer), on utilise le trick (x << 48) >> 48.
        // Le top chunk (bits 63..48) peut directement être un i16 signé,
        // car le sign-extend est ce qu'on veut pour préserver les bits hauts.
        let top_chunk = chunk(48);
        let mid_hi_chunk = chunk(32);
        let mid_lo_chunk = chunk(16);
        let low_chunk = chunk(0);

        let const48 = Self::const_i16(48);

        // Top chunk : on l'utilise tel quel via i16 (sign-extend OK car c'est
        // le top de l'i64).
        let top_signed = top_chunk as i16;
        let top = Self::const_i16(top_signed);
        let top_shifted = top << Self::const_i16(48);

        // Helper pour construire un chunk masqué et shifté à une position.
        // Utilise le trick (x << 48) >> 48 pour zeroer le sign-extend.
        let masked_chunk = |c: u16| -> Self {
            let signed = c as i16; // peut sign-extend, OK car on masque ensuite
            let raw = Self::const_i16(signed);
            // (raw << 48) >> 48 = juste les 16 bits bas
            let up = raw << const48;
            up >> const48
        };

        let mid_hi = masked_chunk(mid_hi_chunk) << Self::const_i16(32);
        let mid_lo = masked_chunk(mid_lo_chunk) << Self::const_i16(16);
        let low = masked_chunk(low_chunk);

        // Combine via BitOr.
        top_shifted | mid_hi | mid_lo | low
    }
}

/// Vérifie qu'un slice de Tracers est non-vide, que `len` tient dans
/// `i16::MAX` (contrainte KASM count), et que les indices sont contigus.
/// Retourne `(base, count)` prêts à passer à `Node::reduce_*`.
fn check_contiguous(items: &[Tracer], op_name: &str) -> (u16, u16) {
    assert!(
        !items.is_empty(),
        "Tracer::{op_name}: items must be non-empty (KASM Reduce* requires count >= 1)"
    );
    assert!(
        items.len() <= i16::MAX as usize,
        "Tracer::{op_name}: count {} exceeds KASM limit i16::MAX",
        items.len()
    );
    let base = items[0].node_idx;
    for (i, t) in items.iter().enumerate() {
        let expected = base.checked_add(i as u16).unwrap_or_else(|| {
            panic!("Tracer::{op_name}: contiguous range overflows u16 at i={i}")
        });
        assert_eq!(
            t.node_idx, expected,
            "Tracer::{op_name}: items must be contiguous in builder; \
             items[{i}].node_idx = {} expected {} (base = {base}). \
             Reduce* requires the slice to map to a contiguous KASM index range.",
            t.node_idx, expected
        );
    }
    (base, items.len() as u16)
}

// ---------------------------------------------------------------------------
// BoolTracer (Ω-2.0.1)
// ---------------------------------------------------------------------------

/// Tracer bool. Produit par les comparaisons (`Tracer::eq/lt/le`) ;
/// supporte les opérations logiques (`&`, `|`, `not`) et le `select`
/// ternaire qui retombe sur i64.
#[derive(Clone, Copy, Debug)]
pub struct BoolTracer {
    node_idx: u16,
}

impl BoolTracer {
    /// Négation logique. KASM NotBool est bijective (Landauer-cost zéro).
    pub fn not(self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::not(self.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Sélection ternaire : `if self { then_branch } else { else_branch }`.
    pub fn select(self, then_branch: Tracer, else_branch: Tracer) -> Tracer {
        with_builder(|b| {
            let idx = b.push(Node::select_i64(
                self.node_idx,
                then_branch.node_idx,
                else_branch.node_idx,
            ))?;
            Ok(Tracer { node_idx: idx })
        })
    }
}

impl std::ops::BitAnd for BoolTracer {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::and(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl std::ops::BitOr for BoolTracer {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::or(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

// ---------------------------------------------------------------------------
// std::ops::* — opérateurs Rust → nœuds KASM
// ---------------------------------------------------------------------------

impl Add for Tracer {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::add(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Sub for Tracer {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::sub(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Mul for Tracer {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::mul(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl BitAnd for Tracer {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::bit_and(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl BitOr for Tracer {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::bit_or(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl BitXor for Tracer {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::bit_xor(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Shl for Tracer {
    type Output = Self;
    fn shl(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::shl(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Shr for Tracer {
    type Output = Self;
    fn shr(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::shr(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

// Ergonomie : Tracer + i64 (et symétrique).
impl Add<i64> for Tracer {
    type Output = Self;
    fn add(self, rhs: i64) -> Self {
        let c = Tracer::const_(rhs);
        self + c
    }
}

impl Mul<i64> for Tracer {
    type Output = Self;
    fn mul(self, rhs: i64) -> Self {
        let c = Tracer::const_(rhs);
        self * c
    }
}

impl Sub<i64> for Tracer {
    type Output = Self;
    fn sub(self, rhs: i64) -> Self {
        let c = Tracer::const_(rhs);
        self - c
    }
}

// ---------------------------------------------------------------------------
// extract() — point d'entrée principal
// ---------------------------------------------------------------------------

/// Extrait une fonction `Fn(Vec<Tracer>) -> Tracer` en `kasm::Program`.
///
/// `n_inputs` doit être ≥ 1 et ≤ `MAX_SLOTS` (16). Le programme produit
/// a 1 output (le Tracer retourné par la closure).
///
/// Aucune extraction nichée n'est supportée — appeler `extract()` à
/// l'intérieur d'une autre extraction renvoie `NestedExtraction`.
pub fn extract<F>(n_inputs: u8, f: F) -> Result<Program, ExtractError>
where
    F: FnOnce(Vec<Tracer>) -> Tracer,
{
    if n_inputs == 0 || n_inputs as usize > 16 {
        return Err(ExtractError::TooManySlots);
    }

    // Setup builder.
    let already_active = BUILDER.with(|b| b.borrow().is_some());
    if already_active {
        return Err(ExtractError::NestedExtraction);
    }
    BUILDER.with(|b| {
        *b.borrow_mut() = Some(Builder::new(n_inputs));
    });

    // Construit les Tracers d'entrée.
    let inputs: Vec<Tracer> = (0..n_inputs).map(Tracer::from_input_slot).collect();

    // Run the closure. On capture le résultat AVANT de finaliser le builder
    // pour que les éventuels panics dans f libèrent la TLS proprement.
    let result_idx = {
        // Wrap dans un guard pour reset le TLS si f panique.
        let guard = ExtractGuard;
        let result = f(inputs);
        std::mem::forget(guard); // succès — pas besoin de cleanup panic-side
        result.node_idx
    };

    // Récupère le builder, ajoute output, finalise.
    let builder = BUILDER
        .with(|b| b.borrow_mut().take())
        .ok_or(ExtractError::BuilderCorrupted)?;

    let mut nodes = builder.nodes;
    let total_nodes_before_output = nodes.len();
    if total_nodes_before_output >= MAX_NODES {
        return Err(ExtractError::TooManyNodes);
    }
    nodes.push(Node::output(result_idx, Ty::I64));

    let total = nodes.len() as u32;
    let program = Program::new(Target::Cpu, builder.inputs, 1, total, nodes)?;
    Ok(program)
}

/// Guard RAII : si `extract()` panique au milieu, libère la TLS.
struct ExtractGuard;
impl Drop for ExtractGuard {
    fn drop(&mut self) {
        BUILDER.with(|b| {
            *b.borrow_mut() = None;
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::execute;

    fn exec_one(p: &Program, args: &[i64]) -> Vec<u8> {
        let bytes: Vec<u8> = args.iter().flat_map(|v| v.to_le_bytes()).collect();
        execute(p, &bytes).expect("kasm execute")
    }

    fn read_one_i64(out: &[u8]) -> i64 {
        i64::from_le_bytes(out[..8].try_into().unwrap())
    }

    #[test]
    fn extract_identity_function() {
        let prog = extract(1, |inputs| inputs[0]).unwrap();
        for x in [-100, -1, 0, 1, 42, 1_000_000] {
            let out = exec_one(&prog, &[x]);
            assert_eq!(read_one_i64(&out), x);
        }
    }

    #[test]
    fn extract_addition() {
        let prog = extract(2, |inputs| inputs[0] + inputs[1]).unwrap();
        for (a, b) in [(0, 0), (1, 2), (-3, 7), (1_000, 2_000)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a + b);
        }
    }

    #[test]
    fn extract_affine_via_const() {
        // f(x) = 7 * x + 3 — utilise const_ via i64 ergonomique.
        let prog = extract(1, |inputs| inputs[0] * 7 + 3).unwrap();
        for x in [-10, 0, 1, 5, 100] {
            let out = exec_one(&prog, &[x]);
            assert_eq!(read_one_i64(&out), 7 * x + 3);
        }
    }

    #[test]
    fn extract_complex_arithmetic() {
        // f(x, y) = (x + y) * (x - y) = x² - y²
        let prog = extract(2, |inputs| {
            let sum = inputs[0] + inputs[1];
            let diff = inputs[0] - inputs[1];
            sum * diff
        })
        .unwrap();
        for (x, y) in [(3, 2), (10, 7), (-5, 3), (100, 99)] {
            let out = exec_one(&prog, &[x, y]);
            assert_eq!(read_one_i64(&out), x * x - y * y);
        }
    }

    #[test]
    fn extract_bitwise_ops() {
        // f(x, y) = (x & y) | (x ^ y)
        let prog = extract(2, |inputs| {
            (inputs[0] & inputs[1]) | (inputs[0] ^ inputs[1])
        })
        .unwrap();
        for (x, y) in [(0b1010, 0b1100), (0xff00, 0x00ff), (-1, 1)] {
            let out = exec_one(&prog, &[x, y]);
            let expected = (x & y) | (x ^ y);
            assert_eq!(read_one_i64(&out), expected);
        }
    }

    #[test]
    fn extract_shifts() {
        // f(x, k) = (x << k) | (x >> k)
        let prog = extract(2, |inputs| {
            (inputs[0] << inputs[1]) | (inputs[0] >> inputs[1])
        })
        .unwrap();
        for (x, k) in [(1i64, 3i64), (0xff, 4), (1024, 10)] {
            let out = exec_one(&prog, &[x, k]);
            let kasm_shl = (x as u64).wrapping_shl((k as u64 & 63) as u32) as i64;
            let kasm_shr = (x as u64).wrapping_shr((k as u64 & 63) as u32) as i64;
            assert_eq!(read_one_i64(&out), kasm_shl | kasm_shr);
        }
    }

    #[test]
    fn extract_min_max() {
        let prog = extract(2, |inputs| inputs[0].min(inputs[1])).unwrap();
        let out = exec_one(&prog, &[3, 7]);
        assert_eq!(read_one_i64(&out), 3);
        let prog = extract(2, |inputs| inputs[0].max(inputs[1])).unwrap();
        let out = exec_one(&prog, &[3, 7]);
        assert_eq!(read_one_i64(&out), 7);
    }

    #[test]
    fn extract_div_mod() {
        // f(a, b) = a / b + a mod b
        let prog = extract(2, |inputs| {
            inputs[0].div_checked(inputs[1]) + inputs[0].mod_checked(inputs[1])
        })
        .unwrap();
        for (a, b) in [(10, 3), (100, 7), (-5, 2)] {
            let out = exec_one(&prog, &[a, b]);
            // KASM div_checked retourne 0 si b==0 ; mod_checked = a.checked_rem(b).unwrap_or(0).
            // Pour b ≠ 0, c'est a/b + a%b.
            let expected = a.checked_div(b).unwrap_or(0) + a.checked_rem(b).unwrap_or(0);
            assert_eq!(read_one_i64(&out), expected);
        }
    }

    #[test]
    fn extract_sat_arithmetic() {
        let prog = extract(2, |inputs| inputs[0].sat_add(inputs[1])).unwrap();
        let out = exec_one(&prog, &[i64::MAX, 100]);
        assert_eq!(read_one_i64(&out), i64::MAX); // saturé.
    }

    #[test]
    fn extracted_program_is_content_addressed() {
        let prog1 = extract(1, |inputs| inputs[0] * 7 + 3).unwrap();
        let prog2 = extract(1, |inputs| inputs[0] * 7 + 3).unwrap();
        assert_eq!(
            prog1.canonical_hash_hex().unwrap(),
            prog2.canonical_hash_hex().unwrap(),
            "deux extractions de la même closure → même hash canonique"
        );
    }

    #[test]
    fn extracted_distinct_closures_have_distinct_hashes() {
        let p_add = extract(2, |inputs| inputs[0] + inputs[1]).unwrap();
        let p_mul = extract(2, |inputs| inputs[0] * inputs[1]).unwrap();
        assert_ne!(
            p_add.canonical_hash_hex().unwrap(),
            p_mul.canonical_hash_hex().unwrap()
        );
    }

    #[test]
    fn nested_extraction_errors() {
        let result = extract(1, |inputs| {
            // Tentative d'extraction interne.
            let _ = extract(1, |inner| inner[0]);
            inputs[0]
        });
        // L'inner extract() retourne Err(NestedExtraction), mais la closure
        // externe fait inputs[0], donc l'extract externe RÉUSSIT, juste
        // l'inner a été ignorée. Ce test vérifie que l'extract externe
        // ne crashe PAS et retourne le bon programme.
        assert!(result.is_ok());
    }

    #[test]
    fn nested_extraction_inner_returns_error() {
        let _outer_result = extract(1, |inputs| {
            // Inner extract() doit retourner NestedExtraction.
            let inner = extract(1, |inner_inputs| inner_inputs[0]);
            assert!(matches!(inner, Err(ExtractError::NestedExtraction)));
            inputs[0]
        });
    }

    #[test]
    fn extracted_program_has_correct_io_counts() {
        let prog = extract(2, |inputs| inputs[0] + inputs[1]).unwrap();
        assert_eq!(prog.inputs(), 2);
        assert_eq!(prog.outputs(), 1);
    }

    #[test]
    fn extracted_through_meta_embedding_is_hashable() {
        // Cross-cap : Ω-2.0 + Ω-4.1. L'extraction produit un Program qui
        // s'embed via meta::embed_program.
        use crate::meta::embed_program;
        let prog = extract(1, |inputs| inputs[0] + 7).unwrap();
        let term = embed_program(&prog);
        let h = term.hash();
        assert_ne!(h, [0u8; 32]);
    }

    // ----- Bool ops + Select (Ω-2.0.1) -----

    #[test]
    fn extract_eq_via_bool() {
        // f(x, y) = if x == y then 100 else 0
        let prog = extract(2, |inputs| {
            let cond = inputs[0].eq(inputs[1]);
            cond.select(Tracer::const_(100), Tracer::const_(0))
        })
        .unwrap();
        let out = exec_one(&prog, &[5, 5]);
        assert_eq!(read_one_i64(&out), 100);
        let out = exec_one(&prog, &[5, 6]);
        assert_eq!(read_one_i64(&out), 0);
    }

    #[test]
    fn extract_lt_le_select() {
        // f(x, y) = if x < y then x else y (= min)
        let prog = extract(2, |inputs| {
            inputs[0].lt(inputs[1]).select(inputs[0], inputs[1])
        })
        .unwrap();
        for (a, b) in [(3i64, 7), (10, 5), (-1, 1)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a.min(b));
        }
    }

    #[test]
    fn extract_bool_and_or_not() {
        // f(x, y, z) = if (x < y) && !(y < z) then 1 else 0
        let prog = extract(3, |inputs| {
            let lt_xy = inputs[0].lt(inputs[1]);
            let lt_yz = inputs[1].lt(inputs[2]);
            let cond = lt_xy & lt_yz.not();
            cond.select(Tracer::const_(1), Tracer::const_(0))
        })
        .unwrap();
        let out = exec_one(&prog, &[1, 5, 3]); // 1<5 et !(5<3) → 1
        assert_eq!(read_one_i64(&out), 1);
        let out = exec_one(&prog, &[1, 2, 3]); // 1<2 et !(2<3) = !true = false → 0
        assert_eq!(read_one_i64(&out), 0);
    }

    #[test]
    fn extract_bool_or_combines_predicates() {
        // f(x) = if x < 0 || x > 100 then -1 else x
        let prog = extract(1, |inputs| {
            let too_low = inputs[0].lt(Tracer::const_(0));
            let too_high = Tracer::const_(100).lt(inputs[0]);
            (too_low | too_high).select(Tracer::const_(-1), inputs[0])
        })
        .unwrap();
        for x in [-5i64, 0, 50, 100, 150] {
            let out = exec_one(&prog, &[x]);
            let expected = if x < 0 || x > 100 { -1 } else { x };
            assert_eq!(read_one_i64(&out), expected);
        }
    }

    // ----- Clamp (Ω-2.0.3) -----

    #[test]
    fn extract_clamp() {
        let prog = extract(1, |inputs| {
            inputs[0].clamp(Tracer::const_(-10), Tracer::const_(10))
        })
        .unwrap();
        for x in [-100i64, -10, 0, 10, 100] {
            let out = exec_one(&prog, &[x]);
            assert_eq!(read_one_i64(&out), x.clamp(-10, 10));
        }
    }

    // ----- Hash64 (Ω-2.0.4) -----

    #[test]
    fn extract_hash64_is_deterministic() {
        let prog = extract(1, |inputs| inputs[0].hash64()).unwrap();
        let h1 = exec_one(&prog, &[42]);
        let h2 = exec_one(&prog, &[42]);
        assert_eq!(h1, h2, "hash déterministe pour même input");
        let h3 = exec_one(&prog, &[43]);
        assert_ne!(h1, h3, "hash distingue inputs distincts");
    }

    // ----- Const wide (Ω-2.0.6) -----

    #[test]
    fn const_i64_wide_in_i16_range_is_single_node() {
        let prog = extract(1, |inputs| inputs[0] + Tracer::const_i64_wide(100)).unwrap();
        // 1 input + 1 const (i16 case) + 1 add + 1 output = 4 nodes.
        assert_eq!(prog.nodes().len(), 4);
        let out = exec_one(&prog, &[5]);
        assert_eq!(read_one_i64(&out), 105);
    }

    #[test]
    fn const_i64_wide_outside_i16_works() {
        let target: i64 = 0x1234_5678_9ABC_DEF0_u64 as i64;
        let prog = extract(1, |inputs| {
            inputs[0] + Tracer::const_i64_wide(target)
        })
        .unwrap();
        // Doit fonctionner — même si plusieurs nœuds.
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), target);
    }

    #[test]
    fn const_i64_wide_negative_large() {
        let target: i64 = -1_234_567_890_i64;
        let prog = extract(1, |inputs| {
            inputs[0] + Tracer::const_i64_wide(target)
        })
        .unwrap();
        let out = exec_one(&prog, &[1_000_000_000]);
        assert_eq!(read_one_i64(&out), 1_000_000_000 + target);
    }

    #[test]
    fn const_i64_wide_max_value() {
        let prog = extract(1, |inputs| {
            inputs[0] | Tracer::const_i64_wide(i64::MAX)
        })
        .unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), i64::MAX);
    }

    // ----- Reduce (Ω-2.0.2) -----

    #[test]
    fn extract_reduce_add_4_items() {
        // sum(1, 2, 3, 4) = 10. Les 4 consts sont créées dans le builder
        // dans l'ordre, donc contiguës. ReduceAdd les réduit en une somme.
        let prog = extract(1, |_inputs| {
            let items = [
                Tracer::const_i16(1),
                Tracer::const_i16(2),
                Tracer::const_i16(3),
                Tracer::const_i16(4),
            ];
            Tracer::reduce_add(&items)
        })
        .unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), 10);
    }

    #[test]
    fn extract_reduce_mul_3_items() {
        // prod(2, 3, 5) = 30.
        let prog = extract(1, |_inputs| {
            let items = [
                Tracer::const_i16(2),
                Tracer::const_i16(3),
                Tracer::const_i16(5),
            ];
            Tracer::reduce_mul(&items)
        })
        .unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), 30);
    }

    #[test]
    #[should_panic(expected = "must be contiguous")]
    fn extract_reduce_non_contiguous_panics() {
        // On insère un nœud entre deux consts, qui casse la contiguïté.
        let _ = extract(2, |inputs| {
            let a = Tracer::const_i16(1);
            // Cette opération crée un nœud entre `a` et `b` → casse contiguïté.
            let _filler = inputs[0] + inputs[1];
            let b = Tracer::const_i16(2);
            // a et b ne sont PAS contigus : panic attendu.
            Tracer::reduce_add(&[a, b])
        });
    }

    #[test]
    #[should_panic(expected = "non-empty")]
    fn extract_reduce_empty_panics() {
        let _ = extract(1, |_inputs| Tracer::reduce_add(&[]));
    }

    #[test]
    fn extract_reduce_add_with_inputs_contiguous() {
        // 2 inputs sont déjà placés en %0 et %1 (contigus). Reduce sur
        // les inputs eux-mêmes est valide.
        let prog = extract(2, |inputs| Tracer::reduce_add(&inputs)).unwrap();
        for (a, b) in [(3i64, 7), (-5, 5), (100, 200)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a + b);
        }
    }

    // ----- Cross-cap : Ω-2 + Ω-6 (Landauer cost on bool/select programs) -----

    #[test]
    fn extracted_select_program_has_landauer_cost() {
        use crate::landauer::program_cost;
        let prog = extract(2, |inputs| {
            inputs[0].lt(inputs[1]).select(inputs[0], inputs[1])
        })
        .unwrap();
        let cost = program_cost(&prog);
        // Le programme contient au moins une comparaison (127 bits) + un select (65 bits).
        assert!(cost.total_bits_erased >= 127 + 65);
    }

    #[test]
    fn extracted_program_survives_canonicalization() {
        let prog = extract(2, |inputs| (inputs[0] + 0) * 1).unwrap();
        // Le canonicalize doit éliminer les opérations triviales (+0, *1).
        let canon = prog.canonical().unwrap();
        // Le programme canonique a strictement moins de nœuds.
        assert!(canon.nodes().len() < prog.nodes().len());
    }
}

}

pub use tracer::{extract, BoolTracer, ExtractError, Tracer};

pub mod tensor_tracer {
//! Ω-2.0.7 — Extraction tensorielle (first mile).
//!
//! Pendant que `tracer.rs` extrait des fonctions Rust scalaires vers
//! `kasm::Program`, ce module extrait des compositions tensorielles
//! vers `kasm::tensor::TensorProgram`.
//!
//! ## Scope honnête de cette première étape
//!
//! - dtype : **F32 uniquement**.
//! - shapes : **vec(N) 1-D uniquement** (compatible avec `AddF32` /
//!   `MulF32` élémentwise).
//! - ops : `Input`, `Const`, `AddF32`, `MulF32`, `Output`.
//!
//! Ce qui est délibérément **reporté** (Ω-2.0.7.x) :
//!
//! - shapes 2-D (matrices, matmul) — `MatmulTile` exige une
//!   manipulation différente du shape result.
//! - dtypes Rational / Posit16 / Posit32.
//! - reduce / softmax / activations (relu, tanh, sigmoid, gelu).
//! - extraction depuis une closure Rust pure (analogue à `extract()`
//!   pour les scalaires) — exigerait un `TensorTracer` opérateur-overloadé,
//!   plus la machinerie thread-local de `tracer.rs`.
//!
//! L'API actuelle est un **builder fluent** : on construit un
//! `TensorProgram` étape par étape, et on appelle `.build()` à la fin.

use crate::kasm::tensor::{
    TensorError, TensorNode, TensorProgram, TensorShape, TensorTy, TENSOR_MAX_NODES,
};

/// Erreur de l'extraction tensorielle.
#[derive(Debug)]
pub enum TensorExtractError {
    /// Trop de nœuds dans le builder.
    TooManyNodes,
    /// Référence vers un handle invalide (out-of-bounds).
    BadHandle(u32),
    /// Erreur pendant la finalisation `TensorProgram::new`.
    Tensor(TensorError),
    /// Pas d'output défini avant `build()`.
    NoOutput,
    /// Shapes incompatibles entre opérandes.
    ShapeMismatch,
    /// dtypes incompatibles entre opérandes.
    DtypeMismatch,
    /// Shape invalide à la construction (ex: `vec(0)`).
    BadShape(TensorError),
}

impl std::fmt::Display for TensorExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TensorExtractError::TooManyNodes => write!(f, "too many tensor nodes"),
            TensorExtractError::BadHandle(h) => write!(f, "bad tensor handle {h}"),
            TensorExtractError::Tensor(e) => write!(f, "tensor: {e}"),
            TensorExtractError::NoOutput => write!(f, "no output set before build()"),
            TensorExtractError::ShapeMismatch => write!(f, "operands have mismatching shape"),
            TensorExtractError::DtypeMismatch => write!(f, "operands have mismatching dtype"),
            TensorExtractError::BadShape(e) => write!(f, "bad shape: {e}"),
        }
    }
}

impl std::error::Error for TensorExtractError {}

impl From<TensorError> for TensorExtractError {
    fn from(e: TensorError) -> Self {
        TensorExtractError::Tensor(e)
    }
}

/// Handle opaque vers un nœud `TensorProgram` en cours de construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorHandle {
    idx: u32,
    shape: TensorShape,
    dtype: TensorTy,
}

impl TensorHandle {
    pub fn shape(&self) -> TensorShape {
        self.shape
    }
    pub fn dtype(&self) -> TensorTy {
        self.dtype
    }
}

/// Builder fluent pour `TensorProgram`. Première implémentation
/// volontairement réduite : F32 + shape vec(N) + Add/Mul/Output.
///
/// L'invariant : tout `TensorHandle` retourné par les méthodes ci-dessous
/// référence un nœud déjà inséré dans `nodes` et ne deviendra jamais
/// invalide pendant la vie du builder.
pub struct TensorTracer {
    nodes: Vec<TensorNode>,
    const_pool: Vec<u8>,
    inputs: u8,
    output_idx: Option<u32>,
}

impl TensorTracer {
    /// Crée un nouveau builder vide.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            const_pool: Vec::new(),
            inputs: 0,
            output_idx: None,
        }
    }

    fn push(&mut self, node: TensorNode) -> Result<u32, TensorExtractError> {
        if self.nodes.len() >= TENSOR_MAX_NODES {
            return Err(TensorExtractError::TooManyNodes);
        }
        let idx = self.nodes.len() as u32;
        self.nodes.push(node);
        Ok(idx)
    }

    /// Ajoute un slot d'input F32 de shape donnée. Le slot est
    /// numéroté automatiquement (0, 1, 2, ... dans l'ordre d'appel).
    pub fn input_f32(&mut self, shape: TensorShape) -> Result<TensorHandle, TensorExtractError> {
        let slot = self.inputs;
        let node = TensorNode::input(slot, TensorTy::F32, shape);
        let idx = self.push(node)?;
        self.inputs = self.inputs.checked_add(1).ok_or(TensorExtractError::TooManyNodes)?;
        Ok(TensorHandle { idx, shape, dtype: TensorTy::F32 })
    }

    /// Ajoute une constante F32 de shape vec(N). Le slice `data` doit
    /// contenir exactement `shape.elements()` valeurs.
    pub fn const_f32(
        &mut self,
        shape: TensorShape,
        data: &[f32],
    ) -> Result<TensorHandle, TensorExtractError> {
        if data.len() != shape.elements() {
            return Err(TensorExtractError::ShapeMismatch);
        }
        let pool_offset = self.const_pool.len() as u32;
        for v in data {
            self.const_pool.extend_from_slice(&v.to_le_bytes());
        }
        let pool_len = (data.len() * 4) as u32;
        let node = TensorNode::const_at(pool_offset, pool_len, TensorTy::F32, shape);
        let idx = self.push(node)?;
        Ok(TensorHandle { idx, shape, dtype: TensorTy::F32 })
    }

    /// Addition élémentwise. `a` et `b` doivent avoir la même shape +
    /// dtype.
    pub fn add(
        &mut self,
        a: TensorHandle,
        b: TensorHandle,
    ) -> Result<TensorHandle, TensorExtractError> {
        if a.shape != b.shape {
            return Err(TensorExtractError::ShapeMismatch);
        }
        if a.dtype != b.dtype {
            return Err(TensorExtractError::DtypeMismatch);
        }
        let node = TensorNode::add(a.idx, b.idx, a.dtype, a.shape);
        let idx = self.push(node)?;
        Ok(TensorHandle { idx, shape: a.shape, dtype: a.dtype })
    }

    /// Multiplication élémentwise. Mêmes contraintes que `add`.
    pub fn mul(
        &mut self,
        a: TensorHandle,
        b: TensorHandle,
    ) -> Result<TensorHandle, TensorExtractError> {
        if a.shape != b.shape {
            return Err(TensorExtractError::ShapeMismatch);
        }
        if a.dtype != b.dtype {
            return Err(TensorExtractError::DtypeMismatch);
        }
        let node = TensorNode::mul(a.idx, b.idx, a.dtype, a.shape);
        let idx = self.push(node)?;
        Ok(TensorHandle { idx, shape: a.shape, dtype: a.dtype })
    }

    /// Marque `h` comme output (le programme n'en supporte qu'un seul
    /// pour cette première étape).
    pub fn output(&mut self, h: TensorHandle) -> Result<(), TensorExtractError> {
        let node = TensorNode::output(h.idx, h.dtype, h.shape);
        let idx = self.push(node)?;
        self.output_idx = Some(idx);
        Ok(())
    }

    /// Finalise le builder en `TensorProgram`. Échoue si aucun output
    /// n'a été défini.
    pub fn build(self) -> Result<TensorProgram, TensorExtractError> {
        if self.output_idx.is_none() {
            return Err(TensorExtractError::NoOutput);
        }
        let fuel = self.nodes.len() as u32;
        let outputs = 1u8;
        let prog = TensorProgram::new(self.inputs, outputs, fuel, self.nodes, self.const_pool)?;
        Ok(prog)
    }
}

impl Default for TensorTracer {
    fn default() -> Self {
        Self::new()
    }
}

/// Sucre : extrait via une closure qui prend un `&mut TensorTracer`.
///
/// Convention : la closure DOIT appeler `tracer.output(...)` exactement
/// une fois ; sinon `build()` échoue avec `NoOutput`.
pub fn extract_tensor<F>(f: F) -> Result<TensorProgram, TensorExtractError>
where
    F: FnOnce(&mut TensorTracer) -> Result<(), TensorExtractError>,
{
    let mut tracer = TensorTracer::new();
    f(&mut tracer)?;
    tracer.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::tensor::execute_tensor;

    #[test]
    fn tensor_tracer_identity_via_input_output() {
        // f([x; 4]) = x : un seul Input → Output.
        let shape = TensorShape::vec(4).unwrap();
        let prog = extract_tensor(|t| {
            let x = t.input_f32(shape)?;
            t.output(x)?;
            Ok(())
        })
        .expect("build");
        assert_eq!(prog.inputs(), 1);
        assert_eq!(prog.outputs(), 1);

        // Exécution : input = [1.0, 2.0, 3.0, 4.0] → output identique.
        let out = execute_tensor(&prog, &[vec![1.0, 2.0, 3.0, 4.0]]).expect("exec");
        assert_eq!(out.as_slice(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn tensor_tracer_add_two_inputs() {
        // f(a, b) = a + b sur vec(3).
        let shape = TensorShape::vec(3).unwrap();
        let prog = extract_tensor(|t| {
            let a = t.input_f32(shape)?;
            let b = t.input_f32(shape)?;
            let s = t.add(a, b)?;
            t.output(s)?;
            Ok(())
        })
        .expect("build");
        assert_eq!(prog.inputs(), 2);

        let out = execute_tensor(
            &prog,
            &[vec![1.0, 2.0, 3.0], vec![10.0, 20.0, 30.0]],
        )
        .expect("exec");
        assert_eq!(out.as_slice(), &[11.0, 22.0, 33.0]);
    }

    #[test]
    fn tensor_tracer_const_plus_input() {
        // f(x) = x + [1, 2, 3, 4]
        let shape = TensorShape::vec(4).unwrap();
        let prog = extract_tensor(|t| {
            let x = t.input_f32(shape)?;
            let c = t.const_f32(shape, &[1.0, 2.0, 3.0, 4.0])?;
            let s = t.add(x, c)?;
            t.output(s)?;
            Ok(())
        })
        .expect("build");

        let out = execute_tensor(&prog, &[vec![10.0, 20.0, 30.0, 40.0]]).expect("exec");
        assert_eq!(out.as_slice(), &[11.0, 22.0, 33.0, 44.0]);
    }

    #[test]
    fn tensor_tracer_mul_then_add() {
        // f(a, b) = a * b + b sur vec(2).
        let shape = TensorShape::vec(2).unwrap();
        let prog = extract_tensor(|t| {
            let a = t.input_f32(shape)?;
            let b = t.input_f32(shape)?;
            let p = t.mul(a, b)?;
            let s = t.add(p, b)?;
            t.output(s)?;
            Ok(())
        })
        .expect("build");

        let out = execute_tensor(&prog, &[vec![2.0, 3.0], vec![5.0, 7.0]]).expect("exec");
        // a*b + b = [2*5+5, 3*7+7] = [15, 28]
        assert_eq!(out.as_slice(), &[15.0, 28.0]);
    }

    #[test]
    fn tensor_tracer_shape_mismatch_rejected() {
        let s4 = TensorShape::vec(4).unwrap();
        let s3 = TensorShape::vec(3).unwrap();
        let result = extract_tensor(|t| {
            let a = t.input_f32(s4)?;
            let b = t.input_f32(s3)?;
            let _ = t.add(a, b)?; // doit Err(ShapeMismatch)
            Ok(())
        });
        assert!(matches!(result, Err(TensorExtractError::ShapeMismatch)));
    }

    #[test]
    fn tensor_tracer_no_output_fails() {
        let result = extract_tensor(|t| {
            let _ = t.input_f32(TensorShape::vec(2).unwrap())?;
            // pas d'appel à t.output(...)
            Ok(())
        });
        assert!(matches!(result, Err(TensorExtractError::NoOutput)));
    }
}

}

// ----- Ω-2.0.5 — Macros ergonomiques -----
//
// Macros `macro_rules!` (zéro proc-macro, zéro dépendance) qui enveloppent
// les patterns courants pour réduire le boilerplate sans introduire de
// sémantique nouvelle.

/// Sucre syntaxique pour `crate::extract::extract(n, |inputs| { ... })`.
///
/// # Exemple
/// ```ignore
/// let prog = tracer_extract!(2, |inputs| inputs[0] + inputs[1]).unwrap();
/// ```
#[macro_export]
macro_rules! tracer_extract {
    ($n:expr, |$inputs:ident| $body:expr) => {
        $crate::extract::extract($n, |$inputs: ::std::vec::Vec<$crate::extract::Tracer>| $body)
    };
}

/// Construit un `Tracer` constant pour n'importe quelle valeur i64.
/// Délègue à `Tracer::const_i64_wide` (1 nœud pour i16, jusqu'à ~15 nœuds
/// hors i16).
#[macro_export]
macro_rules! tracer_const {
    ($v:expr) => {
        $crate::extract::Tracer::const_i64_wide($v as i64)
    };
}

// ----- Ω-2.0.8 — Extraction directe vers `meta::Term` -----

/// Extrait une closure Rust en `meta::Term` en un seul pas.
/// Équivaut à `embed_program(extract(n_inputs, f)?)`.
pub fn extract_to_term<F>(n_inputs: u8, f: F) -> Result<crate::meta::Term, ExtractError>
where
    F: FnOnce(Vec<Tracer>) -> Tracer,
{
    let prog = extract(n_inputs, f)?;
    Ok(crate::meta::embed_program(&prog))
}

#[cfg(test)]
mod fold_tests {
    use super::*;
    use crate::kasm::{execute, Program};

    fn exec_one(p: &Program, args: &[i64]) -> Vec<u8> {
        let bytes: Vec<u8> = args.iter().flat_map(|v| v.to_le_bytes()).collect();
        execute(p, &bytes).expect("kasm execute")
    }

    fn read_one_i64(out: &[u8]) -> i64 {
        i64::from_le_bytes(out[..8].try_into().unwrap())
    }

    #[test]
    fn macro_extract_simple() {
        let prog = tracer_extract!(2, |inputs| inputs[0] + inputs[1]).unwrap();
        for (a, b) in [(0i64, 0), (1, 2), (-3, 7), (1_000, 2_000)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a + b);
        }
    }

    #[test]
    fn macro_const_wide_works_with_large_value() {
        let target: i64 = 0x1234_5678_9ABC_DEF0_u64 as i64;
        let prog = extract(1, |inputs| inputs[0] + tracer_const!(target)).unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), target);
    }

    #[test]
    fn macro_extract_matches_direct_extract_hash() {
        let p_macro = tracer_extract!(1, |inputs| inputs[0] * 7 + 3).unwrap();
        let p_direct = extract(1, |inputs: Vec<Tracer>| inputs[0] * 7 + 3).unwrap();
        assert_eq!(
            p_macro.canonical_hash_hex().unwrap(),
            p_direct.canonical_hash_hex().unwrap(),
        );
    }

    #[test]
    fn extract_to_term_identity_function() {
        let term = extract_to_term(1, |inputs| inputs[0]).expect("extract_to_term");
        assert_ne!(term.hash(), [0u8; 32]);
    }

    #[test]
    fn extract_to_term_arithmetic() {
        let term = extract_to_term(2, |inputs| inputs[0] * inputs[1] + 7).expect("extract_to_term");
        assert_ne!(term.hash(), [0u8; 32]);
    }

    #[test]
    fn extract_to_term_distinct_closures() {
        let t_add = extract_to_term(2, |inputs| inputs[0] + inputs[1]).unwrap();
        let t_mul = extract_to_term(2, |inputs| inputs[0] * inputs[1]).unwrap();
        assert_ne!(t_add.hash(), t_mul.hash());
    }

    #[test]
    fn extract_to_term_deterministic() {
        let t1 = extract_to_term(1, |inputs| inputs[0] * 7 + 3).unwrap();
        let t2 = extract_to_term(1, |inputs| inputs[0] * 7 + 3).unwrap();
        assert_eq!(t1.hash(), t2.hash());
    }
}

}
