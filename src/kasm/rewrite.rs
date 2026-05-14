//! Wave 1a (Phase Π.3, 2026-05-02) — Mathematica-style rewrite rules
//! pour KASM.
//!
//! **Origine** : Mathematica `f[x_] := ...`. Le moteur cherche un
//! pattern dans l'AST, le remplace par un nouveau, et réapplique
//! récursivement jusqu'à fixpoint. Les patterns supportent :
//!
//! - `Any` — wildcard, match any node
//! - `Op(op)` — match exact opcode (ignore operands)
//! - `Literal(value)` — match exact `Op::ConstI64` with value
//! - `OpWith { op, a, b, imm }` — match opcode + operand patterns
//!
//! **Différence avec l'optimizer existant** : le optimizer fait des
//! rewrites hand-coded au cas par cas (`simplify_add(0, x) → x`).
//! Cette infrastructure permet de **déclarer** une règle
//! `rewrite!(Add(0, x) => x)` et la voir s'appliquer automatiquement
//! sur tout le DAG.
//!
//! **ROI Forge** : avec 60+ opcodes (v0.x + v1.0 + v1.1 Vec), il y
//! a beaucoup d'identités évidentes (`x + 0 = x`, `x * 1 = x`,
//! `VReverse(VReverse(v)) = v`, `VLen(VRange(n)) = n`, etc.) qui
//! ne sont pas toutes dans l'optimizer hand-coded. Un rewrite rule
//! engine déclaratif les capture toutes uniformément + permet aux
//! futures Φ.μ.3 atomes d'auto-publier des règles.
//!
//! **Wave 1a — minimal viable** : pattern matcher + rule applicator
//! + seed library de 8 règles. Fixpoint iteration limited à 16
//! passes (anti-runaway).

use super::types::{Node, Op, Ty};
use super::program::Program;

/// Pattern langage minimal — matche un sous-arbre KASM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// Wildcard — match any node.
    Any,
    /// Match exact opcode + operand patterns.
    /// `a` and `b` are sub-patterns matching `node.a` / `node.b` slots.
    /// `imm` matches the immediate field if `Some`.
    Op {
        op: Op,
        a: Box<Pattern>,
        b: Box<Pattern>,
        imm: Option<i16>,
    },
    /// Match `Op::ConstI64` with the given value as `imm`.
    LiteralI64(i64),
    /// Capture the matched node's slot index for use in `Replace`.
    /// Multiple captures with the same name must match the same slot.
    Capture(&'static str),
}

/// Replace template — décrit le sous-arbre de remplacement.
#[derive(Clone, Debug)]
pub enum Replace {
    /// Reuse the slot bound by this `Capture` name in the pattern.
    Slot(&'static str),
    /// Emit a new `Op::ConstI64` node with this value.
    LiteralI64(i64),
    /// Emit a new node, with sub-replacements for `a` and `b`.
    Op {
        op: Op,
        ty: Ty,
        a: Box<Replace>,
        b: Box<Replace>,
        imm: i16,
    },
}

/// A rewrite rule = (pattern, replace template).
pub struct Rewrite {
    pub name: &'static str,
    pub pattern: Pattern,
    pub replace: Replace,
}

/// Result of a single application — Some(new program) or None if no rule
/// matched.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// Number of rewrites applied during this fixpoint pass.
    pub rewrites_applied: usize,
    /// Names of rules that fired (in order).
    pub fired_rules: Vec<&'static str>,
}

/// Fixpoint iteration cap — safety against runaway rules.
const FIXPOINT_MAX: usize = 16;

/// Apply rules to a program until fixpoint (no more rewrites possible)
/// or `FIXPOINT_MAX` passes reached. Returns the rewritten program +
/// stats.
///
/// **Wave 1a minimal** : the rewriter operates on a flat node list,
/// scanning each node once per pass. A more sophisticated bottom-up
/// rewriter (with subtree memoization) is a future optimization.
pub fn rewrite_program(
    program: &Program,
    rules: &[Rewrite],
) -> (Program, ApplyOutcome) {
    let mut nodes: Vec<Node> = program.nodes().to_vec();
    let mut outcome = ApplyOutcome {
        rewrites_applied: 0,
        fired_rules: Vec::new(),
    };

    for _pass in 0..FIXPOINT_MAX {
        let pass_count_before = outcome.rewrites_applied;
        for i in 0..nodes.len() {
            for rule in rules {
                if let Some(new_node) = try_apply(&nodes, i, rule) {
                    nodes[i] = new_node;
                    outcome.rewrites_applied += 1;
                    outcome.fired_rules.push(rule.name);
                    // After a hit, restart this index — the new node
                    // might match another rule. Cap is fixpoint pass.
                    break;
                }
            }
        }
        if outcome.rewrites_applied == pass_count_before {
            break; // No rules fired this pass — fixpoint reached.
        }
    }

    let new_prog = Program::new(
        program.target(),
        program.inputs(),
        program.outputs(),
        program.fuel(),
        nodes,
    )
    .unwrap_or_else(|_| program.clone());
    (new_prog, outcome)
}

