//! Banger engine — native GPU lifecycle (P1a).
//!
//! Owns a wgpu Instance / Adapter / Device / Queue tied to the Banger overlay's
//! lifecycle. The frontend calls [`engine_start`] / [`engine_stop`] in sync with
//! its own state machine (active / suspended / shutdown), so the native GPU
//! resources only live while the user is actually viewing Banger.
//!
//! P1a is rendering-agnostic — it just proves the lifecycle plumbing. Native
//! rendering (P1b) will reuse the same Device/Queue without changing the IPC
//! surface.

use std::{
    env,
    fs,
    path::PathBuf,
    process::Command,
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// State exposed to the frontend.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EngineState {
    Idle,
    Active,
    Stopped, // explicitly stopped (suspend / shutdown distinction lives in JS)
}

/// Snapshot returned by `banger_engine_status`.
#[derive(Clone, Debug, Serialize)]
pub struct EngineStatus {
    pub state: EngineState,
    pub backend: Option<String>,    // "Vulkan" / "Dx12" / "Metal" / ...
    pub adapter_name: Option<String>,
    pub adapter_kind: Option<String>, // "DiscreteGpu" / "IntegratedGpu" / ...
    pub vendor_id: Option<u32>,
}

impl EngineStatus {
    fn idle() -> Self {
        Self {
            state: EngineState::Idle,
            backend: None,
            adapter_name: None,
            adapter_kind: None,
            vendor_id: None,
        }
    }
}

/// Live native GPU resources. Dropping this struct frees the Device/Queue.
struct Live {
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    _device: wgpu::Device,
    _queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
}

/// Engine wrapper. `Mutex<Option<Live>>` so we can claim/release on demand.
pub struct BangerEngine {
    live: Mutex<Option<Live>>,
}

impl BangerEngine {
    pub const fn new() -> Self {
        Self { live: Mutex::new(None) }
    }

    /// Idempotent: returns the current status whether already active or freshly started.
    pub fn start(&self) -> Result<EngineStatus, String> {
        let mut guard = self.live.lock().map_err(|e| format!("engine lock poisoned: {e}"))?;
        if let Some(live) = guard.as_ref() {
            return Ok(status_from(live, EngineState::Active));
        }

        // wgpu 29's InstanceDescriptor has several non-Default fields whose
        // surface keeps shifting between minor releases; the safest cross-version
        // path is `Instance::default()`, which honours WGPU_BACKEND env vars and
        // picks the right primary backend per OS.
        let instance = wgpu::Instance::default();

        // High-performance discrete GPU first; fall back to whatever we get.
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|e| format!("no GPU adapter available: {e}"))?;

        let info = adapter.get_info();

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("banger-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("device request failed: {e}"))?;

        let live = Live {
            _instance: instance,
            _adapter: adapter,
            _device: device,
            _queue: queue,
            info,
        };
        let snapshot = status_from(&live, EngineState::Active);
        *guard = Some(live);
        Ok(snapshot)
    }

    /// Idempotent: drops Device/Queue if alive, otherwise no-op.
    pub fn stop(&self) -> Result<EngineStatus, String> {
        let mut guard = self.live.lock().map_err(|e| format!("engine lock poisoned: {e}"))?;
        guard.take(); // explicit drop while holding the lock
        Ok(EngineStatus::idle().with_state(EngineState::Stopped))
    }

    pub fn status(&self) -> Result<EngineStatus, String> {
        let guard = self.live.lock().map_err(|e| format!("engine lock poisoned: {e}"))?;
        Ok(match guard.as_ref() {
            Some(live) => status_from(live, EngineState::Active),
            None => EngineStatus::idle(),
        })
    }
}

impl EngineStatus {
    fn with_state(mut self, s: EngineState) -> Self { self.state = s; self }
}

fn status_from(live: &Live, state: EngineState) -> EngineStatus {
    EngineStatus {
        state,
        backend: Some(format!("{:?}", live.info.backend)),
        adapter_name: Some(live.info.name.clone()),
        adapter_kind: Some(format!("{:?}", live.info.device_type)),
        vendor_id: Some(live.info.vendor),
    }
}

// ---------- Tauri command wrappers ----------

