use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::BTreeMap;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::forge_fbc_host;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeKernelProjection {
    pub seq: u64,
    pub shell: JsonValue,
    pub section: JsonValue,
    pub left_panel: JsonValue,
    pub canvas: JsonValue,
    pub chatbar: JsonValue,
    pub right_panel: JsonValue,
    pub jobs: JsonValue,
    pub hardware: Option<JsonValue>,
    pub mode: String,
    pub active_section: String,
    pub active_sections: Vec<String>,
    pub left_panel_collapsed: bool,
    pub panels: BTreeMap<String, bool>,
    pub overlays: BTreeMap<String, bool>,
    pub last_window_control: Option<String>,
    pub last_window_label: Option<String>,
    pub onboarding: BTreeMap<String, String>,
    pub last_event_kind: Option<String>,
    pub last_fbc_proof: Option<JsonValue>,
}

#[derive(Debug)]
pub struct ForgeKernel {
    event_path: PathBuf,
    seq: u64,
    phase: String,
    mode: String,
    active_section: String,
    active_sections: Vec<String>,
    left_panel_collapsed: bool,
    canvas: BTreeMap<String, JsonValue>,
    chatbar: BTreeMap<String, JsonValue>,
    right_panel: BTreeMap<String, JsonValue>,
    jobs: BTreeMap<String, JsonValue>,
    panels: BTreeMap<String, bool>,
    overlays: BTreeMap<String, bool>,
    last_window_control: Option<String>,
    last_window_label: Option<String>,
    onboarding: BTreeMap<String, String>,
    hardware: Option<JsonValue>,
    last_event_kind: Option<String>,
    last_fbc_proof: Option<JsonValue>,
}

impl ForgeKernel {
    pub fn new(store_path: PathBuf) -> Self {
        let mut kernel = Self {
            event_path: store_path.join("forge_kernel_events.jsonl"),
            seq: 0,
            phase: "boot".to_string(),
            mode: "forge".to_string(),
            active_section: "alpha".to_string(),
            active_sections: vec!["alpha".to_string()],
            left_panel_collapsed: false,
            canvas: BTreeMap::new(),
            chatbar: BTreeMap::new(),
            right_panel: BTreeMap::new(),
            jobs: BTreeMap::new(),
            panels: BTreeMap::new(),
            overlays: BTreeMap::new(),
            last_window_control: None,
            last_window_label: None,
            onboarding: BTreeMap::new(),
            hardware: None,
            last_event_kind: None,
            last_fbc_proof: None,
        };
        kernel.replay();
        kernel
    }

    pub fn project(&self) -> ForgeKernelProjection {
        let shell = json!({
            "seq": self.seq,
            "phase": self.phase.clone(),
            "mode": self.mode.clone(),
            "lastEventKind": self.last_event_kind.clone(),
            "lastFbcProof": self.last_fbc_proof.clone(),
        });
        let section = json!({
            "active": self.active_section.clone(),
            "activeSections": self.active_sections.clone(),
        });
        let left_panel = json!({
            "collapsed": self.left_panel_collapsed,
        });
        let right_panel = json!({
            "open": self.right_panel.get("open").and_then(JsonValue::as_bool).unwrap_or(false),
            "mode": self.right_panel.get("mode").and_then(JsonValue::as_str).unwrap_or("proof"),
            "state": self.right_panel.clone(),
        });
        ForgeKernelProjection {
            seq: self.seq,
            shell,
            section,
            left_panel,
            canvas: json!(self.canvas.clone()),
            chatbar: json!(self.chatbar.clone()),
            right_panel,
            jobs: json!(self.jobs.clone()),
            hardware: self.hardware.clone(),
            mode: self.mode.clone(),
            active_section: self.active_section.clone(),
            active_sections: self.active_sections.clone(),
            left_panel_collapsed: self.left_panel_collapsed,
            panels: self.panels.clone(),
            overlays: self.overlays.clone(),
            last_window_control: self.last_window_control.clone(),
            last_window_label: self.last_window_label.clone(),
            onboarding: self.onboarding.clone(),
            last_event_kind: self.last_event_kind.clone(),
            last_fbc_proof: self.last_fbc_proof.clone(),
        }
    }

