use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObsoleteFrontPath {
    pub path: String,
    pub kind: String,
    pub deletion_class: String,
    pub exists: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObsoleteFrontManifest {
    pub schema: String,
    pub source: String,
    pub obsolete_paths: Vec<ObsoleteFrontPath>,
    pub protected_backend_paths: Vec<String>,
    pub anti_regression_rules: Vec<String>,
    pub deletion_ready: bool,
    pub blocker_summary: String,
    pub proof_hash: String,
}

pub fn build_obsolete_front_manifest() -> ObsoleteFrontManifest {
    let obsolete_paths = obsolete_front_paths()
        .into_iter()
        .map(|(path, kind, deletion_class)| ObsoleteFrontPath {
            exists: repo_path(path).exists(),
            path: path.to_string(),
            kind: kind.to_string(),
            deletion_class: deletion_class.to_string(),
        })
        .collect::<Vec<_>>();
    let protected_backend_paths = Vec::new();
    let anti_regression_rules = vec![
        "normal startup must use examples/ingen_native_front".to_string(),
        "no new obsolete product UI source".to_string(),
        "no browser-document shell route".to_string(),
        "no client-script app dependency for normal operation".to_string(),
        "external web display remains a contained peripheral".to_string(),
    ];
    let existing_obsolete = obsolete_paths.iter().filter(|item| item.exists).count();
    let deletion_ready = existing_obsolete == 0;
    let blocker_summary = if deletion_ready {
        "obsolete app-shell paths are absent".to_string()
    } else {
        format!(
            "{existing_obsolete} obsolete app-shell paths still exist; delete only after rollback commit and native extraction"
        )
    };
    let mut manifest = ObsoleteFrontManifest {
        schema: "ingen.native_front.stage11_obsolete_front_manifest.v1".to_string(),
        source: "examples/ingen_native_front/src/obsolete_front.rs".to_string(),
        obsolete_paths,
        protected_backend_paths,
        anti_regression_rules,
        deletion_ready,
        blocker_summary,
        proof_hash: String::new(),
    };
    manifest.proof_hash = stable_hash(&(
        &manifest.schema,
        &manifest.source,
        &manifest.obsolete_paths,
        &manifest.protected_backend_paths,
        &manifest.anti_regression_rules,
        manifest.deletion_ready,
        &manifest.blocker_summary,
    ));
    manifest
}

fn obsolete_front_paths() -> Vec<(&'static str, &'static str, &'static str)> {
    Vec::new()
}

fn stable_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize obsolete front manifest hash input");
    format!("{:x}", Sha256::digest(bytes))
}

fn repo_path(path: &str) -> PathBuf {
    let direct = Path::new(path);
    if direct.exists() {
        return direct.to_path_buf();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsolete_front_manifest_tracks_final_cutover() {
        let manifest = build_obsolete_front_manifest();

        assert_eq!(
            manifest.schema,
            "ingen.native_front.stage11_obsolete_front_manifest.v1"
        );
        assert!(manifest.obsolete_paths.is_empty());
        assert!(manifest.protected_backend_paths.is_empty());
        assert!(manifest.deletion_ready);
        assert_eq!(manifest.proof_hash.len(), 64);
    }

    #[test]
    fn anti_regression_rules_keep_external_web_peripheral_only() {
        let manifest = build_obsolete_front_manifest();

        assert!(manifest
            .anti_regression_rules
            .iter()
            .any(|rule| rule.contains("normal startup")));
        assert!(manifest
            .anti_regression_rules
            .iter()
            .any(|rule| rule.contains("external web display")));
    }
}