#[tauri::command]
pub fn banger_engine_start(state: tauri::State<'_, BangerEngine>) -> Result<EngineStatus, String> {
    state.start()
}

#[tauri::command]
pub fn banger_engine_stop(state: tauri::State<'_, BangerEngine>) -> Result<EngineStatus, String> {
    state.stop()
}

#[tauri::command]
pub fn banger_engine_status(state: tauri::State<'_, BangerEngine>) -> Result<EngineStatus, String> {
    state.status()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPrinterDevice {
    pub id: String,
    pub port: Option<String>,
    pub name: String,
    pub vendor: Option<String>,
    pub status: Option<String>,
    pub source: String,
    pub likely_3d_printer: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterProfile {
    pub id: String,
    pub label: String,
    pub nozzle_mm: f32,
    pub build_volume_mm: [u32; 3],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterDiscoveryResponse {
    pub detected: Vec<DetectedPrinterDevice>,
    pub profiles: Vec<PrinterProfile>,
    pub warnings: Vec<String>,
    pub backend: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoomRustConsoleRunRequest {
    pub script: String,
    pub context_json: String,
    pub script_hash: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoomRustConsoleRunResponse {
    pub ok: bool,
    pub status: String,
    pub run_hash: String,
    pub script_hash: String,
    pub compiler: Option<String>,
    pub stored_at: String,
    pub stdout: String,
    pub stderr: String,
    pub binary_path: Option<String>,
    pub source_path: String,
    pub exit_code: Option<i32>,
}

fn boom_store_dir() -> PathBuf {
    if let Some(raw) = env::var_os("FORGE_STORE_DIR") {
        let trimmed = raw.to_string_lossy().trim().to_string();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    crate::forge_store_dir()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn rust_string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 16);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn find_rustc_executable() -> Option<PathBuf> {
    if let Some(raw) = env::var_os("RUSTC") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Some(path);
        }
    }
    for base in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let path = base.join(if cfg!(windows) { "rustc.exe" } else { "rustc" });
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn build_boom_rust_console_source(script: &str, context_json: &str, script_hash: &str) -> String {
    let helpers = format!(
        "const BOOM_CONTEXT_JSON: &str = {};\nconst BOOM_SCRIPT_HASH: &str = {};\nfn boom_context_json() -> &'static str {{ BOOM_CONTEXT_JSON }}\nfn boom_script_hash() -> &'static str {{ BOOM_SCRIPT_HASH }}\nfn boom_println(value: impl AsRef<str>) {{ println!(\"{{}}\", value.as_ref()); }}\n",
        rust_string_literal(context_json),
        rust_string_literal(script_hash),
    );
    if script.contains("fn main") {
        format!("{helpers}\n{script}\n")
    } else {
        format!("{helpers}\nfn main() {{\n{script}\n}}\n")
    }
}

fn default_printer_profiles() -> Vec<PrinterProfile> {
    vec![
        PrinterProfile {
            id: "corexy_220".into(),
            label: "CoreXY 220 × 220 × 250".into(),
            nozzle_mm: 0.4,
            build_volume_mm: [220, 220, 250],
        },
        PrinterProfile {
            id: "bedslinger_220".into(),
            label: "Bedslinger 220 × 220 × 250".into(),
            nozzle_mm: 0.4,
            build_volume_mm: [220, 220, 250],
        },
        PrinterProfile {
            id: "highflow_300".into(),
            label: "High-flow 300 × 300 × 300".into(),
            nozzle_mm: 0.6,
            build_volume_mm: [300, 300, 300],
        },
        PrinterProfile {
            id: "resin_192".into(),
            label: "Resin 192 × 120 × 200".into(),
            nozzle_mm: 0.0,
            build_volume_mm: [192, 120, 200],
        },
    ]
}

fn looks_like_3d_printer_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "printer", "marlin", "klipper", "prusa", "bambu", "voron", "ender", "creality", "anker",
        "elegoo", "anycubic", "snapmaker", "smoothie", "duet", "mks", "usb serial", "ch340",
        "silicon labs", "ft232", "uart",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(target_os = "windows")]
fn detect_windows_printers() -> Result<Vec<DetectedPrinterDevice>, String> {
    let script = r#"
$ports = @()
try {
  $ports = Get-CimInstance Win32_SerialPort | Select-Object DeviceID, Name, Description, Status, PNPDeviceID
} catch {}
$entities = @()
try {
  $entities = Get-CimInstance Win32_PnPEntity | Where-Object {
    $_.Name -match 'Marlin|Klipper|Prusa|Bambu|Voron|Ender|Creality|Anker|Elegoo|Anycubic|Snapmaker|USB Serial|CH340|FT232|Silicon Labs|3D'
  } | Select-Object Name, Manufacturer, Status, PNPDeviceID
} catch {}
[PSCustomObject]@{
  serialPorts = $ports
  entities = $entities
} | ConvertTo-Json -Depth 4
"#;
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .output()
        .map_err(|e| format!("powershell printer detection failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "powershell printer detection exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("printer detection JSON parse failed: {e}"))?;
    let mut devices = Vec::new();
    let serial_ports = value
        .get("serialPorts")
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]));
    let serial_ports = match serial_ports {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => vec![],
        other => vec![other],
    };
    for (index, port) in serial_ports.iter().enumerate() {
        let port_name = port.get("DeviceID").and_then(|v| v.as_str()).map(|s| s.to_string());
        let name = port
            .get("Name")
            .or_else(|| port.get("Description"))
            .and_then(|v| v.as_str())
            .unwrap_or("Serial device")
            .to_string();
        devices.push(DetectedPrinterDevice {
            id: format!("serial-{index}"),
            port: port_name.clone(),
            name: name.clone(),
            vendor: port.get("PNPDeviceID").and_then(|v| v.as_str()).map(|s| s.to_string()),
            status: port.get("Status").and_then(|v| v.as_str()).map(|s| s.to_string()),
            source: "win32_serial_port".into(),
            likely_3d_printer: looks_like_3d_printer_name(&name)
                || port_name.as_deref().unwrap_or_default().to_ascii_lowercase().starts_with("com"),
        });
    }
    let entities = value
        .get("entities")
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]));
    let entities = match entities {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => vec![],
        other => vec![other],
    };
    for (index, entity) in entities.iter().enumerate() {
        let name = entity
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("Printer-like USB device")
            .to_string();
        let already_known = devices.iter().any(|device| device.name == name);
        if already_known {
            continue;
        }
        devices.push(DetectedPrinterDevice {
            id: format!("entity-{index}"),
            port: None,
            name: name.clone(),
            vendor: entity
                .get("Manufacturer")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            status: entity.get("Status").and_then(|v| v.as_str()).map(|s| s.to_string()),
            source: "win32_pnp_entity".into(),
            likely_3d_printer: looks_like_3d_printer_name(&name),
        });
    }
    Ok(devices)
}

