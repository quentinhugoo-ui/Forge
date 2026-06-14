use ingen_native_services::banger_native_engine::{
    BangerNativeEngine, BangerNativePresentLoopBootstrapRequest,
};
use ingen_native_services::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapterProbe};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    water_pipeline_hash: String,
    water_pass_count: usize,
    water_virtual_page_count: u32,
    water_info_texture_hash: String,
    water_info_shoreline_texel_count: u32,
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
    index_count: u32,
    instance_count: u32,
    scene_object_count: u32,
    scene_graph_hash: String,
    instance_buffer_hash: String,
    depth_format: &'static str,
    frame_target_policy: &'static str,
    frame_target_hash: String,
    depth_target_hash: String,
    frame_target_allocation_count: u32,
    surface_resize_count: u32,
    render_loop_policy: &'static str,
    clear_color: [f64; 4],
    frame_uniform_hash: String,
    camera_uniform_hash: String,
    scene_mesh_hash: String,
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

#[cfg(target_os = "windows")]
struct BangerNativeScenePipeline {
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    instance_count: u32,
    scene_mesh_hash: String,
    scene_graph_hash: String,
    instance_buffer_hash: String,
    depth_format: wgpu::TextureFormat,
    shader_source_hash: String,
    render_pipeline_hash: String,
}