/// Try to apply a single rule to the node at `idx`. Returns the
/// rewritten node if the pattern matches, else `None`.
fn try_apply(nodes: &[Node], idx: usize, rule: &Rewrite) -> Option<Node> {
    let mut env = MatchEnv::default();
    if !match_node(nodes, idx, &rule.pattern, &mut env) {
        return None;
    }
    Some(emit_replace(nodes, idx, &rule.replace, &env))
}

/// Captures bound during a match — name → slot index.
#[derive(Default)]
struct MatchEnv {
    captures: Vec<(&'static str, u16)>,
}

impl MatchEnv {
    fn bind(&mut self, name: &'static str, slot: u16) -> bool {
        if let Some(&(_, prev)) = self.captures.iter().find(|(n, _)| *n == name) {
            // Re-binding — must be the same slot (linear capture).
            return prev == slot;
        }
        self.captures.push((name, slot));
        true
    }

    fn get(&self, name: &str) -> Option<u16> {
        self.captures.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
    }
}

fn match_node(nodes: &[Node], idx: usize, pat: &Pattern, env: &mut MatchEnv) -> bool {
    let node = match nodes.get(idx) {
        Some(n) => *n,
        None => return false,
    };
    match pat {
        Pattern::Any => true,
        Pattern::Capture(name) => env.bind(name, idx as u16),
        Pattern::LiteralI64(v) => node.op == Op::ConstI64 && node.imm as i64 == *v,
        Pattern::Op { op, a, b, imm } => {
            if node.op != *op {
                return false;
            }
            if let Some(want_imm) = imm {
                if node.imm != *want_imm {
                    return false;
                }
            }
            match_node(nodes, node.a as usize, a, env)
                && match_node(nodes, node.b as usize, b, env)
        }
    }
}

fn emit_replace(nodes: &[Node], orig_idx: usize, repl: &Replace, env: &MatchEnv) -> Node {
    match repl {
        Replace::Slot(name) => {
            // For Wave 1a minimal, we just return the captured node
            // verbatim. A full rewriter would emit a new node referencing
            // the captured slot, but that requires graph surgery beyond
            // the per-node scope of this pass.
            let slot = env.get(name).unwrap_or(orig_idx as u16) as usize;
            nodes.get(slot).copied().unwrap_or(nodes[orig_idx])
        }
        Replace::LiteralI64(v) => {
            // Emit Op::ConstI64 with this value, type defaulting to I64.
            // (Wave 6 cut : `let orig = nodes[orig_idx]` retiré — pas
            // d'autres champs structurel à hériter, le `..orig` était
            // un dead-code propagator.)
            Node {
                op: Op::ConstI64,
                ty: Ty::I64,
                a: 0,
                b: 0,
                imm: *v as i16,
            }
        }
        Replace::Op { op, ty, a: _, b: _, imm } => {
            // For Wave 1a minimal : we don't recursively emit sub-trees
            // (would require allocating new node slots and remapping
            // references). We just rewrite the op/ty/imm in place.
            // Sub-patterns in `Replace::Op` are reserved for a future
            // wave; today we keep the original `a` and `b` slot refs.
            let orig = nodes[orig_idx];
            Node {
                op: *op,
                ty: *ty,
                a: orig.a,
                b: orig.b,
                imm: *imm,
            }
        }
    }
}

/// Seed library — 8 obvious identities. Wave 1a starting set, more
/// can be added as the synthesizer surfaces patterns.
pub fn seed_rewrites() -> Vec<Rewrite> {
    use Pattern as P;
    use Replace as R;

    vec![
        // x + 0 = x  (rewrite to a no-op identity by replacing the Add
        // node's `op` to a "passthrough" — concretely : the Wave 1a
        // minimal rewriter can't restructure refs, so this fires only
        // when the whole expression is `Add(x, 0)` and the parent uses
        // the result. We mark the Add as a comptime no-op.
        Rewrite {
            name: "add_zero_right",
            pattern: P::Op {
                op: Op::AddI64,
                a: Box::new(P::Capture("x")),
                b: Box::new(P::LiteralI64(0)),
                imm: None,
            },
            replace: R::Slot("x"),
        },
        Rewrite {
            name: "add_zero_left",
            pattern: P::Op {
                op: Op::AddI64,
                a: Box::new(P::LiteralI64(0)),
                b: Box::new(P::Capture("x")),
                imm: None,
            },
            replace: R::Slot("x"),
        },
        // x * 1 = x
        Rewrite {
            name: "mul_one_right",
            pattern: P::Op {
                op: Op::MulI64,
                a: Box::new(P::Capture("x")),
                b: Box::new(P::LiteralI64(1)),
                imm: None,
            },
            replace: R::Slot("x"),
        },
        Rewrite {
            name: "mul_one_left",
            pattern: P::Op {
                op: Op::MulI64,
                a: Box::new(P::LiteralI64(1)),
                b: Box::new(P::Capture("x")),
                imm: None,
            },
            replace: R::Slot("x"),
        },
        // x * 0 = 0
        Rewrite {
            name: "mul_zero_right",
            pattern: P::Op {
                op: Op::MulI64,
                a: Box::new(P::Any),
                b: Box::new(P::LiteralI64(0)),
                imm: None,
            },
            replace: R::LiteralI64(0),
        },
        Rewrite {
            name: "mul_zero_left",
            pattern: P::Op {
                op: Op::MulI64,
                a: Box::new(P::LiteralI64(0)),
                b: Box::new(P::Any),
                imm: None,
            },
            replace: R::LiteralI64(0),
        },
        // x - 0 = x
        Rewrite {
            name: "sub_zero_right",
            pattern: P::Op {
                op: Op::SubI64,
                a: Box::new(P::Capture("x")),
                b: Box::new(P::LiteralI64(0)),
                imm: None,
            },
            replace: R::Slot("x"),
        },
        // x ^ x = 0  (caller must use same slot for both — linear capture
        // on "x" in BOTH a and b enforces this).
        Rewrite {
            name: "xor_self_zero",
            pattern: P::Op {
                op: Op::BitXorI64,
                a: Box::new(P::Capture("x")),
                b: Box::new(P::Capture("x")),
                imm: None,
            },
            replace: R::LiteralI64(0),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};

    fn build_add_zero_program() -> Program {
        // Program: input(0) + 0 → output
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),       // 0: input
                Node::const_i64(0),   // 1: const 0
                Node::add(0, 1),      // 2: input + 0
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn rewrite_add_zero_fires() {
        let prog = build_add_zero_program();
        let rules = seed_rewrites();
        let (_new_prog, outcome) = rewrite_program(&prog, &rules);
        assert!(outcome.rewrites_applied > 0,
            "add_zero rule should fire on x + 0");
        assert!(
            outcome.fired_rules.iter().any(|n| n.starts_with("add_zero")),
            "fired rules : {:?}", outcome.fired_rules
        );
    }

    #[test]
    fn rewrite_mul_zero_fires() {
        // x * 0 = 0
        let prog = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let (_new_prog, outcome) = rewrite_program(&prog, &seed_rewrites());
        assert!(outcome.fired_rules.iter().any(|n| n.starts_with("mul_zero")));
    }

    #[test]
    fn rewrite_no_match_no_fire() {
        // Program with no obvious identity : input * 7 + 3
        let prog = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::const_i64(3),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        ).unwrap();
        let (_new_prog, outcome) = rewrite_program(&prog, &seed_rewrites());
        assert_eq!(outcome.rewrites_applied, 0,
            "no rule should fire on (input*7)+3");
    }

    #[test]
    fn rewrite_xor_self_fires() {
        // Op::BitXorI64(x, x) — but we need both refs to point to the
        // same node for the linear capture to bind.
        let prog = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::bit_xor(0, 0),  // input ^ input
                Node::output(1, Ty::I64),
            ],
        ).unwrap();
        let (_new_prog, outcome) = rewrite_program(&prog, &seed_rewrites());
        assert!(outcome.fired_rules.iter().any(|n| *n == "xor_self_zero"),
            "xor_self_zero should fire on x^x");
    }

    #[test]
    fn rewrite_fixpoint_terminates() {
        // The fixpoint cap (FIXPOINT_MAX) must guard against runaway
        // rule application. With our seed rules (all reductive), no
        // program should hit the cap, but verify with a simple case.
        let prog = build_add_zero_program();
        let (_new_prog, outcome) = rewrite_program(&prog, &seed_rewrites());
        assert!(outcome.rewrites_applied < FIXPOINT_MAX * 100,
            "rewrite should not blow up");
    }
}
