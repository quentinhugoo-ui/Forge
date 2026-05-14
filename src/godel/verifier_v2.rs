//! Omega-7.0.3 first mile -- Verifier v2.
//!
//! Etend le pouvoir d'expression du verifier Godel-machine pour accepter
//! des `ProgramSubstitution { from: Hash, to: Hash }` en plus des
//! `ConfigPatch` historiques. Vit en parallele de `super::verifier` (Codex)
//! sans le modifier -- option B documentee dans
//! `docs/OMEGA_OMEGA70_AGENT_ROADMAP.md`.
//!
//! Semantique de `ProgramSubstitution` :
//!  - `from`, `to` doivent etre chargeables depuis le store de la node.
//!  - L'invariant d'equivalence semantique entre `from` et `to` est la
//!    *responsabilite de l'agent qui a produit la substitution* (pas
//!    re-verifie ici -- c'est un compromis de scope first mile).
//!  - Le verifier_v2 garantit l'integrite referentielle : pas d'oubli,
//!    pas de hash bidon.
//!
//! Pour des semantiques fortes (re-executer sur des inputs sample, verifier
//! preuve Omega-4 d'equivalence, etc.) voir Omega-7.0.3.1 reporte.

use std::collections::BTreeMap;

use crate::{Hash, MonsterNode};

/// Rewrite v2. Une variante mirror du `ConfigPatch` pour rester compatible
/// avec les usages existants ; une variante nouvelle `ProgramSubstitution`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteV2 {
    /// Patch de configuration (mirror de `verifier::RewriteKind::ConfigPatch`).
    ConfigPatch(BTreeMap<&'static str, i64>),
    /// Substitution d'un programme entier par un autre. `from` et `to`
    /// sont les hashes content-addressed des programmes. L'equivalence
    /// semantique est *presumee* (responsabilite de l'agent producteur).
    ProgramSubstitution { from: Hash, to: Hash },
}

/// Resultat de la verification d'une rewrite v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcomeV2 {
    /// Tous les checks referentiels passent.
    Accept,
    /// Au moins un check a echoue -- `reasons` listent les motifs.
    Reject { reasons: Vec<String> },
}

impl VerificationOutcomeV2 {
    pub fn is_accept(&self) -> bool {
        matches!(self, VerificationOutcomeV2::Accept)
    }
    pub fn reasons(&self) -> Option<&[String]> {
        if let VerificationOutcomeV2::Reject { reasons } = self {
            Some(reasons)
        } else {
            None
        }
    }
}

/// Verifie une rewrite v2 contre une node.
///
/// Pour `ConfigPatch` : check minimal -- toutes les cles non-vides, valeurs
/// dans la plage `[1, 1_000_000_000]` (mirror du applicator existant).
///
/// Pour `ProgramSubstitution` : check referentiel -- `from` et `to` doivent
/// etre chargeables depuis `node.store()`.
pub fn verify_v2(rewrite: &RewriteV2, node: &MonsterNode) -> VerificationOutcomeV2 {
    let mut reasons = Vec::new();
    match rewrite {
        RewriteV2::ConfigPatch(map) => {
            if map.is_empty() {
                reasons.push("empty config patch".to_string());
            }
            for (k, v) in map {
                if k.is_empty() {
                    reasons.push("empty key in config patch".to_string());
                }
                if !(1..=1_000_000_000).contains(v) {
                    reasons.push(format!("value {v} for key {k} out of allowed range [1, 1e9]"));
                }
            }
        }
        RewriteV2::ProgramSubstitution { from, to } => {
            if from == to {
                reasons.push("trivial substitution: from == to".to_string());
            }
            if node.store().load(from).is_none() {
                reasons.push(format!("source program {from:?} not in store"));
            }
            if node.store().load(to).is_none() {
                reasons.push(format!("target program {to:?} not in store"));
            }
        }
    }
    if reasons.is_empty() {
        VerificationOutcomeV2::Accept
    } else {
        VerificationOutcomeV2::Reject { reasons }
    }
}

// ---------------------------------------------------------------------------
// Ω-7.0.3.1 — Re-vérification sémantique sample-based
// ---------------------------------------------------------------------------

