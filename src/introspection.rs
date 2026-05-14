//! Ω-Φ — Ghost storage : introspection content-addressed.
//!
//! Φ.μ.7 : aplati de `src/introspection/{mod,snapshot}.rs` à
//! `src/introspection.rs` (le sous-dossier ne contenait qu'un fichier
//! réel + un mod.rs de 9 lignes — pure indirection).
//!
//! Module qui prépare le terrain pour Ω-Φ.0..Ω-Φ.4 (cf. Git history). Le
//! premier sous-cap est `LiveSnapshot` — capture content-addressed et
//! reproductible de l'état logique d'une `MonsterNode`.
//!
//! ----- LiveSnapshot (Ω-Φ.0) -----
//!
//! `MonsterNode::live_snapshot()` content-addressed.
//!
//! Capture lecture-seule de l'état logique d'une `MonsterNode` : ensemble
//! de programmes chargés + oracles actifs + un compteur d'epoch monotone.
//! Le hash du snapshot est déterministe (sha256 sur la projection canonique
//! triée+dedupée), donc deux snapshots avec les mêmes contenus → même hash.
//!
//! `validate(snap, node)` re-vérifie que chaque programme du snapshot est
//! toujours chargeable depuis le store de la node. Ne ré-exécute pas, ne
//! mute pas — c'est un contrôle d'intégrité référentiel pur.
//!
//! ## Doctrine
//!
//! - Pure Rust + std + sha2. Aucune dépendance externe.
//! - Aucune manipulation mémoire OS-spécifique.
//! - Lecture seule sur la `MonsterNode` — pas de mutation, pas de
//!   re-construction d'état pendant le snapshot.
//! - Reconstructible : `validate(snap, node)` doit retourner le même
//!   nombre de programmes que `snap.programs.len()` si le store contient
//!   toujours tous les hashes — sinon, l'écart est observable.

use sha2::{Digest, Sha256};

use crate::godel::observer;
use crate::{Hash, MonsterNode};

/// Snapshot d'état logique d'une `MonsterNode`.
///
/// Champs triés et dédupés à la capture pour garantir l'invariance du hash
/// sous l'ordre d'observation (deux nodes avec les mêmes programmes
/// chargés produisent le même `snapshot_hash`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSnapshot {
    /// Hashes content-addressed des programmes chargés.
    pub programs: Vec<Hash>,
    /// Hashes des oracles actifs.
    pub oracles: Vec<Hash>,
    /// Compteur monotone — vient de `observer::ObserverFrame::epoch`.
    pub epoch: u64,
}

impl LiveSnapshot {
    /// Constructeur direct (utile pour les tests). Trie + dédupe en place
    /// pour préserver l'invariant de canonicité du hash.
    pub fn new(mut programs: Vec<Hash>, mut oracles: Vec<Hash>, epoch: u64) -> Self {
        programs.sort();
        programs.dedup();
        oracles.sort();
        oracles.dedup();
        Self { programs, oracles, epoch }
    }
}

/// Capture l'état logique d'une `MonsterNode` (lecture seule, non perturbant).
pub fn capture(node: &MonsterNode) -> LiveSnapshot {
    let frame = observer::capture(node);
    LiveSnapshot::new(frame.programs_loaded, frame.oracles_active, frame.epoch)
}

/// Hash sha-256 déterministe d'un snapshot. Deux snapshots avec les mêmes
/// `programs` (set), `oracles` (set) et `epoch` produisent le même hash.
pub fn snapshot_hash(snap: &LiveSnapshot) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"scan-omega-phi-0-snapshot-v1");
    // Programs : len préfixée + chaque hash en bytes triés.
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

/// Résultat de la validation d'un snapshot contre une node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotValidation {
    /// Nombre de programmes du snapshot effectivement chargeables depuis
    /// le store de la node au moment de la validation.
    pub programs_loaded: usize,
    /// Programmes du snapshot absents du store (référentiellement vides).
    pub programs_missing: Vec<Hash>,
}

impl SnapshotValidation {
    pub fn is_intact(&self) -> bool {
        self.programs_missing.is_empty()
    }
}

/// Valide un snapshot contre une `MonsterNode`. Pour chaque hash de
/// `snap.programs`, tente `node.store().load(hash)`. Ne ré-exécute pas,
/// ne mute pas l'état de la node.
///
/// Retourne le nombre de programmes valides + la liste des hashes
/// manquants. `snapshot.is_intact()` est `true` ssi tous les programmes
/// du snapshot sont retrouvés dans le store actuel.
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
        // Deux snapshots avec les mêmes contenus → même hash.
        let node = fresh_node("deterministic");
        let snap1 = capture(&node);
        let snap2 = capture(&node);
        assert_eq!(snap1, snap2);
        assert_eq!(snapshot_hash(&snap1), snapshot_hash(&snap2));
    }

    #[test]
    fn snapshot_hash_is_order_independent() {
        // Sortie de `LiveSnapshot::new` triée → même hash pour deux ordres.
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
        // Snapshot fabriqué avec des hashes qui n'existent pas dans la node.
        let node = fresh_node("validate-missing");
        let bogus_hash = Hash::for_blob(b"nonexistent program");
        let snap = LiveSnapshot::new(vec![bogus_hash], vec![], 0);
        let v = validate(&snap, &node);
        assert!(!v.is_intact());
        assert_eq!(v.programs_loaded, 0);
        assert_eq!(v.programs_missing, vec![bogus_hash]);
    }

    #[test]
    fn snapshot_after_program_train_validates() {
        // On entraîne un programme dans la node, on capture, on valide.
        let node = fresh_node("after-train");
        let examples = [(-4i64, -25i64), (-1, -4), (0, 3), (2, 17), (5, 38)];
        let cfg = crate::MonsterTrainingConfig {
            max_nodes: 20,
            beam_width: 256,
            progress: None,
        };
        let _ = node.train_i64_program(&examples, cfg);
        let snap = capture(&node);
        let v = validate(&snap, &node);
        // Tout programme listé dans le snapshot doit être chargeable (la
        // node ne purge rien dans ce test).
        assert!(
            v.is_intact(),
            "snapshot après train doit valider — missing = {:?}",
            v.programs_missing
        );
        assert_eq!(v.programs_loaded, snap.programs.len());
    }
}