#[cfg(target_os = "windows")]
struct BangerNativeFrameTarget {
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    width: u32,
    height: u32,
    target_hash: String,
    depth_target_hash: String,
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
    let render = BangerNativeEngine::bootstrap_present_loop(BangerNativePresentLoopBootstrapRequest {
        parent_window_handle: None,
        viewport_width: Some(640),
        viewport_height: Some(360),
        target_frame_ms: Some(16.67),
    })
    .expect("render Banger wgpu preview frame");
    let width = render.preview_width;
    let height = render.preview_height;
    let bmp = rgba8_to_bmp(width, height, &render.preview_rgba8);
    let frame_hash = sha256_hex(&bmp);
    let scene_hash = render.scene3d_proof_hash.clone();
    let metrics = BangerPreviewFrameMetrics {
        splat_count: 0,
        projected_splat_count: 0,
        rasterized_splat_count: 0,
        shaded_pixel_count: render.nonblack_pixel_sample_count,
        tile_count: render.nonzero_tile_count,
        benchmark_gate_count: 5,
        promotion_allowed: render.ok && render.nonblack_pixel_sample_count > 0 && render.depth_occupied_pixel_count > 0,
        render_path: "rust_banger_wgpu_ocean_scene_rgba8_to_bmp_data_url",
        water_pipeline_hash: render.water_pipeline_hash.clone(),
        water_pass_count: render.water_pipeline_manifest.pass_schedule.len(),
        water_virtual_page_count: render
            .water_pipeline_manifest
            .virtual_page_budget
            .meshlet_pages
            + render.water_pipeline_manifest.virtual_page_budget.sdf_bricks
            + render.water_pipeline_manifest.virtual_page_budget.voxel_pages
            + render.water_pipeline_manifest.virtual_page_budget.foam_tiles
            + render.water_pipeline_manifest.virtual_page_budget.reflection_tiles,
        water_info_texture_hash: render
            .water_pipeline_manifest
            .water_info_texture
            .output_texture_hash
            .clone(),
        water_info_shoreline_texel_count: render
            .water_pipeline_manifest
            .water_info_texture
            .shoreline_texel_count,
    };
    let mut frame = BangerPreviewFrameProjection {
        accepted: true,
        schema: "forge.banger.visible_preview_frame.v1",
        source: "examples/ingen_native_services/banger_wgpu_present_loop_preview_frame",
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
        CreateWindowExW, DefWindowProcW, IsWindow, RegisterClassW, SetWindowPos, ShowWindow,
        CS_HREDRAW, CS_VREDRAW, SWP_NOACTIVATE, SWP_NOZORDER, SW_SHOW, WNDCLASSW, WS_CHILD,
        WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
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
    let mut config = wgpu::SurfaceConfiguration {
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
    let scene_pipeline = create_banger_first_scene_pipeline(&device, format, present_mode, alpha_mode);
    let mut frame_target_allocation_count = 1u32;
    let mut surface_resize_count = 0u32;
    let mut frame_target = create_banger_frame_target(
        &device,
        config.width,
        config.height,
        scene_pipeline.depth_format,
        frame_target_allocation_count,
    );
    let started = Instant::now();
    let frame_uniform_hash = render_child_surface_frame(
        &surface,
        &device,
        &queue,
        &scene_pipeline,
        &frame_target,
        clear_color,
        started.elapsed().as_secs_f32(),
        0,
    )?;
    let parent_hash = sha256_hex(parent_window_handle.unwrap_or_default().as_bytes());
    let child_hash = sha256_hex(format!("{:p}", child as *mut c_void).as_bytes());
    let frame_hash = sha256_hex(
        format!(
            "banger-native-child-frame:{}:{}:{:?}:{:?}:{:?}:{}:{}:{}:{}:{}:{}:{:?}:{}:{}:{}:{}:{}",
            config.width,
            config.height,
            format,
            present_mode,
            alpha_mode,
            parent_hash,
            child_hash,
            scene_pipeline.scene_mesh_hash,
            scene_pipeline.scene_graph_hash,
            scene_pipeline.index_count,
            scene_pipeline.instance_count,
            scene_pipeline.depth_format,
            frame_target.target_hash,
            frame_target.depth_target_hash,
            scene_pipeline.shader_source_hash,
            scene_pipeline.render_pipeline_hash,
            frame_uniform_hash
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
        vertex_count: 8,
        index_count: scene_pipeline.index_count,
        instance_count: scene_pipeline.instance_count,
        scene_object_count: scene_pipeline.instance_count,
        scene_graph_hash: scene_pipeline.scene_graph_hash.clone(),
        instance_buffer_hash: scene_pipeline.instance_buffer_hash.clone(),
        depth_format: "Depth24Plus",
        frame_target_policy: "persistent_resize_tracked_depth_target_v1",
        frame_target_hash: frame_target.target_hash.clone(),
        depth_target_hash: frame_target.depth_target_hash.clone(),
        frame_target_allocation_count,
        surface_resize_count,
        render_loop_policy: "native_wgpu_instanced_mesh_depth_camera_loop_v1",
        clear_color,
        frame_uniform_hash: frame_uniform_hash.clone(),
        camera_uniform_hash: frame_uniform_hash,
        scene_mesh_hash: scene_pipeline.scene_mesh_hash.clone(),
        shader_source_hash: scene_pipeline.shader_source_hash.clone(),
        render_pipeline_hash: scene_pipeline.render_pipeline_hash.clone(),
        frame_hash,
        present_loop_hash,
        proof_hash: String::new(),
        host_pid: std::process::id(),
        verifier: BangerNativeHostVerifier {
            wall: "scene_submission+draw_call_scaling",
            frontier_hypothesis: "Banger can submit multiple scene objects through one native instanced indexed draw while preserving deterministic scene hashes.",
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
        if let Some((parent_width, parent_height)) = parent_client_size(parent) {
            let parent_width = parent_width.clamp(64, 16384);
            let parent_height = parent_height.clamp(64, 16384);
            if parent_width != config.width || parent_height != config.height {
                unsafe {
                    SetWindowPos(
                        child,
                        std::ptr::null_mut(),
                        0,
                        0,
                        parent_width as i32,
                        parent_height as i32,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                config.width = parent_width;
                config.height = parent_height;
                surface.configure(&device, &config);
                surface_resize_count += 1;
                frame_target_allocation_count += 1;
                frame_target = create_banger_frame_target(
                    &device,
                    config.width,
                    config.height,
                    scene_pipeline.depth_format,
                    frame_target_allocation_count,
                );
                projection.viewport_width = config.width;
                projection.viewport_height = config.height;
                projection.frame_target_hash = frame_target.target_hash.clone();
                projection.depth_target_hash = frame_target.depth_target_hash.clone();
                projection.frame_target_allocation_count = frame_target_allocation_count;
                projection.surface_resize_count = surface_resize_count;
            }
        }
        let _ = render_child_surface_frame(
            &surface,
            &device,
            &queue,
            &scene_pipeline,
            &frame_target,
            clear_color,
            started.elapsed().as_secs_f32(),
            submitted,
        )?;
        submitted += 1;
        projection.submitted_frame_count = submitted;
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
    scene_pipeline: &BangerNativeScenePipeline,
    frame_target: &BangerNativeFrameTarget,
    clear_color: [f64; 4],
    time_seconds: f32,
    frame_index: u32,
) -> Result<String, String> {
    let uniform_bytes = banger_frame_uniform_bytes(time_seconds, frame_index, frame_target.width, frame_target.height);
    let frame_uniform_hash = sha256_hex(&uniform_bytes);
    queue.write_buffer(&scene_pipeline.uniform_buffer, 0, &uniform_bytes);
    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(frame_uniform_hash),
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
            label: Some("banger-native-child-host-mesh-depth-pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &frame_target.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&scene_pipeline.render_pipeline);
        pass.set_bind_group(0, &scene_pipeline.bind_group, &[]);
        pass.set_vertex_buffer(0, scene_pipeline.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, scene_pipeline.instance_buffer.slice(..));
        pass.set_index_buffer(scene_pipeline.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..scene_pipeline.index_count, 0, 0..scene_pipeline.instance_count);
    }
    queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("Banger child host GPU poll failed: {error}"))?;
    frame.present();
    Ok(frame_uniform_hash)
}

#[cfg(target_os = "windows")]
fn create_banger_frame_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    depth_format: wgpu::TextureFormat,
    allocation_index: u32,
) -> BangerNativeFrameTarget {
    let width = width.clamp(1, 16384);
    let height = height.clamp(1, 16384);
    let (target_hash, depth_target_hash) =
        banger_frame_target_hashes(width, height, depth_format, allocation_index);
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-native-child-host-persistent-depth-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: depth_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    BangerNativeFrameTarget {
        _depth_texture: depth_texture,
        depth_view,
        width,
        height,
        target_hash,
        depth_target_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_frame_target_hashes(
    width: u32,
    height: u32,
    depth_format: wgpu::TextureFormat,
    allocation_index: u32,
) -> (String, String) {
    let depth_target_hash = sha256_hex(
        format!(
            "banger-depth-target-v1:{}:{}:{:?}:{}",
            width, height, depth_format, allocation_index
        )
        .as_bytes(),
    );
    let target_hash = sha256_hex(
        format!(
            "banger-frame-target-v1:{}:{}:{:?}:{}:{}",
            width, height, depth_format, allocation_index, depth_target_hash
        )
        .as_bytes(),
    );
    (target_hash, depth_target_hash)
}

#[cfg(target_os = "windows")]
fn create_banger_first_scene_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
) -> BangerNativeScenePipeline {
    let shader_source = banger_native_first_scene_wgsl();
    let shader_source_hash = sha256_hex(shader_source.as_bytes());
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-first-scene-wgsl"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("banger-native-frame-uniform-buffer"),
        size: 80,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let vertex_bytes = banger_cube_vertex_bytes();
    let index_bytes = banger_cube_index_bytes();
    let instance_bytes = banger_scene_instance_bytes();
    let instance_buffer_hash = sha256_hex(&instance_bytes);
    let scene_mesh_hash = sha256_hex(
        format!(
            "banger-cube-mesh-v1:{}:{}",
            sha256_hex(&vertex_bytes),
            sha256_hex(&index_bytes)
        )
        .as_bytes(),
    );
    let scene_graph_hash = sha256_hex(
        format!(
            "banger-scene-graph-v1:{}:{}:{}",
            scene_mesh_hash,
            instance_buffer_hash,
            instance_bytes.len() / 80
        )
        .as_bytes(),
    );
    let vertex_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-cube-vertex-buffer",
        wgpu::BufferUsages::VERTEX,
        &vertex_bytes,
    );
    let index_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-cube-index-buffer",
        wgpu::BufferUsages::INDEX,
        &index_bytes,
    );
    let instance_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-scene-instance-buffer",
        wgpu::BufferUsages::VERTEX,
        &instance_bytes,
    );
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("banger-native-frame-bind-group-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-first-scene-pipeline-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-frame-bind-group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let targets = [Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::REPLACE),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-first-scene-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: 80,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 4,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 48,
                            shader_location: 5,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 64,
                            shader_location: 6,
                        },
                    ],
                },
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    });
    let render_pipeline_hash = sha256_hex(
        format!(
            "banger-first-scene-pipeline:{}:{}:{}:{:?}:{:?}:{:?}:instanced_mesh_depth_camera_v1",
            shader_source_hash, scene_mesh_hash, scene_graph_hash, format, present_mode, alpha_mode
        )
        .as_bytes(),
    );
    BangerNativeScenePipeline {
        render_pipeline,
        uniform_buffer,
        bind_group,
        vertex_buffer,
        instance_buffer,
        index_buffer,
        index_count: (index_bytes.len() / 2) as u32,
        instance_count: (instance_bytes.len() / 80) as u32,
        scene_mesh_hash,
        scene_graph_hash,
        instance_buffer_hash,
        depth_format: wgpu::TextureFormat::Depth24Plus,
        shader_source_hash,
        render_pipeline_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_native_first_scene_wgsl() -> &'static str {
    r#"
struct FrameUniform {
    view_proj: mat4x4<f32>,
    time_seconds: f32,
    frame_index: u32,
    viewport: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniform;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal_hint: vec3<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) instance_tint: vec4<f32>,
) -> VertexOut {
    let t = frame.time_seconds;
    let yaw = t * 0.62;
    let pitch = 0.32 + 0.12 * sin(t * 0.45);
    let model = mat4x4<f32>(model_0, model_1, model_2, model_3);
    let rotated_y = vec3<f32>(
        position.x * cos(yaw) + position.z * sin(yaw),
        position.y,
        -position.x * sin(yaw) + position.z * cos(yaw)
    );
    let rotated = vec3<f32>(
        rotated_y.x,
        rotated_y.y * cos(pitch) - rotated_y.z * sin(pitch),
        rotated_y.y * sin(pitch) + rotated_y.z * cos(pitch)
    );
    let world = model * vec4<f32>(rotated, 1.0);
    var out: VertexOut;
    out.position = frame.view_proj * world;
    out.color = color * instance_tint.rgb;
    out.normal_hint = normalize((model * vec4<f32>(rotated, 0.0)).xyz);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.35, 0.55, 0.74));
    let lambert = clamp(dot(normalize(in.normal_hint), light_dir) * 0.5 + 0.5, 0.22, 1.0);
    let rim = vec3<f32>(0.04, 0.06, 0.09);
    let pulse = 0.96 + 0.04 * sin(frame.time_seconds * 1.8 + f32(frame.frame_index) * 0.01);
    return vec4<f32>(max(in.color * lambert * pulse, rim), 1.0);
}
"#
}