    pub fn apply(&mut self, op: &str, payload: JsonValue) -> Result<ForgeKernelProjection, String> {
        let op = op.trim();
        if op == "snapshot" {
            return Ok(self.project());
        }
        let (emitted_op, emitted_payload, proof) = self.execute_fbc_kernel_project(op, &payload)?;
        self.mutate(&emitted_op, &emitted_payload)?;
        self.last_fbc_proof = Some(proof.clone());
        self.record(&emitted_op, payload_with_fbc_proof(emitted_payload, proof))?;
        Ok(self.project())
    }

    fn execute_fbc_kernel_project(
        &self,
        op: &str,
        payload: &JsonValue,
    ) -> Result<(String, JsonValue, JsonValue), String> {
        let response = forge_fbc_host::execute_kernel_project(None, &self.active_section, op, payload)?;
        let emitted_op = response
            .emitted_op
            .ok_or_else(|| format!("FBC kernel projection missing op for {op}"))?;
        let emitted_payload = response
            .emitted_payload
            .ok_or_else(|| format!("FBC kernel projection missing payload for {op}"))?;
        let mut proof = response.proof;
        if let Some(object) = proof.as_object_mut() {
            object.insert("source".to_string(), json!("forge_kernel"));
        }
        Ok((emitted_op, emitted_payload, proof))
    }

