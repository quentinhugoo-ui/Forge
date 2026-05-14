//! Ω-5.4 — Applicator : applique un `Rewrite` à un état config mutable
//! (whitelist + bornes), produit un `AppliedSnapshot` permettant un
//! rollback parfait byte-pour-byte.
//!
//! Les rewrites du first mile ne touchent QUE des paramètres runtime
//! whitelistés (`beam_width`, `max_nodes`, `oracle_threshold`, `fuel`).
//! Aucune modification de fichier source. La modification de code source
//! self-modify est une dette explicite Ω-5.4.x.

use std::collections::BTreeMap;

use crate::godel::observer::ObserverFrame;
use crate::godel::proposer::config_metric_key;
use crate::godel::verifier::{Rewrite, RewriteKind};

/// Whitelist des clés patchables. Hors whitelist → `UnknownKey`.
pub const ALLOWED_KEYS: &[&str] = &[
    "beam_width",
    "max_nodes",
    "oracle_threshold",
    "fuel",
];

const MIN_VALUE: i64 = 1;
const MAX_VALUE: i64 = 1_000_000_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GodelMutableConfig {
    values: BTreeMap<&'static str, i64>,
}

impl GodelMutableConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construit la config avec les valeurs par défaut Ω-5 demo.
    pub fn with_defaults() -> Self {
        let mut s = Self::new();
        s.values.insert("beam_width".into(), 256);
        s.values.insert("max_nodes".into(), 100);
        s.values.insert("oracle_threshold".into(), 10);
        s.values.insert("fuel".into(), 100);
        s
    }

    pub fn get(&self, key: &str) -> Option<i64> {
        self.values.get(key).copied()
    }

    pub fn set(&mut self, key: &'static str, value: i64) {
        self.values.insert(key, value);
    }

    pub fn unset(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, i64)> {
        self.values.iter().map(|(k, v)| (*k, *v))
    }

    /// Injecte les valeurs config dans `frame.metrics` sous le préfixe
    /// `config:*`. Utilisé pour que le proposer/verifier voient les vraies
    /// valeurs courantes.
    pub fn attach_to_frame(&self, mut frame: ObserverFrame) -> ObserverFrame {
        for (k, v) in &self.values {
            frame
                .metrics
                .insert(config_metric_key(k), (*v).max(0) as u64);
        }
        frame
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedSnapshot {
    pub rewrite_id: u64,
    /// Pour chaque clé patchée, valeur précédente (None si la clé n'existait pas).
    pub prev_config: BTreeMap<&'static str, Option<i64>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApplicatorError {
    UnknownKey(&'static str),
    OutOfRange { key: &'static str, value: i64 },
}

impl std::fmt::Display for ApplicatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicatorError::UnknownKey(k) => write!(f, "applicator: unknown config key '{k}'"),
            ApplicatorError::OutOfRange { key, value } => {
                write!(f, "applicator: value {value} for '{key}' out of range")
            }
        }
    }
}

impl std::error::Error for ApplicatorError {}

/// Applique un `Rewrite::ConfigPatch` à la config. Valide la whitelist
/// et les bornes AVANT toute modification (atomicité). Retourne un
/// snapshot pour rollback.
pub fn apply(
    rewrite: &Rewrite,
    config: &mut GodelMutableConfig,
) -> Result<AppliedSnapshot, ApplicatorError> {
    let RewriteKind::ConfigPatch(patch) = &rewrite.kind;

    // Validation pré-mutation (atomicité).
    for (key, value) in patch {
        if !ALLOWED_KEYS.contains(key) {
            return Err(ApplicatorError::UnknownKey(key));
        }
        if *value < MIN_VALUE || *value > MAX_VALUE {
            return Err(ApplicatorError::OutOfRange { key, value: *value });
        }
    }

    // Snapshot des valeurs AVANT mutation.
    let mut prev = BTreeMap::new();
    for key in patch.keys() {
        prev.insert(*key, config.get(key));
    }

    // Mutation.
    for (key, value) in patch {
        config.set(key, *value);
    }

    Ok(AppliedSnapshot {
        rewrite_id: rewrite.id,
        prev_config: prev,
    })
}

