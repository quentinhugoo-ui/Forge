use ingen_native_services::banger_native_engine::{
    BangerGaussianSplatRasterizeRequest, BangerNativeEngine,
    BangerNativePresentLoopBootstrapRequest,
};
use ingen_native_services::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapterProbe};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerPreviewFrameProjection {
    accepted: bool,
    schema: &'static str,
    source: &'static str,
    width: u32,
    height: u32,
    frame_data_url: String,
    frame_hash: String,
    scene_hash: String,
    proof_hash: String,
    metrics: BangerPreviewFrameMetrics,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerPreviewFrameMetrics {
    splat_count: usize,
    projected_splat_count: usize,
    rasterized_splat_count: usize,
    shaded_pixel_count: u32,
    tile_count: u32,
    benchmark_gate_count: u32,
    promotion_allowed: bool,
    render_path: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerNativeHostProjection {
    ok: bool,
    schema: &'static str,
    engine: &'static str,
    lane: &'static str,
    native_domain: &'static str,
    route_status: &'static str,
    parent_window_handle_hash: String,
    child_window_handle_hash: String,
    viewport_width: u32,
    viewport_height: u32,
    target_frame_ms: f32,
    selected_adapter: Option<String>,
    adapter_count: usize,
    backend: String,
    surface_kind: &'static str,
    swapchain_format: String,
    present_mode: String,
    alpha_mode: String,
    render_pass_count: u32,
    submitted_frame_count: u32,
    draw_call_count: u32,
    vertex_count: u32,
    clear_color: [f64; 4],
    shader_source_hash: String,
    render_pipeline_hash: String,
    frame_hash: String,
    present_loop_hash: String,
    proof_hash: String,
    host_pid: u32,
    verifier: BangerNativeHostVerifier,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerNativeHostVerifier {
    wall: &'static str,
    frontier_hypothesis: &'static str,
    local_gate: &'static str,
    rollback_path: &'static str,
}

fn main() {
    if env::args().any(|argument| argument == "--banger-native-host") {
        let parent = env::var("FORGE_BANGER_PARENT_HWND").ok();
        let width = env::var("FORGE_BANGER_VIEWPORT_WIDTH").ok().and_then(|value| value.parse().ok()).unwrap_or(1280);
        let height = env::var("FORGE_BANGER_VIEWPORT_HEIGHT").ok().and_then(|value| value.parse().ok()).unwrap_or(720);
        let frame_limit = env::var("FORGE_BANGER_HOST_FRAMES").ok().and_then(|value| value.parse().ok());
        run_banger_native_host(parent.as_deref(), width, height, frame_limit).expect("run banger native host");
        return;
    }
    if env::args().any(|argument| argument == "--banger-present-loop-bootstrap") {
        let frame = BangerNativeEngine::bootstrap_present_loop(BangerNativePresentLoopBootstrapRequest {
            parent_window_handle: env::var("FORGE_BANGER_PARENT_HWND").ok(),
            viewport_width: env::var("FORGE_BANGER_VIEWPORT_WIDTH").ok().and_then(|value| value.parse().ok()),
            viewport_height: env::var("FORGE_BANGER_VIEWPORT_HEIGHT").ok().and_then(|value| value.parse().ok()),
            target_frame_ms: env::var("FORGE_BANGER_TARGET_FRAME_MS").ok().and_then(|value| value.parse().ok()),
        })
        .expect("bootstrap banger native present loop");
        println!("{}", serde_json::to_string(&frame).expect("serialize banger present loop bootstrap"));
        return;
    }
    if env::args().any(|argument| argument == "--banger-preview-frame") {
        let frame = banger_preview_frame();
        println!("{}", serde_json::to_string(&frame).expect("serialize banger preview frame"));
        return;
    }
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

fn banger_preview_frame() -> BangerPreviewFrameProjection {
    let width = 320;
    let height = 200;
    let ply_path = preview_splat_path();
    fs::create_dir_all(ply_path.parent().expect("preview ply parent")).expect("create banger preview dir");
    fs::write(&ply_path, preview_splat_ply()).expect("write banger preview ply");

    let raster = BangerNativeEngine::rasterize_gaussian_splat_asset(BangerGaussianSplatRasterizeRequest {
        asset_id: Some("electron_header_banger_preview".to_string()),
        ply_path: ply_path.to_string_lossy().to_string(),
        width,
        height,
        camera_position: Some([0.0, 0.08, -4.25]),
        camera_target: Some([0.0, 0.02, 0.0]),
        camera_up: Some([0.0, 1.0, 0.0]),
        fov_y_degrees: Some(42.0),
        near_plane: Some(0.01),
        max_splats: None,
        tile_size: Some(16),
        background_rgba: Some([0.015, 0.018, 0.024, 1.0]),
    })
    .expect("rasterize banger preview splats");

    let bmp = rgba8_to_bmp(width, height, &raster.rgba8);
    let frame_hash = sha256_hex(&bmp);
    let scene_hash = sha256_hex(preview_splat_ply().as_bytes());
    let metrics = BangerPreviewFrameMetrics {
        splat_count: raster.splat_count,
        projected_splat_count: raster.projected_splat_count,
        rasterized_splat_count: raster.rasterized_splat_count,
        shaded_pixel_count: raster.shaded_pixel_count,
        tile_count: raster.tile_count,
        benchmark_gate_count: 5,
        promotion_allowed: raster.ok && raster.projected_splat_count > 0 && raster.shaded_pixel_count > 0,
        render_path: "rust_banger_gaussian_splat_rgba8_to_bmp_data_url",
    };
    let mut frame = BangerPreviewFrameProjection {
        accepted: true,
        schema: "forge.banger.visible_preview_frame.v1",
        source: "examples/ingen_native_services/banger_preview_frame",
        width,
        height,
        frame_data_url: format!("data:image/bmp;base64,{}", base64_encode(&bmp)),
        frame_hash,
        scene_hash,
        proof_hash: String::new(),
        metrics,
    };
    frame.proof_hash = proof_hash(&frame);
    frame
}

fn preview_splat_path() -> PathBuf {
    env::temp_dir()
        .join("forge-banger-preview-frame")
        .join("electron_header_banger_preview.ply")
}

fn preview_splat_ply() -> &'static str {
    r#"ply
format ascii 1.0
element vertex 12
property float x
property float y
property float z
property float f_dc_0
property float f_dc_1
property float f_dc_2
property float opacity
property float scale_0
property float scale_1
property float scale_2
property float rot_0
property float rot_1
property float rot_2
property float rot_3
end_header
0.00 0.00 0.00 1.55 0.18 0.10 4.4 -1.14 -1.20 -1.32 1 0 0 0
0.34 0.06 0.10 0.12 1.35 0.30 3.9 -1.33 -1.25 -1.45 1 0 0 0
-0.34 -0.07 0.04 0.14 0.38 1.42 3.8 -1.38 -1.28 -1.48 1 0 0 0
0.02 0.36 0.14 1.26 0.98 0.18 3.6 -1.42 -1.36 -1.54 1 0 0 0
0.00 -0.34 0.16 0.18 1.08 1.24 3.5 -1.44 -1.40 -1.56 1 0 0 0
0.46 0.26 0.28 1.36 0.38 0.86 3.2 -1.58 -1.50 -1.68 1 0 0 0
-0.46 0.23 0.22 0.28 1.22 0.88 3.2 -1.58 -1.50 -1.68 1 0 0 0
0.42 -0.28 0.24 1.12 0.80 0.20 3.1 -1.60 -1.55 -1.70 1 0 0 0
-0.42 -0.28 0.24 0.20 0.72 1.22 3.1 -1.60 -1.55 -1.70 1 0 0 0
0.00 0.02 0.48 1.42 1.28 1.02 2.9 -1.72 -1.64 -1.82 1 0 0 0
0.18 -0.02 -0.22 0.90 0.30 1.18 3.4 -1.50 -1.42 -1.60 1 0 0 0
-0.18 0.03 -0.20 0.30 1.10 1.04 3.4 -1.50 -1.42 -1.60 1 0 0 0
"#
}

fn rgba8_to_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(rgba.len(), width as usize * height as usize * 4);
    let pixel_bytes = width as usize * height as usize * 4;
    let file_size = 54usize + pixel_bytes;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..height as usize).rev() {
        let row = y * width as usize * 4;
        for x in 0..width as usize {
            let offset = row + x * 4;
            bmp.push(rgba[offset + 2]);
            bmp.push(rgba[offset + 1]);
            bmp.push(rgba[offset]);
            bmp.push(rgba[offset + 3]);
        }
    }
    bmp
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(target_os = "windows")]
fn run_banger_native_host(
    parent_window_handle: Option<&str>,
    width: u32,
    height: u32,
    frame_limit: Option<u32>,
) -> Result<BangerNativeHostProjection, String> {
    use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
    use std::ffi::c_void;
    use std::num::NonZeroIsize;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, IsWindow, RegisterClassW, ShowWindow, CS_HREDRAW,
        CS_VREDRAW, SW_SHOW, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
    };

    unsafe extern "system" fn banger_wnd_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    let parent = parse_hwnd(parent_window_handle)
        .ok_or_else(|| "FORGE_BANGER_PARENT_HWND is required for the native child host".to_string())?;
    unsafe {
        if IsWindow(parent) == 0 {
            return Err("FORGE_BANGER_PARENT_HWND does not reference a live Win32 window".to_string());
        }
    }

    let class_name = wide_null("ForgeBangerNativeChildSurface");
    let title = wide_null("Banger Native Surface");
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(banger_wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wc);
    }
    let child = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            0,
            0,
            width.clamp(64, 16384) as i32,
            height.clamp(64, 16384) as i32,
            parent,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        )
    };
    if child.is_null() {
        return Err("failed to create Banger Win32 child window".to_string());
    }
    unsafe {
        ShowWindow(child, SW_SHOW);
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: Default::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: Default::default(),
    });
    let mut child_handle = Win32WindowHandle::new(
        NonZeroIsize::new(child as isize).ok_or_else(|| "Banger child HWND was null".to_string())?,
    );
    child_handle.hinstance = NonZeroIsize::new(hinstance as isize);
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: None,
            raw_window_handle: RawWindowHandle::Win32(child_handle),
        })
    }
    .map_err(|error| format!("failed to create Banger wgpu child surface: {error}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .map_err(|error| format!("failed to select adapter for Banger child surface: {error}"))?;
    let info = adapter.get_info();
    let capabilities = surface.get_capabilities(&adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .or_else(|| capabilities.formats.first().copied())
        .ok_or_else(|| "Banger child surface reported no swapchain formats".to_string())?;
    let present_mode = capabilities
        .present_modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
        .or_else(|| capabilities.present_modes.first().copied())
        .unwrap_or(wgpu::PresentMode::AutoVsync);
    let alpha_mode = capabilities
        .alpha_modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
        .or_else(|| capabilities.alpha_modes.first().copied())
        .unwrap_or(wgpu::CompositeAlphaMode::Opaque);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("banger-native-child-host-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::default(),
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("failed to create Banger child host device: {error}"))?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.clamp(64, 16384),
        height: height.clamp(64, 16384),
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    let clear_color = [0.015, 0.018, 0.024, 1.0];
    let shader_source = banger_native_first_scene_wgsl();
    let shader_source_hash = sha256_hex(shader_source.as_bytes());
    let render_pipeline = create_banger_first_scene_pipeline(&device, format, shader_source);
    let render_pipeline_hash = sha256_hex(
        format!(
            "banger-first-scene-pipeline:{}:{:?}:{:?}:{:?}",
            shader_source_hash, format, present_mode, alpha_mode
        )
        .as_bytes(),
    );
    render_child_surface_frame(&surface, &device, &queue, &render_pipeline, clear_color)?;
    let parent_hash = sha256_hex(parent_window_handle.unwrap_or_default().as_bytes());
    let child_hash = sha256_hex(format!("{:p}", child as *mut c_void).as_bytes());
    let frame_hash = sha256_hex(
        format!(
            "banger-native-child-frame:{}:{}:{:?}:{:?}:{:?}:{}:{}:{}:{}",
            config.width,
            config.height,
            format,
            present_mode,
            alpha_mode,
            parent_hash,
            child_hash,
            shader_source_hash,
            render_pipeline_hash
        )
        .as_bytes(),
    );
    let present_loop_hash = sha256_hex(
        format!("banger-native-child-loop:{frame_hash}:{}:{}", info.name, info.driver_info).as_bytes(),
    );
    let mut projection = BangerNativeHostProjection {
        ok: true,
        schema: "forge.banger.native_present_loop_bootstrap.v1",
        engine: "banger_rust_native_engine",
        lane: "native_tandem_render",
        native_domain: "render_3d",
        route_status: "native_child_surface_host_presented",
        parent_window_handle_hash: parent_hash,
        child_window_handle_hash: child_hash,
        viewport_width: config.width,
        viewport_height: config.height,
        target_frame_ms: 16.67,
        selected_adapter: Some(info.name),
        adapter_count: pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all())).len(),
        backend: format!("{:?}", info.backend),
        surface_kind: "win32_child_window_wgpu_surface",
        swapchain_format: format!("{:?}", format),
        present_mode: format!("{:?}", present_mode),
        alpha_mode: format!("{:?}", alpha_mode),
        render_pass_count: 1,
        submitted_frame_count: 1,
        draw_call_count: 1,
        vertex_count: 3,
        clear_color,
        shader_source_hash,
        render_pipeline_hash,
        frame_hash,
        present_loop_hash,
        proof_hash: String::new(),
        host_pid: std::process::id(),
        verifier: BangerNativeHostVerifier {
            wall: "latency+native_surface+ui_branching",
            frontier_hypothesis: "Banger owns a persistent Win32 child surface with a programmable wgpu render pipeline under Electron chrome.",
            local_gate: "forge-cargo run --manifest-path examples\\ingen_native_services\\Cargo.toml --bin ingen_electron_backend_bridge -- --banger-native-host",
            rollback_path: "fall back to --banger-present-loop-bootstrap offscreen target",
        },
    };
    projection.proof_hash = proof_hash(&projection);
    println!("{}", serde_json::to_string(&projection).expect("serialize banger native host"));
    io::stdout().flush().map_err(|error| format!("failed to flush Banger host readiness JSON: {error}"))?;

    let requested_frames = frame_limit.unwrap_or(u32::MAX);
    let mut submitted = 1u32;
    while submitted < requested_frames && unsafe { IsWindow(parent) } != 0 && unsafe { IsWindow(child) } != 0 {
        pump_win32_messages();
        render_child_surface_frame(&surface, &device, &queue, &render_pipeline, clear_color)?;
        submitted += 1;
        thread::sleep(Duration::from_millis(16));
    }

    Ok(projection)
}

