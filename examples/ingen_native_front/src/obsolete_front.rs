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
    let protected_backend_paths = vec![
        "examples/forge_tauri_ui/src-tauri/src/forge_agent_runtime.rs".to_string(),
        "examples/forge_tauri_ui/src-tauri/src/forge_brain_runtime.rs".to_string(),
        "examples/forge_tauri_ui/src-tauri/src/collection_os.rs".to_string(),
        "examples/forge_tauri_ui/src-tauri/src/banger_native_engine.rs".to_string(),
        "examples/forge_tauri_ui/src-tauri/src/trading_core.rs".to_string(),
        "examples/forge_tauri_ui/src-tauri/src/real_estate_harvester.rs".to_string(),
    ];
    let anti_regression_rules = vec![
        "normal startup must use examples/ingen_native_front, not Tauri main-window WebView".to_string(),
        "no new product UI source under examples/forge_tauri_ui/ui/src".to_string(),
        "no Dioxus/WASM route under examples/forge_tauri_ui/front-rs".to_string(),
        "no app-shell HTML/CSS/TypeScript/JavaScript dependency for normal operation".to_string(),
        "WRY/WebView2 remains allowed only for isolated WebExplorer peripheral".to_string(),
    ];
    let existing_obsolete = obsolete_paths.iter().filter(|item| item.exists).count();
    let deletion_ready = existing_obsolete == 0;
    let blocker_summary = if deletion_ready {
        "obsolete app-shell paths are absent".to_string()
    } else {
        format!(
            "{existing_obsolete} obsolete app-shell paths still exist; delete only after rollback commit and extraction of protected backend services"
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
    vec![
        (
            "examples/forge_tauri_ui/ui",
            "global-webview-ui-tree",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/index.html",
            "html-app-host",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/src",
            "typescript-app-shell",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/dist",
            "generated-browser-runtime",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/styles.css",
            "css-app-shell",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/rust-front.html",
            "legacy-wasm-host",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/rust-front-poc.html",
            "legacy-wasm-host",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/front-rs",
            "dioxus-wasm-front",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/native-front",
            "misplaced-native-front",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/node_modules",
            "npm-front-dependencies",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/package.json",
            "npm-front-build",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/package-lock.json",
            "npm-front-build",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/tsconfig.json",
            "typescript-front-config",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/scripts/build-ui-runtime.mjs",
            "browser-runtime-build",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/scripts/forge-front-rs-cutover-audit.mjs",
            "legacy-front-audit",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/scripts/forge-ui-smoke.mjs",
            "legacy-webview-smoke",
            "delete-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/SECTION_CONTRACT.md",
            "legacy-section-doc",
            "rewrite-after-native-cutover",
        ),
        (
            "examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json",
            "legacy-section-doc",
            "rewrite-after-native-cutover",
        ),
    ]
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
    fn obsolete_front_manifest_tracks_known_legacy_shell_paths() {
        let manifest = build_obsolete_front_manifest();

        assert_eq!(
            manifest.schema,
            "ingen.native_front.stage11_obsolete_front_manifest.v1"
        );
        assert!(manifest
            .obsolete_paths
            .iter()
            .any(|path| path.path == "examples/forge_tauri_ui/ui/src"));
        assert!(manifest
            .obsolete_paths
            .iter()
            .any(|path| path.path == "examples/forge_tauri_ui/front-rs"));
        assert!(!manifest.protected_backend_paths.is_empty());
        assert_eq!(manifest.proof_hash.len(), 64);
    }

    #[test]
    fn anti_regression_rules_keep_webview_peripheral_only() {
        let manifest = build_obsolete_front_manifest();

        assert!(manifest
            .anti_regression_rules
            .iter()
            .any(|rule| rule.contains("normal startup")));
        assert!(manifest
            .anti_regression_rules
            .iter()
            .any(|rule| rule.contains("WebExplorer peripheral")));
    }
}
