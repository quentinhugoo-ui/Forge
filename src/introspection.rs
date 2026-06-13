//! Î©-Î¦ â€” Ghost storage : introspection content-addressed.
//!
//! Î¦.Î¼.7 : aplati de `src/introspection/{mod,snapshot}.rs` Ã 
//! `src/introspection.rs` (le sous-dossier ne contenait qu'un fichier
//! rÃ©el + un mod.rs de 9 lignes â€” pure indirection).
//!
//! Module qui prÃ©pare le terrain pour Î©-Î¦.0..Î©-Î¦.4 (cf. Git history). Le
//! premier sous-cap est `LiveSnapshot` â€” capture content-addressed et
//! reproductible de l'Ã©tat logique d'une `MonsterNode`.
//!
//! ----- LiveSnapshot (Î©-Î¦.0) -----
//!
//! `MonsterNode::live_snapshot()` content-addressed.
//!
//! Capture lecture-seule de l'Ã©tat logique d'une `MonsterNode` : ensemble
//! de programmes chargÃ©s + oracles actifs + un compteur d'epoch monotone.
//! Le hash du snapshot est dÃ©terministe (sha256 sur la projection canonique
//! triÃ©e+dedupÃ©e), donc deux snapshots avec les mÃªmes contenus â†’ mÃªme hash.
//!
//! `validate(snap, node)` re-vÃ©rifie que chaque programme du snapshot est
//! toujours chargeable depuis le store de la node. Ne rÃ©-exÃ©cute pas, ne
//! mute pas â€” c'est un contrÃ´le d'intÃ©gritÃ© rÃ©fÃ©rentiel pur.
//!
//! ## Doctrine
//!
//! - Pure Rust + std + sha2. Aucune dÃ©pendance externe.
//! - Aucune manipulation mÃ©moire OS-spÃ©cifique.
//! - Lecture seule sur la `MonsterNode` â€” pas de mutation, pas de
//!   re-construction d'Ã©tat pendant le snapshot.
//! - Reconstructible : `validate(snap, node)` doit retourner le mÃªme
//!   nombre de programmes que `snap.programs.len()` si le store contient
//!   toujours tous les hashes â€” sinon, l'Ã©cart est observable.

use sha2::{Digest, Sha256};

use crate::godel::observer;
use crate::{Hash, MonsterNode};

/// Snapshot d'Ã©tat logique d'une `MonsterNode`.
///
/// Champs triÃ©s et dÃ©dupÃ©s Ã  la capture pour garantir l'invariance du hash
/// sous l'ordre d'observation (deux nodes avec les mÃªmes programmes
/// chargÃ©s produisent le mÃªme `snapshot_hash`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSnapshot {
    /// Hashes content-addressed des programmes chargÃ©s.
    pub programs: Vec<Hash>,
    /// Hashes des oracles actifs.
    pub oracles: Vec<Hash>,
    /// Compteur monotone â€” vient de `observer::ObserverFrame::epoch`.
    pub epoch: u64,
}

impl LiveSnapshot {
    /// Constructeur direct (utile pour les tests). Trie + dÃ©dupe en place
    /// pour prÃ©server l'invariant de canonicitÃ© du hash.
    pub fn new(mut programs: Vec<Hash>, mut oracles: Vec<Hash>, epoch: u64) -> Self {
        programs.sort();
        programs.dedup();
        oracles.sort();
        oracles.dedup();
        Self { programs, oracles, epoch }
    }
}

/// Capture l'Ã©tat logique d'une `MonsterNode` (lecture seule, non perturbant).
pub fn capture(node: &MonsterNode) -> LiveSnapshot {
    let frame = observer::capture(node);
    LiveSnapshot::new(frame.programs_loaded, frame.oracles_active, frame.epoch)
}

/// Hash sha-256 dÃ©terministe d'un snapshot. Deux snapshots avec les mÃªmes
/// `programs` (set), `oracles` (set) et `epoch` produisent le mÃªme hash.
pub fn snapshot_hash(snap: &LiveSnapshot) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"scan-omega-phi-0-snapshot-v1");
    // Programs : len prÃ©fixÃ©e + chaque hash en bytes triÃ©s.
    h.update((snap.programs.len() as u64).to_le_bytes());
    for hash in &snap.programs {
        h.update(hash.as_bytes());
    }
    // Oracles : idem.
    h.update((snap.oracles.len() as u64).to_le_bytes());
    for hash in &snap.oracles {
        h.update(hash.as_bytes());
    }
    h.update(snap.epoch.to_le_bytes());
    h.finalize().into()
}

/// RÃ©sultat de la validation d'un snapshot contre une node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotValidation {
    /// Nombre de programmes du snapshot effectivement chargeables depuis
    /// le store de la node au moment de la validation.
    pub programs_loaded: usize,
    /// Programmes du snapshot absents du store (rÃ©fÃ©rentiellement vides).
    pub programs_missing: Vec<Hash>,
}

impl SnapshotValidation {
    pub fn is_intact(&self) -> bool {
        self.programs_missing.is_empty()
    }
}

