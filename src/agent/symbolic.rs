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