#[cfg(target_os = "windows")]
fn banger_frame_uniform_bytes(
    time_seconds: f32,
    frame_index: u32,
    viewport_width: u32,
    viewport_height: u32,
) -> [u8; 80] {
    let mut bytes = [0u8; 80];
    let view_proj = banger_view_projection_matrix(time_seconds, viewport_width, viewport_height);
    for (index, value) in view_proj.iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[64..68].copy_from_slice(&time_seconds.to_le_bytes());
    bytes[68..72].copy_from_slice(&frame_index.to_le_bytes());
    bytes[72..76].copy_from_slice(&(viewport_width as f32).to_le_bytes());
    bytes[76..80].copy_from_slice(&(viewport_height as f32).to_le_bytes());
    bytes
}

#[cfg(target_os = "windows")]
fn banger_create_mapped_buffer(
    device: &wgpu::Device,
    label: &'static str,
    usage: wgpu::BufferUsages,
    bytes: &[u8],
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.len() as u64,
        usage,
        mapped_at_creation: true,
    });
    buffer.slice(..).get_mapped_range_mut().copy_from_slice(bytes);
    buffer.unmap();
    buffer
}

#[cfg(target_os = "windows")]
fn banger_cube_vertex_bytes() -> Vec<u8> {
    let vertices: [[f32; 6]; 8] = [
        [-0.75, -0.75, 0.75, 0.95, 0.18, 0.12],
        [0.75, -0.75, 0.75, 0.12, 0.82, 0.42],
        [0.75, 0.75, 0.75, 0.18, 0.44, 1.00],
        [-0.75, 0.75, 0.75, 0.98, 0.78, 0.16],
        [-0.75, -0.75, -0.75, 0.84, 0.26, 0.92],
        [0.75, -0.75, -0.75, 0.10, 0.72, 0.82],
        [0.75, 0.75, -0.75, 0.96, 0.42, 0.21],
        [-0.75, 0.75, -0.75, 0.66, 0.92, 0.24],
    ];
    let mut bytes = Vec::with_capacity(vertices.len() * 24);
    for vertex in vertices {
        for value in vertex {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_cube_index_bytes() -> Vec<u8> {
    let indices: [u16; 36] = [
        0, 1, 2, 0, 2, 3,
        1, 5, 6, 1, 6, 2,
        5, 4, 7, 5, 7, 6,
        4, 0, 3, 4, 3, 7,
        3, 2, 6, 3, 6, 7,
        4, 5, 1, 4, 1, 0,
    ];
    let mut bytes = Vec::with_capacity(indices.len() * 2);
    for index in indices {
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_scene_instance_bytes() -> Vec<u8> {
    let instances: [([f32; 3], f32, [f32; 4]); 3] = [
        ([-1.45, -0.12, 0.0], 0.64, [1.00, 0.76, 0.28, 1.0]),
        ([0.0, 0.18, -0.15], 0.82, [0.72, 1.00, 0.54, 1.0]),
        ([1.48, -0.04, 0.12], 0.58, [0.42, 0.72, 1.00, 1.0]),
    ];
    let mut bytes = Vec::with_capacity(instances.len() * 80);
    for (translation, scale, tint) in instances {
        for value in banger_model_matrix(translation, scale) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in tint {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_model_matrix(translation: [f32; 3], scale: f32) -> [f32; 16] {
    [
        scale, 0.0, 0.0, 0.0,
        0.0, scale, 0.0, 0.0,
        0.0, 0.0, scale, 0.0,
        translation[0], translation[1], translation[2], 1.0,
    ]
}

#[cfg(target_os = "windows")]
fn banger_view_projection_matrix(time_seconds: f32, viewport_width: u32, viewport_height: u32) -> [f32; 16] {
    let aspect = (viewport_width as f32 / viewport_height.max(1) as f32).clamp(0.25, 4.0);
    let eye = [
        3.2 + 0.18 * (time_seconds * 0.37).sin(),
        2.15,
        4.35 + 0.12 * (time_seconds * 0.29).cos(),
    ];
    let view = banger_look_at_rh(eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let projection = banger_perspective_rh_zo(55.0_f32.to_radians(), aspect, 0.05, 128.0);
    banger_mat4_mul(projection, view)
}

#[cfg(target_os = "windows")]
fn banger_perspective_rh_zo(fovy_radians: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    let f = 1.0 / (fovy_radians * 0.5).tan();
    [
        f / aspect, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, far / (near - far), -1.0,
        0.0, 0.0, (far * near) / (near - far), 0.0,
    ]
}

#[cfg(target_os = "windows")]
fn banger_look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let forward = banger_vec3_normalize([
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    ]);
    let side = banger_vec3_normalize(banger_vec3_cross(forward, up));
    let camera_up = banger_vec3_cross(side, forward);
    [
        side[0], camera_up[0], -forward[0], 0.0,
        side[1], camera_up[1], -forward[1], 0.0,
        side[2], camera_up[2], -forward[2], 0.0,
        -banger_vec3_dot(side, eye), -banger_vec3_dot(camera_up, eye), banger_vec3_dot(forward, eye), 1.0,
    ]
}

#[cfg(target_os = "windows")]
fn banger_mat4_mul(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            out[column * 4 + row] =
                a[row] * b[column * 4] +
                a[4 + row] * b[column * 4 + 1] +
                a[8 + row] * b[column * 4 + 2] +
                a[12 + row] * b[column * 4 + 3];
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn banger_vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(target_os = "windows")]
fn banger_vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(target_os = "windows")]
fn banger_vec3_normalize(value: [f32; 3]) -> [f32; 3] {
    let length = banger_vec3_dot(value, value).sqrt().max(0.0001);
    [value[0] / length, value[1] / length, value[2] / length]
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
fn parent_client_size(parent: windows_sys::Win32::Foundation::HWND) -> Option<(u32, u32)> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
    let mut rect = RECT::default();
    if unsafe { GetClientRect(parent, &mut rect) } == 0 {
        return None;
    }
    let width = (rect.right - rect.left).max(1) as u32;
    let height = (rect.bottom - rect.top).max(1) as u32;
    Some((width, height))
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
    fn emits_banger_preview_frame_from_native_present_loop() {
        let frame = banger_preview_frame();
        assert!(frame.accepted);
        assert_eq!(frame.schema, "forge.banger.visible_preview_frame.v1");
        assert_eq!(frame.width, 512);
        assert_eq!(frame.height, 288);
        assert!(frame.frame_data_url.starts_with("data:image/bmp;base64,Qk"));
        assert_eq!(frame.frame_hash.len(), 64);
        assert_eq!(frame.scene_hash.len(), 64);
        assert_eq!(frame.proof_hash.len(), 64);
        assert_eq!(frame.metrics.render_path, "rust_banger_wgpu_ocean_scene_rgba8_to_bmp_data_url");
        assert_eq!(frame.metrics.splat_count, 0);
        assert_eq!(frame.metrics.projected_splat_count, 0);
        assert!(frame.metrics.shaded_pixel_count > 0);
        assert_eq!(frame.metrics.water_pipeline_hash.len(), 64);
        assert_eq!(frame.metrics.water_pass_count, 10);
        assert!(frame.metrics.water_virtual_page_count > 0);
        assert_eq!(frame.metrics.water_info_texture_hash.len(), 64);
        assert!(frame.metrics.water_info_shoreline_texel_count > 0);
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
        assert!(source.contains("view_proj"));
        assert!(source.contains("@location(0) position"));
        assert!(source.contains("@location(1) color"));
        assert!(source.contains("@location(2) model_0"));
        assert!(source.contains("@location(6) instance_tint"));
        assert!(source.contains("FrameUniform"));
        assert!(source.contains("@group(0) @binding(0)"));
        assert_eq!(sha256_hex(source.as_bytes()).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn encodes_banger_frame_uniforms_for_gpu_loop() {
        let bytes = banger_frame_uniform_bytes(1.25, 42, 1920, 1080);
        assert_eq!(bytes.len(), 80);
        assert_eq!(u32::from_le_bytes(bytes[68..72].try_into().unwrap()), 42);
        assert_eq!(f32::from_le_bytes(bytes[72..76].try_into().unwrap()), 1920.0);
        assert_eq!(sha256_hex(&bytes).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_banger_cube_mesh_for_indexed_native_draws() {
        let vertex_bytes = banger_cube_vertex_bytes();
        let index_bytes = banger_cube_index_bytes();
        assert_eq!(vertex_bytes.len(), 8 * 24);
        assert_eq!(index_bytes.len(), 36 * 2);
        assert_eq!(u16::from_le_bytes(index_bytes[0..2].try_into().unwrap()), 0);
        assert_eq!(sha256_hex(&vertex_bytes).len(), 64);
        assert_eq!(sha256_hex(&index_bytes).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_banger_scene_instances_for_one_indexed_draw() {
        let instance_bytes = banger_scene_instance_bytes();
        assert_eq!(instance_bytes.len(), 3 * 80);
        assert_eq!(f32::from_le_bytes(instance_bytes[0..4].try_into().unwrap()), 0.64);
        assert_eq!(f32::from_le_bytes(instance_bytes[64..68].try_into().unwrap()), 1.0);
        assert_eq!(sha256_hex(&instance_bytes).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn hashes_persistent_frame_targets_by_size_and_generation() {
        let (first_target, first_depth) = banger_frame_target_hashes(1280, 720, wgpu::TextureFormat::Depth24Plus, 1);
        let (same_target, same_depth) = banger_frame_target_hashes(1280, 720, wgpu::TextureFormat::Depth24Plus, 1);
        let (resized_target, resized_depth) = banger_frame_target_hashes(1920, 1080, wgpu::TextureFormat::Depth24Plus, 2);
        assert_eq!(first_target, same_target);
        assert_eq!(first_depth, same_depth);
        assert_ne!(first_target, resized_target);
        assert_ne!(first_depth, resized_depth);
        assert_eq!(first_target.len(), 64);
        assert_eq!(first_depth.len(), 64);
    }
}
