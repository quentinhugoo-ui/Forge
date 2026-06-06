use crate::banger_viewport::{
    render_banger_viewport_frame_with, BangerViewportRequest, BANGER_VIEWPORT_HEIGHT,
    BANGER_VIEWPORT_WIDTH,
};
use crate::state::{NativeJob, NativeJobStatus, ProviderStatus};
use crate::webview_probe::{run_webview_probe, WebViewProbe};
use crate::wgpu_probe::{run_wgpu_probe, WgpuProbe};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{sync::mpsc, thread, time::Duration};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSnapshot {
    pub os: String,
    pub gpu: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSnapshot {
    pub hardware: HardwareSnapshot,
    pub provider: ProviderStatus,
    pub sessions: Vec<SessionSummary>,
    pub jobs: Vec<NativeJob>,
    pub brain_status: String,
    pub monster_status: String,
    pub banger_status: String,
    pub webexplorer_status: String,
    pub trading_status: String,
    pub real_estate_status: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStreamEvent {
    pub job: NativeJob,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NativeServiceCommand {
    RefreshSnapshot,
    CaptureControl { section: String, label: String },
    SubmitChat { draft: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeServiceCommandResult {
    pub accepted: bool,
    pub message: String,
    pub snapshot_hash: String,
    pub emitted_jobs: Vec<NativeJob>,
    pub proof_hash: String,
}

pub trait NativeUiServices {
    fn hardware(&self) -> HardwareSnapshot;
    fn provider_status(&self) -> ProviderStatus;
    fn sessions(&self) -> Vec<SessionSummary>;
    fn jobs(&self) -> Vec<NativeJob>;
    fn brain_status(&self) -> String;
    fn monster_status(&self) -> String;
    fn banger_status(&self) -> String;
    fn webexplorer_status(&self) -> String;
    fn trading_status(&self) -> String;
    fn real_estate_status(&self) -> String;

    fn snapshot(&self) -> ServiceSnapshot {
        let mut snapshot = ServiceSnapshot {
            hardware: self.hardware(),
            provider: self.provider_status(),
            sessions: self.sessions(),
            jobs: self.jobs(),
            brain_status: self.brain_status(),
            monster_status: self.monster_status(),
            banger_status: self.banger_status(),
            webexplorer_status: self.webexplorer_status(),
            trading_status: self.trading_status(),
            real_estate_status: self.real_estate_status(),
            proof_hash: String::new(),
        };
        snapshot.proof_hash = service_snapshot_hash(&snapshot);
        snapshot
    }
}

pub trait NativeCommandServices: NativeUiServices {
    fn handle_command(&self, command: NativeServiceCommand) -> NativeServiceCommandResult {
        let snapshot = self.snapshot();
        let emitted_jobs = match &command {
            NativeServiceCommand::SubmitChat { draft } if !draft.trim().is_empty() => {
                vec![NativeJob {
                    id: format!("direct-chat-{}", stable_label_hash(draft)),
                    label: draft.trim().to_string(),
                    status: NativeJobStatus::Queued,
                    proof_hash: None,
                }]
            }
            _ => Vec::new(),
        };
        let accepted = !matches!(
            &command,
            NativeServiceCommand::SubmitChat { draft } if draft.trim().is_empty()
        );
        let message = match &command {
            NativeServiceCommand::RefreshSnapshot => {
                "direct Rust service snapshot refreshed without browser IPC".to_string()
            }
            NativeServiceCommand::CaptureControl { section, label } => format!(
                "direct Rust service captured control '{label}' in section '{section}'"
            ),
            NativeServiceCommand::SubmitChat { draft } if draft.trim().is_empty() => {
                "empty chat command rejected by direct Rust service".to_string()
            }
            NativeServiceCommand::SubmitChat { draft } => {
                format!("direct Rust service queued chat command '{}'", draft.trim())
            }
        };
        let mut result = NativeServiceCommandResult {
            accepted,
            message,
            snapshot_hash: snapshot.proof_hash,
            emitted_jobs,
            proof_hash: String::new(),
        };
        result.proof_hash = service_command_result_hash(&result);
        result
    }
}

impl<T: NativeUiServices> NativeCommandServices for T {}

#[derive(Clone, Debug)]
pub struct DirectNativeServices {
    wgpu: WgpuProbe,
    webview: WebViewProbe,
}

impl DirectNativeServices {
    pub fn probe_local() -> Self {
        Self {
            wgpu: run_wgpu_probe(),
            webview: run_webview_probe(),
        }
    }

    pub fn from_probes(wgpu: WgpuProbe, webview: WebViewProbe) -> Self {
        Self { wgpu, webview }
    }
}

impl NativeUiServices for DirectNativeServices {
    fn hardware(&self) -> HardwareSnapshot {
        let gpu = if self.wgpu.available {
            format!(
                "{} {} texture={}",
                self.wgpu.backend, self.wgpu.adapter_name, self.wgpu.texture_probe
            )
        } else {
            format!(
                "gpu unavailable {}",
                self.wgpu.error.as_deref().unwrap_or("unknown")
            )
        };
        let mut hardware = HardwareSnapshot {
            os: std::env::consts::OS.to_string(),
            gpu,
            proof_hash: String::new(),
        };
        hardware.proof_hash = hardware_hash(&hardware.os, &hardware.gpu);
        hardware
    }

    fn provider_status(&self) -> ProviderStatus {
        ProviderStatus {
            provider: "direct-rust".to_string(),
            model: "local-native-front".to_string(),
            online: true,
        }
    }

    fn sessions(&self) -> Vec<SessionSummary> {
        vec![SessionSummary {
            id: "native-front-stage3".to_string(),
            title: "Migration Front Stage 3".to_string(),
        }]
    }

    fn jobs(&self) -> Vec<NativeJob> {
        vec![NativeJob {
            id: "direct-probe-1".to_string(),
            label: "local wgpu + webview service probe".to_string(),
            status: NativeJobStatus::Done,
            proof_hash: Some(stable_label_hash(&format!(
                "{}:{}",
                self.wgpu.summary(),
                self.webview.summary()
            ))),
        }]
    }

    fn brain_status(&self) -> String {
        "brain=direct service boundary ready".to_string()
    }

    fn monster_status(&self) -> String {
        format!(
            "monster=direct boundary gpu={} texture={}",
            self.wgpu.adapter_name, self.wgpu.texture_probe
        )
    }

    fn banger_status(&self) -> String {
        let frame = render_banger_viewport_frame_with(
            BangerViewportRequest {
                scene_id: "stage4-native-fixture".to_string(),
                width: BANGER_VIEWPORT_WIDTH,
                height: BANGER_VIEWPORT_HEIGHT,
                frame_index: 0,
            },
            self.wgpu.clone(),
        );
        format!(
            "banger=stage4-native viewport={}x{} backend={} texture_probe={} frame={} bridge={}",
            frame.width,
            frame.height,
            self.wgpu.backend,
            self.wgpu.texture_probe,
            frame.frame_hash,
            frame.slint_texture_bridge.proof_hash
        )
    }

    fn webexplorer_status(&self) -> String {
        format!(
            "webexplorer=direct {} child={} focus_resize_proof={}",
            self.webview.backend,
            self.webview.child_view_required,
            self.webview.focus_resize_proof_required
        )
    }

    fn trading_status(&self) -> String {
        "trading=direct service boundary ready no-browser-ipc".to_string()
    }

    fn real_estate_status(&self) -> String {
        "real-estate=direct service boundary ready no-browser-ipc".to_string()
    }
}

#[derive(Clone, Debug)]
pub struct FakeNativeServices {
    hardware: HardwareSnapshot,
    provider: ProviderStatus,
    sessions: Vec<SessionSummary>,
    jobs: Vec<NativeJob>,
    brain_status: String,
    monster_status: String,
    banger_status: String,
    webexplorer_status: String,
    trading_status: String,
    real_estate_status: String,
}

impl Default for FakeNativeServices {
    fn default() -> Self {
        let mut hardware = HardwareSnapshot {
            os: std::env::consts::OS.to_string(),
            gpu: "fixture-gpu".to_string(),
            proof_hash: String::new(),
        };
        hardware.proof_hash = hardware_hash(&hardware.os, &hardware.gpu);
        Self {
            hardware,
            provider: ProviderStatus {
                provider: "fixture-provider".to_string(),
                model: "fixture-model".to_string(),
                online: true,
            },
            sessions: vec![SessionSummary {
                id: "native-front-migration".to_string(),
                title: "Migration Front".to_string(),
            }],
            jobs: vec![NativeJob {
                id: "fixture-job-1".to_string(),
                label: "deterministic native service proof".to_string(),
                status: NativeJobStatus::Done,
                proof_hash: Some("fixture-proof".to_string()),
            }],
            brain_status: "brain=fake evidence-aware".to_string(),
            monster_status: "monster=fake compute-library".to_string(),
            banger_status: "banger=fake viewport-service".to_string(),
            webexplorer_status: "webexplorer=fake peripheral-service".to_string(),
            trading_status: "trading=fake dashboard-service".to_string(),
            real_estate_status: "real-estate=fake vertical-service".to_string(),
        }
    }
}

impl NativeUiServices for FakeNativeServices {
    fn hardware(&self) -> HardwareSnapshot {
        self.hardware.clone()
    }

    fn provider_status(&self) -> ProviderStatus {
        self.provider.clone()
    }

    fn sessions(&self) -> Vec<SessionSummary> {
        self.sessions.clone()
    }

    fn jobs(&self) -> Vec<NativeJob> {
        self.jobs.clone()
    }

    fn brain_status(&self) -> String {
        self.brain_status.clone()
    }

    fn monster_status(&self) -> String {
        self.monster_status.clone()
    }

    fn banger_status(&self) -> String {
        self.banger_status.clone()
    }

    fn webexplorer_status(&self) -> String {
        self.webexplorer_status.clone()
    }

    fn trading_status(&self) -> String {
        self.trading_status.clone()
    }

    fn real_estate_status(&self) -> String {
        self.real_estate_status.clone()
    }
}

pub fn fake_service_snapshot() -> ServiceSnapshot {
    FakeNativeServices::default().snapshot()
}

pub fn local_service_snapshot() -> ServiceSnapshot {
    DirectNativeServices::probe_local().snapshot()
}

pub fn local_service_command(command: NativeServiceCommand) -> NativeServiceCommandResult {
    DirectNativeServices::probe_local().handle_command(command)
}

pub fn spawn_fake_long_job(label: String) -> mpsc::Receiver<ServiceStreamEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let job_id = format!("fake-long-{}", stable_label_hash(&label));
        let events = [
            (NativeJobStatus::Queued, None),
            (NativeJobStatus::Running, None),
            (NativeJobStatus::Done, Some(stable_label_hash(&label))),
        ];
        for (index, (status, proof_hash)) in events.into_iter().enumerate() {
            let event = ServiceStreamEvent {
                job: NativeJob {
                    id: job_id.clone(),
                    label: label.clone(),
                    status,
                    proof_hash,
                },
                sequence: index as u64,
            };
            if sender.send(event).is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
    });
    receiver
}

fn hardware_hash(os: &str, gpu: &str) -> String {
    let bytes = serde_json::to_vec(&(os, gpu)).expect("serialize hardware snapshot");
    format!("{:x}", Sha256::digest(bytes))
}

fn stable_label_hash(label: &str) -> String {
    format!("{:x}", Sha256::digest(label.as_bytes()))
}

fn service_snapshot_hash(snapshot: &ServiceSnapshot) -> String {
    let bytes = serde_json::to_vec(&(
        &snapshot.hardware,
        &snapshot.provider,
        &snapshot.sessions,
        &snapshot.jobs,
        &snapshot.brain_status,
        &snapshot.monster_status,
        &snapshot.banger_status,
        &snapshot.webexplorer_status,
        &snapshot.trading_status,
        &snapshot.real_estate_status,
    ))
    .expect("serialize service snapshot");
    format!("{:x}", Sha256::digest(bytes))
}

fn service_command_result_hash(result: &NativeServiceCommandResult) -> String {
    let bytes = serde_json::to_vec(&(
        result.accepted,
        &result.message,
        &result.snapshot_hash,
        &result.emitted_jobs,
    ))
    .expect("serialize native service command result");
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_services_snapshot_is_deterministic() {
        let first = fake_service_snapshot();
        let second = fake_service_snapshot();

        assert_eq!(first, second);
        assert_eq!(first.provider.provider, "fixture-provider");
        assert_eq!(first.jobs[0].status, NativeJobStatus::Done);
        assert!(first.monster_status.contains("compute-library"));
        assert!(first.webexplorer_status.contains("peripheral"));
        assert!(!first.proof_hash.is_empty());
    }

    #[test]
    fn fake_long_job_streams_statuses_in_order() {
        let receiver = spawn_fake_long_job("fixture command".to_string());
        let mut statuses = Vec::new();
        let mut final_hash = None;
        for _ in 0..3 {
            let event = receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("fake long job event");
            statuses.push(event.job.status);
            final_hash = event.job.proof_hash;
        }

        assert_eq!(
            statuses,
            vec![
                NativeJobStatus::Queued,
                NativeJobStatus::Running,
                NativeJobStatus::Done
            ]
        );
        assert!(final_hash.is_some());
    }

    #[test]
    fn direct_services_project_real_probe_boundaries() {
        let services = DirectNativeServices::from_probes(
            WgpuProbe::synthetic_available(),
            WebViewProbe::synthetic_windows_capability(),
        );
        let snapshot = services.snapshot();

        assert!(snapshot.hardware.gpu.contains("Vulkan"));
        assert_eq!(snapshot.provider.provider, "direct-rust");
        assert!(snapshot.webexplorer_status.contains("wry/WebView2"));
        assert!(snapshot.banger_status.contains("texture_probe=true"));
        assert!(!snapshot.proof_hash.is_empty());
    }

    #[test]
    fn direct_service_commands_are_proofed_without_browser_ipc() {
        let services = DirectNativeServices::from_probes(
            WgpuProbe::synthetic_available(),
            WebViewProbe::synthetic_windows_capability(),
        );

        let result = services.handle_command(NativeServiceCommand::SubmitChat {
            draft: "/newcompute_ sdf".to_string(),
        });

        assert!(result.accepted);
        assert_eq!(result.emitted_jobs.len(), 1);
        assert!(result.message.contains("direct Rust service"));
        assert!(!result.snapshot_hash.is_empty());
        assert!(!result.proof_hash.is_empty());
    }
}
