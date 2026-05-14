//! Ω-5.6 — Applicator v2 : applique des `RewriteV2` y compris ProgramSubstitution.

use crate::godel::verifier_v2::{verify_v2, RewriteV2, VerificationOutcomeV2};
use crate::{Hash, MonsterNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicatorV2Error {
    /// Verification a rejeté la rewrite.
    Rejected { reasons: Vec<String> },
    /// Le programme `from` n'est pas chargeable (déjà checké par verify, mais defensive).
    SourceMissing(Hash),
    /// Le programme `to` n'est pas chargeable.
    TargetMissing(Hash),
}

/// Trace d'une application : avant/après pour permettre rollback.
#[derive(Debug, Clone)]
pub struct ApplicationTrace {
    pub rewrite: RewriteV2,
    pub previous_active: Option<Hash>,
}

/// Applicator v2. Maintient l'ensemble des programmes "actifs" — un set
/// de hashes qui représente les Programs que la node considère comme sa
/// version courante. Une ProgramSubstitution remplace `from` par `to` dans
/// ce set ; un rollback restaure.
///
/// L'état "active programs" est une abstraction interne au verifier de
/// l'agent — pas un endroit où la node modifie son store. Le store reste
/// append-only.
#[derive(Debug, Default)]
pub struct ApplicatorV2 {
    active_programs: std::collections::BTreeSet<Hash>,
}

impl ApplicatorV2 {
    pub fn new() -> Self {
        Self { active_programs: std::collections::BTreeSet::new() }
    }

    /// Initialise l'ensemble actif avec un programme.
    pub fn activate(&mut self, hash: Hash) {
        self.active_programs.insert(hash);
    }

    /// `is_active(hash)` retourne true si le hash est dans le set actif.
    pub fn is_active(&self, hash: &Hash) -> bool {
        self.active_programs.contains(hash)
    }

    /// Applique une RewriteV2. Pour ProgramSubstitution :
    ///  1. verify_v2 doit Accept.
    ///  2. `from` doit être actif (sinon Reject).
    ///  3. Remplace `from` par `to` dans active_programs.
    ///  4. Retourne ApplicationTrace pour rollback.
    pub fn apply(
        &mut self,
        rewrite: RewriteV2,
        node: &MonsterNode,
    ) -> Result<ApplicationTrace, ApplicatorV2Error> {
        match verify_v2(&rewrite, node) {
            VerificationOutcomeV2::Accept => {}
            VerificationOutcomeV2::Reject { reasons } => {
                return Err(ApplicatorV2Error::Rejected { reasons });
            }
        }
        match &rewrite {
            RewriteV2::ConfigPatch(_) => {
                // Pour ConfigPatch, on délègue conceptuellement à l'applicator v1.
                // Ici on enregistre juste la trace ; l'effet config est externe.
                Ok(ApplicationTrace { rewrite, previous_active: None })
            }
            RewriteV2::ProgramSubstitution { from, to } => {
                let from = *from;
                let to = *to;
                if !self.active_programs.contains(&from) {
                    return Err(ApplicatorV2Error::Rejected {
                        reasons: vec![format!("source {from:?} not in active set")],
                    });
                }
                self.active_programs.remove(&from);
                self.active_programs.insert(to);
                Ok(ApplicationTrace {
                    rewrite,
                    previous_active: Some(from),
                })
            }
        }
    }

    /// Rollback d'une trace : restaure l'état avant `apply`.
    pub fn rollback(&mut self, trace: &ApplicationTrace) {
        match &trace.rewrite {
            RewriteV2::ConfigPatch(_) => {}
            RewriteV2::ProgramSubstitution { from, to } => {
                self.active_programs.remove(to);
                if let Some(prev) = trace.previous_active {
                    debug_assert_eq!(prev, *from);
                    self.active_programs.insert(prev);
                }
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_programs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};
    use crate::{MemoryGovernor, Store};
    use std::path::PathBuf;
    

    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("scan-applicator-v2", tag)
    }

    fn fresh_node(tag: &str) -> MonsterNode {
        MonsterNode::new(
            Store::open(fresh_path(tag)).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        )
    }

    fn store_program(node: &MonsterNode, p: &Program) -> Hash {
        node.store().store(p.bytes()).expect("store")
    }

    fn affine_program() -> Program {
        Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::const_i64(3),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        ).unwrap()
    }

    fn id_program() -> Program {
        Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap()
    }

    #[test]
    fn applicator_v2_starts_empty() {
        let app = ApplicatorV2::new();
        assert_eq!(app.active_count(), 0);
    }

    #[test]
    fn activate_adds_to_active_set() {
        let mut app = ApplicatorV2::new();
        let h = Hash::for_blob(b"x");
        app.activate(h);
        assert!(app.is_active(&h));
        assert_eq!(app.active_count(), 1);
    }

    #[test]
    fn apply_substitution_swaps_active() {
        let node = fresh_node("swap");
        let mut app = ApplicatorV2::new();
        let h_from = store_program(&node, &affine_program());
        let h_to = store_program(&node, &id_program());
        app.activate(h_from);
        let trace = app.apply(
            RewriteV2::ProgramSubstitution { from: h_from, to: h_to },
            &node,
        ).expect("accept");
        assert!(!app.is_active(&h_from));
        assert!(app.is_active(&h_to));
        assert_eq!(trace.previous_active, Some(h_from));
    }

    #[test]
    fn rollback_restores_active_set() {
        let node = fresh_node("rollback");
        let mut app = ApplicatorV2::new();
        let h_from = store_program(&node, &affine_program());
        let h_to = store_program(&node, &id_program());
        app.activate(h_from);
        let trace = app.apply(
            RewriteV2::ProgramSubstitution { from: h_from, to: h_to },
            &node,
        ).unwrap();
        app.rollback(&trace);
        assert!(app.is_active(&h_from));
        assert!(!app.is_active(&h_to));
    }

    #[test]
    fn apply_rejects_when_source_not_active() {
        let node = fresh_node("not-active");
        let mut app = ApplicatorV2::new();
        let h_from = store_program(&node, &affine_program());
        let h_to = store_program(&node, &id_program());
        // h_from N'EST PAS activé.
        let result = app.apply(
            RewriteV2::ProgramSubstitution { from: h_from, to: h_to },
            &node,
        );
        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicatorV2Error::Rejected { reasons } => {
                assert!(reasons.iter().any(|s| s.contains("not in active set")));
            }
            _ => panic!("expected Rejected"),
        }
    }

    #[test]
    fn apply_rejects_unverified_substitution() {
        let node = fresh_node("unverified");
        let mut app = ApplicatorV2::new();
        // from et to identiques → trivial, rejeté par verify_v2.
        let h = store_program(&node, &affine_program());
        app.activate(h);
        let result = app.apply(
            RewriteV2::ProgramSubstitution { from: h, to: h },
            &node,
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_handles_config_patch_no_active_change() {
        let node = fresh_node("config");
        let mut app = ApplicatorV2::new();
        let mut map = std::collections::BTreeMap::new();
        map.insert("beam_width", 100);
        let trace = app.apply(RewriteV2::ConfigPatch(map), &node).expect("accept");
        assert_eq!(app.active_count(), 0);
        assert!(matches!(trace.rewrite, RewriteV2::ConfigPatch(_)));
    }

    #[test]
    fn jour_zero_program_substitution_is_recorded_in_trace() {
        // Cross-cap : agent propose, applicator_v2 applique, trace conservée.
        use crate::agent::SymbolicAgent;
        let node = fresh_node("jour-zero-program");
        let p_input = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let h_input = store_program(&node, &p_input);
        let mut app = ApplicatorV2::new();
        app.activate(h_input);

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p_input);
        assert!(!candidates.is_empty());
        for c in &candidates {
            let _ = store_program(&node, &c.program);
        }
        let rewrites = crate::agent::symbolic::candidates_as_rewrites_v2(&p_input, &candidates);
        let mut applied = 0;
        for rw in rewrites {
            if app.apply(rw, &node).is_ok() {
                applied += 1;
                break; // Une seule substitution par session pour ce test.
            }
        }
        assert!(applied >= 1, "au moins une substitution doit être appliquée");
        assert_eq!(app.active_count(), 1, "set actif doit avoir taille 1 après swap");
    }
}