#[cfg(not(target_os = "windows"))]
fn run_banger_native_host(
    _parent_window_handle: Option<&str>,
    _width: u32,
    _height: u32,
    _frame_limit: Option<u32>,
) -> Result<BangerNativeHostProjection, String> {
    Err("Banger native child host is currently implemented for Win32 HWND surfaces".to_string())
}

#[cfg(target_os = "windows")]
fn render_child_surface_frame(
    surface: &wgpu::Surface<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    render_pipeline: &wgpu::RenderPipeline,
    clear_color: [f64; 4],
) -> Result<(), String> {
    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
        wgpu::CurrentSurfaceTexture::Outdated => {
            return Err("Banger child surface became outdated before resize handling was promoted".to_string())
        }
        wgpu::CurrentSurfaceTexture::Lost => {
            return Err("Banger child surface was lost before surface recreation was promoted".to_string())
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            return Err("Banger child surface reported a validation error".to_string())
        }
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("banger-native-child-host-encoder"),
    });
    {
        let color_attachments = [Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: clear_color[0],
                    g: clear_color[1],
                    b: clear_color[2],
                    a: clear_color[3],
                }),
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("banger-native-child-host-clear-pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(render_pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("Banger child host GPU poll failed: {error}"))?;
    frame.present();
    Ok(())
}