/// Politique de re-vérification sémantique pour ProgramSubstitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticPolicy {
    /// Pas de re-vérification — équivalence présumée. C'est le comportement
    /// de `verify_v2` original.
    Trust,
    /// Re-exécute les deux programmes sur N inputs déterministes et exige
    /// que tous les outputs concordent. Plus coûteux, mais détecte les
    /// substitutions qui ne préservent pas la sémantique.
    SampleBased { samples: usize },
}

/// Verifier v2 + re-vérification sémantique. Pour ConfigPatch, identique à
/// `verify_v2`. Pour ProgramSubstitution avec policy SampleBased, charge
/// les deux programmes, les exécute sur `samples` jeux d'inputs déterministes,
/// et compare les outputs.
pub fn verify_v2_with_policy(
    rewrite: &RewriteV2,
    node: &MonsterNode,
    policy: SemanticPolicy,
) -> VerificationOutcomeV2 {
    // 1. Vérification référentielle (mirror verify_v2).
    let base = verify_v2(rewrite, node);
    if !base.is_accept() {
        return base;
    }

    // 2. Re-vérification sémantique uniquement pour ProgramSubstitution + SampleBased.
    let RewriteV2::ProgramSubstitution { from, to } = rewrite else {
        return VerificationOutcomeV2::Accept;
    };
    let SemanticPolicy::SampleBased { samples } = policy else {
        return VerificationOutcomeV2::Accept;
    };

    // Charge les deux programmes.
    let from_bytes = match node.store().load(from) {
        Some(b) => b,
        None => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("source program {from:?} not loadable")],
            };
        }
    };
    let to_bytes = match node.store().load(to) {
        Some(b) => b,
        None => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("target program {to:?} not loadable")],
            };
        }
    };

    let from_p = match crate::kasm::Program::from_bytes(&from_bytes) {
        Ok(p) => p,
        Err(e) => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("source program failed to parse: {e}")],
            };
        }
    };
    let to_p = match crate::kasm::Program::from_bytes(&to_bytes) {
        Ok(p) => p,
        Err(e) => {
            return VerificationOutcomeV2::Reject {
                reasons: vec![format!("target program failed to parse: {e}")],
            };
        }
    };

    // Vérifie que les deux programmes ont le même profil IO.
    if from_p.inputs() != to_p.inputs() {
        return VerificationOutcomeV2::Reject {
            reasons: vec![format!(
                "input arity mismatch: from has {}, to has {}",
                from_p.inputs(), to_p.inputs(),
            )],
        };
    }
    if from_p.outputs() != to_p.outputs() {
        return VerificationOutcomeV2::Reject {
            reasons: vec![format!(
                "output arity mismatch: from has {}, to has {}",
                from_p.outputs(), to_p.outputs(),
            )],
        };
    }

    // Génère `samples` jeux d'inputs déterministes et compare les outputs.
    let mut reasons: Vec<String> = Vec::new();
    for sample_idx in 0..samples {
        let inputs = generate_sample_inputs(from_p.inputs() as usize, sample_idx as u64);
        let from_out = match crate::kasm::execute(&from_p, &inputs) {
            Ok(b) => b,
            Err(e) => {
                reasons.push(format!("from execute on sample {sample_idx} failed: {e}"));
                continue;
            }
        };
        let to_out = match crate::kasm::execute(&to_p, &inputs) {
            Ok(b) => b,
            Err(e) => {
                reasons.push(format!("to execute on sample {sample_idx} failed: {e}"));
                continue;
            }
        };
        if from_out != to_out {
            reasons.push(format!(
                "output mismatch on sample {sample_idx}: from={:?} to={:?}",
                from_out, to_out,
            ));
        }
    }

    if reasons.is_empty() {
        VerificationOutcomeV2::Accept
    } else {
        VerificationOutcomeV2::Reject { reasons }
    }
}