/// Valide un snapshot contre une `MonsterNode`. Pour chaque hash de
/// `snap.programs`, tente `node.store().load(hash)`. Ne rÃ©-exÃ©cute pas,
/// ne mute pas l'Ã©tat de la node.
///
/// Retourne le nombre de programmes valides + la liste des hashes
/// manquants. `snapshot.is_intact()` est `true` ssi tous les programmes
/// du snapshot sont retrouvÃ©s dans le store actuel.
pub fn validate(snap: &LiveSnapshot, node: &MonsterNode) -> SnapshotValidation {
    let store = node.store();
    let mut loaded = 0usize;
    let mut missing = Vec::new();
    for hash in &snap.programs {
        if store.load(hash).is_some() {
            loaded += 1;
        } else {
            missing.push(*hash);
        }
    }
    SnapshotValidation {
        programs_loaded: loaded,
        programs_missing: missing,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryGovernor, Store};
    use std::path::PathBuf;
    

    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("scan-snapshot", tag)
    }

    fn fresh_node(tag: &str) -> MonsterNode {
        MonsterNode::new(
            Store::open(fresh_path(tag)).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        )
    }

    #[test]
    fn capture_empty_node_yields_empty_snapshot() {
        let node = fresh_node("empty-capture");
        let snap = capture(&node);
        assert!(snap.programs.is_empty());
        assert!(snap.oracles.is_empty());
    }

    #[test]
    fn snapshot_hash_is_deterministic() {
        // Deux snapshots avec les mÃªmes contenus â†’ mÃªme hash.
        let node = fresh_node("deterministic");
        let snap1 = capture(&node);
        let snap2 = capture(&node);
        assert_eq!(snap1, snap2);
        assert_eq!(snapshot_hash(&snap1), snapshot_hash(&snap2));
    }

    #[test]
    fn snapshot_hash_is_order_independent() {
        // Sortie de `LiveSnapshot::new` triÃ©e â†’ mÃªme hash pour deux ordres.
        let h1 = Hash::for_blob(b"program-1");
        let h2 = Hash::for_blob(b"program-2");
        let h3 = Hash::for_blob(b"oracle-1");
        let snap_a = LiveSnapshot::new(vec![h1, h2], vec![h3], 42);
        let snap_b = LiveSnapshot::new(vec![h2, h1], vec![h3], 42);
        assert_eq!(snapshot_hash(&snap_a), snapshot_hash(&snap_b));
    }

    #[test]
    fn snapshot_hash_distinguishes_programs() {
        let h1 = Hash::for_blob(b"x");
        let h2 = Hash::for_blob(b"y");
        let snap_a = LiveSnapshot::new(vec![h1], vec![], 0);
        let snap_b = LiveSnapshot::new(vec![h2], vec![], 0);
        assert_ne!(snapshot_hash(&snap_a), snapshot_hash(&snap_b));
    }

    #[test]
    fn snapshot_hash_distinguishes_epoch() {
        let h1 = Hash::for_blob(b"x");
        let snap_a = LiveSnapshot::new(vec![h1], vec![], 1);
        let snap_b = LiveSnapshot::new(vec![h1], vec![], 2);
        assert_ne!(snapshot_hash(&snap_a), snapshot_hash(&snap_b));
    }

    #[test]
    fn snapshot_hash_dedupes_programs() {
        let h1 = Hash::for_blob(b"x");
        let snap_with_dup = LiveSnapshot::new(vec![h1, h1, h1], vec![], 0);
        let snap_single = LiveSnapshot::new(vec![h1], vec![], 0);
        assert_eq!(snap_with_dup.programs.len(), 1);
        assert_eq!(snapshot_hash(&snap_with_dup), snapshot_hash(&snap_single));
    }

    #[test]
    fn validate_empty_snapshot_is_intact() {
        let node = fresh_node("validate-empty");
        let snap = capture(&node);
        let v = validate(&snap, &node);
        assert!(v.is_intact());
        assert_eq!(v.programs_loaded, 0);
        assert!(v.programs_missing.is_empty());
    }

    #[test]
    fn validate_detects_missing_programs() {
        // Snapshot fabriquÃ© avec des hashes qui n'existent pas dans la node.
        let node = fresh_node("validate-missing");
        let bogus_hash = Hash::for_blob(b"nonexistent program");
        let snap = LiveSnapshot::new(vec![bogus_hash], vec![], 0);
        let v = validate(&snap, &node);
        assert!(!v.is_intact());
        assert_eq!(v.programs_loaded, 0);
        assert_eq!(v.programs_missing, vec![bogus_hash]);
    }
    #[test]
    fn snapshot_after_program_execution_validates() {
        let node = fresh_node("after-exec");
        let program = crate::kasm::Program::new(
            crate::kasm::Target::Cpu,
            1,
            1,
            4,
            vec![
                crate::kasm::Node::input(0),
                crate::kasm::Node::const_i64(7),
                crate::kasm::Node::add(0, 1),
                crate::kasm::Node::output(2, crate::kasm::Ty::I64),
            ],
        )
        .unwrap();
        let hash = node.store().store(program.bytes()).unwrap();
        let _ = node.call_bytes(&hash, &5i64.to_le_bytes()).unwrap();
        let snap = capture(&node);
        let v = validate(&snap, &node);
        assert!(
            v.is_intact(),
            "snapshot after execution must validate: missing = {:?}",
            v.programs_missing
        );
        assert_eq!(v.programs_loaded, snap.programs.len());
    }
}
