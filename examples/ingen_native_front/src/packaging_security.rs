use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const APP_VENDOR: &str = "InGen";
const APP_NAME: &str = "NativeFront";
const PROTECTED_EVE_MAP: &str = r"C:\Users\quent\Documents\EVE\MAP";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeAppPaths {
    pub install_root_policy: String,
    pub app_data_dir: String,
    pub logs_dir: String,
    pub crash_recovery_dir: String,
    pub secrets_dir: String,
    pub external_web_profile_dir: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeCapabilityPolicy {
    pub schema: String,
    pub protected_roots: Vec<String>,
    pub writable_roots: Vec<String>,
    pub external_web_local_file_access: String,
    pub external_web_profile_isolated: bool,
    pub secrets_in_logs_allowed: bool,
    pub updater_policy: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalPathDecision {
    pub path: String,
    pub allowed: bool,
    pub reason: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CrashRecoveryRecord {
    pub schema: String,
    pub record_path: String,
    pub last_stage: String,
    pub replay_state_hash: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackagingSecurityManifest {
    pub schema: String,
    pub package_target: String,
    pub binary_name: String,
    pub slint_desktop_supported: bool,
    pub obsolete_shell_required: bool,
    pub app_paths: NativeAppPaths,
    pub capability_policy: NativeCapabilityPolicy,
    pub protected_path_decision: LocalPathDecision,
    pub app_data_path_decision: LocalPathDecision,
    pub crash_recovery: CrashRecoveryRecord,
    pub proof_hash: String,
}

pub fn build_packaging_security_manifest(replay_state_hash: &str) -> PackagingSecurityManifest {
    let app_paths = native_app_paths();
    let capability_policy = native_capability_policy(&app_paths);
    let protected_path_decision =
        decide_local_path_access(&capability_policy, PROTECTED_EVE_MAP);
    let app_data_path_decision = decide_local_path_access(&capability_policy, &app_paths.app_data_dir);
    let crash_recovery = crash_recovery_record(&app_paths, replay_state_hash);
    let mut manifest = PackagingSecurityManifest {
        schema: "ingen.native_front.stage10_packaging_security.v1".to_string(),
        package_target: "windows-x86_64-native-slint".to_string(),
        binary_name: "ingen-native-front.exe".to_string(),
        slint_desktop_supported: cfg!(windows),
        obsolete_shell_required: false,
        app_paths,
        capability_policy,
        protected_path_decision,
        app_data_path_decision,
        crash_recovery,
        proof_hash: String::new(),
    };
    manifest.proof_hash = stable_hash(&(
        &manifest.schema,
        &manifest.package_target,
        &manifest.binary_name,
        manifest.slint_desktop_supported,
        manifest.obsolete_shell_required,
        &manifest.app_paths,
        &manifest.capability_policy,
        &manifest.protected_path_decision,
        &manifest.app_data_path_decision,
        &manifest.crash_recovery,
    ));
    manifest
}

pub fn native_app_paths() -> NativeAppPaths {
    let app_data = base_app_data_dir().join(APP_VENDOR).join(APP_NAME);
    let logs = app_data.join("logs");
    let crash = app_data.join("crash-recovery");
    let secrets = app_data.join("secrets");
    let external_web_profile = app_data.join("external-web-profile");
    let mut paths = NativeAppPaths {
        install_root_policy: "install binaries outside mutable app-data; never store user data next to the exe"
            .to_string(),
        app_data_dir: path_string(&app_data),
        logs_dir: path_string(&logs),
        crash_recovery_dir: path_string(&crash),
        secrets_dir: path_string(&secrets),
        external_web_profile_dir: path_string(&external_web_profile),
        proof_hash: String::new(),
    };
    paths.proof_hash = stable_hash(&(
        &paths.install_root_policy,
        &paths.app_data_dir,
        &paths.logs_dir,
        &paths.crash_recovery_dir,
        &paths.secrets_dir,
        &paths.external_web_profile_dir,
    ));
    paths
}

pub fn native_capability_policy(paths: &NativeAppPaths) -> NativeCapabilityPolicy {
    let writable_roots = vec![
        paths.app_data_dir.clone(),
        paths.logs_dir.clone(),
        paths.crash_recovery_dir.clone(),
        paths.external_web_profile_dir.clone(),
    ];
    let mut policy = NativeCapabilityPolicy {
        schema: "ingen.native_front.capability_policy.v1".to_string(),
        protected_roots: vec![PROTECTED_EVE_MAP.to_string()],
        writable_roots,
        external_web_local_file_access: "deny local files; use isolated WebExplorer profile only"
            .to_string(),
        external_web_profile_isolated: true,
        secrets_in_logs_allowed: false,
        updater_policy: "manual/update-gated; no auto-replacement before signed native package gate"
            .to_string(),
        proof_hash: String::new(),
    };
    policy.proof_hash = stable_hash(&(
        &policy.schema,
        &policy.protected_roots,
        &policy.writable_roots,
        &policy.external_web_local_file_access,
        policy.external_web_profile_isolated,
        policy.secrets_in_logs_allowed,
        &policy.updater_policy,
    ));
    policy
}

pub fn decide_local_path_access(
    policy: &NativeCapabilityPolicy,
    candidate: &str,
) -> LocalPathDecision {
    let normalized = normalize_path(candidate);
    let protected = policy
        .protected_roots
        .iter()
        .map(|root| normalize_path(root))
        .any(|root| normalized == root || normalized.starts_with(&(root + "\\")));
    let writable = policy
        .writable_roots
        .iter()
        .map(|root| normalize_path(root))
        .any(|root| normalized == root || normalized.starts_with(&(root + "\\")));
    let (allowed, reason) = if protected {
        (false, "blocked protected root".to_string())
    } else if writable {
        (true, "allowed native app-data root".to_string())
    } else {
        (false, "blocked until explicit user-selected capability grants access".to_string())
    };
    let mut decision = LocalPathDecision {
        path: candidate.to_string(),
        allowed,
        reason,
        proof_hash: String::new(),
    };
    decision.proof_hash = stable_hash(&(
        &decision.path,
        decision.allowed,
        &decision.reason,
        &policy.proof_hash,
    ));
    decision
}

pub fn crash_recovery_record(
    paths: &NativeAppPaths,
    replay_state_hash: &str,
) -> CrashRecoveryRecord {
    let record_path = Path::new(&paths.crash_recovery_dir).join("last-session.json");
    let mut record = CrashRecoveryRecord {
        schema: "ingen.native_front.crash_recovery.v1".to_string(),
        record_path: path_string(&record_path),
        last_stage: "stage10-packaging-security".to_string(),
        replay_state_hash: replay_state_hash.to_string(),
        proof_hash: String::new(),
    };
    record.proof_hash = stable_hash(&(
        &record.schema,
        &record.record_path,
        &record.last_stage,
        &record.replay_state_hash,
    ));
    record
}

fn base_app_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\quent\AppData\Local"))
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

fn stable_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize packaging security hash input");
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_packaging_manifest_blocks_protected_map() {
        let manifest = build_packaging_security_manifest("abc123");

        assert_eq!(
            manifest.schema,
            "ingen.native_front.stage10_packaging_security.v1"
        );
        assert!(!manifest.obsolete_shell_required);
        assert!(!manifest.protected_path_decision.allowed);
        assert!(manifest
            .protected_path_decision
            .reason
            .contains("protected root"));
        assert_eq!(manifest.proof_hash.len(), 64);
    }

    #[test]
    fn app_data_is_the_only_default_write_capability() {
        let paths = native_app_paths();
        let policy = native_capability_policy(&paths);
        let app_data = decide_local_path_access(&policy, &paths.app_data_dir);
        let documents = decide_local_path_access(&policy, r"C:\Users\quent\Documents");

        assert!(app_data.allowed);
        assert!(!documents.allowed);
        assert!(!policy.secrets_in_logs_allowed);
        assert!(policy.external_web_profile_isolated);
        assert_eq!(policy.proof_hash.len(), 64);
    }

    #[test]
    fn crash_recovery_record_is_deterministic() {
        let paths = native_app_paths();
        let first = crash_recovery_record(&paths, "state-hash");
        let second = crash_recovery_record(&paths, "state-hash");

        assert_eq!(first, second);
        assert!(first.record_path.ends_with("last-session.json"));
        assert_eq!(first.proof_hash.len(), 64);
    }
}
