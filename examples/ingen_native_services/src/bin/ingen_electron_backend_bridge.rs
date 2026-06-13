use ingen_native_services::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapterProbe};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ElectronBackendProjection {
    schema: &'static str,
    source: &'static str,
    backend: &'static str,
    generated_at_unix_ms: u128,
    active_section: &'static str,
    section_title: &'static str,
    sessions: Vec<SessionProjection>,
    transcript: Vec<TranscriptProjection>,
    native_status: NativeStatusProjection,
    proof_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionProjection {
    session_id: &'static str,
    label: &'static str,
    date: &'static str,
    section: &'static str,
    pinned: bool,
    working: bool,
    automated: bool,
    archived: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptProjection {
    id: &'static str,
    role: &'static str,
    text: &'static str,
    proof_hash: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeStatusProjection {
    state_owner: String,
    jobs: String,
    banger: String,
    webexplorer: String,
    monster: String,
    brain: String,
    provider: String,
    proof: String,
    gpu_probe: NativeGpuAdapterProbe,
}

fn main() {
    let projection = projection();
    println!("{}", serde_json::to_string(&projection).expect("serialize electron projection"));
}

fn projection() -> ElectronBackendProjection {
    let generated_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let sessions = vec![
        SessionProjection {
            session_id: "native-front-migration",
            label: "Electron cutover",
            date: "2026-06-09",
            section: "forge",
            pinned: true,
            working: false,
            automated: false,
            archived: false,
        },
        SessionProjection {
            session_id: "test-session-example",
            label: "test session example",
            date: "2026-06-10",
            section: "forge",
            pinned: false,
            working: true,
            automated: false,
            archived: false,
        },
        SessionProjection {
            session_id: "banger-native-surface",
            label: "Banger native surface",
            date: "2026-06-09",
            section: "banger",
            pinned: false,
            working: false,
            automated: true,
            archived: false,
        },
        SessionProjection {
            session_id: "webexplorer-rust-webview",
            label: "WebExplorer Rust WebView",
            date: "2026-06-09",
            section: "webexplorer",
            pinned: false,
            working: false,
            automated: false,
            archived: false,
        },
        SessionProjection {
            session_id: "monster-compute-proof",
            label: "Monster compute proof",
            date: "2026-06-08",
            section: "forge",
            pinned: false,
            working: false,
            automated: false,
            archived: false,
        },
    ];
    let transcript = Vec::new();
    let gpu_probe = native_gpu_adapter_probe();
    let banger_status = banger_status_from_probe(&gpu_probe);
    let native_status = NativeStatusProjection {
        state_owner: "ingen_native_services::electron_backend_bridge".to_string(),
        jobs: "queued=0 running=0 done=0 failed=0".to_string(),
        banger: banger_status,
        webexplorer: "rust-owned-webview-slot=ready policy=locked".to_string(),
        monster: "local-compute=ready proof-cache=ready".to_string(),
        brain: "evidence-aware-memory=ready godel=ready".to_string(),
        provider: "provider=openai ready=true source=local-auth".to_string(),
        proof: "electron-cutover-rust-projection".to_string(),
        gpu_probe,
    };
    let mut projection = ElectronBackendProjection {
        schema: "ingen.native_services.electron_backend_projection.v1",
        source: "examples/ingen_native_services",
        backend: "rust",
        generated_at_unix_ms,
        active_section: "forge",
        section_title: "Forge",
        sessions,
        transcript,
        native_status,
        proof_hash: String::new(),
    };
    projection.proof_hash = proof_hash(&projection);
    projection
}

fn banger_status_from_probe(probe: &NativeGpuAdapterProbe) -> String {
    match &probe.selected {
        Some(adapter) => format!(
            "child-window-slot=ready renderer=rust-banger/wgpu selected={} backend={} type={} adapters={}",
            adapter.name,
            adapter.backend,
            adapter.device_type,
            probe.adapters.len()
        ),
        None => "child-window-slot=pending renderer=rust-banger/wgpu selected=unavailable adapters=0".to_string(),
    }
}

fn proof_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("projection hash serialization");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