    fn mutate(&mut self, op: &str, payload: &JsonValue) -> Result<(), String> {
        match op {
            "activate_section" => {
                if let Some(section) = payload.get("section").and_then(JsonValue::as_str) {
                    let section = normalize_section(section);
                    self.active_section = section.clone();
                    if !self.active_sections.iter().any(|item| item == &section) {
                        self.active_sections.push(section);
                    }
                }
            }
            "set_section_active" => {
                let section = payload
                    .get("section")
                    .and_then(JsonValue::as_str)
                    .map(normalize_section)
                    .unwrap_or_else(|| self.active_section.clone());
                let active = payload.get("active").and_then(JsonValue::as_bool).unwrap_or(true);
                if active {
                    if !self.active_sections.iter().any(|item| item == &section) {
                        self.active_sections.push(section.clone());
                    }
                    self.active_section = section;
                } else {
                    self.active_sections.retain(|item| item != &section);
                    if self.active_section == section {
                        self.active_section = if self.mode == "agence-immo" {
                            "real-estate-main".to_string()
                        } else {
                            "alpha".to_string()
                        };
                    }
                }
            }
            "set_surface_active" => {
                let section = payload
                    .get("section")
                    .and_then(JsonValue::as_str)
                    .map(normalize_section)
                    .unwrap_or_else(|| self.active_section.clone());
                let active = payload.get("active").and_then(JsonValue::as_bool).unwrap_or(true);
                if active {
                    if !self.active_sections.iter().any(|item| item == &section) {
                        self.active_sections.push(section.clone());
                    }
                    self.active_section = section;
                } else {
                    self.active_sections.retain(|item| item != &section);
                    let fallback = payload
                        .get("fallbackSection")
                        .and_then(JsonValue::as_str)
                        .map(normalize_section)
                        .unwrap_or_else(|| {
                            if self.mode == "agence-immo" {
                                "real-estate-main".to_string()
                            } else {
                                "alpha".to_string()
                            }
                        });
                    self.active_section = fallback.clone();
                    if !self.active_sections.iter().any(|item| item == &fallback) {
                        self.active_sections.push(fallback);
                    }
                }
            }
            "set_mode" => {
                if let Some(mode) = payload.get("mode").and_then(JsonValue::as_str) {
                    self.mode = normalize_mode(mode);
                    if self.mode == "agence-immo" && self.active_section == "alpha" {
                        self.active_section = "real-estate-main".to_string();
                    } else if self.mode == "forge" {
                        self.panels.insert("real-estate-tools".to_string(), false);
                        self.panels.insert("real-estate-contacts".to_string(), false);
                        self.overlays.insert("real-estate-tools".to_string(), false);
                        self.overlays.insert("real-estate-contacts".to_string(), false);
                        self.onboarding.insert("scope".to_string(), "real-estate".to_string());
                        self.onboarding.insert("status".to_string(), "idle".to_string());
                        self.onboarding.insert("questionId".to_string(), String::new());
                    }
                    if !self.active_sections.iter().any(|item| item == &self.active_section) {
                        self.active_sections.push(self.active_section.clone());
                    }
                }
            }
            "set_real_estate_mode" => {
                let active = payload.get("active").and_then(JsonValue::as_bool).unwrap_or(false);
                let web_explorer_active = payload
                    .get("webExplorerActive")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false);
                self.mode = if active { "agence-immo" } else { "forge" }.to_string();
                if !active {
                    self.panels.insert("real-estate-tools".to_string(), false);
                    self.panels.insert("real-estate-contacts".to_string(), false);
                    self.overlays.insert("real-estate-tools".to_string(), false);
                    self.overlays.insert("real-estate-contacts".to_string(), false);
                    self.onboarding.insert("scope".to_string(), "real-estate".to_string());
                    self.onboarding.insert("status".to_string(), "idle".to_string());
                    self.onboarding.insert("questionId".to_string(), String::new());
                }
                set_active_flag(&mut self.active_sections, "real-estate", active);
                set_active_flag(
                    &mut self.active_sections,
                    "real-estate-main",
                    active && !web_explorer_active,
                );
                set_active_flag(&mut self.active_sections, "alpha", !active);
                self.active_section = if active {
                    if web_explorer_active {
                        "webexplorer".to_string()
                    } else {
                        "real-estate-main".to_string()
                    }
                } else {
                    "alpha".to_string()
                };
                if !self.active_sections.iter().any(|item| item == &self.active_section) {
                    self.active_sections.push(self.active_section.clone());
                }
            }
            "toggle_left_panel" => {
                self.left_panel_collapsed = !self.left_panel_collapsed;
            }
            "set_canvas" => {
                merge_json_object(&mut self.canvas, payload);
            }
            "set_chatbar" => {
                merge_json_object(&mut self.chatbar, payload);
            }
            "set_right_panel" => {
                merge_json_object(&mut self.right_panel, payload);
            }
            "set_jobs" => {
                merge_json_object(&mut self.jobs, payload);
            }
            "fbc_runtime_snapshot" => {
                self.jobs.insert("fbcRuntime".to_string(), payload.clone());
                self.right_panel.insert("mode".to_string(), json!("proof"));
                self.right_panel.insert("open".to_string(), json!(true));
                self.right_panel.insert("fbcRuntime".to_string(), payload.clone());
            }
            "fbc_guard" => {
                let proof = payload
                    .get("_fbcProof")
                    .cloned()
                    .or_else(|| payload.get("proof").cloned())
                    .unwrap_or_else(|| payload.clone());
                self.last_fbc_proof = Some(proof.clone());
                self.right_panel.insert("mode".to_string(), json!("proof"));
                self.right_panel.insert("open".to_string(), json!(true));
                self.right_panel.insert("fbcGuard".to_string(), proof);
            }
            "set_panel" => {
                if let Some(panel) = payload.get("panel").and_then(JsonValue::as_str) {
                    let open = payload.get("open").and_then(JsonValue::as_bool).unwrap_or(false);
                    self.panels.insert(panel.to_string(), open);
                }
            }
            "set_overlay" => {
                if let Some(overlay) = payload.get("overlay").and_then(JsonValue::as_str) {
                    let open = payload.get("open").and_then(JsonValue::as_bool).unwrap_or(false);
                    self.overlays.insert(overlay.to_string(), open);
                }
            }
            "window_control" => {
                if let Some(command) = payload.get("command").and_then(JsonValue::as_str) {
                    self.last_window_control = Some(command.to_string());
                }
                if let Some(label) = payload.get("label").and_then(JsonValue::as_str) {
                    self.last_window_label = Some(label.to_string());
                }
            }
            "set_onboarding" => {
                let scope = payload
                    .get("scope")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("real-estate");
                let status = payload
                    .get("status")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("idle");
                let question_id = payload
                    .get("questionId")
                    .or_else(|| payload.get("question_id"))
                    .and_then(JsonValue::as_str)
                    .unwrap_or("");
                self.onboarding.insert("scope".to_string(), scope.to_string());
                self.onboarding.insert("status".to_string(), normalize_onboarding_status(status));
                self.onboarding.insert("questionId".to_string(), question_id.to_string());
            }
            "hardware_observed" => {
                self.hardware = payload.get("hardware").cloned().or(Some(payload.clone()));
            }
            "boot_ready" => {
                self.phase = "ready".to_string();
            }
            "boot_error" => {
                self.phase = "error".to_string();
            }
            _ => return Err(format!("unknown forge kernel op: {op}")),
        }
        Ok(())
    }

    fn record(&mut self, kind: &str, payload: JsonValue) -> Result<(), String> {
        self.seq = self.seq.saturating_add(1);
        self.last_event_kind = Some(kind.to_string());
        if let Some(parent) = self.event_path.parent() {
            create_dir_all(parent).map_err(|err| format!("kernel event dir: {err}"))?;
        }
        let event = json!({
            "seq": self.seq,
            "atMs": now_ms(),
            "kind": kind,
            "payload": payload,
            "projection": self.project(),
        });
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.event_path)
            .map_err(|err| format!("kernel event log open: {err}"))?;
        serde_json::to_writer(&mut file, &event).map_err(|err| format!("kernel event encode: {err}"))?;
        file.write_all(b"\n")
            .map_err(|err| format!("kernel event write: {err}"))
    }

    fn replay(&mut self) {
        let Ok(file) = File::open(&self.event_path) else {
            return;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(event) = serde_json::from_str::<JsonValue>(&line) else {
                continue;
            };
            let Some(kind) = event.get("kind").and_then(JsonValue::as_str) else {
                continue;
            };
            let payload = event.get("payload").cloned().unwrap_or(JsonValue::Null);
            if let Some(seq) = event.get("seq").and_then(JsonValue::as_u64) {
                self.seq = self.seq.max(seq);
            }
            let _ = self.mutate(kind, &payload);
            if let Some(proof) = payload.get("_fbcProof").cloned() {
                self.last_fbc_proof = Some(proof);
            }
            self.last_event_kind = Some(kind.to_string());
        }
    }
}