#[cfg(target_os = "windows")]
fn create_banger_first_scene_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader_source: &'static str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-first-scene-wgsl"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let targets = [Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::REPLACE),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-first-scene-pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(target_os = "windows")]
fn banger_native_first_scene_wgsl() -> &'static str {
    r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-0.72, -0.58),
        vec2<f32>( 0.72, -0.50),
        vec2<f32>( 0.02,  0.72)
    );
    var colors = array<vec3<f32>, 3>(
        vec3<f32>(0.95, 0.18, 0.12),
        vec3<f32>(0.12, 0.82, 0.42),
        vec3<f32>(0.18, 0.44, 1.00)
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.color = colors[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let rim = vec3<f32>(0.06, 0.08, 0.11);
    return vec4<f32>(max(in.color, rim), 1.0);
}
"#
}

#[cfg(target_os = "windows")]
fn parse_hwnd(value: Option<&str>) -> Option<windows_sys::Win32::Foundation::HWND> {
    let raw = value?.trim();
    let parsed = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .and_then(|hex| usize::from_str_radix(hex, 16).ok())
        .or_else(|| raw.parse::<usize>().ok())?;
    if parsed == 0 {
        None
    } else {
        Some(parsed as windows_sys::Win32::Foundation::HWND)
    }
}

#[cfg(target_os = "windows")]
fn pump_win32_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_banger_preview_frame_from_native_rasterizer() {
        let frame = banger_preview_frame();
        assert!(frame.accepted);
        assert_eq!(frame.schema, "forge.banger.visible_preview_frame.v1");
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 200);
        assert!(frame.frame_data_url.starts_with("data:image/bmp;base64,Qk"));
        assert_eq!(frame.frame_hash.len(), 64);
        assert_eq!(frame.scene_hash.len(), 64);
        assert_eq!(frame.proof_hash.len(), 64);
        assert_eq!(frame.metrics.splat_count, 12);
        assert!(frame.metrics.projected_splat_count > 0);
        assert!(frame.metrics.shaded_pixel_count > 0);
        assert!(frame.metrics.promotion_allowed);
    }

    #[test]
    fn encodes_rgba8_as_browser_bmp() {
        let bmp = rgba8_to_bmp(1, 1, &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(bmp.len(), 58);
        assert_eq!(&bmp[54..58], &[0x33, 0x22, 0x11, 0x44]);
        assert_eq!(base64_encode(b"BM"), "Qk0=");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn embeds_first_scene_wgsl_pipeline_artifact() {
        let source = banger_native_first_scene_wgsl();
        assert!(source.contains("@vertex"));
        assert!(source.contains("@fragment"));
        assert!(source.contains("@builtin(vertex_index)"));
        assert_eq!(sha256_hex(source.as_bytes()).len(), 64);
    }
}
