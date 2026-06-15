use ingen_native_services::banger_native_engine::{
    BangerNativeEngine, BangerNativePresentLoopBootstrapRequest,
};
use ingen_native_services::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapterProbe};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{env, fs};
use std::io::{self, Write};
use std::path::PathBuf;
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
    maps_tileset_contract: Option<BangerMapsTilesetContract>,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsTilesetContract {
    schema: &'static str,
    provider: &'static str,
    renderer_contract: &'static str,
    root_tileset_endpoint: &'static str,
    root_request_ttl_hours: u32,
    native_streamer: BangerMapsNative3DTilesStreamer,
    georeference: BangerMapsGeoreference,
    traversal: BangerMapsTraversalPolicy,
    cache: BangerMapsResidencyCache,
    attribution: BangerMapsAttributionPolicy,
    credential_policy: &'static str,
    interop_floor: BangerMapsInteropFloor,
    contract_hash: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsNative3DTilesStreamer {
    schema: &'static str,
    authority: &'static str,
    status: &'static str,
    root_ingestion_stage: &'static str,
    traversal_stage: &'static str,
    content_decode_stage: &'static str,
    georeference_stage: &'static str,
    gpu_submission_stage: &'static str,
    visual_fallback: &'static str,
    blocker: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsGeoreference {
    ellipsoid: &'static str,
    origin_latitude: f64,
    origin_longitude: f64,
    origin_height_meters: f32,
    world_origin_policy: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsTraversalPolicy {
    lod_policy: &'static str,
    max_screen_space_error: f32,
    skip_level_of_detail: bool,
    max_simultaneous_tile_loads: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsResidencyCache {
    authority: &'static str,
    max_resident_tile_bytes: u64,
    session_cache_key_policy: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsAttributionPolicy {
    required: bool,
    mode: &'static str,
    policy: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsInteropFloor {
    cesium_for_unreal: &'static str,
    cesium_js: &'static str,
    tileset: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsRootIngestProjection {
    ok: bool,
    schema: &'static str,
    source: &'static str,
    root_tileset_url: String,
    cache_dir: String,
    cache_path: String,
    cache_hit: bool,
    network_fetch_attempted: bool,
    root_hash: String,
    root_byte_count: usize,
    tile_count: usize,
    content_uri_count: usize,
    geometric_error: Option<f64>,
    asset_version: String,
    traversal_seed_hash: String,
    traversal_seed: BangerMapsTraversalSeed,
    verifier: BangerMapsRootIngestVerifier,
    error: Option<BangerNativeError>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsTraversalSeed {
    schema: &'static str,
    priority_model: &'static str,
    max_queued_tiles: usize,
    queued_tile_count: usize,
    total_tile_count: usize,
    total_content_uri_count: usize,
    deepest_level: usize,
    plan_hash: String,
    tiles: Vec<BangerMapsTraversalTile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsTraversalTile {
    tile_id: String,
    parent_tile_id: Option<String>,
    depth: usize,
    child_count: usize,
    geometric_error: Option<f64>,
    refine: String,
    bounding_volume_kind: String,
    bounding_volume_hash: String,
    transform_hash: String,
    content_uris: Vec<String>,
    priority_key: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsRootIngestVerifier {
    wall: &'static str,
    frontier_hypothesis: &'static str,
    local_gate: &'static str,
    rollback_path: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerNativeError {
    code: &'static str,
    message: String,
    proof_hash: String,
}

impl BangerMapsTilesetContract {
    fn google_photorealistic_default() -> Self {
        let contract_seed = "forge.banger.maps_photorealistic_3d_tiles_contract.v1:google_photorealistic_3d_tiles:Cesium3DTileset_style_native_streamer:WGS84";
        Self {
            schema: "forge.banger.maps_photorealistic_3d_tiles_contract.v1",
            provider: "google_photorealistic_3d_tiles",
            renderer_contract: "Cesium3DTileset_style_native_streamer",
            root_tileset_endpoint: "https://tile.googleapis.com/v1/3dtiles/root.json",
            root_request_ttl_hours: 3,
            native_streamer: BangerMapsNative3DTilesStreamer {
                schema: "forge.banger.native_3d_tiles_streamer.v1",
                authority: "banger_native_engine",
                status: "contract_ready_visual_fallback_active",
                root_ingestion_stage: "3d_tiles_root_json_manifest_ingestion",
                traversal_stage: "screen_space_error_priority_queue_with_tile_budget",
                content_decode_stage: "b3dm_glb_gltf_mesh_material_texture_decode",
                georeference_stage: "wgs84_ecef_to_enu_floating_origin",
                gpu_submission_stage: "meshlet_or_indexed_mesh_upload_pending",
                visual_fallback: "cesiumjs_photorealistic_tiles_until_native_submission_promoted",
                blocker: "native_gltf_material_texture_submission_not_promoted",
            },
            georeference: BangerMapsGeoreference {
                ellipsoid: "WGS84",
                origin_latitude: 37.42207,
                origin_longitude: -122.08409,
                origin_height_meters: 0.0,
                world_origin_policy: "CesiumGeoreference_style_floating_origin",
            },
            traversal: BangerMapsTraversalPolicy {
                lod_policy: "screen_space_error",
                max_screen_space_error: 16.0,
                skip_level_of_detail: true,
                max_simultaneous_tile_loads: 18,
            },
            cache: BangerMapsResidencyCache {
                authority: "banger_tileset_residency_cache",
                max_resident_tile_bytes: 512 * 1024 * 1024,
                session_cache_key_policy: "root_session_hash_plus_tile_uri_without_api_key",
            },
            attribution: BangerMapsAttributionPolicy {
                required: true,
                mode: "visible_on_screen",
                policy: "google_maps_platform_terms",
            },
            credential_policy: "api_key_redacted_from_logs_proofs_and_renderer_state",
            interop_floor: BangerMapsInteropFloor {
                cesium_for_unreal: "1.12+",
                cesium_js: "1.91+",
                tileset: "OGC_3D_Tiles",
            },
            contract_hash: sha256_hex(contract_seed.as_bytes()),
        }
    }
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
    if env::args().any(|argument| argument == "--banger-maps-root-ingest") {
        let ingest = banger_maps_root_ingest();
        println!("{}", serde_json::to_string(&ingest).expect("serialize banger maps root ingest"));
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

fn banger_maps_root_ingest() -> BangerMapsRootIngestProjection {
    let url = env::var("FORGE_BANGER_MAPS_ROOT_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://forge-6cai.onrender.com/api/banger/google-tiles/root.json".to_string());
    let cache_dir = banger_maps_cache_dir();
    let url_hash = sha256_hex(url.as_bytes());
    let cache_path = cache_dir.join(format!("{url_hash}.root.json"));
    let _ = fs::create_dir_all(&cache_dir);
    let fetched = fetch_banger_maps_root(&url);
    let (bytes, source, cache_hit, error) = match fetched {
        Ok(bytes) => {
            let _ = fs::write(&cache_path, &bytes);
            (bytes, "network", false, None)
        }
        Err(message) => match fs::read(&cache_path) {
            Ok(bytes) => (bytes, "cache_after_network_error", true, None),
            Err(_) => {
                let proof_hash = sha256_hex(format!("{url}:{message}").as_bytes());
                return BangerMapsRootIngestProjection {
                    ok: false,
                    schema: "forge.banger.native_3d_tiles_root_ingest.v1",
                    source: "network_error_no_cache",
                    root_tileset_url: redact_url_secret(&url),
                    cache_dir: cache_dir.display().to_string(),
                    cache_path: cache_path.display().to_string(),
                    cache_hit: false,
                    network_fetch_attempted: true,
                    root_hash: String::new(),
                    root_byte_count: 0,
                    tile_count: 0,
                    content_uri_count: 0,
                    geometric_error: None,
                    asset_version: String::new(),
                    traversal_seed_hash: proof_hash.clone(),
                    traversal_seed: empty_banger_maps_traversal_seed(),
                    verifier: banger_maps_root_ingest_verifier(),
                    error: Some(BangerNativeError {
                        code: "root_fetch_failed",
                        message,
                        proof_hash,
                    }),
                };
            }
        },
    };
    summarize_banger_maps_root(&url, &cache_dir, &cache_path, &bytes, source, cache_hit, error)
}

fn banger_maps_cache_dir() -> PathBuf {
    env::var("FORGE_BANGER_TILE_CACHE_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| env::var("LOCALAPPDATA").ok().map(|base| PathBuf::from(base).join("InGen").join("BangerTilesCache")))
        .unwrap_or_else(|| env::temp_dir().join("Forge").join("BangerTilesCache"))
}

fn fetch_banger_maps_root(url: &str) -> Result<Vec<u8>, String> {
    if let Some(path) = url.strip_prefix("file://") {
        let local_path = if cfg!(windows) {
            path.trim_start_matches('/')
        } else {
            path
        };
        return fs::read(local_path).map_err(|error| format!("root file: {error}"));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return fs::read(url).map_err(|error| format!("root file: {error}"));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("forge-banger-native-3d-tiles/0.1")
        .build()
        .map_err(|error| format!("root client: {error}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("root request: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(format!("root status {}: {}", status.as_u16(), body.chars().take(240).collect::<String>()));
    }
    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|error| format!("root body: {error}"))
}

fn summarize_banger_maps_root(
    url: &str,
    cache_dir: &std::path::Path,
    cache_path: &std::path::Path,
    bytes: &[u8],
    source: &'static str,
    cache_hit: bool,
    error: Option<BangerNativeError>,
) -> BangerMapsRootIngestProjection {
    let root_hash = sha256_hex(bytes);
    let json_bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let parsed = serde_json::from_slice::<Value>(json_bytes).ok();
    let root = parsed.as_ref().and_then(|value| value.get("root")).or(parsed.as_ref());
    let tile_count = root.map(count_banger_tiles).unwrap_or(0);
    let content_uri_count = root.map(count_banger_tile_content_uris).unwrap_or(0);
    let geometric_error = root.and_then(|value| value.get("geometricError")).and_then(Value::as_f64);
    let traversal_seed = build_banger_maps_traversal_seed(root);
    let asset_version = parsed
        .as_ref()
        .and_then(|value| value.get("asset"))
        .and_then(|asset| asset.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let traversal_seed_hash = sha256_hex(
        format!(
            "{root_hash}:{tile_count}:{content_uri_count}:{geometric_error:?}:{asset_version}:{}",
            traversal_seed.plan_hash
        )
        .as_bytes(),
    );
    BangerMapsRootIngestProjection {
        ok: parsed.is_some(),
        schema: "forge.banger.native_3d_tiles_root_ingest.v1",
        source,
        root_tileset_url: redact_url_secret(url),
        cache_dir: cache_dir.display().to_string(),
        cache_path: cache_path.display().to_string(),
        cache_hit,
        network_fetch_attempted: true,
        root_hash,
        root_byte_count: bytes.len(),
        tile_count,
        content_uri_count,
        geometric_error,
        asset_version,
        traversal_seed_hash,
        traversal_seed,
        verifier: banger_maps_root_ingest_verifier(),
        error,
    }
}

fn empty_banger_maps_traversal_seed() -> BangerMapsTraversalSeed {
    BangerMapsTraversalSeed {
        schema: "forge.banger.native_3d_tiles_traversal_seed.v1",
        priority_model: "parent_first_screen_space_error_seed",
        max_queued_tiles: 0,
        queued_tile_count: 0,
        total_tile_count: 0,
        total_content_uri_count: 0,
        deepest_level: 0,
        plan_hash: sha256_hex(b"empty_banger_maps_traversal_seed"),
        tiles: Vec::new(),
    }
}

fn build_banger_maps_traversal_seed(root: Option<&Value>) -> BangerMapsTraversalSeed {
    let max_queued_tiles = env::var("FORGE_BANGER_MAPS_TRAVERSAL_MAX_TILES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(64);
    let mut tiles = Vec::new();
    if let Some(root) = root {
        collect_banger_maps_traversal_tiles(root, None, 0, "0".to_string(), &mut tiles);
    }
    let total_tile_count = tiles.len();
    let total_content_uri_count = tiles.iter().map(|tile| tile.content_uris.len()).sum();
    let deepest_level = tiles.iter().map(|tile| tile.depth).max().unwrap_or(0);
    tiles.truncate(max_queued_tiles);
    let plan_hash = sha256_hex(
        tiles
            .iter()
            .map(|tile| {
                format!(
                    "{}:{}:{:?}:{}:{}:{};",
                    tile.tile_id,
                    tile.depth,
                    tile.geometric_error,
                    tile.bounding_volume_hash,
                    tile.transform_hash,
                    tile.content_uris.join(",")
                )
            })
            .collect::<String>()
            .as_bytes(),
    );
    BangerMapsTraversalSeed {
        schema: "forge.banger.native_3d_tiles_traversal_seed.v1",
        priority_model: "parent_first_screen_space_error_seed",
        max_queued_tiles,
        queued_tile_count: tiles.len(),
        total_tile_count,
        total_content_uri_count,
        deepest_level,
        plan_hash,
        tiles,
    }
}

fn collect_banger_maps_traversal_tiles(
    tile: &Value,
    parent_tile_id: Option<String>,
    depth: usize,
    path: String,
    out: &mut Vec<BangerMapsTraversalTile>,
) {
    let content_uris = banger_tile_content_uris(tile);
    let geometric_error = tile.get("geometricError").and_then(Value::as_f64);
    let refine = tile
        .get("refine")
        .and_then(Value::as_str)
        .unwrap_or("INHERIT")
        .to_string();
    let bounding_volume = tile.get("boundingVolume");
    let bounding_volume_kind = banger_bounding_volume_kind(bounding_volume).to_string();
    let bounding_volume_hash = banger_value_hash(bounding_volume);
    let transform_hash = banger_value_hash(tile.get("transform"));
    let tile_id = format!(
        "tile_{}",
        &sha256_hex(
            format!(
                "{path}:{depth}:{geometric_error:?}:{bounding_volume_hash}:{transform_hash}:{}",
                content_uris.join(",")
            )
            .as_bytes()
        )[..16]
    );
    let children = tile.get("children").and_then(Value::as_array);
    let child_count = children.map(|children| children.len()).unwrap_or(0);
    let priority_key = geometric_error.unwrap_or(0.0) / ((depth + 1) as f64);
    out.push(BangerMapsTraversalTile {
        tile_id: tile_id.clone(),
        parent_tile_id,
        depth,
        child_count,
        geometric_error,
        refine,
        bounding_volume_kind,
        bounding_volume_hash,
        transform_hash,
        content_uris,
        priority_key,
    });
    if let Some(children) = children {
        let mut indexed_children = children.iter().enumerate().collect::<Vec<_>>();
        indexed_children.sort_by(|(_, left), (_, right)| {
            let left_error = left.get("geometricError").and_then(Value::as_f64).unwrap_or(0.0);
            let right_error = right.get("geometricError").and_then(Value::as_f64).unwrap_or(0.0);
            right_error
                .partial_cmp(&left_error)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (index, child) in indexed_children {
            collect_banger_maps_traversal_tiles(
                child,
                Some(tile_id.clone()),
                depth + 1,
                format!("{path}.{index}"),
                out,
            );
        }
    }
}

fn banger_tile_content_uris(tile: &Value) -> Vec<String> {
    let mut uris = Vec::new();
    if let Some(uri) = tile
        .get("content")
        .and_then(|content| content.get("uri").or_else(|| content.get("url")))
        .and_then(Value::as_str)
    {
        uris.push(uri.to_string());
    }
    if let Some(contents) = tile.get("contents").and_then(Value::as_array) {
        uris.extend(
            contents
                .iter()
                .filter_map(|content| content.get("uri").or_else(|| content.get("url")).and_then(Value::as_str))
                .map(str::to_string),
        );
    }
    uris
}

fn banger_bounding_volume_kind(value: Option<&Value>) -> &'static str {
    let Some(value) = value else {
        return "none";
    };
    if value.get("region").is_some() {
        "region"
    } else if value.get("box").is_some() {
        "box"
    } else if value.get("sphere").is_some() {
        "sphere"
    } else {
        "unknown"
    }
}

fn banger_value_hash(value: Option<&Value>) -> String {
    value
        .and_then(|value| serde_json::to_vec(value).ok())
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_else(|| sha256_hex(b"none"))
}

fn count_banger_tiles(tile: &Value) -> usize {
    1 + tile
        .get("children")
        .and_then(Value::as_array)
        .map(|children| children.iter().map(count_banger_tiles).sum::<usize>())
        .unwrap_or(0)
}

fn count_banger_tile_content_uris(tile: &Value) -> usize {
    banger_tile_content_uris(tile).len()
        + tile
            .get("children")
            .and_then(Value::as_array)
            .map(|children| children.iter().map(count_banger_tile_content_uris).sum::<usize>())
            .unwrap_or(0)
}

fn redact_url_secret(url: &str) -> String {
    let Some((head, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let redacted = query
        .split('&')
        .map(|part| {
            let key = part.split_once('=').map(|(key, _)| key).unwrap_or(part);
            if key.eq_ignore_ascii_case("key") || key.eq_ignore_ascii_case("access_token") {
                format!("{key}=redacted")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{head}?{redacted}")
}

fn banger_maps_root_ingest_verifier() -> BangerMapsRootIngestVerifier {
    BangerMapsRootIngestVerifier {
        wall: "native_3d_tiles_root_ingestion",
        frontier_hypothesis: "Banger can promote Google/Cesium geospatial rendering by first owning root tileset ingestion and cache hashing.",
        local_gate: "ingen_electron_backend_bridge --banger-maps-root-ingest",
        rollback_path: "CesiumJS visual fallback remains authoritative until native glTF GPU submission is promoted.",
    }
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
    let viewport_x = env::var("FORGE_BANGER_VIEWPORT_X").ok().and_then(|value| value.parse::<i32>().ok()).unwrap_or(0);
    let viewport_y = env::var("FORGE_BANGER_VIEWPORT_Y").ok().and_then(|value| value.parse::<i32>().ok()).unwrap_or(0);
    let fixed_viewport = env::var("FORGE_BANGER_VIEWPORT_FIXED").ok().as_deref() == Some("1");
    let scene_kind = env::var("FORGE_BANGER_SCENE_KIND").unwrap_or_else(|_| "dense_meshlet_field".to_string());
    let child = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            viewport_x,
            viewport_y,
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
    let scene_pipeline = create_banger_first_scene_pipeline(&device, format, present_mode, alpha_mode, &scene_kind);
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
            "banger-native-child-frame:{}:{}:{:?}:{:?}:{:?}:{}:{}:{}:{}:{}:{}:{:?}:{}:{}:{}:{}:{}:{}",
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
            scene_kind,
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
        render_loop_policy: if scene_kind == "maps_sphere" {
            "native_wgpu_maps_sphere_depth_camera_loop_v1"
        } else {
            "native_wgpu_dense_meshlet_field_depth_camera_loop_v2"
        },
        clear_color,
        frame_uniform_hash: frame_uniform_hash.clone(),
        camera_uniform_hash: frame_uniform_hash,
        scene_mesh_hash: scene_pipeline.scene_mesh_hash.clone(),
        shader_source_hash: scene_pipeline.shader_source_hash.clone(),
        render_pipeline_hash: scene_pipeline.render_pipeline_hash.clone(),
        maps_tileset_contract: if scene_kind == "maps_sphere" {
            Some(BangerMapsTilesetContract::google_photorealistic_default())
        } else {
            None
        },
        frame_hash,
        present_loop_hash,
        proof_hash: String::new(),
        host_pid: std::process::id(),
        verifier: BangerNativeHostVerifier {
            wall: if scene_kind == "maps_sphere" { "maps_webview_to_native_3d" } else { "visible_hd_scene_density" },
            frontier_hypothesis: if scene_kind == "maps_sphere" {
                "Maps CodeAct can open Banger native 3D directly and render a globe sphere instead of booting Google Earth WebView."
            } else {
                "Banger can render a dense Nanite-shaped field through one native indexed instanced draw before promoting full meshlet/cluster GPU streaming."
            },
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
        if !fixed_viewport {
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
    scene_kind: &str,
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
    let (vertex_bytes, index_bytes, instance_bytes) = if scene_kind == "maps_sphere" {
        (
            banger_sphere_vertex_bytes(24, 48),
            banger_sphere_index_bytes(24, 48),
            banger_maps_sphere_instance_bytes(),
        )
    } else {
        (
            banger_cube_vertex_bytes(),
            banger_cube_index_bytes(),
            banger_scene_instance_bytes(),
        )
    };
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
            "banger-first-scene-pipeline:{}:{}:{}:{:?}:{:?}:{:?}:{}:instanced_mesh_depth_camera_v1",
            shader_source_hash, scene_mesh_hash, scene_graph_hash, format, present_mode, alpha_mode, scene_kind
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
    @location(2) world_pos: vec3<f32>,
    @location(3) material_kind: f32,
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
    let model = mat4x4<f32>(model_0, model_1, model_2, model_3);
    let world = model * vec4<f32>(position, 1.0);
    var out: VertexOut;
    out.position = frame.view_proj * world;
    out.color = color * instance_tint.rgb;
    out.normal_hint = normalize((model * vec4<f32>(position, 0.0)).xyz);
    out.world_pos = world.xyz;
    out.material_kind = instance_tint.a;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal_hint);
    let sun_dir = normalize(vec3<f32>(0.42, 0.72, 0.48));
    let view_fade = clamp(length(in.world_pos.xz) / 58.0, 0.0, 1.0);
    let lambert = clamp(dot(normal, sun_dir) * 0.62 + 0.38, 0.18, 1.0);
    let sky = mix(vec3<f32>(0.02, 0.035, 0.065), vec3<f32>(0.95, 0.48, 0.18), clamp(in.world_pos.y * 0.04 + 0.35, 0.0, 1.0));
    let bounced = vec3<f32>(0.05, 0.13, 0.16) * (1.0 - clamp(normal.y, -0.15, 0.85));
    let water_glint = smoothstep(2.5, 3.5, in.material_kind) * pow(max(dot(reflect(-sun_dir, normal), normalize(vec3<f32>(0.0, 0.22, 1.0))), 0.0), 18.0);
    let voxel_heat = 0.08 * sin(in.world_pos.x * 0.35 + in.world_pos.z * 0.21 + frame.time_seconds);
    let lit = in.color * (lambert + 0.18) + bounced + vec3<f32>(1.0, 0.72, 0.38) * water_glint + voxel_heat;
    let fog_color = vec3<f32>(0.11, 0.16, 0.22) + sky * 0.18;
    let fogged = mix(lit, fog_color, smoothstep(0.35, 1.0, view_fade));
    return vec4<f32>(max(fogged, vec3<f32>(0.015, 0.018, 0.026)), 1.0);
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
fn banger_sphere_vertex_bytes(lat_segments: u16, lon_segments: u16) -> Vec<u8> {
    let lat_segments = lat_segments.max(3);
    let lon_segments = lon_segments.max(6);
    let mut bytes = Vec::with_capacity(((lat_segments as usize + 1) * (lon_segments as usize + 1)) * 24);
    for lat in 0..=lat_segments {
        let v = lat as f32 / lat_segments as f32;
        let theta = v * std::f32::consts::PI;
        let y = theta.cos();
        let radius = theta.sin();
        for lon in 0..=lon_segments {
            let u = lon as f32 / lon_segments as f32;
            let phi = u * std::f32::consts::TAU;
            let x = radius * phi.cos();
            let z = radius * phi.sin();
            let land = ((phi * 2.7).sin() + (theta * 4.1).cos() + (phi * 0.7 + theta * 1.3).sin()) > 0.55;
            let polar = y.abs() > 0.82;
            let color = if polar {
                [0.86, 0.92, 0.94]
            } else if land {
                [0.20 + 0.18 * v, 0.52 + 0.12 * (u * 6.0).sin().abs(), 0.24]
            } else {
                [0.06, 0.32 + 0.10 * (v * 3.0).sin().abs(), 0.58 + 0.10 * u]
            };
            for value in [x, y, z, color[0], color[1], color[2]] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_sphere_index_bytes(lat_segments: u16, lon_segments: u16) -> Vec<u8> {
    let lat_segments = lat_segments.max(3);
    let lon_segments = lon_segments.max(6);
    let row = lon_segments + 1;
    let mut bytes = Vec::with_capacity(lat_segments as usize * lon_segments as usize * 12);
    for lat in 0..lat_segments {
        for lon in 0..lon_segments {
            let a = lat * row + lon;
            let b = a + row;
            let indices = [a, b, a + 1, a + 1, b, b + 1];
            for index in indices {
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_maps_sphere_instance_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(80);
    for value in banger_model_matrix([0.0, -0.15, 0.0], [2.25, 2.25, 2.25]) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [1.0_f32, 1.0, 1.0, 3.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_scene_instance_bytes() -> Vec<u8> {
    let mut instances = Vec::with_capacity(3400);
    for z in -22..22 {
        for x in -32..32 {
            let xf = x as f32;
            let zf = z as f32;
            let ridge = (xf * 0.21).sin() * 0.45 + (zf * 0.17).cos() * 0.34;
            let distance = (xf * xf + zf * zf).sqrt();
            let height = (ridge - distance * 0.012).max(-0.85);
            let material = if z < -9 { 3.0 } else { 1.0 };
            let tint = if z < -9 {
                [
                    0.08 + 0.02 * (xf * 0.3).sin(),
                    0.34 + 0.04 * (zf * 0.2).cos(),
                    0.52 + 0.06 * (xf * 0.13).sin(),
                    material,
                ]
            } else {
                [
                    0.20 + 0.12 * (height + 0.8).clamp(0.0, 1.0),
                    0.28 + 0.18 * (xf * 0.12).sin().abs(),
                    0.20 + 0.08 * (zf * 0.15).cos().abs(),
                    material,
                ]
            };
            instances.push((
                [xf * 0.92, height - 0.95, zf * 0.92],
                [0.44, 0.05 + height.abs() * 0.05, 0.44],
                tint,
            ));
        }
    }
    for ring in 0..8 {
        let radius = 4.0 + ring as f32 * 1.72;
        let count = 20 + ring * 6;
        for step in 0..count {
            let angle = step as f32 / count as f32 * std::f32::consts::TAU + ring as f32 * 0.19;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius + 4.0;
            let tower_height = 0.35 + ((step * 17 + ring * 11) % 13) as f32 * 0.09;
            instances.push((
                [x, -0.35 + tower_height, z],
                [0.18 + ring as f32 * 0.008, tower_height, 0.18 + ring as f32 * 0.008],
                [
                    0.62 + 0.16 * (angle * 1.7).sin().abs(),
                    0.50 + 0.14 * (ring as f32 * 0.6).cos().abs(),
                    0.86,
                    2.0,
                ],
            ));
        }
    }
    for row in 0..7 {
        for column in 0..11 {
            let x = (column as f32 - 5.0) * 1.15;
            let z = 9.5 + row as f32 * 1.1;
            let height = 0.55 + ((row * 5 + column * 3) % 9) as f32 * 0.16;
            instances.push((
                [x, -0.55 + height, z],
                [0.32, height, 0.32],
                [0.78, 0.42 + row as f32 * 0.035, 0.24 + column as f32 * 0.02, 2.0],
            ));
        }
    }
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
fn banger_model_matrix(translation: [f32; 3], scale: [f32; 3]) -> [f32; 16] {
    [
        scale[0], 0.0, 0.0, 0.0,
        0.0, scale[1], 0.0, 0.0,
        0.0, 0.0, scale[2], 0.0,
        translation[0], translation[1], translation[2], 1.0,
    ]
}

#[cfg(target_os = "windows")]
fn banger_view_projection_matrix(time_seconds: f32, viewport_width: u32, viewport_height: u32) -> [f32; 16] {
    let aspect = (viewport_width as f32 / viewport_height.max(1) as f32).clamp(0.25, 4.0);
    let orbit = time_seconds * 0.08;
    let eye = [
        19.5 * orbit.cos(),
        9.6 + 0.55 * (time_seconds * 0.21).sin(),
        26.0 + 19.5 * orbit.sin(),
    ];
    let view = banger_look_at_rh(eye, [0.0, -0.35, 2.5], [0.0, 1.0, 0.0]);
    let projection = banger_perspective_rh_zo(58.0_f32.to_radians(), aspect, 0.05, 280.0);
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
        assert!(source.contains("world_pos"));
        assert!(source.contains("material_kind"));
        assert!(source.contains("water_glint"));
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
    fn packs_banger_maps_sphere_mesh_for_native_draws() {
        let vertex_bytes = banger_sphere_vertex_bytes(24, 48);
        let index_bytes = banger_sphere_index_bytes(24, 48);
        let instance_bytes = banger_maps_sphere_instance_bytes();
        assert_eq!(vertex_bytes.len(), 25 * 49 * 24);
        assert_eq!(index_bytes.len(), 24 * 48 * 6 * 2);
        assert_eq!(instance_bytes.len(), 80);
        assert_eq!(f32::from_le_bytes(instance_bytes[0..4].try_into().unwrap()), 2.25);
        assert_eq!(f32::from_le_bytes(instance_bytes[76..80].try_into().unwrap()), 3.0);
        assert_eq!(sha256_hex(&vertex_bytes).len(), 64);
        assert_eq!(sha256_hex(&index_bytes).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_banger_scene_instances_for_one_indexed_draw() {
        let instance_bytes = banger_scene_instance_bytes();
        assert!(instance_bytes.len() >= 3000 * 80);
        assert_eq!(instance_bytes.len() % 80, 0);
        assert_eq!(f32::from_le_bytes(instance_bytes[0..4].try_into().unwrap()), 0.44);
        assert!(f32::from_le_bytes(instance_bytes[64..68].try_into().unwrap()) > 0.0);
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

    #[test]
    fn summarizes_3d_tiles_root_for_native_traversal_seed() {
        let root = serde_json::json!({
            "asset": { "version": "1.1" },
            "root": {
                "geometricError": 4096.0,
                "boundingVolume": { "region": [0.0, 0.0, 0.1, 0.1, 0.0, 100.0] },
                "content": { "uri": "root.b3dm" },
                "children": [
                    { "geometricError": 4096.0, "content": { "url": "higher-priority-child.glb" } },
                    { "geometricError": 2048.0, "content": { "uri": "child.glb" } },
                    { "geometricError": 2048.0 }
                ]
            }
        });
        let root_tile = root.get("root").unwrap();
        assert_eq!(count_banger_tiles(root_tile), 4);
        assert_eq!(count_banger_tile_content_uris(root_tile), 3);
        let bytes = serde_json::to_vec(&root).unwrap();
        let projection = summarize_banger_maps_root(
            "https://tile.googleapis.com/v1/3dtiles/root.json?key=secret&foo=bar",
            std::path::Path::new("cache"),
            std::path::Path::new("cache/root.json"),
            &bytes,
            "test",
            false,
            None,
        );
        assert!(projection.ok);
        assert_eq!(projection.schema, "forge.banger.native_3d_tiles_root_ingest.v1");
        assert_eq!(projection.tile_count, 4);
        assert_eq!(projection.content_uri_count, 3);
        assert_eq!(projection.geometric_error, Some(4096.0));
        assert_eq!(projection.asset_version, "1.1");
        assert_eq!(projection.root_hash.len(), 64);
        assert_eq!(projection.traversal_seed_hash.len(), 64);
        assert_eq!(
            projection.traversal_seed.schema,
            "forge.banger.native_3d_tiles_traversal_seed.v1"
        );
        assert_eq!(projection.traversal_seed.total_tile_count, 4);
        assert_eq!(projection.traversal_seed.total_content_uri_count, 3);
        assert_eq!(projection.traversal_seed.queued_tile_count, 4);
        assert_eq!(projection.traversal_seed.deepest_level, 1);
        assert_eq!(projection.traversal_seed.plan_hash.len(), 64);
        assert_eq!(projection.traversal_seed.tiles[0].depth, 0);
        assert_eq!(projection.traversal_seed.tiles[0].bounding_volume_kind, "region");
        assert_eq!(
            projection.traversal_seed.tiles[1].content_uris,
            vec!["higher-priority-child.glb".to_string()]
        );
        assert_eq!(
            projection.traversal_seed.tiles[1].parent_tile_id,
            Some(projection.traversal_seed.tiles[0].tile_id.clone())
        );
        assert_eq!(
            projection.root_tileset_url,
            "https://tile.googleapis.com/v1/3dtiles/root.json?key=redacted&foo=bar"
        );
    }
}