#[tauri::command]
pub fn forge_kernel(
    state: tauri::State<'_, Mutex<ForgeKernel>>,
    op: String,
    payload: Option<JsonValue>,
) -> Result<ForgeKernelProjection, String> {
    state
        .lock()
        .map_err(|err| err.to_string())?
        .apply(&op, payload.unwrap_or(JsonValue::Null))
}

pub fn record_window_control(state: &Mutex<ForgeKernel>, command: &str, label: Option<&str>) {
    if let Ok(mut kernel) = state.lock() {
        let _ = kernel.apply("window_control", json!({ "command": command, "label": label }));
    }
}

pub fn record_hardware(state: &Mutex<ForgeKernel>, hardware: JsonValue) {
    if let Ok(mut kernel) = state.lock() {
        let _ = kernel.apply("hardware_observed", json!({ "hardware": hardware }));
    }
}

pub fn record_fbc_runtime_snapshot(state: &Mutex<ForgeKernel>, snapshot: JsonValue) {
    if let Ok(mut kernel) = state.lock() {
        let _ = kernel.apply("fbc_runtime_snapshot", snapshot);
    }
}

pub fn record_preverified_fbc_guard(state: &Mutex<ForgeKernel>, proof: JsonValue) {
    if let Ok(mut kernel) = state.lock() {
        kernel.last_fbc_proof = Some(proof.clone());
        let _ = kernel.mutate("fbc_guard", &json!({ "_fbcProof": proof.clone() }));
        let _ = kernel.record("fbc_guard", json!({ "_fbcProof": proof }));
    }
}

fn normalize_mode(mode: &str) -> String {
    match mode {
        "agence-immo" | "real-estate" | "immo" => "agence-immo".to_string(),
        _ => "forge".to_string(),
    }
}

fn normalize_section(section: &str) -> String {
    match section {
        "forge" | "webexplorer" | "real-estate" | "real-estate-main" | "trading" | "banger" => {
            section.to_string()
        }
        _ => "alpha".to_string(),
    }
}

fn set_active_flag(active_sections: &mut Vec<String>, section: &str, active: bool) {
    if active {
        if !active_sections.iter().any(|item| item == section) {
            active_sections.push(section.to_string());
        }
    } else {
        active_sections.retain(|item| item != section);
    }
}

fn merge_json_object(target: &mut BTreeMap<String, JsonValue>, payload: &JsonValue) {
    let Some(object) = payload.as_object() else {
        return;
    };
    for (key, value) in object {
        target.insert(key.clone(), value.clone());
    }
}

fn payload_with_fbc_proof(payload: JsonValue, proof: JsonValue) -> JsonValue {
    match payload {
        JsonValue::Object(mut object) => {
            object.insert("_fbcProof".to_string(), proof);
            JsonValue::Object(object)
        }
        other => json!({
            "value": other,
            "_fbcProof": proof,
        }),
    }
}

fn normalize_onboarding_status(status: &str) -> String {
    match status {
        "initializing" | "asking" | "complete" | "error" => status.to_string(),
        _ => "idle".to_string(),
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
