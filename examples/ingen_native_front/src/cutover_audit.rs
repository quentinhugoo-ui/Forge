use crate::obsolete_front::{
    build_obsolete_front_manifest, ObsoleteFrontManifest, ObsoleteFrontPath,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedBackendService {
    pub path: String,
    pub exists: bool,
    pub extraction_action: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeShellManifestCheck {
    pub path: String,
    pub exists: bool,
    pub forbidden_dependencies: Vec<String>,
    pub app_shell_uses_tauri: bool,
    pub app_shell_uses_dioxus: bool,
    pub app_shell_uses_wasm_bindgen: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CutoverAuditReport {
    pub schema: String,
    pub source: String,
    pub obsolete_front: ObsoleteFrontManifest,
    pub legacy_front_blockers: Vec<ObsoleteFrontPath>,
    pub protected_backend_services: Vec<ProtectedBackendService>,
    pub native_shell_manifest: NativeShellManifestCheck,
    pub rollback_required: bool,
    pub backend_extraction_required: bool,
    pub tauri_backend_retirement_required: bool,
    pub full_tauri_retirement_ready: bool,
    pub cutover_ready: bool,
    pub blocking_summary: Vec<String>,
    pub retirement_summary: Vec<String>,
    pub proof_hash: String,
}

pub fn build_cutover_audit_report() -> CutoverAuditReport {
    let obsolete_front = build_obsolete_front_manifest();
    let legacy_front_blockers = obsolete_front
        .obsolete_paths
        .iter()
        .filter(|path| path.exists)
        .cloned()
        .collect::<Vec<_>>();
    let protected_backend_services = obsolete_front
        .protected_backend_paths
        .iter()
        .map(|path| ProtectedBackendService {
            exists: repo_path(path).exists(),
            path: path.clone(),
            extraction_action: protected_backend_action(path),
        })
        .collect::<Vec<_>>();
    let native_shell_manifest =
        native_shell_manifest_check("examples/ingen_native_front/Cargo.toml");
    let rollback_required = !legacy_front_blockers.is_empty();
    let backend_extraction_required = protected_backend_services.iter().any(|service| service.exists);
    let tauri_backend_retirement_required = backend_extraction_required;
    let mut blocking_summary = Vec::new();
    if rollback_required {
        blocking_summary.push(format!(
            "{} obsolete app-shell paths still exist",
            legacy_front_blockers.len()
        ));
    }
    if !native_shell_manifest.forbidden_dependencies.is_empty() {
        blocking_summary.push(format!(
            "native shell manifest contains forbidden shell dependencies: {}",
            native_shell_manifest.forbidden_dependencies.join(", ")
        ));
    }
    if blocking_summary.is_empty() {
        blocking_summary.push("native front cutover audit is clear".to_string());
    }
    let mut retirement_summary = Vec::new();
    if backend_extraction_required {
        retirement_summary.push(format!(
            "{} protected backend services still live under the old Tauri tree",
            protected_backend_services
                .iter()
                .filter(|service| service.exists)
                .count()
        ));
    }
    if retirement_summary.is_empty() {
        retirement_summary.push("old Tauri backend tree can be retired".to_string());
    }
    let cutover_ready = obsolete_front.deletion_ready
        && !rollback_required
        && native_shell_manifest.forbidden_dependencies.is_empty();
    let full_tauri_retirement_ready = cutover_ready && !tauri_backend_retirement_required;
    let mut report = CutoverAuditReport {
        schema: "ingen.native_front.stage11_cutover_audit.v1".to_string(),
        source: "examples/ingen_native_front/src/cutover_audit.rs".to_string(),
        obsolete_front,
        legacy_front_blockers,
        protected_backend_services,
        native_shell_manifest,
        rollback_required,
        backend_extraction_required,
        tauri_backend_retirement_required,
        full_tauri_retirement_ready,
        cutover_ready,
        blocking_summary,
        retirement_summary,
        proof_hash: String::new(),
    };
    report.proof_hash = stable_hash(&(
        &report.schema,
        &report.source,
        &report.obsolete_front,
        &report.legacy_front_blockers,
        &report.protected_backend_services,
        &report.native_shell_manifest,
        report.rollback_required,
        report.backend_extraction_required,
        report.tauri_backend_retirement_required,
        report.full_tauri_retirement_ready,
        report.cutover_ready,
        &report.blocking_summary,
        &report.retirement_summary,
    ));
    report
}

fn native_shell_manifest_check(path: &str) -> NativeShellManifestCheck {
    let resolved_path = repo_path(path);
    let content = fs::read_to_string(&resolved_path).unwrap_or_default();
    let forbidden_dependencies = ["tauri", "dioxus", "wasm-bindgen"]
        .into_iter()
        .filter(|name| manifest_has_dependency(&content, name))
        .map(str::to_string)
        .collect::<Vec<_>>();
    NativeShellManifestCheck {
        path: path.to_string(),
        exists: resolved_path.exists(),
        app_shell_uses_tauri: forbidden_dependencies.iter().any(|name| name == "tauri"),
        app_shell_uses_dioxus: forbidden_dependencies.iter().any(|name| name == "dioxus"),
        app_shell_uses_wasm_bindgen: forbidden_dependencies
            .iter()
            .any(|name| name == "wasm-bindgen"),
        forbidden_dependencies,
    }
}

fn manifest_has_dependency(content: &str, dependency: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&format!("{dependency} ="))
            || trimmed.starts_with(&format!("{dependency}="))
            || trimmed == format!("[dependencies.{dependency}]")
            || trimmed == format!("[target.'cfg(windows)'.dependencies.{dependency}]")
    })
}

fn protected_backend_action(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    match name {
        "forge_agent_runtime.rs" | "forge_brain_runtime.rs" => {
            "extract into native Rust agent/brain service crate before deleting Tauri shell".to_string()
        }
        "collection_os.rs" => {
            "extract as shared Collection OS kernel service before deleting Tauri shell".to_string()
        }
        "banger_native_engine.rs" => {
            "move into native Banger viewport/engine module before deleting Tauri shell".to_string()
        }
        "trading_core.rs" => {
            "move into native Trading adapter module before deleting Tauri shell".to_string()
        }
        "real_estate_harvester.rs" => {
            "move into native Real Estate adapter module before deleting Tauri shell".to_string()
        }
        _ => "review and extract or retire before deleting Tauri shell".to_string(),
    }
}

fn stable_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize cutover audit hash input");
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
    fn native_manifest_check_rejects_forbidden_shell_dependencies() {
        let check = native_shell_manifest_check("examples/ingen_native_front/Cargo.toml");

        assert!(check.exists);
        assert!(!check.app_shell_uses_tauri);
        assert!(!check.app_shell_uses_dioxus);
        assert!(!check.app_shell_uses_wasm_bindgen);
        assert!(check.forbidden_dependencies.is_empty());
    }

    #[test]
    fn cutover_audit_tracks_front_cutover_and_backend_retirement_separately() {
        let report = build_cutover_audit_report();

        assert_eq!(report.schema, "ingen.native_front.stage11_cutover_audit.v1");
        assert_eq!(report.rollback_required, !report.legacy_front_blockers.is_empty());
        assert_eq!(
            report.cutover_ready,
            report.obsolete_front.deletion_ready
                && !report.rollback_required
                && report.native_shell_manifest.forbidden_dependencies.is_empty()
        );
        assert!(report.backend_extraction_required);
        assert!(report.tauri_backend_retirement_required);
        assert!(!report.full_tauri_retirement_ready);
        assert_eq!(report.proof_hash.len(), 64);
    }
}
