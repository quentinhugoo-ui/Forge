use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebExplorerBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebExplorerPolicy {
    pub schema: String,
    pub allowed_schemes: Vec<String>,
    pub allowed_hosts: Vec<String>,
    pub blocked_schemes: Vec<String>,
    pub devtools_enabled: bool,
    pub host_objects_enabled: bool,
    pub ipc_enabled: bool,
    pub downloads_allowed: bool,
    pub external_open_policy: String,
    pub initialization_script_hash: String,
    pub fixture_hash: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebExplorerNavigationDecision {
    pub url: String,
    pub allowed: bool,
    pub reason: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebExplorerIsolationProof {
    pub schema: String,
    pub policy_hash: String,
    pub fixture_hash: String,
    pub bounds_hash: String,
    pub allowed_navigation: WebExplorerNavigationDecision,
    pub blocked_navigation: WebExplorerNavigationDecision,
    pub focus_handoff_required: bool,
    pub crash_recreate_policy: String,
    pub proof_hash: String,
}

impl Default for WebExplorerBounds {
    fn default() -> Self {
        Self {
            x: 236,
            y: 82,
            width: 520,
            height: 340,
        }
    }
}

impl WebExplorerPolicy {
    pub fn default_locked() -> Self {
        let allowed_schemes = vec!["about".to_string(), "https".to_string()];
        let allowed_hosts = vec![
            "example.com".to_string(),
            "docs.rs".to_string(),
            "learn.microsoft.com".to_string(),
            "slint.dev".to_string(),
        ];
        let blocked_schemes = vec![
            "file".to_string(),
            "javascript".to_string(),
            "data".to_string(),
            "vbscript".to_string(),
            "ms-appx".to_string(),
        ];
        let initialization_script_hash = sha256_hex(webexplorer_initialization_script().as_bytes());
        let fixture_hash = sha256_hex(webexplorer_fixture_html().as_bytes());
        let mut policy = Self {
            schema: "ingen.webexplorer.isolation_policy.v1".to_string(),
            allowed_schemes,
            allowed_hosts,
            blocked_schemes,
            devtools_enabled: false,
            host_objects_enabled: false,
            ipc_enabled: false,
            downloads_allowed: false,
            external_open_policy: "deny-by-default; promote only through verified atlas refs"
                .to_string(),
            initialization_script_hash,
            fixture_hash,
            proof_hash: String::new(),
        };
        policy.proof_hash = hash_json(&(
            &policy.schema,
            &policy.allowed_schemes,
            &policy.allowed_hosts,
            &policy.blocked_schemes,
            policy.devtools_enabled,
            policy.host_objects_enabled,
            policy.ipc_enabled,
            policy.downloads_allowed,
            &policy.external_open_policy,
            &policy.initialization_script_hash,
            &policy.fixture_hash,
        ));
        policy
    }

    pub fn decide_navigation(&self, url: &str) -> WebExplorerNavigationDecision {
        let trimmed = url.trim();
        let scheme = trimmed
            .split_once(':')
            .map(|(scheme, _)| scheme.to_ascii_lowercase())
            .unwrap_or_default();
        let host = host_from_url(trimmed);
        let (allowed, reason) = if scheme == "about" && trimmed == "about:blank" {
            (true, "local fixture about:blank navigation".to_string())
        } else if self.blocked_schemes.iter().any(|item| item == &scheme) {
            (false, format!("blocked dangerous scheme '{scheme}'"))
        } else if scheme != "https" {
            (false, format!("scheme '{scheme}' is not in the WebExplorer allowlist"))
        } else if host
            .as_ref()
            .is_some_and(|host| self.allowed_hosts.iter().any(|item| item == host))
        {
            (true, format!("allowed https host '{}'", host.unwrap()))
        } else {
            (
                false,
                format!(
                    "https host '{}' requires external-open promotion",
                    host.unwrap_or_else(|| "missing".to_string())
                ),
            )
        };
        let proof_hash = hash_json(&(trimmed, allowed, &reason, &self.proof_hash));
        WebExplorerNavigationDecision {
            url: trimmed.to_string(),
            allowed,
            reason,
            proof_hash,
        }
    }
}

pub fn webexplorer_isolation_proof() -> WebExplorerIsolationProof {
    let policy = WebExplorerPolicy::default_locked();
    let bounds = WebExplorerBounds::default();
    let allowed_navigation = policy.decide_navigation("https://example.com/");
    let blocked_navigation = policy.decide_navigation("javascript:alert(1)");
    let bounds_hash = hash_json(&bounds);
    let mut proof = WebExplorerIsolationProof {
        schema: "ingen.webexplorer.isolation_proof.v1".to_string(),
        policy_hash: policy.proof_hash.clone(),
        fixture_hash: policy.fixture_hash.clone(),
        bounds_hash,
        allowed_navigation,
        blocked_navigation,
        focus_handoff_required: true,
        crash_recreate_policy: "drop child WebView, keep Slint shell alive, recreate only through policy"
            .to_string(),
        proof_hash: String::new(),
    };
    proof.proof_hash = hash_json(&(
        &proof.schema,
        &proof.policy_hash,
        &proof.fixture_hash,
        &proof.bounds_hash,
        &proof.allowed_navigation,
        &proof.blocked_navigation,
        proof.focus_handoff_required,
        &proof.crash_recreate_policy,
    ));
    proof
}

pub fn webexplorer_fixture_html() -> &'static str {
    include_str!("../fixtures/webview_stage0.html")
}

pub fn webexplorer_initialization_script() -> &'static str {
    r#"
Object.defineProperty(window, "open", {
  value: function () { return null; },
  writable: false,
  configurable: false
});
window.__INGEN_WEBEXPLORER_ISOLATED__ = true;
"#
}

fn host_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .split('@')
        .last()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn hash_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize webexplorer proof input");
    sha256_hex(&bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webexplorer_policy_blocks_dangerous_schemes() {
        let policy = WebExplorerPolicy::default_locked();

        for url in ["javascript:alert(1)", "file:///C:/secret.txt", "data:text/html,x"] {
            let decision = policy.decide_navigation(url);
            assert!(!decision.allowed, "{url}");
            assert_eq!(decision.proof_hash.len(), 64);
        }
    }

    #[test]
    fn webexplorer_policy_allows_only_known_https_hosts() {
        let policy = WebExplorerPolicy::default_locked();

        assert!(policy.decide_navigation("https://example.com/").allowed);
        assert!(!policy.decide_navigation("http://example.com/").allowed);
        assert!(!policy.decide_navigation("https://evil.example/").allowed);
    }

    #[test]
    fn webexplorer_isolation_proof_is_deterministic() {
        let first = webexplorer_isolation_proof();
        let second = webexplorer_isolation_proof();

        assert_eq!(first, second);
        assert!(first.allowed_navigation.allowed);
        assert!(!first.blocked_navigation.allowed);
        assert_eq!(first.proof_hash.len(), 64);
    }
}