/// Annule un apply en restaurant l'état pré-snapshot byte-pour-byte.
/// Pour les clés qui n'existaient pas avant, on les supprime.
pub fn rollback(snap: &AppliedSnapshot, config: &mut GodelMutableConfig) {
    for (key, opt_value) in &snap.prev_config {
        match opt_value {
            Some(v) => config.set(key, *v),
            None => config.unset(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn rewrite_for(key: &'static str, value: i64) -> Rewrite {
        let mut p: BTreeMap<&'static str, i64> = BTreeMap::new();
        p.insert(key, value);
        Rewrite::config_patch(format!("set_{key}_to_{value}"), p)
    }

    #[test]
    fn apply_sets_value_in_config() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let r = rewrite_for("beam_width", 1024);
        let snap = apply(&r, &mut cfg).unwrap();
        assert_eq!(cfg.get("beam_width"), Some(1024));
        assert_eq!(snap.rewrite_id, r.id);
    }

    #[test]
    fn rollback_restores_previous_value() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let original = cfg.get("beam_width").unwrap();
        let r = rewrite_for("beam_width", 1024);
        let snap = apply(&r, &mut cfg).unwrap();
        rollback(&snap, &mut cfg);
        assert_eq!(cfg.get("beam_width"), Some(original));
    }

    #[test]
    fn rollback_removes_keys_that_were_new() {
        let mut cfg = GodelMutableConfig::new(); // empty
        let r = rewrite_for("beam_width", 512);
        let snap = apply(&r, &mut cfg).unwrap();
        assert_eq!(cfg.get("beam_width"), Some(512));
        rollback(&snap, &mut cfg);
        assert_eq!(cfg.get("beam_width"), None, "clé nouvelle doit être supprimée au rollback");
    }

    #[test]
    fn apply_rejects_unknown_key() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let r = rewrite_for("forbidden_key", 42);
        let err = apply(&r, &mut cfg).unwrap_err();
        assert!(matches!(err, ApplicatorError::UnknownKey(k) if k == "forbidden_key"));
        // Pas d'effet de bord.
        assert_eq!(cfg.get("forbidden_key"), None);
    }

    #[test]
    fn apply_rejects_out_of_range_value() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let r_neg = rewrite_for("beam_width", -1);
        assert!(matches!(
            apply(&r_neg, &mut cfg).unwrap_err(),
            ApplicatorError::OutOfRange { .. }
        ));
        let r_huge = rewrite_for("beam_width", i64::MAX);
        assert!(matches!(
            apply(&r_huge, &mut cfg).unwrap_err(),
            ApplicatorError::OutOfRange { .. }
        ));
        // beam_width inchangé.
        assert_eq!(cfg.get("beam_width"), Some(256));
    }

    #[test]
    fn apply_is_atomic_on_validation_failure() {
        // Une rewrite multi-clés où une clé est invalide → AUCUNE mutation.
        let mut cfg = GodelMutableConfig::with_defaults();
        let mut p = BTreeMap::new();
        p.insert("beam_width", 1024); // valide
        p.insert("forbidden", 1); // invalide
        let r = Rewrite::config_patch("multi_invalid", p);
        let err = apply(&r, &mut cfg).unwrap_err();
        assert!(matches!(err, ApplicatorError::UnknownKey(_)));
        // beam_width inchangé.
        assert_eq!(cfg.get("beam_width"), Some(256));
    }

    #[test]
    fn double_apply_then_rollback_returns_to_original() {
        let mut cfg = GodelMutableConfig::with_defaults();
        let original = cfg.get("beam_width").unwrap();
        let r1 = rewrite_for("beam_width", 1024);
        let r2 = rewrite_for("beam_width", 2048);
        let snap1 = apply(&r1, &mut cfg).unwrap();
        let snap2 = apply(&r2, &mut cfg).unwrap();
        assert_eq!(cfg.get("beam_width"), Some(2048));
        rollback(&snap2, &mut cfg);
        assert_eq!(cfg.get("beam_width"), Some(1024));
        rollback(&snap1, &mut cfg);
        assert_eq!(cfg.get("beam_width"), Some(original));
    }

    #[test]
    fn attach_config_injects_metrics() {
        use std::collections::BTreeMap as BTM;
        let cfg = GodelMutableConfig::with_defaults();
        let frame = ObserverFrame {
            epoch: 0,
            programs_loaded: vec![],
            oracles_active: vec![],
            cache_hot_paths: vec![],
            metrics: BTM::new(),
        };
        let attached = cfg.attach_to_frame(frame);
        assert_eq!(attached.metrics.get("config:beam_width"), Some(&256u64));
        assert_eq!(attached.metrics.get("config:max_nodes"), Some(&100u64));
    }
}
