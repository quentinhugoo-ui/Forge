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
    content_cache: BangerMapsContentCacheProjection,
    content_decode: BangerMapsContentDecodeProjection,
    gpu_staging: BangerMapsGpuStagingProjection,
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
struct BangerMapsContentCacheProjection {
    schema: &'static str,
    enabled: bool,
    cache_dir: String,
    max_fetch_count: usize,
    requested_content_count: usize,
    fetched_content_count: usize,
    cache_hit_count: usize,
    failed_content_count: usize,
    skipped_content_count: usize,
    total_byte_count: usize,
    cache_manifest_hash: String,
    records: Vec<BangerMapsContentCacheRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsContentCacheRecord {
    tile_id: String,
    source_uri: String,
    resolved_url: String,
    cache_path: String,
    extension: String,
    content_type: &'static str,
    cache_hit: bool,
    fetched: bool,
    byte_count: usize,
    content_hash: String,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsContentDecodeProjection {
    schema: &'static str,
    enabled: bool,
    decoded_content_count: usize,
    failed_content_count: usize,
    b3dm_count: usize,
    glb_count: usize,
    gltf_count: usize,
    total_glb_byte_count: usize,
    total_bin_chunk_byte_count: usize,
    decode_manifest_hash: String,
    records: Vec<BangerMapsContentDecodeRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsContentDecodeRecord {
    tile_id: String,
    source_uri: String,
    cache_path: String,
    source_content_type: &'static str,
    container: &'static str,
    byte_count: usize,
    content_hash: String,
    b3dm: Option<BangerB3dmHeaderProjection>,
    glb: Option<BangerGlbProjection>,
    gltf: Option<BangerGltfSummaryProjection>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsGpuStagingProjection {
    schema: &'static str,
    enabled: bool,
    staged_content_count: usize,
    failed_content_count: usize,
    unsupported_extension_count: usize,
    primitive_count: usize,
    vertex_buffer_byte_count: usize,
    index_buffer_byte_count: usize,
    material_count: usize,
    texture_byte_count: usize,
    upload_plan_hash: String,
    records: Vec<BangerMapsGpuStageRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsNativeRenderGateProjection {
    ok: bool,
    schema: &'static str,
    root_ok: bool,
    root_error_code: Option<String>,
    root_error_message: Option<String>,
    requested_content_count: usize,
    fetched_content_count: usize,
    decoded_content_count: usize,
    staged_content_count: usize,
    drawable_mesh_ready: bool,
    draw_source: Option<&'static str>,
    vertex_buffer_byte_count: usize,
    index_buffer_byte_count: usize,
    instance_buffer_byte_count: usize,
    draw_index_count: u32,
    draw_instance_count: u32,
    render_gate_hash: String,
    blocker: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsGpuStageRecord {
    tile_id: String,
    source_uri: String,
    cache_path: String,
    source_content_type: &'static str,
    container: &'static str,
    primitive_stages: Vec<BangerMapsGpuPrimitiveStage>,
    material_stages: Vec<BangerMapsMaterialStage>,
    texture_stages: Vec<BangerMapsTextureStage>,
    format_support: BangerMapsGltfFormatSupport,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsGpuPrimitiveStage {
    mesh_index: usize,
    primitive_index: usize,
    material_index: Option<usize>,
    mode: u32,
    position_accessor: usize,
    index_accessor: Option<usize>,
    vertex_count: usize,
    index_count: usize,
    vertex_buffer_byte_count: usize,
    index_buffer_byte_count: usize,
    vertex_buffer_hash: String,
    index_buffer_hash: String,
    index_format: &'static str,
    vertex_stride_bytes: usize,
    wgpu_vertex_usage: &'static str,
    wgpu_index_usage: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsMaterialStage {
    material_index: usize,
    base_color_factor: [f32; 4],
    metallic_factor: f32,
    roughness_factor: f32,
    base_color_texture: Option<usize>,
    material_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsTextureStage {
    texture_index: usize,
    image_index: Option<usize>,
    mime_type: String,
    source_kind: &'static str,
    byte_count: usize,
    content_hash: String,
    wgpu_usage: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerMapsGltfFormatSupport {
    extensions_used: Vec<String>,
    extensions_required: Vec<String>,
    unsupported_used_extensions: Vec<String>,
    unsupported_required_extensions: Vec<String>,
    compression_blocker: Option<String>,
    upload_policy: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerB3dmHeaderProjection {
    version: u32,
    byte_length: u32,
    feature_table_json_byte_length: u32,
    feature_table_binary_byte_length: u32,
    batch_table_json_byte_length: u32,
    batch_table_binary_byte_length: u32,
    glb_byte_offset: usize,
    glb_byte_count: usize,
    feature_table_hash: String,
    batch_table_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerGlbProjection {
    version: u32,
    declared_byte_length: u32,
    json_chunk_byte_count: usize,
    bin_chunk_byte_count: usize,
    chunk_count: usize,
    unknown_chunk_count: usize,
    json_hash: String,
    bin_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BangerGltfSummaryProjection {
    asset_version: String,
    scene_count: usize,
    node_count: usize,
    mesh_count: usize,
    primitive_count: usize,
    material_count: usize,
    texture_count: usize,
    image_count: usize,
    accessor_count: usize,
    buffer_view_count: usize,
    buffer_count: usize,
    extensions_used_count: usize,
    extensions_required_count: usize,
    extensions_used: Vec<String>,
    extensions_required: Vec<String>,
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
                status: "contract_ready_direct_tiles_required",
                root_ingestion_stage: "3d_tiles_root_json_manifest_ingestion",
                traversal_stage: "screen_space_error_priority_queue_with_tile_budget",
                content_decode_stage: "b3dm_glb_gltf_mesh_material_texture_decode",
                georeference_stage: "wgs84_ecef_to_enu_floating_origin",
                gpu_submission_stage: "meshlet_or_indexed_mesh_upload_pending",
                visual_fallback: "none_direct_tiles_required",
                blocker: "tile_content_or_gltf_upload_required_before_visible_maps_draw",
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
    vertex_count: u32,
    index_count: u32,
    instance_count: u32,
    mesh_source: &'static str,
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
    if env::args().any(|argument| argument == "--banger-maps-gpu-stage") {
        let ingest = banger_maps_root_ingest(Some(true), Some(true), Some(true));
        println!("{}", serde_json::to_string(&ingest).expect("serialize banger maps gpu staging"));
        return;
    }
    if env::args().any(|argument| argument == "--banger-maps-native-render-gate") {
        let gate = banger_maps_native_render_gate();
        println!("{}", serde_json::to_string(&gate).expect("serialize banger maps native render gate"));
        return;
    }
    if env::args().any(|argument| argument == "--banger-maps-content-decode") {
        let ingest = banger_maps_root_ingest(Some(true), Some(true), None);
        println!("{}", serde_json::to_string(&ingest).expect("serialize banger maps content decode"));
        return;
    }
    if env::args().any(|argument| argument == "--banger-maps-content-cache") {
        let ingest = banger_maps_root_ingest(Some(true), None, None);
        println!("{}", serde_json::to_string(&ingest).expect("serialize banger maps content cache"));
        return;
    }
    if env::args().any(|argument| argument == "--banger-maps-root-ingest") {
        let ingest = banger_maps_root_ingest(None, None, None);
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

fn banger_maps_root_ingest(
    force_content_fetch: Option<bool>,
    force_content_decode: Option<bool>,
    force_gpu_staging: Option<bool>,
) -> BangerMapsRootIngestProjection {
    let Some(url) = env::var("FORGE_BANGER_MAPS_ROOT_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()) else {
            let cache_dir = banger_maps_cache_dir();
            let message = "Set FORGE_BANGER_MAPS_ROOT_URL to a direct Google 3D Tiles root URL or a local tileset.json; no Render/proxy fallback is used.".to_string();
            return failed_banger_maps_root_ingest(
                "",
                &cache_dir,
                &cache_dir.join("missing-root-url.root.json"),
                "missing_direct_maps_root_url",
                message,
                false,
            );
        };
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
                return failed_banger_maps_root_ingest(
                    &url,
                    &cache_dir,
                    &cache_path,
                    banger_maps_root_error_code(&message),
                    message,
                    true,
                );
            }
        },
    };
    summarize_banger_maps_root(
        &url,
        &cache_dir,
        &cache_path,
        &bytes,
        source,
        cache_hit,
        error,
        force_content_fetch,
        force_content_decode,
        force_gpu_staging,
    )
}

fn failed_banger_maps_root_ingest(
    url: &str,
    cache_dir: &std::path::Path,
    cache_path: &std::path::Path,
    code: &'static str,
    message: String,
    network_fetch_attempted: bool,
) -> BangerMapsRootIngestProjection {
    let proof_hash = sha256_hex(format!("{url}:{code}:{message}").as_bytes());
    BangerMapsRootIngestProjection {
        ok: false,
        schema: "forge.banger.native_3d_tiles_root_ingest.v1",
        source: if network_fetch_attempted { "network_error_no_cache" } else { "missing_direct_root_url" },
        root_tileset_url: redact_url_secret(url),
        cache_dir: cache_dir.display().to_string(),
        cache_path: cache_path.display().to_string(),
        cache_hit: false,
        network_fetch_attempted,
        root_hash: String::new(),
        root_byte_count: 0,
        tile_count: 0,
        content_uri_count: 0,
        geometric_error: None,
        asset_version: String::new(),
        traversal_seed_hash: proof_hash.clone(),
        traversal_seed: empty_banger_maps_traversal_seed(),
        content_cache: empty_banger_maps_content_cache(cache_dir),
        content_decode: empty_banger_maps_content_decode(),
        gpu_staging: empty_banger_maps_gpu_staging(),
        verifier: banger_maps_root_ingest_verifier(),
        error: Some(BangerNativeError {
            code,
            message,
            proof_hash,
        }),
    }
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
    force_content_fetch: Option<bool>,
    force_content_decode: Option<bool>,
    force_gpu_staging: Option<bool>,
) -> BangerMapsRootIngestProjection {
    let root_hash = sha256_hex(bytes);
    let json_bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let parsed = serde_json::from_slice::<Value>(json_bytes).ok();
    let root = parsed.as_ref().and_then(|value| value.get("root")).or(parsed.as_ref());
    let tile_count = root.map(count_banger_tiles).unwrap_or(0);
    let content_uri_count = root.map(count_banger_tile_content_uris).unwrap_or(0);
    let geometric_error = root.and_then(|value| value.get("geometricError")).and_then(Value::as_f64);
    let traversal_seed = build_banger_maps_traversal_seed(root);
    let content_cache = build_banger_maps_content_cache(url, cache_dir, &traversal_seed, force_content_fetch);
    let content_decode = build_banger_maps_content_decode(&content_cache, force_content_decode);
    let gpu_staging = build_banger_maps_gpu_staging(&content_decode, force_gpu_staging);
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
        content_cache,
        content_decode,
        gpu_staging,
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

fn empty_banger_maps_content_cache(cache_dir: &std::path::Path) -> BangerMapsContentCacheProjection {
    BangerMapsContentCacheProjection {
        schema: "forge.banger.native_3d_tiles_content_cache.v1",
        enabled: false,
        cache_dir: cache_dir.join("contents").display().to_string(),
        max_fetch_count: 0,
        requested_content_count: 0,
        fetched_content_count: 0,
        cache_hit_count: 0,
        failed_content_count: 0,
        skipped_content_count: 0,
        total_byte_count: 0,
        cache_manifest_hash: sha256_hex(b"empty_banger_maps_content_cache"),
        records: Vec::new(),
    }
}

fn build_banger_maps_content_cache(
    root_url: &str,
    cache_dir: &std::path::Path,
    traversal_seed: &BangerMapsTraversalSeed,
    force_content_fetch: Option<bool>,
) -> BangerMapsContentCacheProjection {
    let enabled = force_content_fetch.unwrap_or_else(|| {
        env::var("FORGE_BANGER_MAPS_FETCH_TILE_CONTENT")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
    });
    let max_fetch_count = env::var("FORGE_BANGER_MAPS_CONTENT_FETCH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8);
    let content_cache_dir = cache_dir.join("contents");
    let _ = fs::create_dir_all(&content_cache_dir);
    let requested_content_count = traversal_seed
        .tiles
        .iter()
        .map(|tile| tile.content_uris.len())
        .sum::<usize>();
    let mut records = Vec::new();
    let mut attempted_count = 0usize;
    for tile in &traversal_seed.tiles {
        for source_uri in &tile.content_uris {
            let resolved_url = resolve_banger_tile_content_url(root_url, source_uri);
            let extension = banger_content_extension(source_uri);
            let cache_path = content_cache_dir.join(format!("{}.{}", sha256_hex(resolved_url.as_bytes()), extension));
            let content_type = banger_content_type(&extension);
            if !enabled || attempted_count >= max_fetch_count {
                records.push(BangerMapsContentCacheRecord {
                    tile_id: tile.tile_id.clone(),
                    source_uri: source_uri.clone(),
                    resolved_url: redact_url_secret(&resolved_url),
                    cache_path: cache_path.display().to_string(),
                    extension,
                    content_type,
                    cache_hit: false,
                    fetched: false,
                    byte_count: 0,
                    content_hash: String::new(),
                    error: None,
                });
                continue;
            }
            attempted_count += 1;
            match fs::read(&cache_path) {
                Ok(bytes) => {
                    records.push(BangerMapsContentCacheRecord {
                        tile_id: tile.tile_id.clone(),
                        source_uri: source_uri.clone(),
                        resolved_url: redact_url_secret(&resolved_url),
                        cache_path: cache_path.display().to_string(),
                        extension,
                        content_type,
                        cache_hit: true,
                        fetched: false,
                        byte_count: bytes.len(),
                        content_hash: sha256_hex(&bytes),
                        error: None,
                    });
                }
                Err(_) => match fetch_banger_maps_root(&resolved_url) {
                    Ok(bytes) => {
                        let _ = fs::write(&cache_path, &bytes);
                        records.push(BangerMapsContentCacheRecord {
                            tile_id: tile.tile_id.clone(),
                            source_uri: source_uri.clone(),
                            resolved_url: redact_url_secret(&resolved_url),
                            cache_path: cache_path.display().to_string(),
                            extension,
                            content_type,
                            cache_hit: false,
                            fetched: true,
                            byte_count: bytes.len(),
                            content_hash: sha256_hex(&bytes),
                            error: None,
                        });
                    }
                    Err(error) => {
                        records.push(BangerMapsContentCacheRecord {
                            tile_id: tile.tile_id.clone(),
                            source_uri: source_uri.clone(),
                            resolved_url: redact_url_secret(&resolved_url),
                            cache_path: cache_path.display().to_string(),
                            extension,
                            content_type,
                            cache_hit: false,
                            fetched: false,
                            byte_count: 0,
                            content_hash: String::new(),
                            error: Some(error),
                        });
                    }
                },
            }
        }
    }
    let fetched_content_count = records.iter().filter(|record| record.fetched).count();
    let cache_hit_count = records.iter().filter(|record| record.cache_hit).count();
    let failed_content_count = records.iter().filter(|record| record.error.is_some()).count();
    let skipped_content_count = records
        .iter()
        .filter(|record| !record.fetched && !record.cache_hit && record.error.is_none())
        .count();
    let total_byte_count = records.iter().map(|record| record.byte_count).sum();
    let cache_manifest_hash = sha256_hex(
        records
            .iter()
            .map(|record| {
                format!(
                    "{}:{}:{}:{}:{};",
                    record.tile_id,
                    record.source_uri,
                    record.cache_path,
                    record.byte_count,
                    record.content_hash
                )
            })
            .collect::<String>()
            .as_bytes(),
    );
    BangerMapsContentCacheProjection {
        schema: "forge.banger.native_3d_tiles_content_cache.v1",
        enabled,
        cache_dir: content_cache_dir.display().to_string(),
        max_fetch_count,
        requested_content_count,
        fetched_content_count,
        cache_hit_count,
        failed_content_count,
        skipped_content_count,
        total_byte_count,
        cache_manifest_hash,
        records,
    }
}

fn empty_banger_maps_content_decode() -> BangerMapsContentDecodeProjection {
    BangerMapsContentDecodeProjection {
        schema: "forge.banger.native_3d_tiles_content_decode.v1",
        enabled: false,
        decoded_content_count: 0,
        failed_content_count: 0,
        b3dm_count: 0,
        glb_count: 0,
        gltf_count: 0,
        total_glb_byte_count: 0,
        total_bin_chunk_byte_count: 0,
        decode_manifest_hash: sha256_hex(b"empty_banger_maps_content_decode"),
        records: Vec::new(),
    }
}

fn build_banger_maps_content_decode(
    content_cache: &BangerMapsContentCacheProjection,
    force_content_decode: Option<bool>,
) -> BangerMapsContentDecodeProjection {
    let enabled = force_content_decode.unwrap_or_else(|| {
        env::var("FORGE_BANGER_MAPS_DECODE_TILE_CONTENT")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
    });
    if !enabled {
        return empty_banger_maps_content_decode();
    }
    let records = content_cache
        .records
        .iter()
        .filter(|record| record.error.is_none() && (record.fetched || record.cache_hit))
        .map(decode_banger_maps_content_record)
        .collect::<Vec<_>>();
    let decoded_content_count = records.iter().filter(|record| record.error.is_none()).count();
    let failed_content_count = records.iter().filter(|record| record.error.is_some()).count();
    let b3dm_count = records.iter().filter(|record| record.container == "b3dm").count();
    let glb_count = records.iter().filter(|record| record.container == "glb").count();
    let gltf_count = records.iter().filter(|record| record.container == "gltf").count();
    let total_glb_byte_count = records
        .iter()
        .filter_map(|record| record.glb.as_ref().map(|glb| glb.declared_byte_length as usize))
        .sum();
    let total_bin_chunk_byte_count = records
        .iter()
        .filter_map(|record| record.glb.as_ref().map(|glb| glb.bin_chunk_byte_count))
        .sum();
    let decode_manifest_hash = sha256_hex(
        records
            .iter()
            .map(|record| {
                format!(
                    "{}:{}:{}:{}:{};",
                    record.tile_id,
                    record.source_uri,
                    record.container,
                    record.content_hash,
                    record.error.as_deref().unwrap_or("")
                )
            })
            .collect::<String>()
            .as_bytes(),
    );
    BangerMapsContentDecodeProjection {
        schema: "forge.banger.native_3d_tiles_content_decode.v1",
        enabled,
        decoded_content_count,
        failed_content_count,
        b3dm_count,
        glb_count,
        gltf_count,
        total_glb_byte_count,
        total_bin_chunk_byte_count,
        decode_manifest_hash,
        records,
    }
}

fn empty_banger_maps_gpu_staging() -> BangerMapsGpuStagingProjection {
    BangerMapsGpuStagingProjection {
        schema: "forge.banger.native_3d_tiles_gpu_staging.v1",
        enabled: false,
        staged_content_count: 0,
        failed_content_count: 0,
        unsupported_extension_count: 0,
        primitive_count: 0,
        vertex_buffer_byte_count: 0,
        index_buffer_byte_count: 0,
        material_count: 0,
        texture_byte_count: 0,
        upload_plan_hash: sha256_hex(b"empty_banger_maps_gpu_staging"),
        records: Vec::new(),
    }
}

fn build_banger_maps_gpu_staging(
    content_decode: &BangerMapsContentDecodeProjection,
    force_gpu_staging: Option<bool>,
) -> BangerMapsGpuStagingProjection {
    let enabled = force_gpu_staging.unwrap_or_else(|| {
        env::var("FORGE_BANGER_MAPS_STAGE_GLTF_BUFFERS")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
    });
    if !enabled {
        return empty_banger_maps_gpu_staging();
    }
    let records = content_decode
        .records
        .iter()
        .filter(|record| record.error.is_none())
        .map(stage_banger_maps_gpu_record)
        .collect::<Vec<_>>();
    let staged_content_count = records.iter().filter(|record| record.error.is_none()).count();
    let failed_content_count = records.iter().filter(|record| record.error.is_some()).count();
    let unsupported_extension_count = records
        .iter()
        .map(|record| {
            record.format_support.unsupported_required_extensions.len()
                + record.format_support.unsupported_used_extensions.len()
                + usize::from(record.format_support.compression_blocker.is_some())
        })
        .sum();
    let primitive_count = records.iter().map(|record| record.primitive_stages.len()).sum();
    let vertex_buffer_byte_count = records
        .iter()
        .flat_map(|record| record.primitive_stages.iter())
        .map(|stage| stage.vertex_buffer_byte_count)
        .sum();
    let index_buffer_byte_count = records
        .iter()
        .flat_map(|record| record.primitive_stages.iter())
        .map(|stage| stage.index_buffer_byte_count)
        .sum();
    let material_count = records.iter().map(|record| record.material_stages.len()).sum();
    let texture_byte_count = records
        .iter()
        .flat_map(|record| record.texture_stages.iter())
        .map(|stage| stage.byte_count)
        .sum();
    let upload_plan_hash = sha256_hex(
        records
            .iter()
            .map(|record| {
                format!(
                    "{}:{}:{}:{}:{}:{};",
                    record.tile_id,
                    record.container,
                    record.primitive_stages.len(),
                    record.material_stages.len(),
                    record.texture_stages.len(),
                    record.error.as_deref().unwrap_or("")
                )
            })
            .collect::<String>()
            .as_bytes(),
    );
    BangerMapsGpuStagingProjection {
        schema: "forge.banger.native_3d_tiles_gpu_staging.v1",
        enabled,
        staged_content_count,
        failed_content_count,
        unsupported_extension_count,
        primitive_count,
        vertex_buffer_byte_count,
        index_buffer_byte_count,
        material_count,
        texture_byte_count,
        upload_plan_hash,
        records,
    }
}

fn stage_banger_maps_gpu_record(record: &BangerMapsContentDecodeRecord) -> BangerMapsGpuStageRecord {
    let bytes = match fs::read(&record.cache_path) {
        Ok(bytes) => bytes,
        Err(error) => return failed_banger_gpu_stage_record(record, format!("gpu stage read: {error}")),
    };
    let stage_result = match record.container {
        "b3dm" => decode_banger_b3dm(&bytes).and_then(|(_, glb_bytes)| {
            let decoded = decode_banger_glb_full(glb_bytes)?;
            stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk)
        }),
        "glb" => decode_banger_glb_full(&bytes).and_then(|decoded| {
            stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk)
        }),
        "gltf" => parse_banger_gltf_json_value(&bytes).and_then(|value| stage_banger_gltf_payload(&value, &[])),
        _ => Err(format!("gpu staging unsupported container {}", record.container)),
    };
    let format_support = banger_maps_gltf_format_support_for_record(record, &bytes);
    match stage_result {
        Ok((primitive_stages, material_stages, texture_stages)) => BangerMapsGpuStageRecord {
            tile_id: record.tile_id.clone(),
            source_uri: record.source_uri.clone(),
            cache_path: record.cache_path.clone(),
            source_content_type: record.source_content_type,
            container: record.container,
            primitive_stages,
            material_stages,
            texture_stages,
            format_support,
            error: None,
        },
        Err(error) => failed_banger_gpu_stage_record(record, error),
    }
}

fn failed_banger_gpu_stage_record(record: &BangerMapsContentDecodeRecord, error: String) -> BangerMapsGpuStageRecord {
    BangerMapsGpuStageRecord {
        tile_id: record.tile_id.clone(),
        source_uri: record.source_uri.clone(),
        cache_path: record.cache_path.clone(),
        source_content_type: record.source_content_type,
        container: record.container,
        primitive_stages: Vec::new(),
        material_stages: Vec::new(),
        texture_stages: Vec::new(),
        format_support: banger_maps_gltf_format_support_for_record(record, &fs::read(&record.cache_path).unwrap_or_default()),
        error: Some(error),
    }
}

fn stage_banger_gltf_payload(
    gltf: &Value,
    bin_chunk: &[u8],
) -> Result<(Vec<BangerMapsGpuPrimitiveStage>, Vec<BangerMapsMaterialStage>, Vec<BangerMapsTextureStage>), String> {
    let format_support = banger_maps_gltf_format_support(gltf);
    if let Some(blocker) = format_support.compression_blocker.as_ref() {
        return Err(blocker.clone());
    }
    if !format_support.unsupported_required_extensions.is_empty() {
        return Err(format!(
            "glTF required extensions not yet supported for native upload: {}",
            format_support.unsupported_required_extensions.join(",")
        ));
    }
    let mut primitive_stages = Vec::new();
    let meshes = gltf.get("meshes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh.get("primitives").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            primitive_stages.push(stage_banger_gltf_primitive(
                gltf,
                bin_chunk,
                mesh_index,
                primitive_index,
                primitive,
            )?);
        }
    }
    let material_stages = stage_banger_gltf_materials(gltf);
    let texture_stages = stage_banger_gltf_textures(gltf, bin_chunk)?;
    Ok((primitive_stages, material_stages, texture_stages))
}

fn stage_banger_gltf_primitive(
    gltf: &Value,
    bin_chunk: &[u8],
    mesh_index: usize,
    primitive_index: usize,
    primitive: &Value,
) -> Result<BangerMapsGpuPrimitiveStage, String> {
    if let Some(extensions) = primitive.get("extensions").and_then(Value::as_object) {
        if extensions.contains_key("KHR_draco_mesh_compression") {
            return Err("KHR_draco_mesh_compression primitive decode pending before native upload".to_string());
        }
        if extensions.contains_key("EXT_meshopt_compression") {
            return Err("EXT_meshopt_compression primitive decode pending before native upload".to_string());
        }
    }
    let attributes = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing attributes"))?;
    let position_accessor = attributes
        .get("POSITION")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing POSITION accessor"))?
        as usize;
    let position = banger_gltf_accessor_stage(gltf, bin_chunk, position_accessor)?;
    if position.component_type != 5126 || position.accessor_type != "VEC3" {
        return Err(format!(
            "mesh {mesh_index} primitive {primitive_index} POSITION must be FLOAT VEC3, got {} {}",
            position.component_type, position.accessor_type
        ));
    }
    let index_accessor = primitive.get("indices").and_then(Value::as_u64).map(|value| value as usize);
    let (index_bytes, index_count, index_format) = match index_accessor {
        Some(accessor_index) => {
            let indices = banger_gltf_accessor_stage(gltf, bin_chunk, accessor_index)?;
            if indices.accessor_type != "SCALAR" {
                return Err(format!(
                    "mesh {mesh_index} primitive {primitive_index} indices must be SCALAR, got {}",
                    indices.accessor_type
                ));
            }
            match indices.component_type {
                5121 => {
                    let mut expanded = Vec::with_capacity(indices.bytes.len() * 2);
                    for index in indices.bytes {
                        expanded.extend_from_slice(&(index as u16).to_le_bytes());
                    }
                    (expanded, indices.count, "uint16_expanded_from_uint8")
                }
                5123 => (indices.bytes, indices.count, "uint16"),
                5125 => (indices.bytes, indices.count, "uint32"),
                other => return Err(format!("unsupported index component type {other}")),
            }
        }
        None => (Vec::new(), 0, "none"),
    };
    Ok(BangerMapsGpuPrimitiveStage {
        mesh_index,
        primitive_index,
        material_index: primitive.get("material").and_then(Value::as_u64).map(|value| value as usize),
        mode: primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) as u32,
        position_accessor,
        index_accessor,
        vertex_count: position.count,
        index_count,
        vertex_buffer_byte_count: position.bytes.len(),
        index_buffer_byte_count: index_bytes.len(),
        vertex_buffer_hash: sha256_hex(&position.bytes),
        index_buffer_hash: sha256_hex(&index_bytes),
        index_format,
        vertex_stride_bytes: 12,
        wgpu_vertex_usage: "VERTEX|COPY_DST",
        wgpu_index_usage: "INDEX|COPY_DST",
    })
}

fn banger_maps_gltf_format_support_for_record(
    record: &BangerMapsContentDecodeRecord,
    bytes: &[u8],
) -> BangerMapsGltfFormatSupport {
    let value = match record.container {
        "b3dm" => decode_banger_b3dm(bytes)
            .and_then(|(_, glb_bytes)| decode_banger_glb_full(glb_bytes).map(|decoded| decoded.gltf_value)),
        "glb" => decode_banger_glb_full(bytes).map(|decoded| decoded.gltf_value),
        "gltf" => parse_banger_gltf_json_value(bytes),
        _ => Err(format!("format support unsupported container {}", record.container)),
    };
    value
        .as_ref()
        .map(banger_maps_gltf_format_support)
        .unwrap_or_else(|_| banger_maps_unknown_gltf_format_support())
}

fn banger_maps_unknown_gltf_format_support() -> BangerMapsGltfFormatSupport {
    BangerMapsGltfFormatSupport {
        extensions_used: Vec::new(),
        extensions_required: Vec::new(),
        unsupported_used_extensions: Vec::new(),
        unsupported_required_extensions: Vec::new(),
        compression_blocker: Some("glTF format could not be inspected before native upload".to_string()),
        upload_policy: "raw_float32_position_u16_u32_indices_material_color_texture_staging_v1",
    }
}

fn banger_maps_gltf_format_support(gltf: &Value) -> BangerMapsGltfFormatSupport {
    let extensions_used = banger_gltf_string_array(gltf, "extensionsUsed");
    let extensions_required = banger_gltf_string_array(gltf, "extensionsRequired");
    let supported_extensions = ["KHR_materials_unlit"];
    let unsupported_used_extensions = extensions_used
        .iter()
        .filter(|extension| !supported_extensions.contains(&extension.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let unsupported_required_extensions = extensions_required
        .iter()
        .filter(|extension| !supported_extensions.contains(&extension.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let compression_blocker = ["KHR_draco_mesh_compression", "EXT_meshopt_compression", "KHR_mesh_quantization"]
        .iter()
        .find(|extension| {
            extensions_used.iter().any(|item| item == **extension)
                || extensions_required.iter().any(|item| item == **extension)
        })
        .map(|extension| match *extension {
            "KHR_draco_mesh_compression" => "KHR_draco_mesh_compression decode is required before native vertex/index upload".to_string(),
            "EXT_meshopt_compression" => "EXT_meshopt_compression decode is required before native vertex/index upload".to_string(),
            "KHR_mesh_quantization" => "KHR_mesh_quantization dequantization is required before float32 POSITION upload".to_string(),
            _ => format!("{extension} support pending before native upload"),
        });
    BangerMapsGltfFormatSupport {
        extensions_used,
        extensions_required,
        unsupported_used_extensions,
        unsupported_required_extensions,
        compression_blocker,
        upload_policy: "raw_float32_position_u16_u32_indices_material_color_texture_staging_v1",
    }
}

fn banger_gltf_string_array(gltf: &Value, key: &str) -> Vec<String> {
    gltf.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn stage_banger_gltf_materials(gltf: &Value) -> Vec<BangerMapsMaterialStage> {
    gltf.get("materials")
        .and_then(Value::as_array)
        .map(|materials| {
            materials
                .iter()
                .enumerate()
                .map(|(material_index, material)| {
                    let pbr = material.get("pbrMetallicRoughness");
                    let base_color_factor = pbr
                        .and_then(|value| value.get("baseColorFactor"))
                        .and_then(Value::as_array)
                        .map(|items| {
                            let mut factor = [1.0f32, 1.0, 1.0, 1.0];
                            for (index, item) in items.iter().take(4).enumerate() {
                                factor[index] = item.as_f64().unwrap_or(1.0) as f32;
                            }
                            factor
                        })
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let metallic_factor = pbr
                        .and_then(|value| value.get("metallicFactor"))
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0) as f32;
                    let roughness_factor = pbr
                        .and_then(|value| value.get("roughnessFactor"))
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0) as f32;
                    let base_color_texture = pbr
                        .and_then(|value| value.get("baseColorTexture"))
                        .and_then(|value| value.get("index"))
                        .and_then(Value::as_u64)
                        .map(|value| value as usize);
                    let material_hash = sha256_hex(
                        format!(
                            "{material_index}:{base_color_factor:?}:{metallic_factor}:{roughness_factor}:{base_color_texture:?}"
                        )
                        .as_bytes(),
                    );
                    BangerMapsMaterialStage {
                        material_index,
                        base_color_factor,
                        metallic_factor,
                        roughness_factor,
                        base_color_texture,
                        material_hash,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn stage_banger_gltf_textures(gltf: &Value, bin_chunk: &[u8]) -> Result<Vec<BangerMapsTextureStage>, String> {
    let textures = gltf.get("textures").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let images = gltf.get("images").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut texture_stages = Vec::new();
    for (texture_index, texture) in textures.iter().enumerate() {
        let image_index = texture.get("source").and_then(Value::as_u64).map(|value| value as usize);
        let Some(image_index) = image_index else {
            texture_stages.push(BangerMapsTextureStage {
                texture_index,
                image_index: None,
                mime_type: "image/unknown".to_string(),
                source_kind: "missing_image_source",
                byte_count: 0,
                content_hash: sha256_hex(b"missing_image_source"),
                wgpu_usage: "TEXTURE_BINDING|COPY_DST",
            });
            continue;
        };
        let image = images
            .get(image_index)
            .ok_or_else(|| format!("texture {texture_index} references missing image {image_index}"))?;
        let mime_type = image
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("image/unknown")
            .to_string();
        if let Some(buffer_view_index) = image.get("bufferView").and_then(Value::as_u64).map(|value| value as usize) {
            let image_bytes = banger_gltf_buffer_view_bytes(gltf, bin_chunk, buffer_view_index)?;
            texture_stages.push(BangerMapsTextureStage {
                texture_index,
                image_index: Some(image_index),
                mime_type,
                source_kind: "embedded_buffer_view",
                byte_count: image_bytes.len(),
                content_hash: sha256_hex(&image_bytes),
                wgpu_usage: "TEXTURE_BINDING|COPY_DST",
            });
        } else {
            let source_kind = if image.get("uri").and_then(Value::as_str).map(|uri| uri.starts_with("data:")).unwrap_or(false) {
                "data_uri_pending"
            } else {
                "external_uri_pending"
            };
            texture_stages.push(BangerMapsTextureStage {
                texture_index,
                image_index: Some(image_index),
                mime_type,
                source_kind,
                byte_count: 0,
                content_hash: sha256_hex(source_kind.as_bytes()),
                wgpu_usage: "TEXTURE_BINDING|COPY_DST",
            });
        }
    }
    Ok(texture_stages)
}

struct BangerGltfAccessorStage {
    bytes: Vec<u8>,
    count: usize,
    component_type: u32,
    accessor_type: String,
}

fn banger_gltf_accessor_stage(gltf: &Value, bin_chunk: &[u8], accessor_index: usize) -> Result<BangerGltfAccessorStage, String> {
    let accessors = gltf
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| "gltf accessors array missing".to_string())?;
    let accessor = accessors
        .get(accessor_index)
        .ok_or_else(|| format!("accessor {accessor_index} missing"))?;
    if accessor.get("sparse").is_some() {
        return Err(format!("accessor {accessor_index} sparse upload not implemented"));
    }
    let buffer_view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("accessor {accessor_index} missing bufferView"))? as usize;
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("accessor {accessor_index} missing componentType"))? as u32;
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("accessor {accessor_index} missing count"))? as usize;
    let accessor_type = accessor
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("accessor {accessor_index} missing type"))?
        .to_string();
    let component_size = banger_gltf_component_size(component_type)?;
    let component_count = banger_gltf_type_component_count(&accessor_type)?;
    let element_size = component_size * component_count;
    let (view_offset, view_length, byte_stride) = banger_gltf_buffer_view_layout(gltf, buffer_view_index)?;
    let accessor_offset = accessor.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let start = view_offset + accessor_offset;
    let stride = byte_stride.unwrap_or(element_size);
    if stride < element_size {
        return Err(format!("accessor {accessor_index} byteStride {stride} is smaller than element size {element_size}"));
    }
    let final_byte = if count == 0 {
        start
    } else {
        start + stride * (count - 1) + element_size
    };
    if final_byte > view_offset + view_length || final_byte > bin_chunk.len() {
        return Err(format!("accessor {accessor_index} exceeds GLB BIN chunk"));
    }
    let mut bytes = Vec::with_capacity(count * element_size);
    for item in 0..count {
        let offset = start + item * stride;
        bytes.extend_from_slice(&bin_chunk[offset..offset + element_size]);
    }
    Ok(BangerGltfAccessorStage {
        bytes,
        count,
        component_type,
        accessor_type,
    })
}

fn banger_gltf_buffer_view_bytes(gltf: &Value, bin_chunk: &[u8], buffer_view_index: usize) -> Result<Vec<u8>, String> {
    let (view_offset, view_length, _) = banger_gltf_buffer_view_layout(gltf, buffer_view_index)?;
    let end = view_offset + view_length;
    if end > bin_chunk.len() {
        return Err(format!("bufferView {buffer_view_index} exceeds GLB BIN chunk"));
    }
    Ok(bin_chunk[view_offset..end].to_vec())
}

fn banger_gltf_buffer_view_layout(gltf: &Value, buffer_view_index: usize) -> Result<(usize, usize, Option<usize>), String> {
    let buffer_views = gltf
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| "gltf bufferViews array missing".to_string())?;
    let buffer_view = buffer_views
        .get(buffer_view_index)
        .ok_or_else(|| format!("bufferView {buffer_view_index} missing"))?;
    let buffer_index = buffer_view.get("buffer").and_then(Value::as_u64).unwrap_or(0);
    if buffer_index != 0 {
        return Err(format!("bufferView {buffer_view_index} uses external buffer {buffer_index}"));
    }
    let byte_offset = buffer_view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let byte_length = buffer_view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("bufferView {buffer_view_index} missing byteLength"))? as usize;
    let byte_stride = buffer_view.get("byteStride").and_then(Value::as_u64).map(|value| value as usize);
    Ok((byte_offset, byte_length, byte_stride))
}

fn banger_gltf_component_size(component_type: u32) -> Result<usize, String> {
    match component_type {
        5120 | 5121 => Ok(1),
        5122 | 5123 => Ok(2),
        5125 | 5126 => Ok(4),
        _ => Err(format!("unsupported glTF component type {component_type}")),
    }
}

fn banger_gltf_type_component_count(accessor_type: &str) -> Result<usize, String> {
    match accessor_type {
        "SCALAR" => Ok(1),
        "VEC2" => Ok(2),
        "VEC3" => Ok(3),
        "VEC4" => Ok(4),
        "MAT2" => Ok(4),
        "MAT3" => Ok(9),
        "MAT4" => Ok(16),
        _ => Err(format!("unsupported glTF accessor type {accessor_type}")),
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
struct BangerMapsUploadedTileBuffers {
    vertex_buffers: Vec<wgpu::Buffer>,
    index_buffers: Vec<wgpu::Buffer>,
    texture_staging_buffers: Vec<wgpu::Buffer>,
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn upload_banger_maps_gltf_payload_to_wgpu(
    device: &wgpu::Device,
    gltf: &Value,
    bin_chunk: &[u8],
) -> Result<BangerMapsUploadedTileBuffers, String> {
    let mut vertex_buffers = Vec::new();
    let mut index_buffers = Vec::new();
    let meshes = gltf.get("meshes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh.get("primitives").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing attributes"))?;
            let position_accessor = attributes
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing POSITION accessor"))?
                as usize;
            let position = banger_gltf_accessor_stage(gltf, bin_chunk, position_accessor)?;
            if position.component_type != 5126 || position.accessor_type != "VEC3" {
                return Err(format!(
                    "mesh {mesh_index} primitive {primitive_index} POSITION must be FLOAT VEC3 before wgpu upload"
                ));
            }
            vertex_buffers.push(banger_create_mapped_buffer(
                device,
                "banger maps gltf position vertex buffer",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                &position.bytes,
            ));
            if let Some(index_accessor) = primitive.get("indices").and_then(Value::as_u64).map(|value| value as usize) {
                let indices = banger_gltf_accessor_stage(gltf, bin_chunk, index_accessor)?;
                let index_bytes = match indices.component_type {
                    5121 => {
                        let mut expanded = Vec::with_capacity(indices.bytes.len() * 2);
                        for index in indices.bytes {
                            expanded.extend_from_slice(&(index as u16).to_le_bytes());
                        }
                        expanded
                    }
                    5123 | 5125 => indices.bytes,
                    other => return Err(format!("unsupported index component type {other} before wgpu upload")),
                };
                index_buffers.push(banger_create_mapped_buffer(
                    device,
                    "banger maps gltf index buffer",
                    wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    &index_bytes,
                ));
            }
        }
    }
    let texture_stages = stage_banger_gltf_textures(gltf, bin_chunk)?;
    let mut texture_staging_buffers = Vec::new();
    for texture_stage in texture_stages.iter().filter(|stage| stage.source_kind == "embedded_buffer_view" && stage.byte_count > 0) {
        let texture_index = texture_stage.texture_index;
        let textures = gltf.get("textures").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        let images = gltf.get("images").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        let image_index = textures
            .get(texture_index)
            .and_then(|texture| texture.get("source"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("texture {texture_index} missing source before wgpu staging upload"))?
            as usize;
        let buffer_view_index = images
            .get(image_index)
            .and_then(|image| image.get("bufferView"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("texture {texture_index} image {image_index} missing bufferView before wgpu staging upload"))?
            as usize;
        let image_bytes = banger_gltf_buffer_view_bytes(gltf, bin_chunk, buffer_view_index)?;
        texture_staging_buffers.push(banger_create_mapped_buffer(
            device,
            "banger maps gltf texture staging buffer",
            wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            &image_bytes,
        ));
    }
    Ok(BangerMapsUploadedTileBuffers {
        vertex_buffers,
        index_buffers,
        texture_staging_buffers,
    })
}

fn decode_banger_maps_content_record(record: &BangerMapsContentCacheRecord) -> BangerMapsContentDecodeRecord {
    let bytes = match fs::read(&record.cache_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failed_banger_decode_record(record, "missing", format!("decode read: {error}"));
        }
    };
    let content_hash = sha256_hex(&bytes);
    match record.extension.as_str() {
        "b3dm" => match decode_banger_b3dm(&bytes) {
            Ok((b3dm, glb_bytes)) => match decode_banger_glb(glb_bytes) {
                Ok((glb, gltf)) => BangerMapsContentDecodeRecord {
                    tile_id: record.tile_id.clone(),
                    source_uri: record.source_uri.clone(),
                    cache_path: record.cache_path.clone(),
                    source_content_type: record.content_type,
                    container: "b3dm",
                    byte_count: bytes.len(),
                    content_hash,
                    b3dm: Some(b3dm),
                    glb: Some(glb),
                    gltf: Some(gltf),
                    error: None,
                },
                Err(error) => failed_banger_decode_record(record, "b3dm", error),
            },
            Err(error) => failed_banger_decode_record(record, "b3dm", error),
        },
        "glb" => match decode_banger_glb(&bytes) {
            Ok((glb, gltf)) => BangerMapsContentDecodeRecord {
                tile_id: record.tile_id.clone(),
                source_uri: record.source_uri.clone(),
                cache_path: record.cache_path.clone(),
                source_content_type: record.content_type,
                container: "glb",
                byte_count: bytes.len(),
                content_hash,
                b3dm: None,
                glb: Some(glb),
                gltf: Some(gltf),
                error: None,
            },
            Err(error) => failed_banger_decode_record(record, "glb", error),
        },
        "gltf" => match decode_banger_gltf_json(&bytes) {
            Ok(gltf) => BangerMapsContentDecodeRecord {
                tile_id: record.tile_id.clone(),
                source_uri: record.source_uri.clone(),
                cache_path: record.cache_path.clone(),
                source_content_type: record.content_type,
                container: "gltf",
                byte_count: bytes.len(),
                content_hash,
                b3dm: None,
                glb: None,
                gltf: Some(gltf),
                error: None,
            },
            Err(error) => failed_banger_decode_record(record, "gltf", error),
        },
        _ => failed_banger_decode_record(record, "opaque", format!("unsupported content extension {}", record.extension)),
    }
}

fn failed_banger_decode_record(
    record: &BangerMapsContentCacheRecord,
    container: &'static str,
    error: String,
) -> BangerMapsContentDecodeRecord {
    BangerMapsContentDecodeRecord {
        tile_id: record.tile_id.clone(),
        source_uri: record.source_uri.clone(),
        cache_path: record.cache_path.clone(),
        source_content_type: record.content_type,
        container,
        byte_count: record.byte_count,
        content_hash: record.content_hash.clone(),
        b3dm: None,
        glb: None,
        gltf: None,
        error: Some(error),
    }
}

fn decode_banger_b3dm(bytes: &[u8]) -> Result<(BangerB3dmHeaderProjection, &[u8]), String> {
    if bytes.len() < 28 {
        return Err("b3dm header shorter than 28 bytes".to_string());
    }
    if &bytes[0..4] != b"b3dm" {
        return Err("b3dm magic mismatch".to_string());
    }
    let version = read_u32_le(bytes, 4)?;
    let byte_length = read_u32_le(bytes, 8)?;
    if byte_length as usize > bytes.len() {
        return Err(format!("b3dm declared length {byte_length} exceeds {} bytes", bytes.len()));
    }
    let feature_table_json_byte_length = read_u32_le(bytes, 12)?;
    let feature_table_binary_byte_length = read_u32_le(bytes, 16)?;
    let batch_table_json_byte_length = read_u32_le(bytes, 20)?;
    let batch_table_binary_byte_length = read_u32_le(bytes, 24)?;
    let glb_byte_offset = 28usize
        + feature_table_json_byte_length as usize
        + feature_table_binary_byte_length as usize
        + batch_table_json_byte_length as usize
        + batch_table_binary_byte_length as usize;
    if glb_byte_offset > byte_length as usize {
        return Err("b3dm table lengths exceed declared byte length".to_string());
    }
    let feature_start = 28usize;
    let feature_end = feature_start + feature_table_json_byte_length as usize + feature_table_binary_byte_length as usize;
    let batch_end = feature_end + batch_table_json_byte_length as usize + batch_table_binary_byte_length as usize;
    let glb = &bytes[glb_byte_offset..byte_length as usize];
    Ok((
        BangerB3dmHeaderProjection {
            version,
            byte_length,
            feature_table_json_byte_length,
            feature_table_binary_byte_length,
            batch_table_json_byte_length,
            batch_table_binary_byte_length,
            glb_byte_offset,
            glb_byte_count: glb.len(),
            feature_table_hash: sha256_hex(&bytes[feature_start..feature_end]),
            batch_table_hash: sha256_hex(&bytes[feature_end..batch_end]),
        },
        glb,
    ))
}

struct BangerDecodedGlb<'a> {
    projection: BangerGlbProjection,
    gltf_summary: BangerGltfSummaryProjection,
    gltf_value: Value,
    bin_chunk: &'a [u8],
}

fn decode_banger_glb(bytes: &[u8]) -> Result<(BangerGlbProjection, BangerGltfSummaryProjection), String> {
    let decoded = decode_banger_glb_full(bytes)?;
    Ok((decoded.projection, decoded.gltf_summary))
}

fn decode_banger_glb_full(bytes: &[u8]) -> Result<BangerDecodedGlb<'_>, String> {
    if bytes.len() < 20 {
        return Err("glb shorter than header plus first chunk".to_string());
    }
    if &bytes[0..4] != b"glTF" {
        return Err("glb magic mismatch".to_string());
    }
    let version = read_u32_le(bytes, 4)?;
    let declared_byte_length = read_u32_le(bytes, 8)?;
    if version != 2 {
        return Err(format!("unsupported glb version {version}"));
    }
    if declared_byte_length as usize > bytes.len() {
        return Err(format!("glb declared length {declared_byte_length} exceeds {} bytes", bytes.len()));
    }
    let mut cursor = 12usize;
    let mut chunk_count = 0usize;
    let mut unknown_chunk_count = 0usize;
    let mut json_chunk: Option<&[u8]> = None;
    let mut bin_chunk: Option<&[u8]> = None;
    while cursor + 8 <= declared_byte_length as usize {
        let chunk_length = read_u32_le(bytes, cursor)? as usize;
        let chunk_type = read_u32_le(bytes, cursor + 4)?;
        let data_start = cursor + 8;
        let data_end = data_start + chunk_length;
        if data_end > declared_byte_length as usize {
            return Err("glb chunk exceeds declared length".to_string());
        }
        chunk_count += 1;
        match chunk_type {
            0x4E4F534A => {
                if json_chunk.is_none() {
                    json_chunk = Some(&bytes[data_start..data_end]);
                }
            }
            0x004E4942 => {
                if bin_chunk.is_none() {
                    bin_chunk = Some(&bytes[data_start..data_end]);
                }
            }
            _ => unknown_chunk_count += 1,
        }
        cursor = data_end;
    }
    let json_chunk = json_chunk.ok_or_else(|| "glb JSON chunk missing".to_string())?;
    let gltf_value = parse_banger_gltf_json_value(json_chunk)?;
    let gltf_summary = summarize_banger_gltf_value(&gltf_value);
    let bin = bin_chunk.unwrap_or(&[]);
    Ok(BangerDecodedGlb {
        projection: BangerGlbProjection {
            version,
            declared_byte_length,
            json_chunk_byte_count: json_chunk.len(),
            bin_chunk_byte_count: bin.len(),
            chunk_count,
            unknown_chunk_count,
            json_hash: sha256_hex(json_chunk),
            bin_hash: sha256_hex(bin),
        },
        gltf_summary,
        gltf_value,
        bin_chunk: bin,
    })
}

fn decode_banger_gltf_json(bytes: &[u8]) -> Result<BangerGltfSummaryProjection, String> {
    let value = parse_banger_gltf_json_value(bytes)?;
    Ok(summarize_banger_gltf_value(&value))
}

fn parse_banger_gltf_json_value(bytes: &[u8]) -> Result<Value, String> {
    let json_bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let json_text = std::str::from_utf8(json_bytes)
        .map_err(|error| format!("gltf json utf8: {error}"))?
        .trim_end_matches(|character| character == ' ' || character == '\0');
    serde_json::from_str::<Value>(json_text).map_err(|error| format!("gltf json parse: {error}"))
}

fn summarize_banger_gltf_value(value: &Value) -> BangerGltfSummaryProjection {
    BangerGltfSummaryProjection {
        asset_version: value
            .get("asset")
            .and_then(|asset| asset.get("version"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        scene_count: json_array_len(&value, "scenes"),
        node_count: json_array_len(&value, "nodes"),
        mesh_count: json_array_len(&value, "meshes"),
        primitive_count: value
            .get("meshes")
            .and_then(Value::as_array)
            .map(|meshes| {
                meshes
                    .iter()
                    .map(|mesh| mesh.get("primitives").and_then(Value::as_array).map(|items| items.len()).unwrap_or(0))
                    .sum()
            })
            .unwrap_or(0),
        material_count: json_array_len(&value, "materials"),
        texture_count: json_array_len(&value, "textures"),
        image_count: json_array_len(&value, "images"),
        accessor_count: json_array_len(&value, "accessors"),
        buffer_view_count: json_array_len(&value, "bufferViews"),
        buffer_count: json_array_len(&value, "buffers"),
        extensions_used_count: json_array_len(&value, "extensionsUsed"),
        extensions_required_count: json_array_len(&value, "extensionsRequired"),
        extensions_used: banger_gltf_string_array(value, "extensionsUsed"),
        extensions_required: banger_gltf_string_array(value, "extensionsRequired"),
    }
}

fn json_array_len(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_array).map(|items| items.len()).unwrap_or(0)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset + 4;
    if end > bytes.len() {
        return Err(format!("u32 read beyond buffer at offset {offset}"));
    }
    Ok(u32::from_le_bytes(bytes[offset..end].try_into().expect("slice length checked")))
}

#[cfg(test)]
fn push_u32_le(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn resolve_banger_tile_content_url(root_url: &str, content_uri: &str) -> String {
    if content_uri.starts_with("http://") || content_uri.starts_with("https://") || content_uri.starts_with("file://") {
        return content_uri.to_string();
    }
    if root_url.starts_with("http://") || root_url.starts_with("https://") {
        if let Ok(base) = reqwest::Url::parse(root_url) {
            if let Ok(mut resolved) = base.join(content_uri) {
                if resolved.query().is_none() {
                    resolved.set_query(base.query());
                }
                return resolved.to_string();
            }
        }
    }
    if let Some(path) = root_url.strip_prefix("file://") {
        let local_path = if cfg!(windows) { path.trim_start_matches('/') } else { path };
        return std::path::Path::new(local_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
            .join(content_uri)
            .display()
            .to_string();
    }
    std::path::Path::new(root_url)
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""))
        .join(content_uri)
        .display()
        .to_string()
}

fn banger_content_extension(uri: &str) -> String {
    let path = uri.split('?').next().unwrap_or(uri).trim_end_matches('/');
    let extension = path
        .rsplit('/')
        .next()
        .and_then(|file| file.rsplit_once('.').map(|(_, extension)| extension))
        .unwrap_or("bin")
        .to_ascii_lowercase();
    if extension.chars().all(|character| character.is_ascii_alphanumeric()) && extension.len() <= 8 {
        extension
    } else {
        "bin".to_string()
    }
}

fn banger_content_type(extension: &str) -> &'static str {
    match extension {
        "b3dm" => "batched_3d_model",
        "i3dm" => "instanced_3d_model",
        "pnts" => "point_cloud",
        "cmpt" => "composite",
        "glb" => "binary_gltf",
        "gltf" => "json_gltf",
        _ => "opaque_tile_content",
    }
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

fn banger_maps_root_error_code(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("satellite tiles and 3d tiles are not available")
        || (lower.contains("status 403") && lower.contains("permission"))
    {
        "google_tiles_entitlement_or_region_blocked"
    } else if lower.contains("status 401") || lower.contains("api key") || lower.contains("access token") {
        "google_tiles_credential_rejected"
    } else {
        "root_fetch_failed"
    }
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
    let scene_pipeline = create_banger_first_scene_pipeline(&device, format, present_mode, alpha_mode, &scene_kind)?;
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
            "banger-native-child-frame:{}:{}:{:?}:{:?}:{:?}:{}:{}:{}:{}:{}:{}:{}:{:?}:{}:{}:{}:{}:{}:{}",
            config.width,
            config.height,
            format,
            present_mode,
            alpha_mode,
            parent_hash,
            child_hash,
            scene_pipeline.mesh_source,
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
        vertex_count: scene_pipeline.vertex_count,
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
) -> Result<BangerNativeScenePipeline, String> {
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
    let mesh = if scene_kind == "maps_sphere" {
        banger_maps_first_tile_render_mesh_bytes()?
    } else {
        BangerRenderMeshBytes {
            vertex_bytes: banger_cube_vertex_bytes(),
            index_bytes: banger_cube_index_bytes(),
            instance_bytes: banger_scene_instance_bytes(),
            source: "banger_dense_cube_field_fallback",
        }
    };
    let BangerRenderMeshBytes {
        vertex_bytes,
        index_bytes,
        instance_bytes,
        source: mesh_source,
    } = mesh;
    let instance_buffer_hash = sha256_hex(&instance_bytes);
    let scene_mesh_hash = sha256_hex(
        format!(
            "banger-native-render-mesh-v2:{}:{}:{}",
            mesh_source,
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
    Ok(BangerNativeScenePipeline {
        render_pipeline,
        uniform_buffer,
        bind_group,
        vertex_buffer,
        instance_buffer,
        index_buffer,
        vertex_count: (vertex_bytes.len() / 24) as u32,
        index_count: (index_bytes.len() / 2) as u32,
        instance_count: (instance_bytes.len() / 80) as u32,
        mesh_source,
        scene_mesh_hash,
        scene_graph_hash,
        instance_buffer_hash,
        depth_format: wgpu::TextureFormat::Depth24Plus,
        shader_source_hash,
        render_pipeline_hash,
    })
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
struct BangerRenderMeshBytes {
    vertex_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    instance_bytes: Vec<u8>,
    source: &'static str,
}

#[cfg(target_os = "windows")]
fn banger_maps_native_render_gate() -> BangerMapsNativeRenderGateProjection {
    let ingest = banger_maps_root_ingest(Some(true), Some(true), Some(true));
    let root_error_code = ingest.error.as_ref().map(|error| error.code.to_string());
    let root_error_message = ingest.error.as_ref().map(|error| error.message.clone());
    let mesh_result = banger_maps_first_tile_render_mesh_bytes_from_ingest(&ingest);
    let (drawable_mesh_ready, draw_source, vertex_buffer_byte_count, index_buffer_byte_count, instance_buffer_byte_count, draw_index_count, draw_instance_count, blocker) =
        match mesh_result {
            Ok(mesh) => {
                let draw_index_count = (mesh.index_bytes.len() / 2) as u32;
                let draw_instance_count = (mesh.instance_bytes.len() / 80) as u32;
                (
                    true,
                    Some(mesh.source),
                    mesh.vertex_bytes.len(),
                    mesh.index_bytes.len(),
                    mesh.instance_bytes.len(),
                    draw_index_count,
                    draw_instance_count,
                    None,
                )
            }
            Err(error) => (false, None, 0, 0, 0, 0, 0, Some(error)),
        };
    let render_gate_hash = sha256_hex(
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            ingest.root_hash,
            ingest.content_cache.cache_manifest_hash,
            ingest.content_decode.decode_manifest_hash,
            ingest.gpu_staging.upload_plan_hash,
            drawable_mesh_ready,
            draw_source.unwrap_or("none"),
            draw_index_count,
            blocker.as_deref().unwrap_or("")
        )
        .as_bytes(),
    );
    BangerMapsNativeRenderGateProjection {
        ok: ingest.ok && drawable_mesh_ready,
        schema: "forge.banger.native_3d_tiles_render_gate.v1",
        root_ok: ingest.ok,
        root_error_code,
        root_error_message,
        requested_content_count: ingest.content_cache.requested_content_count,
        fetched_content_count: ingest.content_cache.fetched_content_count,
        decoded_content_count: ingest.content_decode.decoded_content_count,
        staged_content_count: ingest.gpu_staging.staged_content_count,
        drawable_mesh_ready,
        draw_source,
        vertex_buffer_byte_count,
        index_buffer_byte_count,
        instance_buffer_byte_count,
        draw_index_count,
        draw_instance_count,
        render_gate_hash,
        blocker,
    }
}

#[cfg(not(target_os = "windows"))]
fn banger_maps_native_render_gate() -> BangerMapsNativeRenderGateProjection {
    BangerMapsNativeRenderGateProjection {
        ok: false,
        schema: "forge.banger.native_3d_tiles_render_gate.v1",
        root_ok: false,
        root_error_code: Some("unsupported_platform".to_string()),
        root_error_message: Some("Banger native render gate currently requires the Windows wgpu path.".to_string()),
        requested_content_count: 0,
        fetched_content_count: 0,
        decoded_content_count: 0,
        staged_content_count: 0,
        drawable_mesh_ready: false,
        draw_source: None,
        vertex_buffer_byte_count: 0,
        index_buffer_byte_count: 0,
        instance_buffer_byte_count: 0,
        draw_index_count: 0,
        draw_instance_count: 0,
        render_gate_hash: sha256_hex(b"unsupported_banger_maps_native_render_gate_platform"),
        blocker: Some("Banger native render gate currently requires the Windows wgpu path.".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_first_tile_render_mesh_bytes() -> Result<BangerRenderMeshBytes, String> {
    let ingest = banger_maps_root_ingest(Some(true), Some(true), Some(true));
    banger_maps_first_tile_render_mesh_bytes_from_ingest(&ingest)
}

#[cfg(target_os = "windows")]
fn banger_maps_first_tile_render_mesh_bytes_from_ingest(
    ingest: &BangerMapsRootIngestProjection,
) -> Result<BangerRenderMeshBytes, String> {
    if !ingest.ok {
        let error = ingest
            .error
            .as_ref()
            .map(|error| format!("{}: {}", error.code, error.message))
            .unwrap_or_else(|| "root ingest failed without error detail".to_string());
        return Err(format!("Banger Maps native render blocked at root ingest: {error}"));
    }
    if ingest.content_cache.requested_content_count == 0 {
        return Err("Banger Maps native render blocked: root tileset exposes no tile content URI".to_string());
    }
    if ingest.content_cache.fetched_content_count == 0 && ingest.content_cache.cache_hit_count == 0 {
        let first_error = ingest
            .content_cache
            .records
            .iter()
            .find_map(|record| record.error.as_deref())
            .unwrap_or("no tile content fetched or cached");
        return Err(format!("Banger Maps native render blocked at tile content fetch/cache: {first_error}"));
    }
    if ingest.content_decode.decoded_content_count == 0 {
        let first_error = ingest
            .content_decode
            .records
            .iter()
            .find_map(|record| record.error.as_deref())
            .unwrap_or("no tile content decoded into b3dm/glb/gltf");
        return Err(format!("Banger Maps native render blocked at b3dm/glTF decode: {first_error}"));
    }
    if ingest.gpu_staging.staged_content_count == 0 {
        let first_error = ingest
            .gpu_staging
            .records
            .iter()
            .find_map(|record| record.error.as_deref())
            .unwrap_or("no decoded tile staged into vertex/index/material/texture buffers");
        return Err(format!("Banger Maps native render blocked at GPU staging: {first_error}"));
    }
    let mut primitive_errors = Vec::new();
    for record in ingest.content_decode.records.iter().filter(|record| record.error.is_none()) {
        let bytes = match fs::read(&record.cache_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                primitive_errors.push(format!("{} read failed: {error}", record.tile_id));
                continue;
            }
        };
        let candidate = match record.container {
            "b3dm" => decode_banger_b3dm(&bytes)
                .and_then(|(_, glb_bytes)| {
                    let decoded = decode_banger_glb_full(glb_bytes)?;
                    banger_maps_render_mesh_from_gltf(&decoded.gltf_value, decoded.bin_chunk)
                }),
            "glb" => decode_banger_glb_full(&bytes)
                .and_then(|decoded| banger_maps_render_mesh_from_gltf(&decoded.gltf_value, decoded.bin_chunk)),
            _ => Err(format!("render mesh unsupported container {}", record.container)),
        };
        if let Ok(mesh) = candidate {
            return Ok(mesh);
        }
        if let Err(error) = candidate {
            primitive_errors.push(format!("{}: {error}", record.tile_id));
        }
    }
    Err(format!(
        "Banger Maps native render blocked: no drawable glTF primitive after staging ({})",
        primitive_errors.join("; ")
    ))
}

#[cfg(target_os = "windows")]
fn banger_maps_render_mesh_from_gltf(gltf: &Value, bin_chunk: &[u8]) -> Result<BangerRenderMeshBytes, String> {
    let meshes = gltf
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "gltf meshes array missing".to_string())?;
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh.get("primitives").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            match banger_maps_render_mesh_from_primitive(gltf, bin_chunk, primitive) {
                Ok(mesh) => return Ok(mesh),
                Err(error) => {
                    let _ = (mesh_index, primitive_index, error);
                }
            }
        }
    }
    Err("no drawable glTF primitive could be converted to Banger render mesh".to_string())
}

#[cfg(target_os = "windows")]
fn banger_maps_render_mesh_from_primitive(
    gltf: &Value,
    bin_chunk: &[u8],
    primitive: &Value,
) -> Result<BangerRenderMeshBytes, String> {
    let attributes = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| "primitive missing attributes".to_string())?;
    let position_accessor = attributes
        .get("POSITION")
        .and_then(Value::as_u64)
        .ok_or_else(|| "primitive missing POSITION accessor".to_string())? as usize;
    let position = banger_gltf_accessor_stage(gltf, bin_chunk, position_accessor)?;
    if position.component_type != 5126 || position.accessor_type != "VEC3" {
        return Err(format!(
            "render primitive POSITION must be FLOAT VEC3, got {} {}",
            position.component_type, position.accessor_type
        ));
    }
    if position.count > u16::MAX as usize {
        return Err(format!("render primitive has {} vertices; current first draw path is u16", position.count));
    }
    let material_color = primitive
        .get("material")
        .and_then(Value::as_u64)
        .and_then(|index| banger_gltf_material_base_color(gltf, index as usize))
        .unwrap_or([0.54, 0.78, 0.92, 1.0]);
    let vertex_bytes = banger_maps_position_bytes_to_render_vertices(&position.bytes, material_color)?;
    let index_bytes = match primitive.get("indices").and_then(Value::as_u64).map(|value| value as usize) {
        Some(index_accessor) => banger_maps_u16_index_bytes_from_accessor(gltf, bin_chunk, index_accessor)?,
        None => banger_maps_generated_u16_index_bytes(position.count)?,
    };
    Ok(BangerRenderMeshBytes {
        vertex_bytes,
        index_bytes,
        instance_bytes: banger_maps_tile_instance_bytes(),
        source: "banger_maps_3d_tiles_gltf_first_primitive",
    })
}

#[cfg(target_os = "windows")]
fn banger_maps_position_bytes_to_render_vertices(
    position_bytes: &[u8],
    material_color: [f32; 4],
) -> Result<Vec<u8>, String> {
    if position_bytes.len() % 12 != 0 {
        return Err("POSITION byte length is not a multiple of Float32x3".to_string());
    }
    let mut positions = Vec::with_capacity(position_bytes.len() / 12);
    for chunk in position_bytes.chunks_exact(12) {
        positions.push([
            f32::from_le_bytes(chunk[0..4].try_into().expect("position x bytes")),
            f32::from_le_bytes(chunk[4..8].try_into().expect("position y bytes")),
            f32::from_le_bytes(chunk[8..12].try_into().expect("position z bytes")),
        ]);
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in &positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let extent = [
        (max[0] - min[0]).abs(),
        (max[1] - min[1]).abs(),
        (max[2] - min[2]).abs(),
    ];
    let scale = extent.into_iter().fold(0.0_f32, f32::max).max(1.0);
    let mut bytes = Vec::with_capacity(positions.len() * 24);
    for position in positions {
        let normalized = [
            (position[0] - center[0]) / scale * 2.8,
            (position[1] - center[1]) / scale * 2.8,
            (position[2] - center[2]) / scale * 2.8,
        ];
        for value in [
            normalized[0],
            normalized[1],
            normalized[2],
            material_color[0],
            material_color[1],
            material_color[2],
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn banger_maps_u16_index_bytes_from_accessor(
    gltf: &Value,
    bin_chunk: &[u8],
    accessor_index: usize,
) -> Result<Vec<u8>, String> {
    let indices = banger_gltf_accessor_stage(gltf, bin_chunk, accessor_index)?;
    if indices.accessor_type != "SCALAR" {
        return Err(format!("render indices must be SCALAR, got {}", indices.accessor_type));
    }
    match indices.component_type {
        5121 => {
            let mut bytes = Vec::with_capacity(indices.bytes.len() * 2);
            for index in indices.bytes {
                bytes.extend_from_slice(&(index as u16).to_le_bytes());
            }
            Ok(bytes)
        }
        5123 => Ok(indices.bytes),
        5125 => {
            let mut bytes = Vec::with_capacity(indices.count * 2);
            for chunk in indices.bytes.chunks_exact(4) {
                let index = u32::from_le_bytes(chunk.try_into().expect("u32 index bytes"));
                if index > u16::MAX as u32 {
                    return Err(format!("u32 index {index} exceeds current u16 draw path"));
                }
                bytes.extend_from_slice(&(index as u16).to_le_bytes());
            }
            Ok(bytes)
        }
        other => Err(format!("unsupported render index component type {other}")),
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_generated_u16_index_bytes(vertex_count: usize) -> Result<Vec<u8>, String> {
    if vertex_count > u16::MAX as usize {
        return Err(format!("cannot generate u16 indices for {vertex_count} vertices"));
    }
    let mut bytes = Vec::with_capacity(vertex_count * 2);
    for index in 0..vertex_count {
        bytes.extend_from_slice(&(index as u16).to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn banger_gltf_material_base_color(gltf: &Value, material_index: usize) -> Option<[f32; 4]> {
    let material = gltf.get("materials").and_then(Value::as_array)?.get(material_index)?;
    material
        .get("pbrMetallicRoughness")
        .and_then(|pbr| pbr.get("baseColorFactor"))
        .and_then(Value::as_array)
        .map(|items| {
            let mut color = [1.0f32, 1.0, 1.0, 1.0];
            for (index, item) in items.iter().take(4).enumerate() {
                color[index] = item.as_f64().unwrap_or(1.0) as f32;
            }
            color
        })
}

#[cfg(target_os = "windows")]
fn banger_maps_tile_instance_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(80);
    for value in banger_model_matrix([0.0, -0.2, 0.0], [1.0, 1.0, 1.0]) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [1.0_f32, 1.0, 1.0, 2.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
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
    fn converts_staged_gltf_into_banger_render_mesh_bytes() {
        let glb = test_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let mesh = banger_maps_render_mesh_from_gltf(&decoded.gltf_value, decoded.bin_chunk).unwrap();
        assert_eq!(mesh.source, "banger_maps_3d_tiles_gltf_first_primitive");
        assert_eq!(mesh.vertex_bytes.len(), 3 * 24);
        assert_eq!(mesh.index_bytes.len(), 3 * 2);
        assert_eq!(mesh.instance_bytes.len(), 80);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[12..16].try_into().unwrap()), 0.7);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[16..20].try_into().unwrap()), 0.82);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[20..24].try_into().unwrap()), 0.9);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[0..2].try_into().unwrap()), 0);
        assert_eq!(sha256_hex(&mesh.vertex_bytes).len(), 64);
        assert_eq!(sha256_hex(&mesh.index_bytes).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn gates_maps_native_render_on_real_tile_content_not_sphere_fallback() {
        let root = serde_json::json!({
            "asset": { "version": "1.1" },
            "root": {
                "geometricError": 1.0,
                "content": { "uri": "tile.glb" }
            }
        });
        let bytes = serde_json::to_vec(&root).unwrap();
        let cache_dir = env::temp_dir().join(format!(
            "forge-banger-render-gate-empty-test-{}",
            sha256_hex(format!("{:?}", SystemTime::now()).as_bytes())
        ));
        fs::create_dir_all(&cache_dir).unwrap();
        let projection = summarize_banger_maps_root(
            "https://tile.googleapis.com/v1/3dtiles/root.json?key=secret",
            &cache_dir,
            &cache_dir.join("root.json"),
            &bytes,
            "test",
            false,
            None,
            Some(false),
            Some(false),
            Some(false),
        );
        let error = match banger_maps_first_tile_render_mesh_bytes_from_ingest(&projection) {
            Ok(_) => panic!("Maps native render gate unexpectedly accepted missing tile content"),
            Err(error) => error,
        };
        assert!(error.contains("tile content fetch/cache"));
        assert!(!error.contains("sphere"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn promotes_cached_b3dm_to_drawable_maps_render_gate_mesh() {
        let cache_dir = env::temp_dir().join(format!(
            "forge-banger-render-gate-b3dm-test-{}",
            sha256_hex(format!("{:?}", SystemTime::now()).as_bytes())
        ));
        let source_dir = cache_dir.join("source");
        fs::create_dir_all(&source_dir).unwrap();
        let root_path = source_dir.join("tileset.json");
        let content_path = source_dir.join("tile.b3dm");
        fs::write(&content_path, test_b3dm_bytes()).unwrap();
        fs::write(
            &root_path,
            br#"{"asset":{"version":"1.1"},"root":{"geometricError":1,"content":{"uri":"tile.b3dm"}}}"#,
        )
        .unwrap();
        let bytes = fs::read(&root_path).unwrap();
        let projection = summarize_banger_maps_root(
            root_path.to_str().unwrap(),
            &cache_dir,
            &cache_dir.join("root.json"),
            &bytes,
            "test",
            false,
            None,
            Some(true),
            Some(true),
            Some(true),
        );
        let mesh = banger_maps_first_tile_render_mesh_bytes_from_ingest(&projection).unwrap();
        assert_eq!(mesh.source, "banger_maps_3d_tiles_gltf_first_primitive");
        assert_eq!(mesh.vertex_bytes.len(), 3 * 24);
        assert_eq!(mesh.index_bytes.len(), 3 * 2);
        assert_eq!(mesh.instance_bytes.len(), 80);
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
            Some(false),
            Some(false),
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
        assert!(!projection.content_cache.enabled);
        assert_eq!(projection.content_cache.requested_content_count, 3);
        assert_eq!(projection.content_cache.skipped_content_count, 3);
    }

    #[test]
    fn fetches_and_caches_traversal_tile_content_from_relative_uri() {
        let cache_dir = env::temp_dir().join(format!(
            "forge-banger-content-cache-test-{}",
            sha256_hex(format!("{:?}", SystemTime::now()).as_bytes())
        ));
        let source_dir = cache_dir.join("source");
        fs::create_dir_all(&source_dir).unwrap();
        let root_path = source_dir.join("tileset.json");
        let content_path = source_dir.join("tile.glb");
        fs::write(&content_path, b"glb-bytes").unwrap();
        fs::write(&root_path, br#"{"asset":{"version":"1.1"},"root":{"geometricError":1,"content":{"uri":"tile.glb"}}}"#).unwrap();
        let bytes = fs::read(&root_path).unwrap();
        let projection = summarize_banger_maps_root(
            root_path.to_str().unwrap(),
            &cache_dir,
            &cache_dir.join("root.json"),
            &bytes,
            "test",
            false,
            None,
            Some(true),
            Some(false),
            None,
        );
        assert!(projection.ok);
        assert!(projection.content_cache.enabled);
        assert_eq!(projection.content_cache.requested_content_count, 1);
        assert_eq!(projection.content_cache.fetched_content_count, 1);
        assert_eq!(projection.content_cache.cache_hit_count, 0);
        assert_eq!(projection.content_cache.failed_content_count, 0);
        assert_eq!(projection.content_cache.total_byte_count, 9);
        assert_eq!(projection.content_cache.records[0].extension, "glb");
        assert_eq!(projection.content_cache.records[0].content_type, "binary_gltf");
        assert_eq!(projection.content_cache.records[0].content_hash, sha256_hex(b"glb-bytes"));
        assert!(std::path::Path::new(&projection.content_cache.records[0].cache_path).exists());
    }

    #[test]
    fn decodes_glb_json_and_bin_chunks_for_gltf_summary() {
        let glb = test_glb_bytes();
        let (glb_projection, gltf) = decode_banger_glb(&glb).unwrap();
        assert_eq!(glb_projection.version, 2);
        assert_eq!(glb_projection.chunk_count, 2);
        assert_eq!(glb_projection.unknown_chunk_count, 0);
        assert!(glb_projection.json_chunk_byte_count > 0);
        assert_eq!(glb_projection.bin_chunk_byte_count, 48);
        assert_eq!(gltf.asset_version, "2.0");
        assert_eq!(gltf.mesh_count, 1);
        assert_eq!(gltf.primitive_count, 1);
        assert_eq!(gltf.material_count, 1);
        assert_eq!(gltf.texture_count, 1);
        assert_eq!(gltf.image_count, 1);
        assert_eq!(gltf.accessor_count, 2);
        assert_eq!(gltf.buffer_view_count, 3);
        assert_eq!(gltf.buffer_count, 1);
        assert!(gltf.extensions_used.is_empty());
        assert!(gltf.extensions_required.is_empty());
    }

    #[test]
    fn reports_compressed_gltf_extension_blockers_before_gpu_staging() {
        let glb = test_draco_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let support = banger_maps_gltf_format_support(&decoded.gltf_value);
        assert_eq!(support.extensions_used, vec!["KHR_draco_mesh_compression".to_string()]);
        assert_eq!(support.extensions_required, vec!["KHR_draco_mesh_compression".to_string()]);
        assert_eq!(support.unsupported_required_extensions, vec!["KHR_draco_mesh_compression".to_string()]);
        assert!(support
            .compression_blocker
            .as_deref()
            .unwrap()
            .contains("KHR_draco_mesh_compression"));
        let error = match stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk) {
            Ok(_) => panic!("compressed glTF unexpectedly staged without Draco decode"),
            Err(error) => error,
        };
        assert!(error.contains("KHR_draco_mesh_compression"));
    }

    #[test]
    fn classifies_google_tiles_entitlement_region_errors() {
        let message = "root status 403: satellite tiles and 3D tiles are not available for your account and region";
        assert_eq!(
            banger_maps_root_error_code(message),
            "google_tiles_entitlement_or_region_blocked"
        );
    }

    #[test]
    fn decodes_cached_b3dm_content_into_embedded_glb_summary() {
        let cache_dir = env::temp_dir().join(format!(
            "forge-banger-content-decode-test-{}",
            sha256_hex(format!("{:?}", SystemTime::now()).as_bytes())
        ));
        let source_dir = cache_dir.join("source");
        fs::create_dir_all(&source_dir).unwrap();
        let root_path = source_dir.join("tileset.json");
        let content_path = source_dir.join("tile.b3dm");
        fs::write(&content_path, test_b3dm_bytes()).unwrap();
        fs::write(
            &root_path,
            br#"{"asset":{"version":"1.1"},"root":{"geometricError":1,"content":{"uri":"tile.b3dm"}}}"#,
        )
        .unwrap();
        let bytes = fs::read(&root_path).unwrap();
        let projection = summarize_banger_maps_root(
            root_path.to_str().unwrap(),
            &cache_dir,
            &cache_dir.join("root.json"),
            &bytes,
            "test",
            false,
            None,
            Some(true),
            Some(true),
            Some(true),
        );
        assert!(projection.ok);
        assert!(projection.content_decode.enabled);
        assert_eq!(projection.content_decode.decoded_content_count, 1);
        assert_eq!(projection.content_decode.failed_content_count, 0);
        assert_eq!(projection.content_decode.b3dm_count, 1);
        assert_eq!(projection.content_decode.glb_count, 0);
        assert_eq!(projection.content_decode.records[0].container, "b3dm");
        assert_eq!(projection.content_decode.records[0].source_content_type, "batched_3d_model");
        let b3dm = projection.content_decode.records[0].b3dm.as_ref().unwrap();
        assert_eq!(b3dm.version, 1);
        assert!(b3dm.glb_byte_count > 0);
        let glb = projection.content_decode.records[0].glb.as_ref().unwrap();
        assert_eq!(glb.version, 2);
        assert_eq!(glb.bin_chunk_byte_count, 48);
        let gltf = projection.content_decode.records[0].gltf.as_ref().unwrap();
        assert_eq!(gltf.mesh_count, 1);
        assert_eq!(gltf.primitive_count, 1);
        assert!(projection.gpu_staging.enabled);
        assert_eq!(projection.gpu_staging.staged_content_count, 1);
        assert_eq!(projection.gpu_staging.failed_content_count, 0);
        assert_eq!(projection.gpu_staging.primitive_count, 1);
        assert_eq!(projection.gpu_staging.vertex_buffer_byte_count, 36);
        assert_eq!(projection.gpu_staging.index_buffer_byte_count, 6);
        assert_eq!(projection.gpu_staging.texture_byte_count, 4);
        let stage = &projection.gpu_staging.records[0];
        assert_eq!(stage.primitive_stages[0].vertex_count, 3);
        assert_eq!(stage.primitive_stages[0].index_count, 3);
        assert_eq!(stage.primitive_stages[0].index_format, "uint16");
        assert_eq!(stage.primitive_stages[0].wgpu_vertex_usage, "VERTEX|COPY_DST");
        assert_eq!(stage.primitive_stages[0].wgpu_index_usage, "INDEX|COPY_DST");
        assert_eq!(stage.material_stages[0].base_color_texture, Some(0));
        assert_eq!(stage.texture_stages[0].source_kind, "embedded_buffer_view");
        assert_eq!(stage.texture_stages[0].wgpu_usage, "TEXTURE_BINDING|COPY_DST");
    }

    #[test]
    fn stages_glb_primitive_buffers_for_wgpu_upload_plan() {
        let glb = test_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let (primitives, materials, textures) = stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk).unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0].position_accessor, 0);
        assert_eq!(primitives[0].index_accessor, Some(1));
        assert_eq!(primitives[0].vertex_buffer_byte_count, 36);
        assert_eq!(primitives[0].index_buffer_byte_count, 6);
        assert_eq!(primitives[0].vertex_buffer_hash.len(), 64);
        assert_eq!(primitives[0].index_buffer_hash.len(), 64);
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].base_color_factor, [0.7, 0.82, 0.9, 1.0]);
        assert_eq!(materials[0].metallic_factor, 0.0);
        assert_eq!(materials[0].roughness_factor, 0.45);
        assert_eq!(textures.len(), 1);
        assert_eq!(textures[0].byte_count, 4);
        assert_eq!(textures[0].content_hash, sha256_hex(&[137, 80, 78, 71]));
    }

    fn test_glb_bytes() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"material":0,"mode":4}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.7,0.82,0.9,1.0],"metallicFactor":0.0,"roughnessFactor":0.45,"baseColorTexture":{"index":0}}}],"textures":[{"source":0}],"images":[{"bufferView":2,"mimeType":"image/png"}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36,"target":34962},{"buffer":0,"byteOffset":36,"byteLength":6,"target":34963},{"buffer":0,"byteOffset":44,"byteLength":4}],"buffers":[{"byteLength":48}]}"#;
        let mut json_chunk = json.to_vec();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(0x20);
        }
        let mut bin_chunk = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin_chunk.extend_from_slice(&value.to_le_bytes());
        }
        for index in [0u16, 1, 2] {
            bin_chunk.extend_from_slice(&index.to_le_bytes());
        }
        bin_chunk.extend_from_slice(&[0, 0]);
        bin_chunk.extend_from_slice(&[137, 80, 78, 71]);
        while bin_chunk.len() % 4 != 0 {
            bin_chunk.push(0);
        }
        let length = 12 + 8 + json_chunk.len() + 8 + bin_chunk.len();
        let mut glb = Vec::with_capacity(length);
        glb.extend_from_slice(b"glTF");
        push_u32_le(&mut glb, 2);
        push_u32_le(&mut glb, length as u32);
        push_u32_le(&mut glb, json_chunk.len() as u32);
        push_u32_le(&mut glb, 0x4E4F534A);
        glb.extend_from_slice(&json_chunk);
        push_u32_le(&mut glb, bin_chunk.len() as u32);
        push_u32_le(&mut glb, 0x004E4942);
        glb.extend_from_slice(&bin_chunk);
        glb
    }

    fn test_draco_glb_bytes() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"},"extensionsUsed":["KHR_draco_mesh_compression"],"extensionsRequired":["KHR_draco_mesh_compression"],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"extensions":{"KHR_draco_mesh_compression":{"bufferView":0,"attributes":{"POSITION":0}}}}]}],"accessors":[{"componentType":5126,"count":3,"type":"VEC3"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":4}],"buffers":[{"byteLength":4}]}"#;
        let mut json_chunk = json.to_vec();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(0x20);
        }
        let mut bin_chunk = vec![0u8, 1, 2, 3];
        while bin_chunk.len() % 4 != 0 {
            bin_chunk.push(0);
        }
        let length = 12 + 8 + json_chunk.len() + 8 + bin_chunk.len();
        let mut glb = Vec::with_capacity(length);
        glb.extend_from_slice(b"glTF");
        push_u32_le(&mut glb, 2);
        push_u32_le(&mut glb, length as u32);
        push_u32_le(&mut glb, json_chunk.len() as u32);
        push_u32_le(&mut glb, 0x4E4F534A);
        glb.extend_from_slice(&json_chunk);
        push_u32_le(&mut glb, bin_chunk.len() as u32);
        push_u32_le(&mut glb, 0x004E4942);
        glb.extend_from_slice(&bin_chunk);
        glb
    }

    fn test_b3dm_bytes() -> Vec<u8> {
        let glb = test_glb_bytes();
        let feature_json = br#"{"BATCH_LENGTH":0}"#;
        let byte_length = 28 + feature_json.len() + glb.len();
        let mut b3dm = Vec::with_capacity(byte_length);
        b3dm.extend_from_slice(b"b3dm");
        push_u32_le(&mut b3dm, 1);
        push_u32_le(&mut b3dm, byte_length as u32);
        push_u32_le(&mut b3dm, feature_json.len() as u32);
        push_u32_le(&mut b3dm, 0);
        push_u32_le(&mut b3dm, 0);
        push_u32_le(&mut b3dm, 0);
        b3dm.extend_from_slice(feature_json);
        b3dm.extend_from_slice(&glb);
        b3dm
    }
}