/// Génère un jeu d'inputs déterministe pour un sample donné. Mélange
/// quelques valeurs corner (0, 1, -1, MIN, MAX) avec des valeurs hashées.
fn generate_sample_inputs(n_inputs: usize, sample_idx: u64) -> Vec<u8> {
    let corners: [i64; 5] = [0, 1, -1, i64::MIN, i64::MAX];
    let mut bytes = Vec::with_capacity(n_inputs * 8);
    for slot in 0..n_inputs {
        let v: i64 = if (sample_idx as usize) < corners.len() {
            corners[sample_idx as usize]
                .wrapping_add((slot as i64).wrapping_mul(17))
        } else {
            // Hash deterministe.
            let mut x = (sample_idx).wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ ((slot as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
            (x ^ (x >> 31)) as i64
        };
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryGovernor, Store};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fresh_path(tag: &str) -> PathBuf {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut p = crate::fresh_tmp_path("scan-verifier-v2", tag);
        p.set_file_name(format!(
            "{}-{seq}",
            p.file_name().unwrap().to_str().unwrap()
        ));
        p
    }

    fn fresh_node(tag: &str) -> MonsterNode {
        MonsterNode::new(
            Store::open(fresh_path(tag)).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        )
    }

    fn store_program_in_node(node: &MonsterNode, p: &crate::kasm::Program) -> Hash {
        node.store().store(p.bytes()).expect("store write")
    }

    fn affine_program() -> crate::kasm::Program {
        use crate::kasm::{Node, Program, Target, Ty};
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

    fn other_program() -> crate::kasm::Program {
        use crate::kasm::{Node, Program, Target, Ty};
        Program::new(
            Target::Cpu, 1, 1, 4,
            vec![
                Node::input(0),
                Node::output(0, Ty::I64),
            ],
        ).unwrap()
    }

    #[test]
    fn verify_v2_accepts_known_program_substitution() {
        let node = fresh_node("accept");
        let h_from = store_program_in_node(&node, &affine_program());
        let h_to = store_program_in_node(&node, &other_program());
        let rw = RewriteV2::ProgramSubstitution { from: h_from, to: h_to };
        let r = verify_v2(&rw, &node);
        assert!(r.is_accept(), "got {r:?}");
    }

    #[test]
    fn verify_v2_rejects_missing_target() {
        let node = fresh_node("missing-to");
        let h_from = store_program_in_node(&node, &affine_program());
        let h_to = Hash::for_blob(b"not in store");
        let rw = RewriteV2::ProgramSubstitution { from: h_from, to: h_to };
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("target")));
    }

    #[test]
    fn verify_v2_rejects_missing_source() {
        let node = fresh_node("missing-from");
        let h_to = store_program_in_node(&node, &other_program());
        let h_from = Hash::for_blob(b"not in store either");
        let rw = RewriteV2::ProgramSubstitution { from: h_from, to: h_to };
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("source")));
    }

    #[test]
    fn verify_v2_rejects_trivial_substitution() {
        let node = fresh_node("trivial");
        let h = store_program_in_node(&node, &affine_program());
        let rw = RewriteV2::ProgramSubstitution { from: h, to: h };
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("trivial")));
    }

    #[test]
    fn verify_v2_config_patch_accepts_valid() {
        let node = fresh_node("config-ok");
        let mut map = BTreeMap::new();
        map.insert("beam_width", 256);
        map.insert("max_nodes", 20);
        let rw = RewriteV2::ConfigPatch(map);
        let r = verify_v2(&rw, &node);
        assert!(r.is_accept(), "got {r:?}");
    }

    #[test]
    fn verify_v2_config_patch_rejects_out_of_range() {
        let node = fresh_node("config-oor");
        let mut map = BTreeMap::new();
        map.insert("beam_width", 0);
        let rw = RewriteV2::ConfigPatch(map);
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
    }

    #[test]
    fn verify_v2_config_patch_rejects_empty() {
        let node = fresh_node("config-empty");
        let rw = RewriteV2::ConfigPatch(BTreeMap::new());
        let r = verify_v2(&rw, &node);
        assert!(!r.is_accept());
    }

    #[test]
    fn agent_candidates_become_rewrites_v2() {
        // Cross-cap : agent symbolique propose des programmes,
        // candidates_as_rewrites_v2 les transforme en RewriteV2.
        use crate::agent::SymbolicAgent;
        use crate::kasm::{Node, Program, Target, Ty};

        let p = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());

        let rewrites = crate::agent::symbolic::candidates_as_rewrites_v2(&p, &candidates);
        assert_eq!(rewrites.len(), candidates.len());

        for rw in &rewrites {
            assert!(matches!(rw, RewriteV2::ProgramSubstitution { .. }));
        }
    }

    #[test]
    fn cross_cap_agent_proposes_then_verifier_v2_accepts() {
        // Pipeline complet : agent -> rewrites_v2 -> verify_v2 doit accepter
        // tant que les programmes sont bien dans le store.
        use crate::agent::SymbolicAgent;
        use crate::kasm::{Node, Program, Target, Ty};

        let node = fresh_node("cross-cap");
        let p = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();

        // Stocke l'input.
        let _from_hash = store_program_in_node(&node, &p);

        let agent = SymbolicAgent::new();
        let candidates = agent.propose_rewrites(&p);
        assert!(!candidates.is_empty());

        // Stocke les programmes candidats aussi.
        for c in &candidates {
            let _ = store_program_in_node(&node, &c.program);
        }

        let rewrites = crate::agent::symbolic::candidates_as_rewrites_v2(&p, &candidates);
        // Au moins une rewrite doit etre Accept (le filtre du store assure
        // que les hashes existent).
        let any_accept = rewrites.iter().any(|rw| verify_v2(rw, &node).is_accept());
        assert!(any_accept, "au moins une rewrite doit etre acceptee");
    }

    // ----- Ω-7.0.3.1 — Re-vérification sémantique sample-based -----

    fn equivalent_program_pair() -> (crate::kasm::Program, crate::kasm::Program) {
        use crate::kasm::{Node, Program, Target, Ty};
        // f(x) = x + 0 (5 nodes)
        let p_with_add_zero = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        // f(x) = x (canonicalisé)
        let p_canonical = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        (p_with_add_zero, p_canonical)
    }

    fn divergent_program_pair() -> (crate::kasm::Program, crate::kasm::Program) {
        use crate::kasm::{Node, Program, Target, Ty};
        // f(x) = x
        let p_id = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        // f(x) = x + 1 — sémantique différente !
        let p_plus_one = Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        (p_id, p_plus_one)
    }

    #[test]
    fn semantic_policy_trust_skips_re_verification() {
        let node = fresh_node("trust");
        let (a, b) = divergent_program_pair();
        let h_a = node.store().store(a.bytes()).unwrap();
        let h_b = node.store().store(b.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h_a, to: h_b };
        // Trust = pas de check sémantique → Accept même si divergent.
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::Trust);
        assert!(r.is_accept(), "Trust ne re-vérifie pas, doit accepter");
    }

    #[test]
    fn semantic_policy_sample_based_accepts_equivalent() {
        let node = fresh_node("sample-equiv");
        let (a, b) = equivalent_program_pair();
        let h_a = node.store().store(a.bytes()).unwrap();
        let h_b = node.store().store(b.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h_a, to: h_b };
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 8 });
        assert!(r.is_accept(), "programmes équivalents doivent passer; got {r:?}");
    }

    #[test]
    fn semantic_policy_sample_based_rejects_divergent() {
        let node = fresh_node("sample-diverg");
        let (a, b) = divergent_program_pair();
        let h_a = node.store().store(a.bytes()).unwrap();
        let h_b = node.store().store(b.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h_a, to: h_b };
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 8 });
        assert!(!r.is_accept());
        let reasons = r.reasons().unwrap();
        assert!(reasons.iter().any(|s| s.contains("output mismatch")));
    }

    #[test]
    fn semantic_policy_rejects_input_arity_mismatch() {
        use crate::kasm::{Node, Program, Target, Ty};
        let node = fresh_node("arity");
        let p1 = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        let p2 = Program::new(
            Target::Cpu, 2, 1, 4,
            vec![Node::input(0), Node::input(1), Node::output(0, Ty::I64)],
        ).unwrap();
        let h1 = node.store().store(p1.bytes()).unwrap();
        let h2 = node.store().store(p2.bytes()).unwrap();
        let rw = RewriteV2::ProgramSubstitution { from: h1, to: h2 };
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 4 });
        assert!(!r.is_accept());
        assert!(r.reasons().unwrap().iter().any(|s| s.contains("arity mismatch")));
    }

    #[test]
    fn semantic_policy_config_patch_unaffected() {
        let node = fresh_node("config-policy");
        let mut map = std::collections::BTreeMap::new();
        map.insert("beam_width", 100);
        let rw = RewriteV2::ConfigPatch(map);
        // Policy SampleBased ne s'applique pas à ConfigPatch.
        let r = verify_v2_with_policy(&rw, &node, SemanticPolicy::SampleBased { samples: 8 });
        assert!(r.is_accept());
    }
}