#[cfg(not(target_os = "windows"))]
fn detect_windows_printers() -> Result<Vec<DetectedPrinterDevice>, String> {
    Ok(vec![])
}

#[tauri::command]
pub fn banger_detect_printers() -> Result<PrinterDiscoveryResponse, String> {
    let mut warnings = Vec::new();
    let detected = match detect_windows_printers() {
        Ok(devices) => {
            if devices.is_empty() {
                warnings.push(
                    "No printer-like serial or USB device detected yet. Keep using profiles, or connect a printer and rescan."
                        .into(),
                );
            }
            devices
        }
        Err(err) => {
            warnings.push(format!("Printer detection fallback used: {err}"));
            Vec::new()
        }
    };
    Ok(PrinterDiscoveryResponse {
        detected,
        profiles: default_printer_profiles(),
        warnings,
        backend: if cfg!(target_os = "windows") {
            "windows-cim".into()
        } else {
            "profile-only".into()
        },
    })
}

#[tauri::command]
pub fn banger_run_rust_console(
    request: BoomRustConsoleRunRequest,
) -> Result<BoomRustConsoleRunResponse, String> {
    let script = request.script.trim().to_string();
    if script.is_empty() {
        return Err("Rust console script is empty.".into());
    }
    let script_hash = request
        .script_hash
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| sha256_hex(script.as_bytes()));
    let run_hash = sha256_hex(
        format!(
            "boom-rust-console-v1\0{}\0{}",
            script,
            request.context_json
        )
        .as_bytes(),
    );
    let store_dir = boom_store_dir()
        .join("boom-console")
        .join("rust")
        .join(&run_hash);
    fs::create_dir_all(&store_dir).map_err(|e| format!("console store mkdir failed: {e}"))?;
    let source = build_boom_rust_console_source(&script, &request.context_json, &script_hash);
    let source_path = store_dir.join("main.rs");
    let request_path = store_dir.join("request.json");
    fs::write(&source_path, source.as_bytes()).map_err(|e| format!("source write failed: {e}"))?;
    let request_json = serde_json::json!({
        "script_hash": script_hash,
        "run_hash": run_hash,
        "context_json": request.context_json,
        "script": script,
    });
    fs::write(&request_path, serde_json::to_vec_pretty(&request_json).map_err(|e| format!("request json encode failed: {e}"))?)
        .map_err(|e| format!("request write failed: {e}"))?;

    let Some(rustc) = find_rustc_executable() else {
        return Ok(BoomRustConsoleRunResponse {
            ok: false,
            status: "rustc_unavailable".into(),
            run_hash,
            script_hash,
            compiler: None,
            stored_at: store_dir.display().to_string(),
            stdout: String::new(),
            stderr: "Rust toolchain not found in PATH. Install rustc/cargo to run BOOM Rust scripts.".into(),
            binary_path: None,
            source_path: source_path.display().to_string(),
            exit_code: None,
        });
    };

    let binary_path = store_dir.join(if cfg!(windows) { "boom_console_run.exe" } else { "boom_console_run" });
    let compile = Command::new(&rustc)
        .arg("--edition=2021")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .current_dir(&store_dir)
        .output()
        .map_err(|e| format!("rustc launch failed: {e}"))?;
    let compile_stdout = String::from_utf8_lossy(&compile.stdout).to_string();
    let compile_stderr = String::from_utf8_lossy(&compile.stderr).to_string();
    fs::write(store_dir.join("compile.stdout.txt"), &compile_stdout)
        .map_err(|e| format!("compile stdout write failed: {e}"))?;
    fs::write(store_dir.join("compile.stderr.txt"), &compile_stderr)
        .map_err(|e| format!("compile stderr write failed: {e}"))?;
    if !compile.status.success() {
        let result = BoomRustConsoleRunResponse {
            ok: false,
            status: "compile_error".into(),
            run_hash,
            script_hash,
            compiler: Some(rustc.display().to_string()),
            stored_at: store_dir.display().to_string(),
            stdout: compile_stdout,
            stderr: compile_stderr,
            binary_path: None,
            source_path: source_path.display().to_string(),
            exit_code: compile.status.code(),
        };
        fs::write(
            store_dir.join("result.json"),
            serde_json::to_vec_pretty(&result).map_err(|e| format!("result json encode failed: {e}"))?,
        )
        .map_err(|e| format!("result write failed: {e}"))?;
        return Ok(result);
    }

    let run = Command::new(&binary_path)
        .current_dir(&store_dir)
        .output()
        .map_err(|e| format!("compiled rust console run failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run.stderr).to_string();
    fs::write(store_dir.join("stdout.txt"), &stdout).map_err(|e| format!("stdout write failed: {e}"))?;
    fs::write(store_dir.join("stderr.txt"), &stderr).map_err(|e| format!("stderr write failed: {e}"))?;
    let result = BoomRustConsoleRunResponse {
        ok: run.status.success(),
        status: if run.status.success() {
            "ok".into()
        } else {
            "runtime_error".into()
        },
        run_hash,
        script_hash,
        compiler: Some(rustc.display().to_string()),
        stored_at: store_dir.display().to_string(),
        stdout,
        stderr,
        binary_path: Some(binary_path.display().to_string()),
        source_path: source_path.display().to_string(),
        exit_code: run.status.code(),
    };
    fs::write(
        store_dir.join("result.json"),
        serde_json::to_vec_pretty(&result).map_err(|e| format!("result json encode failed: {e}"))?,
    )
    .map_err(|e| format!("result write failed: {e}"))?;
    Ok(result)
}
