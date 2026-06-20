use ingen_native_services::banger_native_engine::{
    BangerNativeEngine, BangerNativePresentLoopBootstrapRequest,
};
use ingen_native_services::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapterProbe};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::{env, fs};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
const BANGER_RENDER_VERTEX_STRIDE_BYTES: usize = 64;
#[cfg(target_os = "windows")]
static BANGER_MAPS_CAMERA_DEBUG_LOGGED: AtomicBool = AtomicBool::new(false);

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
    maps_visual_gate: Option<BangerMapsNativeRenderGateProjection>,
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
    global_transform: [f64; 16],
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
    tile_global_transform: [f64; 16],
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
    tile_global_transform: [f64; 16],
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
    nonblack_pixel_count: u32,
    non_fallback_blue_pixel_count: u32,
    frame_hash: String,
    frame_preview_width: u32,
    frame_preview_height: u32,
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
    normal_accessor: Option<usize>,
    texcoord0_accessor: Option<usize>,
    index_accessor: Option<usize>,
    vertex_count: usize,
    index_count: usize,
    source_position_buffer_byte_count: usize,
    vertex_buffer_byte_count: usize,
    index_buffer_byte_count: usize,
    vertex_buffer_hash: String,
    index_buffer_hash: String,
    index_format: &'static str,
    vertex_stride_bytes: usize,
    vertex_layout: &'static str,
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
    normal_texture: Option<usize>,
    normal_scale: f32,
    metallic_roughness_texture: Option<usize>,
    occlusion_texture: Option<usize>,
    occlusion_strength: f32,
    emissive_texture: Option<usize>,
    emissive_factor: [f32; 3],
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
    rtc_center: Option<[f64; 3]>,
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
            root_tileset_endpoint: "ion://google-photorealistic-3d-tiles",
            root_request_ttl_hours: 3,
            native_streamer: BangerMapsNative3DTilesStreamer {
                schema: "forge.banger.native_3d_tiles_streamer.v1",
                authority: "banger_native_engine",
                status: "native_visible_tile_batch_draw_ready_direct_tiles_required",
                root_ingestion_stage: "3d_tiles_root_json_manifest_ingestion",
                traversal_stage: "screen_space_error_priority_queue_with_tile_budget",
                content_decode_stage: "b3dm_glb_gltf_mesh_material_texture_decode",
                georeference_stage: "wgs84_ecef_to_enu_floating_origin_live",
                gpu_submission_stage: "visible_tile_batch_indexed_mesh_wgpu_draw_ready",
                visual_fallback: "none_direct_tiles_required",
                blocker: "screen_space_error_traversal_material_texture_streaming_required_for_full_cesium_parity",
            },
            georeference: BangerMapsGeoreference {
                ellipsoid: "WGS84",
                origin_latitude: banger_env_f64("FORGE_BANGER_MAPS_ORIGIN_LATITUDE").unwrap_or(37.42207),
                origin_longitude: banger_env_f64("FORGE_BANGER_MAPS_ORIGIN_LONGITUDE").unwrap_or(-122.08409),
                origin_height_meters: banger_env_f64("FORGE_BANGER_MAPS_ORIGIN_HEIGHT_METERS")
                    .unwrap_or(0.0) as f32,
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
    sky_present_bind_group_layout: wgpu::BindGroupLayout,
    sky_present_pipeline: wgpu::RenderPipeline,
    ssao_present_bind_group_layout: wgpu::BindGroupLayout,
    ssao_present_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    _indirect_draw_buffer: wgpu::Buffer,
    meshlet_culled_indirect_draw_buffer: wgpu::Buffer,
    meshlet_culled_indirect_seed_buffer: wgpu::Buffer,
    meshlet_cluster_buffer: wgpu::Buffer,
    visible_meshlet_cluster_buffer: wgpu::Buffer,
    meshlet_cull_feedback_buffer: wgpu::Buffer,
    meshlet_cull_param_buffer: wgpu::Buffer,
    meshlet_cull_bind_group_layout: wgpu::BindGroupLayout,
    meshlet_cull_pipeline: wgpu::ComputePipeline,
    _material_buffer: Option<wgpu::Buffer>,
    _material_bin_buffer: wgpu::Buffer,
    _texture_staging_buffers: Vec<wgpu::Buffer>,
    _texture_resources: Vec<BangerNativeTextureResource>,
    _residency_feedback_buffer: wgpu::Buffer,
    _shared_residency_page_table_buffer: wgpu::Buffer,
    _shared_residency_compacted_feedback_buffer: wgpu::Buffer,
    _shared_residency_eviction_plan_buffer: wgpu::Buffer,
    _shared_residency_budget_buffer: wgpu::Buffer,
    _lumen_surface_card_buffer: wgpu::Buffer,
    _lumen_surface_cache_feedback_buffer: wgpu::Buffer,
    _lumen_screen_probe_buffer: wgpu::Buffer,
    _lumen_radiance_cache_buffer: wgpu::Buffer,
    virtual_shadow_map_page_table_buffer: wgpu::Buffer,
    virtual_shadow_map_page_flags_buffer: wgpu::Buffer,
    virtual_shadow_map_page_request_buffer: wgpu::Buffer,
    virtual_shadow_map_physical_page_metadata_buffer: wgpu::Buffer,
    virtual_shadow_map_projection_buffer: wgpu::Buffer,
    virtual_shadow_map_mark_params_buffer: wgpu::Buffer,
    _virtual_shadow_map_physical_page_pool_texture: wgpu::Texture,
    virtual_shadow_map_physical_page_pool_view: wgpu::TextureView,
    _virtual_shadow_map_projection_texture: wgpu::Texture,
    virtual_shadow_map_projection_view: wgpu::TextureView,
    virtual_shadow_map_cache_invalidation_buffer: wgpu::Buffer,
    virtual_shadow_map_projection_params_buffer: wgpu::Buffer,
    virtual_shadow_map_mark_bind_group_layout: wgpu::BindGroupLayout,
    virtual_shadow_map_mark_pipeline: wgpu::ComputePipeline,
    virtual_shadow_map_physical_page_bind_group_layout: wgpu::BindGroupLayout,
    virtual_shadow_map_physical_page_pipeline: wgpu::ComputePipeline,
    virtual_shadow_map_projection_bind_group_layout: wgpu::BindGroupLayout,
    virtual_shadow_map_projection_pipeline: wgpu::ComputePipeline,
    spectral_ocean_bind_group_layout: wgpu::BindGroupLayout,
    spectral_ocean_pipeline: wgpu::ComputePipeline,
    single_layer_water_bind_group_layout: wgpu::BindGroupLayout,
    single_layer_water_pipeline: wgpu::ComputePipeline,
    single_layer_water_present_bind_group_layout: wgpu::BindGroupLayout,
    single_layer_water_present_pipeline: wgpu::RenderPipeline,
    single_layer_water_present_sampler: wgpu::Sampler,
    bloom_present_bind_group_layout: wgpu::BindGroupLayout,
    bloom_present_pipeline: wgpu::RenderPipeline,
    vertex_count: u32,
    index_count: u32,
    instance_count: u32,
    index_format: BangerRenderIndexFormat,
    mesh_source: &'static str,
    mesh_bounds: BangerMeshBounds,
    _selected_tile_id: Option<String>,
    _indirect_args_hash: String,
    _meshlet_cluster_hash: String,
    _meshlet_cluster_count: u32,
    _meshlet_cluster_cull_param_hash: String,
    _meshlet_cluster_cull_feedback_hash: String,
    _material_bin_hash: String,
    _residency_feedback_hash: String,
    _shared_residency_page_table_hash: String,
    _shared_residency_compacted_feedback_hash: String,
    _shared_residency_eviction_plan_hash: String,
    _lumen_surface_card_hash: String,
    _lumen_surface_cache_feedback_hash: String,
    _lumen_screen_probe_hash: String,
    _lumen_radiance_cache_hash: String,
    _virtual_shadow_map_page_table_hash: String,
    _virtual_shadow_map_page_request_hash: String,
    _virtual_shadow_map_projection_hash: String,
    _virtual_shadow_map_physical_pool_hash: String,
    _virtual_shadow_map_cache_invalidation_hash: String,
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
    hzb: BangerNativeHzbResources,
    gbuffer: BangerNativeGBufferResources,
    water: BangerNativeSingleLayerWaterResources,
    width: u32,
    height: u32,
    target_hash: String,
    depth_target_hash: String,
}

#[cfg(target_os = "windows")]
struct BangerNativeGBufferResources {
    _albedo_texture: wgpu::Texture,
    albedo_view: wgpu::TextureView,
    _normal_texture: wgpu::Texture,
    normal_view: wgpu::TextureView,
    _material_texture: wgpu::Texture,
    material_view: wgpu::TextureView,
    _emissive_texture: wgpu::Texture,
    emissive_view: wgpu::TextureView,
    _resource_hash: String,
}

#[cfg(target_os = "windows")]
struct BangerNativeTextureResource {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    _width: u32,
    _height: u32,
    _byte_count: usize,
    _resource_hash: String,
}

#[cfg(target_os = "windows")]
struct BangerMapsCpuPreviewGate {
    nonblack_pixel_count: u32,
    non_fallback_blue_pixel_count: u32,
    frame_hash: String,
    width: u32,
    height: u32,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
struct BangerMeshBounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[cfg(target_os = "windows")]
impl BangerMeshBounds {
    fn empty() -> Self {
        Self {
            min: [f32::INFINITY, f32::INFINITY, f32::INFINITY],
            max: [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY],
        }
    }

    fn include(&mut self, point: [f32; 3]) {
        for axis in 0..3 {
            self.min[axis] = self.min[axis].min(point[axis]);
            self.max[axis] = self.max[axis].max(point[axis]);
        }
    }

    fn valid(self) -> bool {
        self.min.iter().chain(self.max.iter()).all(|value| value.is_finite())
            && self.max[0] >= self.min[0]
            && self.max[1] >= self.min[1]
            && self.max[2] >= self.min[2]
    }

    fn center(self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    fn radius(self) -> f32 {
        let dx = self.max[0] - self.min[0];
        let dy = self.max[1] - self.min[1];
        let dz = self.max[2] - self.min[2];
        ((dx * dx + dy * dy + dz * dz).sqrt() * 0.5).max(1.0)
    }
}

#[cfg(target_os = "windows")]
struct BangerNativeSingleLayerWaterResources {
    _scene_without_water_texture: wgpu::Texture,
    _scene_without_water_view: wgpu::TextureView,
    _depth_without_water_texture: wgpu::Texture,
    _depth_without_water_view: wgpu::TextureView,
    _refraction_mask_texture: wgpu::Texture,
    refraction_mask_view: wgpu::TextureView,
    _composite_texture: wgpu::Texture,
    composite_view: wgpu::TextureView,
    _spectral_state_texture: wgpu::Texture,
    spectral_state_view: wgpu::TextureView,
    _spectral_displacement_texture: wgpu::Texture,
    spectral_displacement_view: wgpu::TextureView,
    _spectral_slope_texture: wgpu::Texture,
    spectral_slope_view: wgpu::TextureView,
    params_buffer: wgpu::Buffer,
    spectral_params_buffer: wgpu::Buffer,
    tile_mask_buffer: wgpu::Buffer,
    _resource_hash: String,
}

#[cfg(target_os = "windows")]
struct BangerNativeHzbResources {
    _texture: wgpu::Texture,
    _views: Vec<wgpu::TextureView>,
    _consumer_view: wgpu::TextureView,
    _consumer_uniform_buffer: wgpu::Buffer,
    _consumer_bind_group_layout: wgpu::BindGroupLayout,
    _consumer_bind_group: wgpu::BindGroup,
    seed_pipeline: wgpu::ComputePipeline,
    reduce_pipeline: wgpu::ComputePipeline,
    seed_bind_group: wgpu::BindGroup,
    reduce_bind_groups: Vec<wgpu::BindGroup>,
    mip_count: u32,
    width: u32,
    height: u32,
    _hzb_hash: String,
    _consumer_hash: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
enum BangerNativeHostCommand {
    Resize { x: i32, y: i32, width: u32, height: u32 },
    Shutdown,
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
    let url = match banger_maps_resolved_root_url() {
        Ok(url) => url,
        Err(message) => {
            let cache_dir = banger_maps_cache_dir();
            return failed_banger_maps_root_ingest(
                "",
                &cache_dir,
                &cache_dir.join("missing-root-url.root.json"),
                "missing_cesium_ion_endpoint",
                message,
                false,
            );
        }
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

fn banger_maps_resolved_root_url() -> Result<String, String> {
    let direct_root_url = env::var("FORGE_BANGER_MAPS_ROOT_URL").ok();
    let cesium_token_broker_url = banger_maps_cesium_token_broker_url();
    let cesium_access_token = env::var("CESIUM_ACCESS_TOKEN")
        .or_else(|_| env::var("VITE_CESIUM_ACCESS_TOKEN"))
        .ok();
    let cesium_asset_id = banger_maps_cesium_ion_asset_id();
    banger_maps_root_url_from_values(
        direct_root_url.as_deref(),
        cesium_token_broker_url.as_deref(),
        cesium_access_token.as_deref(),
        &cesium_asset_id,
    )
}

fn banger_maps_cesium_token_broker_url() -> Option<String> {
    [
        env::var("FORGE_BANGER_CESIUM_ION_TOKEN_URL").ok(),
        env::var("FORGE_CESIUM_ION_TOKEN_URL").ok(),
        env::var("FORGE_REAL_ESTATE_BACKEND_URL")
            .ok()
            .map(|base| {
                format!(
                    "{}/api/banger/cesium-ion-token",
                    base.trim().trim_end_matches('/')
                )
            }),
        env::var("FORGE_RENDER_BACKEND_URL")
            .ok()
            .map(|base| {
                format!(
                    "{}/api/banger/cesium-ion-token",
                    base.trim().trim_end_matches('/')
                )
            }),
        Some("https://forge-6cai.onrender.com/api/banger/cesium-ion-token".to_string()),
    ]
    .into_iter()
    .flatten()
    .map(|value| value.trim().to_string())
    .find(|value| !value.is_empty())
}

fn banger_maps_cesium_ion_asset_id() -> String {
    env::var("FORGE_BANGER_CESIUM_ION_ASSET_ID")
        .or_else(|_| env::var("CESIUM_ION_GOOGLE_PHOTOREALISTIC_3D_TILES_ASSET_ID"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "2275207".to_string())
}

fn banger_maps_root_url_from_values(
    direct_root_url: Option<&str>,
    cesium_token_broker_url: Option<&str>,
    cesium_access_token: Option<&str>,
    cesium_asset_id: &str,
) -> Result<String, String> {
    if let Some(url) = direct_root_url.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(url.to_string());
    }
    if let Some(broker_url) = cesium_token_broker_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return banger_maps_cesium_root_url_from_broker(broker_url, cesium_asset_id);
    }
    if let Some(token) = cesium_access_token.map(str::trim).filter(|value| !value.is_empty()) {
        return banger_maps_cesium_root_url_from_token(token, cesium_asset_id);
    }
    Err("Set FORGE_BANGER_CESIUM_ION_TOKEN_URL or CESIUM_ACCESS_TOKEN. Banger Maps uses Cesium ion for Photorealistic 3D Tiles, not Google Map Tiles API keys.".to_string())
}

fn banger_maps_cesium_root_url_from_broker(
    broker_url: &str,
    cesium_asset_id: &str,
) -> Result<String, String> {
    let bytes = fetch_banger_maps_root(broker_url)?;
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|error| format!("cesium broker json: {error}"))?;
    banger_maps_cesium_root_url_from_endpoint_value(&value, cesium_asset_id)
        .or_else(|| {
            banger_maps_json_string(&value, &["cesiumIonAccessToken", "accessToken", "token"])
                .and_then(|token| banger_maps_cesium_root_url_from_token(&token, cesium_asset_id).ok())
        })
        .ok_or_else(|| {
            "Cesium broker did not return an endpoint URL or Cesium ion access token.".to_string()
        })
}

fn banger_maps_cesium_root_url_from_token(
    token: &str,
    cesium_asset_id: &str,
) -> Result<String, String> {
    let endpoint_url = banger_cesium_ion_asset_endpoint_url(cesium_asset_id, token);
    let bytes = fetch_banger_maps_root(&endpoint_url)?;
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|error| format!("cesium endpoint json: {error}"))?;
    banger_maps_cesium_root_url_from_endpoint_value(&value, cesium_asset_id)
        .ok_or_else(|| "Cesium ion endpoint did not return a 3D Tiles URL.".to_string())
}

fn banger_cesium_ion_asset_endpoint_url(asset_id: &str, token: &str) -> String {
    format!(
        "https://api.cesium.com/v1/assets/{}/endpoint?access_token={}",
        asset_id.trim(),
        token.trim()
    )
}

fn banger_maps_cesium_root_url_from_endpoint_value(
    value: &Value,
    cesium_asset_id: &str,
) -> Option<String> {
    let endpoint = value.get("endpoint").unwrap_or(value);
    let url_keys = ["url", "rootTilesetUrl", "tilesetUrl", "endpointUrl"];
    let url = banger_maps_json_string(endpoint, &url_keys)
        .or_else(|| value.get("options").and_then(|options| banger_maps_json_string(options, &url_keys)))
        .or_else(|| endpoint.get("options").and_then(|options| banger_maps_json_string(options, &url_keys)))?;
    if url.starts_with("ion://") {
        let token = banger_maps_json_string(value, &["cesiumIonAccessToken", "accessToken", "token"])?;
        return banger_maps_cesium_root_url_from_token(&token, cesium_asset_id).ok();
    }
    let token_keys = ["accessToken", "cesiumIonAccessToken", "token"];
    let token = banger_maps_json_string(endpoint, &token_keys)
        .or_else(|| value.get("options").and_then(|options| banger_maps_json_string(options, &token_keys)))
        .or_else(|| endpoint.get("options").and_then(|options| banger_maps_json_string(options, &token_keys)))
        .or_else(|| banger_maps_json_string(value, &["accessToken", "cesiumIonAccessToken", "token"]));
    Some(match token {
        Some(token) => banger_append_query_param(&url, "access_token", &token),
        None => url,
    })
}

fn banger_maps_json_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn banger_append_query_param(url: &str, key: &str, value: &str) -> String {
    let mut parsed = match reqwest::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return url.to_string(),
    };
    if parsed.query_pairs().any(|(existing, _)| existing.eq_ignore_ascii_case(key)) {
        return parsed.to_string();
    }
    parsed.query_pairs_mut().append_pair(key, value);
    parsed.to_string()
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
        collect_banger_maps_traversal_tiles(
            root,
            None,
            banger_identity_mat4_f64(),
            0,
            "0".to_string(),
            &mut tiles,
        );
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
                    "{}:{}:{:?}:{}:{}:{}:{};",
                    tile.tile_id,
                    tile.depth,
                    tile.geometric_error,
                    tile.bounding_volume_hash,
                    tile.transform_hash,
                    banger_transform_hash(&tile.global_transform),
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
    parent_global_transform: [f64; 16],
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
    let local_transform = banger_tile_transform_matrix(tile);
    let global_transform = banger_mat4_mul_f64(parent_global_transform, local_transform);
    let transform_hash = banger_transform_hash(&global_transform);
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
        global_transform,
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
                global_transform,
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
                    tile_global_transform: tile.global_transform,
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
                        tile_global_transform: tile.global_transform,
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
                            tile_global_transform: tile.global_transform,
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
                            tile_global_transform: tile.global_transform,
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
                    "{}:{}:{}:{}:{}:{};",
                    record.tile_id,
                    record.source_uri,
                    record.cache_path,
                    record.byte_count,
                    record.content_hash,
                    banger_transform_hash(&record.tile_global_transform)
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
    if let Some(draco) = primitive
        .get("extensions")
        .and_then(|extensions| extensions.get("KHR_draco_mesh_compression"))
    {
        return stage_banger_draco_gltf_primitive(gltf, bin_chunk, mesh_index, primitive_index, primitive, draco);
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
    banger_maps_float_vec3_accessor_values(&position, "POSITION")?;
    let normal_accessor = attributes.get("NORMAL").and_then(Value::as_u64).map(|value| value as usize);
    let texcoord0_accessor = attributes.get("TEXCOORD_0").and_then(Value::as_u64).map(|value| value as usize);
    let normals = match normal_accessor {
        Some(accessor_index) => Some(banger_maps_float_vec3_accessor_values(
            &banger_gltf_accessor_stage(gltf, bin_chunk, accessor_index)?,
            "NORMAL",
        )?),
        None => None,
    };
    let texcoords = match texcoord0_accessor {
        Some(accessor_index) => Some(banger_maps_float_vec2_accessor_values(
            &banger_gltf_accessor_stage(gltf, bin_chunk, accessor_index)?,
            "TEXCOORD_0",
        )?),
        None => None,
    };
    if normals.as_ref().is_some_and(|values| values.len() != position.count) {
        return Err(format!("mesh {mesh_index} primitive {primitive_index} NORMAL count must match POSITION count"));
    }
    if texcoords.as_ref().is_some_and(|values| values.len() != position.count) {
        return Err(format!("mesh {mesh_index} primitive {primitive_index} TEXCOORD_0 count must match POSITION count"));
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
    let material_color = primitive
        .get("material")
        .and_then(Value::as_u64)
        .and_then(|index| banger_gltf_material_base_color(gltf, index as usize))
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let vertex_bytes = banger_maps_engine_vertex_buffer_bytes(
        &position,
        normals.as_deref(),
        texcoords.as_deref(),
        material_color,
    )?;
    Ok(BangerMapsGpuPrimitiveStage {
        mesh_index,
        primitive_index,
        material_index: primitive.get("material").and_then(Value::as_u64).map(|value| value as usize),
        mode: primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) as u32,
        position_accessor,
        normal_accessor,
        texcoord0_accessor,
        index_accessor,
        vertex_count: position.count,
        index_count,
        source_position_buffer_byte_count: position.bytes.len(),
        vertex_buffer_byte_count: vertex_bytes.len(),
        index_buffer_byte_count: index_bytes.len(),
        vertex_buffer_hash: sha256_hex(&vertex_bytes),
        index_buffer_hash: sha256_hex(&index_bytes),
        index_format,
        vertex_stride_bytes: 48,
        vertex_layout: "float32x3_position_float32x3_normal_float32x2_uv_float32x4_base_color",
        wgpu_vertex_usage: "VERTEX|COPY_DST",
        wgpu_index_usage: "INDEX|COPY_DST",
    })
}

fn stage_banger_draco_gltf_primitive(
    gltf: &Value,
    bin_chunk: &[u8],
    mesh_index: usize,
    primitive_index: usize,
    primitive: &Value,
    draco: &Value,
) -> Result<BangerMapsGpuPrimitiveStage, String> {
    let decoded = banger_decode_draco_primitive(gltf, bin_chunk, primitive, draco)
        .map_err(|error| format!("mesh {mesh_index} primitive {primitive_index} {error}"))?;
    let position = decoded
        .attributes
        .get("POSITION")
        .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing decoded Draco POSITION"))?;
    banger_maps_float_vec3_accessor_values(position, "POSITION")?;
    let normals = match decoded.attributes.get("NORMAL") {
        Some(stage) => Some(banger_maps_float_vec3_accessor_values(stage, "NORMAL")?),
        None => None,
    };
    let texcoords = match decoded.attributes.get("TEXCOORD_0") {
        Some(stage) => Some(banger_maps_float_vec2_accessor_values(stage, "TEXCOORD_0")?),
        None => None,
    };
    if normals.as_ref().is_some_and(|values| values.len() != position.count) {
        return Err(format!("mesh {mesh_index} primitive {primitive_index} decoded NORMAL count must match POSITION count"));
    }
    if texcoords.as_ref().is_some_and(|values| values.len() != position.count) {
        return Err(format!("mesh {mesh_index} primitive {primitive_index} decoded TEXCOORD_0 count must match POSITION count"));
    }
    let material_color = primitive
        .get("material")
        .and_then(Value::as_u64)
        .and_then(|index| banger_gltf_material_base_color(gltf, index as usize))
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let vertex_bytes = banger_maps_engine_vertex_buffer_bytes(
        position,
        normals.as_deref(),
        texcoords.as_deref(),
        material_color,
    )?;
    let primitive_attributes = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing attributes"))?;
    let position_accessor = primitive_attributes
        .get("POSITION")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("mesh {mesh_index} primitive {primitive_index} missing POSITION accessor"))? as usize;
    Ok(BangerMapsGpuPrimitiveStage {
        mesh_index,
        primitive_index,
        material_index: primitive.get("material").and_then(Value::as_u64).map(|value| value as usize),
        mode: primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) as u32,
        position_accessor,
        normal_accessor: primitive_attributes.get("NORMAL").and_then(Value::as_u64).map(|value| value as usize),
        texcoord0_accessor: primitive_attributes.get("TEXCOORD_0").and_then(Value::as_u64).map(|value| value as usize),
        index_accessor: primitive.get("indices").and_then(Value::as_u64).map(|value| value as usize),
        vertex_count: position.count,
        index_count: decoded.index_count,
        source_position_buffer_byte_count: position.bytes.len(),
        vertex_buffer_byte_count: vertex_bytes.len(),
        index_buffer_byte_count: decoded.index_bytes.len(),
        vertex_buffer_hash: sha256_hex(&vertex_bytes),
        index_buffer_hash: sha256_hex(&decoded.index_bytes),
        index_format: decoded.index_format,
        vertex_stride_bytes: 48,
        vertex_layout: "float32x3_position_float32x3_normal_float32x2_uv_float32x4_base_color",
        wgpu_vertex_usage: "VERTEX|COPY_DST",
        wgpu_index_usage: "INDEX|COPY_DST",
    })
}

struct BangerDecodedDracoPrimitive {
    attributes: HashMap<String, BangerGltfAccessorStage>,
    index_bytes: Vec<u8>,
    index_count: usize,
    index_format: &'static str,
}

fn banger_decode_draco_primitive(
    gltf: &Value,
    bin_chunk: &[u8],
    primitive: &Value,
    draco: &Value,
) -> Result<BangerDecodedDracoPrimitive, String> {
    #[cfg(not(feature = "banger-draco"))]
    {
        let _ = (gltf, bin_chunk, primitive, draco);
        return Err("KHR_draco_mesh_compression decode unavailable: rebuild ingen-native-services with feature banger-draco".to_string());
    }
    #[cfg(feature = "banger-draco")]
    {
    let buffer_view_index = draco
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| "KHR_draco_mesh_compression missing bufferView".to_string())? as usize;
    let compressed = banger_gltf_buffer_view_bytes(gltf, bin_chunk, buffer_view_index)?;
    let decoded = std::panic::catch_unwind(|| draco_decoder::decode_mesh_with_config_sync(&compressed.bytes))
        .map_err(|_| "KHR_draco_mesh_compression decode failed".to_string())?
        .ok_or_else(|| "KHR_draco_mesh_compression decode failed".to_string())?;
    let index_count = decoded.config.index_count() as usize;
    let index_length = decoded.config.index_length() as usize;
    if index_length > decoded.data.len() {
        return Err("KHR_draco_mesh_compression decoded index range exceeds output buffer".to_string());
    }
    let index_format = if index_length == index_count * 2 {
        "uint16"
    } else if index_length == index_count * 4 {
        "uint32"
    } else {
        return Err(format!(
            "KHR_draco_mesh_compression decoded index length {index_length} does not match count {index_count}"
        ));
    };
    let primitive_attributes = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| "KHR_draco_mesh_compression primitive missing attributes".to_string())?;
    let draco_attributes = draco
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| "KHR_draco_mesh_compression missing attributes map".to_string())?;
    let mut attributes = HashMap::new();
    for (semantic, draco_attribute_id) in draco_attributes {
        let draco_attribute_id = draco_attribute_id
            .as_u64()
            .ok_or_else(|| format!("KHR_draco_mesh_compression attribute {semantic} id is not an integer"))?
            as usize;
        let decoded_attribute = decoded
            .config
            .get_attribute(draco_attribute_id)
            .ok_or_else(|| format!("KHR_draco_mesh_compression attribute {semantic} id {draco_attribute_id} missing after decode"))?;
        let offset = decoded_attribute.offset() as usize;
        let length = decoded_attribute.lenght() as usize;
        let end = offset + length;
        if end > decoded.data.len() {
            return Err(format!("KHR_draco_mesh_compression attribute {semantic} range exceeds decoded buffer"));
        }
        let component_type = banger_draco_attribute_component_type(decoded_attribute.data_type())?;
        let accessor_type = banger_draco_attribute_accessor_type(decoded_attribute.dim())?;
        let normalized = primitive_attributes
            .get(semantic)
            .and_then(Value::as_u64)
            .and_then(|accessor_index| banger_gltf_accessor_normalized(gltf, accessor_index as usize).ok())
            .unwrap_or(false);
        attributes.insert(
            semantic.clone(),
            BangerGltfAccessorStage {
                bytes: decoded.data[offset..end].to_vec(),
                count: decoded.config.vertex_count() as usize,
                component_type,
                normalized,
                accessor_type,
            },
        );
    }
    if !attributes.contains_key("POSITION") {
        return Err("KHR_draco_mesh_compression decoded primitive missing POSITION".to_string());
    }
    Ok(BangerDecodedDracoPrimitive {
        attributes,
        index_bytes: decoded.data[..index_length].to_vec(),
        index_count,
        index_format,
    })
    }
}

#[cfg(feature = "banger-draco")]
fn banger_draco_attribute_component_type(data_type: draco_decoder::AttributeDataType) -> Result<u32, String> {
    match data_type {
        draco_decoder::AttributeDataType::Int8 => Ok(5120),
        draco_decoder::AttributeDataType::UInt8 => Ok(5121),
        draco_decoder::AttributeDataType::Int16 => Ok(5122),
        draco_decoder::AttributeDataType::UInt16 => Ok(5123),
        draco_decoder::AttributeDataType::UInt32 => Ok(5125),
        draco_decoder::AttributeDataType::Float32 => Ok(5126),
        draco_decoder::AttributeDataType::Int32 => Err("KHR_draco_mesh_compression decoded Int32 vertex attributes are not supported".to_string()),
    }
}

#[cfg(feature = "banger-draco")]
fn banger_draco_attribute_accessor_type(dim: u32) -> Result<String, String> {
    match dim {
        1 => Ok("SCALAR".to_string()),
        2 => Ok("VEC2".to_string()),
        3 => Ok("VEC3".to_string()),
        4 => Ok("VEC4".to_string()),
        other => Err(format!("KHR_draco_mesh_compression decoded attribute dimension {other} is unsupported")),
    }
}

fn banger_maps_float_vec3_accessor_values(stage: &BangerGltfAccessorStage, semantic: &str) -> Result<Vec<[f32; 3]>, String> {
    if stage.accessor_type != "VEC3" {
        return Err(format!("{semantic} must be VEC3, got {} {}", stage.component_type, stage.accessor_type));
    }
    let values = banger_maps_accessor_f32_values(stage, semantic, 3)?;
    Ok(values
        .chunks_exact(3)
        .map(|chunk| {
            let vec3 = [chunk[0], chunk[1], chunk[2]];
            if semantic == "NORMAL" {
                banger_maps_normalize_vec3(vec3)
            } else {
                vec3
            }
        })
        .collect())
}

fn banger_maps_float_vec2_accessor_values(stage: &BangerGltfAccessorStage, semantic: &str) -> Result<Vec<[f32; 2]>, String> {
    if stage.accessor_type != "VEC2" {
        return Err(format!("{semantic} must be VEC2, got {} {}", stage.component_type, stage.accessor_type));
    }
    let values = banger_maps_accessor_f32_values(stage, semantic, 2)?;
    Ok(values.chunks_exact(2).map(|chunk| [chunk[0], chunk[1]]).collect())
}

fn banger_maps_float_vec4_accessor_values(stage: &BangerGltfAccessorStage, semantic: &str) -> Result<Vec<[f32; 4]>, String> {
    if stage.accessor_type != "VEC4" {
        return Err(format!("{semantic} must be VEC4, got {} {}", stage.component_type, stage.accessor_type));
    }
    let values = banger_maps_accessor_f32_values(stage, semantic, 4)?;
    Ok(values.chunks_exact(4).map(|chunk| [chunk[0], chunk[1], chunk[2], chunk[3]]).collect())
}

fn banger_maps_accessor_f32_values(
    stage: &BangerGltfAccessorStage,
    semantic: &str,
    component_count: usize,
) -> Result<Vec<f32>, String> {
    let component_size = banger_gltf_component_size(stage.component_type)?;
    let element_size = component_size * component_count;
    let expected_len = stage.count * element_size;
    if stage.bytes.len() != expected_len {
        return Err(format!(
            "{semantic} accessor byte length {} does not match count {} * element size {}",
            stage.bytes.len(),
            stage.count,
            element_size
        ));
    }
    let mut values = Vec::with_capacity(stage.count * component_count);
    for element in stage.bytes.chunks_exact(element_size) {
        for component in 0..component_count {
            let offset = component * component_size;
            values.push(banger_maps_component_to_f32(
                &element[offset..offset + component_size],
                stage.component_type,
                stage.normalized,
            )?);
        }
    }
    Ok(values)
}

fn banger_maps_component_to_f32(bytes: &[u8], component_type: u32, normalized: bool) -> Result<f32, String> {
    match component_type {
        5120 => {
            let value = i8::from_le_bytes(bytes.try_into().expect("i8 accessor component")) as f32;
            Ok(if normalized { (value / 127.0).max(-1.0) } else { value })
        }
        5121 => {
            let value = u8::from_le_bytes(bytes.try_into().expect("u8 accessor component")) as f32;
            Ok(if normalized { value / 255.0 } else { value })
        }
        5122 => {
            let value = i16::from_le_bytes(bytes.try_into().expect("i16 accessor component")) as f32;
            Ok(if normalized { (value / 32767.0).max(-1.0) } else { value })
        }
        5123 => {
            let value = u16::from_le_bytes(bytes.try_into().expect("u16 accessor component")) as f32;
            Ok(if normalized { value / 65535.0 } else { value })
        }
        5126 => Ok(f32::from_le_bytes(bytes.try_into().expect("f32 accessor component"))),
        other => Err(format!("unsupported {other} component type for float staging")),
    }
}

fn banger_maps_normalize_vec3(value: [f32; 3]) -> [f32; 3] {
    let len = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if len > f32::EPSILON {
        [value[0] / len, value[1] / len, value[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn banger_maps_engine_vertex_buffer_bytes(
    position: &BangerGltfAccessorStage,
    normals: Option<&[[f32; 3]]>,
    texcoords: Option<&[[f32; 2]]>,
    material_color: [f32; 4],
) -> Result<Vec<u8>, String> {
    let positions = banger_maps_float_vec3_accessor_values(position, "POSITION")?;
    let mut bytes = Vec::with_capacity(positions.len() * 48);
    for (index, position) in positions.iter().enumerate() {
        let normal = normals.and_then(|values| values.get(index)).copied().unwrap_or([0.0, 1.0, 0.0]);
        let uv = texcoords.and_then(|values| values.get(index)).copied().unwrap_or([0.0, 0.0]);
        for value in [
            position[0],
            position[1],
            position[2],
            normal[0],
            normal[1],
            normal[2],
            uv[0],
            uv[1],
            material_color[0],
            material_color[1],
            material_color[2],
            material_color[3],
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
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
        upload_policy: "banger_interleaved_pbr_vertex_u16_u32_indices_material_texture_staging_v1",
    }
}

fn banger_maps_gltf_format_support(gltf: &Value) -> BangerMapsGltfFormatSupport {
    let extensions_used = banger_gltf_string_array(gltf, "extensionsUsed");
    let extensions_required = banger_gltf_string_array(gltf, "extensionsRequired");
    let supported_extensions = [
        "KHR_draco_mesh_compression",
        "KHR_materials_unlit",
        "KHR_mesh_quantization",
        "EXT_meshopt_compression",
    ];
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
    let compression_blocker = banger_maps_draco_format_blocker(gltf).or_else(|| banger_maps_meshopt_format_blocker(gltf));
    BangerMapsGltfFormatSupport {
        extensions_used,
        extensions_required,
        unsupported_used_extensions,
        unsupported_required_extensions,
        compression_blocker,
        upload_policy: "banger_interleaved_pbr_vertex_u16_u32_indices_material_texture_staging_v1",
    }
}

fn banger_maps_draco_format_blocker(gltf: &Value) -> Option<String> {
    let meshes = gltf.get("meshes").and_then(Value::as_array)?;
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh.get("primitives").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let Some(draco) = primitive
                .get("extensions")
                .and_then(|extensions| extensions.get("KHR_draco_mesh_compression"))
            else {
                continue;
            };
            let mode = primitive.get("mode").and_then(Value::as_u64).unwrap_or(4);
            if !matches!(mode, 4 | 5) {
                return Some(format!("KHR_draco_mesh_compression mesh {mesh_index} primitive {primitive_index} unsupported mode {mode}"));
            }
            if draco.get("bufferView").and_then(Value::as_u64).is_none() {
                return Some(format!("KHR_draco_mesh_compression mesh {mesh_index} primitive {primitive_index} missing bufferView"));
            }
            let Some(attributes) = draco.get("attributes").and_then(Value::as_object) else {
                return Some(format!("KHR_draco_mesh_compression mesh {mesh_index} primitive {primitive_index} missing attributes"));
            };
            if attributes.get("POSITION").and_then(Value::as_u64).is_none() {
                return Some(format!("KHR_draco_mesh_compression mesh {mesh_index} primitive {primitive_index} missing POSITION attribute id"));
            }
        }
    }
    None
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
                    let metallic_roughness_texture = pbr
                        .and_then(|value| value.get("metallicRoughnessTexture"))
                        .and_then(|value| value.get("index"))
                        .and_then(Value::as_u64)
                        .map(|value| value as usize);
                    let normal_texture = material
                        .get("normalTexture")
                        .and_then(|value| value.get("index"))
                        .and_then(Value::as_u64)
                        .map(|value| value as usize);
                    let normal_scale = material
                        .get("normalTexture")
                        .and_then(|value| value.get("scale"))
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0) as f32;
                    let occlusion_texture = material
                        .get("occlusionTexture")
                        .and_then(|value| value.get("index"))
                        .and_then(Value::as_u64)
                        .map(|value| value as usize);
                    let occlusion_strength = material
                        .get("occlusionTexture")
                        .and_then(|value| value.get("strength"))
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0) as f32;
                    let emissive_texture = material
                        .get("emissiveTexture")
                        .and_then(|value| value.get("index"))
                        .and_then(Value::as_u64)
                        .map(|value| value as usize);
                    let emissive_factor = material
                        .get("emissiveFactor")
                        .and_then(Value::as_array)
                        .map(|items| {
                            let mut factor = [0.0f32, 0.0, 0.0];
                            for (index, item) in items.iter().take(3).enumerate() {
                                factor[index] = item.as_f64().unwrap_or(0.0) as f32;
                            }
                            factor
                        })
                        .unwrap_or([0.0, 0.0, 0.0]);
                    let material_hash = sha256_hex(
                        format!(
                            "{material_index}:{base_color_factor:?}:{metallic_factor}:{roughness_factor}:{base_color_texture:?}:{metallic_roughness_texture:?}:{normal_texture:?}:{normal_scale}:{occlusion_texture:?}:{occlusion_strength}:{emissive_texture:?}:{emissive_factor:?}"
                        )
                        .as_bytes(),
                    );
                    BangerMapsMaterialStage {
                        material_index,
                        base_color_factor,
                        metallic_factor,
                        roughness_factor,
                        base_color_texture,
                        normal_texture,
                        normal_scale,
                        metallic_roughness_texture,
                        occlusion_texture,
                        occlusion_strength,
                        emissive_texture,
                        emissive_factor,
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
                byte_count: image_bytes.bytes.len(),
                content_hash: sha256_hex(&image_bytes.bytes),
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

fn banger_maps_meshopt_format_blocker(gltf: &Value) -> Option<String> {
    let buffer_views = gltf.get("bufferViews").and_then(Value::as_array)?;
    for (buffer_view_index, buffer_view) in buffer_views.iter().enumerate() {
        let Some(meshopt) = buffer_view
            .get("extensions")
            .and_then(|extensions| extensions.get("EXT_meshopt_compression"))
        else {
            continue;
        };
        let mode = meshopt.get("mode").and_then(Value::as_str).unwrap_or("");
        let filter = meshopt.get("filter").and_then(Value::as_str).unwrap_or("NONE");
        let byte_stride = meshopt.get("byteStride").and_then(Value::as_u64).unwrap_or(0);
        let count = meshopt.get("count").and_then(Value::as_u64).unwrap_or(0);
        if !matches!(mode, "ATTRIBUTES" | "TRIANGLES" | "INDICES") {
            return Some(format!("EXT_meshopt_compression bufferView {buffer_view_index} unsupported mode {mode}"));
        }
        if !matches!(filter, "NONE" | "OCTAHEDRAL" | "QUATERNION" | "EXPONENTIAL") {
            return Some(format!("EXT_meshopt_compression bufferView {buffer_view_index} unsupported filter {filter}"));
        }
        if count == 0 || byte_stride == 0 {
            return Some(format!("EXT_meshopt_compression bufferView {buffer_view_index} missing count or byteStride"));
        }
        if matches!(mode, "TRIANGLES" | "INDICES") && !matches!(byte_stride, 2 | 4) {
            return Some(format!("EXT_meshopt_compression bufferView {buffer_view_index} index byteStride must be 2 or 4"));
        }
        if mode == "ATTRIBUTES" && (byte_stride % 4 != 0 || byte_stride > 256) {
            return Some(format!(
                "EXT_meshopt_compression bufferView {buffer_view_index} ATTRIBUTES byteStride must be divisible by 4 and <= 256"
            ));
        }
        if mode == "TRIANGLES" && count % 3 != 0 {
            return Some(format!("EXT_meshopt_compression bufferView {buffer_view_index} TRIANGLES count must be divisible by 3"));
        }
        if matches!(mode, "TRIANGLES" | "INDICES") && filter != "NONE" {
            return Some(format!("EXT_meshopt_compression bufferView {buffer_view_index} index modes require filter NONE"));
        }
    }
    None
}

struct BangerGltfAccessorStage {
    bytes: Vec<u8>,
    count: usize,
    component_type: u32,
    normalized: bool,
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
    let normalized = accessor.get("normalized").and_then(Value::as_bool).unwrap_or(false);
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
    let buffer_view = banger_gltf_buffer_view_bytes(gltf, bin_chunk, buffer_view_index)?;
    let accessor_offset = accessor.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let start = accessor_offset;
    let stride = buffer_view.byte_stride.unwrap_or(element_size);
    if stride < element_size {
        return Err(format!("accessor {accessor_index} byteStride {stride} is smaller than element size {element_size}"));
    }
    let final_byte = if count == 0 {
        start
    } else {
        start + stride * (count - 1) + element_size
    };
    if final_byte > buffer_view.bytes.len() {
        return Err(format!("accessor {accessor_index} exceeds bufferView {buffer_view_index} bytes"));
    }
    let mut bytes = Vec::with_capacity(count * element_size);
    for item in 0..count {
        let offset = start + item * stride;
        bytes.extend_from_slice(&buffer_view.bytes[offset..offset + element_size]);
    }
    Ok(BangerGltfAccessorStage {
        bytes,
        count,
        component_type,
        normalized,
        accessor_type,
    })
}

#[cfg(feature = "banger-draco")]
fn banger_gltf_accessor_normalized(gltf: &Value, accessor_index: usize) -> Result<bool, String> {
    let accessors = gltf
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| "gltf accessors array missing".to_string())?;
    let accessor = accessors
        .get(accessor_index)
        .ok_or_else(|| format!("accessor {accessor_index} missing"))?;
    Ok(accessor.get("normalized").and_then(Value::as_bool).unwrap_or(false))
}

struct BangerGltfBufferViewBytes {
    bytes: Vec<u8>,
    byte_stride: Option<usize>,
}

fn banger_gltf_buffer_view_bytes(gltf: &Value, bin_chunk: &[u8], buffer_view_index: usize) -> Result<BangerGltfBufferViewBytes, String> {
    let buffer_views = gltf
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| "gltf bufferViews array missing".to_string())?;
    let buffer_view = buffer_views
        .get(buffer_view_index)
        .ok_or_else(|| format!("bufferView {buffer_view_index} missing"))?;
    let byte_stride = buffer_view.get("byteStride").and_then(Value::as_u64).map(|value| value as usize);
    if let Some(meshopt) = buffer_view
        .get("extensions")
        .and_then(|extensions| extensions.get("EXT_meshopt_compression"))
    {
        return banger_gltf_meshopt_buffer_view_bytes(gltf, bin_chunk, buffer_view_index, buffer_view, meshopt, byte_stride);
    }
    let (view_offset, view_length, _) = banger_gltf_buffer_view_layout(gltf, buffer_view_index)?;
    let end = view_offset + view_length;
    if end > bin_chunk.len() {
        return Err(format!("bufferView {buffer_view_index} exceeds GLB BIN chunk"));
    }
    Ok(BangerGltfBufferViewBytes {
        bytes: bin_chunk[view_offset..end].to_vec(),
        byte_stride,
    })
}

fn banger_gltf_meshopt_buffer_view_bytes(
    gltf: &Value,
    bin_chunk: &[u8],
    buffer_view_index: usize,
    buffer_view: &Value,
    meshopt: &Value,
    parent_byte_stride: Option<usize>,
) -> Result<BangerGltfBufferViewBytes, String> {
    let byte_length = buffer_view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("bufferView {buffer_view_index} missing byteLength"))? as usize;
    let count = meshopt
        .get("count")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("bufferView {buffer_view_index} EXT_meshopt_compression missing count"))? as usize;
    let byte_stride = meshopt
        .get("byteStride")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("bufferView {buffer_view_index} EXT_meshopt_compression missing byteStride"))? as usize;
    if let Some(parent_stride) = parent_byte_stride {
        if parent_stride != byte_stride {
            return Err(format!(
                "bufferView {buffer_view_index} EXT_meshopt_compression byteStride {byte_stride} does not match parent byteStride {parent_stride}"
            ));
        }
    }
    if byte_length != count * byte_stride {
        return Err(format!(
            "bufferView {buffer_view_index} byteLength {byte_length} does not match EXT_meshopt_compression count {count} * byteStride {byte_stride}"
        ));
    }
    let compressed = banger_gltf_meshopt_compressed_bytes(gltf, bin_chunk, buffer_view_index, meshopt)?;
    let mode = meshopt
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("bufferView {buffer_view_index} EXT_meshopt_compression missing mode"))?;
    let filter = meshopt.get("filter").and_then(Value::as_str).unwrap_or("NONE");
    let mut bytes = vec![0u8; byte_length];
    let result = unsafe {
        match mode {
            "ATTRIBUTES" => meshopt::ffi::meshopt_decodeVertexBuffer(
                bytes.as_mut_ptr().cast(),
                count,
                byte_stride,
                compressed.as_ptr(),
                compressed.len(),
            ),
            "TRIANGLES" => meshopt::ffi::meshopt_decodeIndexBuffer(
                bytes.as_mut_ptr().cast(),
                count,
                byte_stride,
                compressed.as_ptr(),
                compressed.len(),
            ),
            "INDICES" => meshopt::ffi::meshopt_decodeIndexSequence(
                bytes.as_mut_ptr().cast(),
                count,
                byte_stride,
                compressed.as_ptr(),
                compressed.len(),
            ),
            other => return Err(format!("bufferView {buffer_view_index} unsupported EXT_meshopt_compression mode {other}")),
        }
    };
    if result != 0 {
        return Err(format!(
            "bufferView {buffer_view_index} EXT_meshopt_compression decode failed with code {result}"
        ));
    }
    unsafe {
        match filter {
            "NONE" => {}
            "OCTAHEDRAL" => meshopt::ffi::meshopt_decodeFilterOct(bytes.as_mut_ptr().cast(), count, byte_stride),
            "QUATERNION" => meshopt::ffi::meshopt_decodeFilterQuat(bytes.as_mut_ptr().cast(), count, byte_stride),
            "EXPONENTIAL" => meshopt::ffi::meshopt_decodeFilterExp(bytes.as_mut_ptr().cast(), count, byte_stride),
            other => return Err(format!("bufferView {buffer_view_index} unsupported EXT_meshopt_compression filter {other}")),
        }
    }
    Ok(BangerGltfBufferViewBytes {
        bytes,
        byte_stride: Some(byte_stride),
    })
}

fn banger_gltf_meshopt_compressed_bytes(
    gltf: &Value,
    bin_chunk: &[u8],
    buffer_view_index: usize,
    meshopt: &Value,
) -> Result<Vec<u8>, String> {
    let buffer_index = meshopt
        .get("buffer")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("bufferView {buffer_view_index} EXT_meshopt_compression missing buffer"))?;
    if buffer_index != 0 {
        return Err(format!(
            "bufferView {buffer_view_index} EXT_meshopt_compression external buffer {buffer_index} pending"
        ));
    }
    let byte_offset = meshopt.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let byte_length = meshopt
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("bufferView {buffer_view_index} EXT_meshopt_compression missing byteLength"))? as usize;
    let end = byte_offset + byte_length;
    if end > bin_chunk.len() {
        return Err(format!("bufferView {buffer_view_index} EXT_meshopt_compression source exceeds GLB BIN chunk"));
    }
    let buffers = gltf.get("buffers").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    if buffers
        .get(buffer_index as usize)
        .and_then(|buffer| buffer.get("uri"))
        .is_some()
    {
        return Err(format!(
            "bufferView {buffer_view_index} EXT_meshopt_compression external uri buffer {buffer_index} pending"
        ));
    }
    Ok(bin_chunk[byte_offset..end].to_vec())
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
            banger_maps_float_vec3_accessor_values(&position, "POSITION")
                .map_err(|error| format!("mesh {mesh_index} primitive {primitive_index} {error}"))?;
            let normal_accessor = attributes.get("NORMAL").and_then(Value::as_u64).map(|value| value as usize);
            let texcoord0_accessor = attributes.get("TEXCOORD_0").and_then(Value::as_u64).map(|value| value as usize);
            let normals = match normal_accessor {
                Some(accessor_index) => Some(banger_maps_float_vec3_accessor_values(
                    &banger_gltf_accessor_stage(gltf, bin_chunk, accessor_index)?,
                    "NORMAL",
                )?),
                None => None,
            };
            let texcoords = match texcoord0_accessor {
                Some(accessor_index) => Some(banger_maps_float_vec2_accessor_values(
                    &banger_gltf_accessor_stage(gltf, bin_chunk, accessor_index)?,
                    "TEXCOORD_0",
                )?),
                None => None,
            };
            let material_color = primitive
                .get("material")
                .and_then(Value::as_u64)
                .and_then(|index| banger_gltf_material_base_color(gltf, index as usize))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let vertex_bytes = banger_maps_engine_vertex_buffer_bytes(
                &position,
                normals.as_deref(),
                texcoords.as_deref(),
                material_color,
            )?;
            vertex_buffers.push(banger_create_mapped_buffer(
                device,
                "banger maps gltf engine vertex buffer",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                &vertex_bytes,
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
            &image_bytes.bytes,
        ));
    }
    Ok(BangerMapsUploadedTileBuffers {
        vertex_buffers,
        index_buffers,
        texture_staging_buffers,
    })
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
const BANGER_MATERIAL_RECORD_STRIDE: usize = 80;

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn banger_maps_material_resource_bytes(materials: &[BangerMapsMaterialStage]) -> Option<Vec<u8>> {
    if materials.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(materials.len() * BANGER_MATERIAL_RECORD_STRIDE);
    for material in materials {
        for value in material.base_color_factor {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&material.metallic_factor.to_le_bytes());
        bytes.extend_from_slice(&material.roughness_factor.to_le_bytes());
        bytes.extend_from_slice(&(material.base_color_texture.unwrap_or(u32::MAX as usize) as u32).to_le_bytes());
        bytes.extend_from_slice(&(material.material_index as u32).to_le_bytes());
        bytes.extend_from_slice(&(material.normal_texture.unwrap_or(u32::MAX as usize) as u32).to_le_bytes());
        bytes.extend_from_slice(&material.normal_scale.to_le_bytes());
        bytes.extend_from_slice(&(material.metallic_roughness_texture.unwrap_or(u32::MAX as usize) as u32).to_le_bytes());
        bytes.extend_from_slice(&(material.occlusion_texture.unwrap_or(u32::MAX as usize) as u32).to_le_bytes());
        bytes.extend_from_slice(&material.occlusion_strength.to_le_bytes());
        bytes.extend_from_slice(&(material.emissive_texture.unwrap_or(u32::MAX as usize) as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for value in material.emissive_factor {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0.0f32.to_le_bytes());
    }
    Some(bytes)
}

#[cfg(target_os = "windows")]
fn banger_default_material_resource_bytes() -> Vec<u8> {
    banger_maps_material_resource_bytes(&[BangerMapsMaterialStage {
        material_index: 0,
        base_color_factor: [1.0, 1.0, 1.0, 1.0],
        metallic_factor: 0.0,
        roughness_factor: 0.72,
        base_color_texture: None,
        normal_texture: None,
        normal_scale: 1.0,
        metallic_roughness_texture: None,
        occlusion_texture: None,
        occlusion_strength: 1.0,
        emissive_texture: None,
        emissive_factor: [0.0, 0.0, 0.0],
        material_hash: "banger_default_material_resource".to_string(),
    }])
    .expect("default Banger material must produce one material record")
}

#[cfg(target_os = "windows")]
fn banger_first_material_normal_texture_index(material_bytes: &[u8]) -> Option<usize> {
    let record = material_bytes.get(0..BANGER_MATERIAL_RECORD_STRIDE)?;
    let value = u32::from_le_bytes(record[32..36].try_into().ok()?);
    (value != u32::MAX).then_some(value as usize)
}

#[cfg(target_os = "windows")]
fn banger_first_material_metallic_roughness_texture_index(material_bytes: &[u8]) -> Option<usize> {
    let record = material_bytes.get(0..BANGER_MATERIAL_RECORD_STRIDE)?;
    let value = u32::from_le_bytes(record[40..44].try_into().ok()?);
    (value != u32::MAX).then_some(value as usize)
}

#[cfg(target_os = "windows")]
fn banger_first_material_occlusion_texture_index(material_bytes: &[u8]) -> Option<usize> {
    let record = material_bytes.get(0..BANGER_MATERIAL_RECORD_STRIDE)?;
    let value = u32::from_le_bytes(record[44..48].try_into().ok()?);
    (value != u32::MAX).then_some(value as usize)
}

#[cfg(target_os = "windows")]
fn banger_first_material_emissive_texture_index(material_bytes: &[u8]) -> Option<usize> {
    let record = material_bytes.get(0..BANGER_MATERIAL_RECORD_STRIDE)?;
    let value = u32::from_le_bytes(record[52..56].try_into().ok()?);
    (value != u32::MAX).then_some(value as usize)
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn banger_maps_texture_staging_resource_bytes(
    gltf: &Value,
    bin_chunk: &[u8],
    textures: &[BangerMapsTextureStage],
) -> Result<Vec<Vec<u8>>, String> {
    let gltf_textures = gltf.get("textures").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let images = gltf.get("images").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut buffers = Vec::new();
    for texture_stage in textures
        .iter()
        .filter(|stage| stage.source_kind == "embedded_buffer_view" && stage.byte_count > 0)
    {
        let texture_index = texture_stage.texture_index;
        let image_index = gltf_textures
            .get(texture_index)
            .and_then(|texture| texture.get("source"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("texture {texture_index} missing source before maps GPU resource upload"))?
            as usize;
        let buffer_view_index = images
            .get(image_index)
            .and_then(|image| image.get("bufferView"))
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("texture {texture_index} image {image_index} missing bufferView before maps GPU resource upload"))?
            as usize;
        buffers.push(banger_gltf_buffer_view_bytes(gltf, bin_chunk, buffer_view_index)?.bytes);
    }
    Ok(buffers)
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
                    tile_global_transform: record.tile_global_transform,
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
                tile_global_transform: record.tile_global_transform,
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
                tile_global_transform: record.tile_global_transform,
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
        tile_global_transform: record.tile_global_transform,
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
    let feature_json_end = feature_start + feature_table_json_byte_length as usize;
    let feature_end = feature_json_end + feature_table_binary_byte_length as usize;
    let batch_end = feature_end + batch_table_json_byte_length as usize + batch_table_binary_byte_length as usize;
    let glb = &bytes[glb_byte_offset..byte_length as usize];
    let rtc_center = banger_b3dm_rtc_center(
        &bytes[feature_start..feature_json_end],
        &bytes[feature_json_end..feature_end],
    );
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
            rtc_center,
            feature_table_hash: sha256_hex(&bytes[feature_start..feature_end]),
            batch_table_hash: sha256_hex(&bytes[feature_end..batch_end]),
        },
        glb,
    ))
}

#[cfg(target_os = "windows")]
struct BangerNativeSceneGpuResource {
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    indirect_draw_buffer: wgpu::Buffer,
    meshlet_culled_indirect_draw_buffer: wgpu::Buffer,
    meshlet_culled_indirect_seed_buffer: wgpu::Buffer,
    meshlet_cluster_buffer: wgpu::Buffer,
    visible_meshlet_cluster_buffer: wgpu::Buffer,
    meshlet_cull_feedback_buffer: wgpu::Buffer,
    meshlet_cull_param_buffer: wgpu::Buffer,
    material_buffer: Option<wgpu::Buffer>,
    material_bin_buffer: wgpu::Buffer,
    texture_staging_buffers: Vec<wgpu::Buffer>,
    texture_resources: Vec<BangerNativeTextureResource>,
    normal_texture_resource_index: u32,
    metallic_roughness_texture_resource_index: u32,
    occlusion_texture_resource_index: u32,
    emissive_texture_resource_index: u32,
    residency_feedback_buffer: wgpu::Buffer,
    shared_residency_page_table_buffer: wgpu::Buffer,
    shared_residency_compacted_feedback_buffer: wgpu::Buffer,
    shared_residency_eviction_plan_buffer: wgpu::Buffer,
    shared_residency_budget_buffer: wgpu::Buffer,
    lumen_surface_card_buffer: wgpu::Buffer,
    lumen_surface_cache_feedback_buffer: wgpu::Buffer,
    lumen_screen_probe_buffer: wgpu::Buffer,
    lumen_radiance_cache_buffer: wgpu::Buffer,
    virtual_shadow_map_page_table_buffer: wgpu::Buffer,
    virtual_shadow_map_page_flags_buffer: wgpu::Buffer,
    virtual_shadow_map_page_request_buffer: wgpu::Buffer,
    virtual_shadow_map_physical_page_metadata_buffer: wgpu::Buffer,
    virtual_shadow_map_projection_buffer: wgpu::Buffer,
    virtual_shadow_map_mark_params_buffer: wgpu::Buffer,
    virtual_shadow_map_physical_page_pool_texture: wgpu::Texture,
    virtual_shadow_map_physical_page_pool_view: wgpu::TextureView,
    virtual_shadow_map_projection_texture: wgpu::Texture,
    virtual_shadow_map_projection_view: wgpu::TextureView,
    virtual_shadow_map_cache_invalidation_buffer: wgpu::Buffer,
    virtual_shadow_map_projection_params_buffer: wgpu::Buffer,
    vertex_byte_count: usize,
    index_byte_count: usize,
    instance_byte_count: usize,
    vertex_count: u32,
    index_count: u32,
    instance_count: u32,
    index_format: BangerRenderIndexFormat,
    mesh_source: &'static str,
    mesh_bounds: BangerMeshBounds,
    selected_tile_id: Option<String>,
    indirect_args_hash: String,
    meshlet_cluster_hash: String,
    meshlet_cluster_count: u32,
    meshlet_cluster_cull_param_hash: String,
    meshlet_cluster_cull_feedback_hash: String,
    material_bin_hash: String,
    residency_feedback_hash: String,
    shared_residency_page_table_hash: String,
    shared_residency_compacted_feedback_hash: String,
    shared_residency_eviction_plan_hash: String,
    lumen_surface_card_hash: String,
    lumen_surface_cache_feedback_hash: String,
    lumen_screen_probe_hash: String,
    lumen_radiance_cache_hash: String,
    virtual_shadow_map_page_table_hash: String,
    virtual_shadow_map_page_request_hash: String,
    virtual_shadow_map_projection_hash: String,
    virtual_shadow_map_physical_pool_hash: String,
    virtual_shadow_map_cache_invalidation_hash: String,
    resource_hash: String,
}

fn banger_b3dm_rtc_center(feature_json: &[u8], feature_binary: &[u8]) -> Option<[f64; 3]> {
    if feature_json.is_empty() {
        return None;
    }
    let json_text = std::str::from_utf8(feature_json).ok()?.trim_matches(|character| {
        character == ' '
            || character == '\0'
            || character == '\n'
            || character == '\r'
            || character == '\t'
    });
    if json_text.is_empty() {
        return None;
    }
    let feature_table = serde_json::from_str::<Value>(json_text).ok()?;
    let rtc_center = feature_table.get("RTC_CENTER")?;
    if let Some(values) = rtc_center.as_array() {
        return banger_json_f64_vec3(values);
    }
    let byte_offset = rtc_center
        .as_object()
        .and_then(|object| object.get("byteOffset"))
        .and_then(Value::as_u64)? as usize;
    if byte_offset + 12 > feature_binary.len() {
        return None;
    }
    Some([
        read_f32_le(feature_binary, byte_offset).ok()? as f64,
        read_f32_le(feature_binary, byte_offset + 4).ok()? as f64,
        read_f32_le(feature_binary, byte_offset + 8).ok()? as f64,
    ])
}

fn banger_json_f64_vec3(values: &[Value]) -> Option<[f64; 3]> {
    if values.len() < 3 {
        return None;
    }
    let vector = [
        values[0].as_f64()?,
        values[1].as_f64()?,
        values[2].as_f64()?,
    ];
    if vector.iter().all(|value| value.is_finite()) {
        Some(vector)
    } else {
        None
    }
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

fn read_f32_le(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let end = offset + 4;
    if end > bytes.len() {
        return Err(format!("f32 read beyond buffer at offset {offset}"));
    }
    Ok(f32::from_le_bytes(bytes[offset..end].try_into().expect("slice length checked")))
}

#[cfg(test)]
fn push_u32_le(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn resolve_banger_tile_content_url(root_url: &str, content_uri: &str) -> String {
    if content_uri.starts_with("http://") || content_uri.starts_with("https://") {
        if let (Ok(base), Ok(resolved)) = (reqwest::Url::parse(root_url), reqwest::Url::parse(content_uri)) {
            return banger_maps_merge_root_query_into_content_url(base, resolved);
        }
        return content_uri.to_string();
    }
    if content_uri.starts_with("file://") {
        return content_uri.to_string();
    }
    if root_url.starts_with("http://") || root_url.starts_with("https://") {
        if let Ok(base) = reqwest::Url::parse(root_url) {
            if let Ok(resolved) = base.join(content_uri) {
                return banger_maps_merge_root_query_into_content_url(base, resolved);
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

fn banger_maps_merge_root_query_into_content_url(
    base: reqwest::Url,
    mut resolved: reqwest::Url,
) -> String {
    if base.scheme() != resolved.scheme() || base.host_str() != resolved.host_str() {
        return resolved.to_string();
    }
    let root_pairs = base
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    if root_pairs.is_empty() {
        return resolved.to_string();
    }
    let existing_keys = resolved
        .query_pairs()
        .map(|(key, _)| key.to_string())
        .collect::<Vec<_>>();
    {
        let mut query = resolved.query_pairs_mut();
        for (key, value) in root_pairs {
            if !existing_keys.iter().any(|existing| existing == &key) {
                query.append_pair(&key, &value);
            }
        }
    }
    resolved.to_string()
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
    let mut viewport_x = env::var("FORGE_BANGER_VIEWPORT_X").ok().and_then(|value| value.parse::<i32>().ok()).unwrap_or(0);
    let mut viewport_y = env::var("FORGE_BANGER_VIEWPORT_Y").ok().and_then(|value| value.parse::<i32>().ok()).unwrap_or(0);
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
    let scene_pipeline = create_banger_first_scene_pipeline(&device, &queue, format, present_mode, alpha_mode, &scene_kind)?;
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
    let maps_visual_gate = if scene_kind == "maps_sphere" {
        Some(banger_maps_native_render_gate())
    } else {
        None
    };
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
        depth_format: "Depth32Float",
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
        maps_visual_gate,
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

    let command_rx = spawn_banger_native_host_command_reader();
    let requested_frames = frame_limit.unwrap_or(u32::MAX);
    let mut submitted = 1u32;
    let mut shutdown_requested = false;
    while !shutdown_requested && submitted < requested_frames && unsafe { IsWindow(parent) } != 0 && unsafe { IsWindow(child) } != 0 {
        pump_win32_messages();
        while let Ok(command) = command_rx.try_recv() {
            match command {
                BangerNativeHostCommand::Resize { x, y, width, height } => {
                    viewport_x = x;
                    viewport_y = y;
                    unsafe {
                        SetWindowPos(
                            child,
                            std::ptr::null_mut(),
                            viewport_x,
                            viewport_y,
                            width as i32,
                            height as i32,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                    if width != config.width || height != config.height {
                        config.width = width;
                        config.height = height;
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
                BangerNativeHostCommand::Shutdown => {
                    shutdown_requested = true;
                }
            }
        }
        if shutdown_requested {
            break;
        }
        if !fixed_viewport {
            if let Some((parent_width, parent_height)) = parent_client_size(parent) {
            let parent_width = parent_width.clamp(64, 16384);
            let parent_height = parent_height.clamp(64, 16384);
            if parent_width != config.width || parent_height != config.height {
                unsafe {
                    SetWindowPos(
                        child,
                        std::ptr::null_mut(),
                        viewport_x,
                        viewport_y,
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
    _clear_color: [f64; 4],
    time_seconds: f32,
    frame_index: u32,
) -> Result<String, String> {
    let uniform_bytes = banger_frame_uniform_bytes_for_bounds(
        time_seconds,
        frame_index,
        frame_target.width,
        frame_target.height,
        scene_pipeline.mesh_bounds,
    );
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
    present_banger_sky_atmosphere(device, &mut encoder, scene_pipeline, &view);
    {
        let color_attachments = [
            Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &frame_target.gbuffer.albedo_view,
                depth_slice: None,
                resolve_target: None,
                ops: banger_gbuffer_clear_ops([0.0, 0.0, 0.0, 0.0]),
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &frame_target.gbuffer.normal_view,
                depth_slice: None,
                resolve_target: None,
                ops: banger_gbuffer_clear_ops([0.5, 0.5, 1.0, 0.0]),
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &frame_target.gbuffer.material_view,
                depth_slice: None,
                resolve_target: None,
                ops: banger_gbuffer_clear_ops([0.0, 0.0, 0.0, 0.0]),
            }),
            Some(wgpu::RenderPassColorAttachment {
                view: &frame_target.gbuffer.emissive_view,
                depth_slice: None,
                resolve_target: None,
                ops: banger_gbuffer_clear_ops([0.0, 0.0, 0.0, 0.0]),
            }),
        ];
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
        pass.set_index_buffer(scene_pipeline.index_buffer.slice(..), scene_pipeline.index_format.wgpu());
        pass.draw_indexed_indirect(&scene_pipeline.meshlet_culled_indirect_draw_buffer, 0);
    }
    dispatch_banger_hzb_build(&mut encoder, &frame_target.hzb);
    dispatch_banger_meshlet_cluster_cull(device, &mut encoder, scene_pipeline, &frame_target.hzb);
    dispatch_banger_virtual_shadow_map_page_mark(device, &mut encoder, scene_pipeline);
    dispatch_banger_virtual_shadow_map_physical_pages(device, &mut encoder, scene_pipeline);
    dispatch_banger_virtual_shadow_map_projection_filter(device, &mut encoder, scene_pipeline);
    present_banger_screen_space_ambient_occlusion(device, &mut encoder, scene_pipeline, frame_target, &view);
    let spectral_params_bytes =
        banger_spectral_ocean_params_bytes(frame_target.width, frame_target.height, time_seconds, frame_index);
    queue.write_buffer(
        &frame_target.water.spectral_params_buffer,
        0,
        &spectral_params_bytes,
    );
    dispatch_banger_spectral_ocean_compute(device, &mut encoder, scene_pipeline, frame_target);
    dispatch_banger_single_layer_water_composite(device, &mut encoder, scene_pipeline, frame_target);
    present_banger_single_layer_water_composite(device, &mut encoder, scene_pipeline, frame_target, &view);
    present_banger_emissive_bloom(device, &mut encoder, scene_pipeline, frame_target, &view);
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let hzb = create_banger_hzb_resources(device, &depth_texture, width, height, allocation_index);
    let gbuffer = create_banger_gbuffer_resources(device, width, height, allocation_index);
    let water = create_banger_single_layer_water_resources(device, width, height, allocation_index);
    BangerNativeFrameTarget {
        _depth_texture: depth_texture,
        depth_view,
        hzb,
        gbuffer,
        water,
        width,
        height,
        target_hash,
        depth_target_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_gbuffer_clear_ops(color: [f64; 4]) -> wgpu::Operations<wgpu::Color> {
    wgpu::Operations {
        load: wgpu::LoadOp::Clear(wgpu::Color {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        }),
        store: wgpu::StoreOp::Store,
    }
}

#[cfg(target_os = "windows")]
fn banger_gbuffer_color_target_state() -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: wgpu::TextureFormat::Rgba16Float,
        blend: Some(wgpu::BlendState::REPLACE),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

#[cfg(target_os = "windows")]
fn create_banger_gbuffer_resources(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    allocation_index: u32,
) -> BangerNativeGBufferResources {
    let albedo = banger_create_gbuffer_texture(device, "banger-native-gbuffer-albedo", width, height);
    let normal = banger_create_gbuffer_texture(device, "banger-native-gbuffer-normal", width, height);
    let material = banger_create_gbuffer_texture(device, "banger-native-gbuffer-material", width, height);
    let emissive = banger_create_gbuffer_texture(device, "banger-native-gbuffer-emissive", width, height);
    let resource_hash = sha256_hex(
        format!("banger-gbuffer-v1:{width}:{height}:rgba16float:{allocation_index}:albedo-normal-material-emissive")
            .as_bytes(),
    );
    BangerNativeGBufferResources {
        albedo_view: albedo.create_view(&wgpu::TextureViewDescriptor::default()),
        normal_view: normal.create_view(&wgpu::TextureViewDescriptor::default()),
        material_view: material.create_view(&wgpu::TextureViewDescriptor::default()),
        emissive_view: emissive.create_view(&wgpu::TextureViewDescriptor::default()),
        _albedo_texture: albedo,
        _normal_texture: normal,
        _material_texture: material,
        _emissive_texture: emissive,
        _resource_hash: resource_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_create_gbuffer_texture(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[cfg(target_os = "windows")]
fn create_banger_single_layer_water_resources(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    allocation_index: u32,
) -> BangerNativeSingleLayerWaterResources {
    let scene_without_water_texture = banger_create_water_texture(
        device,
        "banger-native-water-scene-without-water-texture",
        width,
        height,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let depth_without_water_texture = banger_create_water_texture(
        device,
        "banger-native-water-depth-without-water-texture",
        width,
        height,
        wgpu::TextureFormat::R32Float,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let refraction_mask_texture = banger_create_water_texture(
        device,
        "banger-native-water-refraction-mask-texture",
        width,
        height,
        wgpu::TextureFormat::R32Float,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let composite_texture = banger_create_water_texture(
        device,
        "banger-native-water-single-layer-composite-texture",
        width,
        height,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let spectral_state_texture = banger_create_water_texture(
        device,
        "banger-native-water-spectral-state-texture",
        width,
        height,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let spectral_displacement_texture = banger_create_water_texture(
        device,
        "banger-native-water-spectral-displacement-texture",
        width,
        height,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let spectral_slope_texture = banger_create_water_texture(
        device,
        "banger-native-water-spectral-slope-texture",
        width,
        height,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
    );
    let params_bytes = banger_single_layer_water_params_bytes(width, height);
    let spectral_params_bytes = banger_spectral_ocean_params_bytes(width, height, 0.0, allocation_index);
    let tile_mask_bytes = banger_single_layer_water_tile_mask_bytes(width, height);
    let params_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-water-single-layer-params-buffer",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &params_bytes,
    );
    let spectral_params_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-water-spectral-ocean-params-buffer",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &spectral_params_bytes,
    );
    let tile_mask_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-water-single-layer-tile-mask-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &tile_mask_bytes,
    );
    let resource_hash = sha256_hex(
        format!(
            "banger-single-layer-water-v1:{width}:{height}:{allocation_index}:scene-depth-refraction-composite-spectral:{}:{}:{}",
            sha256_hex(&params_bytes),
            sha256_hex(&tile_mask_bytes),
            sha256_hex(&spectral_params_bytes)
        )
        .as_bytes(),
    );
    BangerNativeSingleLayerWaterResources {
        _scene_without_water_view: scene_without_water_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        _depth_without_water_view: depth_without_water_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        refraction_mask_view: refraction_mask_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        composite_view: composite_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        spectral_state_view: spectral_state_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        spectral_displacement_view: spectral_displacement_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        spectral_slope_view: spectral_slope_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        _scene_without_water_texture: scene_without_water_texture,
        _depth_without_water_texture: depth_without_water_texture,
        _refraction_mask_texture: refraction_mask_texture,
        _composite_texture: composite_texture,
        _spectral_state_texture: spectral_state_texture,
        _spectral_displacement_texture: spectral_displacement_texture,
        _spectral_slope_texture: spectral_slope_texture,
        params_buffer,
        spectral_params_buffer,
        tile_mask_buffer,
        _resource_hash: resource_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_create_water_texture(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

#[cfg(target_os = "windows")]
fn banger_single_layer_water_params_bytes(width: u32, height: u32) -> [u8; 48] {
    let tile_width = width.max(1).div_ceil(8);
    let tile_height = height.max(1).div_ceil(8);
    let mut bytes = [0u8; 48];
    for (slot, value) in [width.max(1), height.max(1), tile_width, tile_height]
        .into_iter()
        .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (slot, value) in [
        0.18f32, // red absorption
        0.055,  // green absorption
        0.024,  // blue absorption
        0.45,   // minimum water depth proxy
        0.035,  // scattering red
        0.17,   // scattering green
        0.24,   // scattering blue
        1.0,
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 16 + slot * 4;
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_single_layer_water_tile_mask_bytes(width: u32, height: u32) -> Vec<u8> {
    let tile_count = width.max(1).div_ceil(8) * height.max(1).div_ceil(8);
    let word_count = tile_count.max(1).div_ceil(32);
    vec![0u8; word_count as usize * 4]
}

#[cfg(target_os = "windows")]
fn banger_spectral_ocean_params_bytes(
    width: u32,
    height: u32,
    time_seconds: f32,
    frame_index: u32,
) -> [u8; 64] {
    let spectral_size = width.max(height).next_power_of_two().clamp(64, 2048);
    let mut bytes = [0u8; 64];
    for (slot, value) in [
        width.max(1),
        height.max(1),
        spectral_size,
        frame_index,
        8, // reserved butterfly stage count for the promoted inverse FFT path.
        0,
        0,
        0,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (slot, value) in [
        time_seconds,
        9.81f32, // gravity
        21.0,    // wind speed m/s
        160.0,   // domain length meters
        0.72,    // wind x
        0.36,    // wind y
        1.35,    // choppiness
        0.0007,  // Phillips/JONSWAP energy scale seed
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 32 + slot * 4;
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn create_banger_hzb_resources(
    device: &wgpu::Device,
    depth_texture: &wgpu::Texture,
    width: u32,
    height: u32,
    allocation_index: u32,
) -> BangerNativeHzbResources {
    let mip_count = banger_hzb_mip_count(width, height);
    let hzb_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-native-child-host-hzb-r32float-pyramid"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let hzb_views = (0..mip_count)
        .map(|mip| {
            hzb_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("banger-native-child-host-hzb-mip-view"),
                format: Some(wgpu::TextureFormat::R32Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: mip,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
            })
        })
        .collect::<Vec<_>>();
    let hzb_consumer_view = hzb_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("banger-native-child-host-hzb-consumer-pyramid-view"),
        format: Some(wgpu::TextureFormat::R32Float),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(mip_count),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    let depth_source_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("banger-native-child-host-depth-sampled-view"),
        format: Some(wgpu::TextureFormat::Depth32Float),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    let seed_uniform = banger_create_mapped_buffer(
        device,
        "banger-native-child-host-hzb-seed-uniform",
        wgpu::BufferUsages::UNIFORM,
        &banger_u32x4_bytes([width, height, width, height]),
    );
    let reduce_uniforms = (1..mip_count)
        .map(|mip| {
            let src = banger_hzb_mip_size(width, height, mip - 1);
            let dst = banger_hzb_mip_size(width, height, mip);
            banger_create_mapped_buffer(
                device,
                "banger-native-child-host-hzb-reduce-uniform",
                wgpu::BufferUsages::UNIFORM,
                &banger_u32x4_bytes([src[0], src[1], dst[0], dst[1]]),
            )
        })
        .collect::<Vec<_>>();
    let seed_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-child-host-hzb-seed-wgsl"),
        source: wgpu::ShaderSource::Wgsl(banger_hzb_seed_compute_wgsl().into()),
    });
    let reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-child-host-hzb-reduce-wgsl"),
        source: wgpu::ShaderSource::Wgsl(banger_hzb_reduce_compute_wgsl().into()),
    });
    let seed_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("banger-native-child-host-hzb-seed-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::R32Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let reduce_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("banger-native-child-host-hzb-reduce-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::R32Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let consumer_uniform = banger_create_mapped_buffer(
        device,
        "banger-native-child-host-hzb-consumer-uniform",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &banger_hzb_consumer_uniform_bytes(width, height, mip_count, allocation_index),
    );
    let consumer_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("banger-native-child-host-hzb-consumer-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let consumer_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-child-host-hzb-consumer-bind-group"),
        layout: &consumer_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&hzb_consumer_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: consumer_uniform.as_entire_binding(),
            },
        ],
    });
    let seed_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-child-host-hzb-seed-pipeline-layout"),
        bind_group_layouts: &[Some(&seed_bind_group_layout)],
        immediate_size: 0,
    });
    let reduce_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-child-host-hzb-reduce-pipeline-layout"),
        bind_group_layouts: &[Some(&reduce_bind_group_layout)],
        immediate_size: 0,
    });
    let seed_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-child-host-hzb-seed-pipeline"),
        layout: Some(&seed_pipeline_layout),
        module: &seed_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let reduce_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-child-host-hzb-reduce-pipeline"),
        layout: Some(&reduce_pipeline_layout),
        module: &reduce_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let seed_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-child-host-hzb-seed-bind-group"),
        layout: &seed_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&depth_source_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&hzb_views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: seed_uniform.as_entire_binding(),
            },
        ],
    });
    let reduce_bind_groups = (1..mip_count)
        .map(|mip| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("banger-native-child-host-hzb-reduce-bind-group"),
                layout: &reduce_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&hzb_views[(mip - 1) as usize]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&hzb_views[mip as usize]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: reduce_uniforms[(mip - 1) as usize].as_entire_binding(),
                    },
                ],
            })
        })
        .collect::<Vec<_>>();
    BangerNativeHzbResources {
        _texture: hzb_texture,
        _views: hzb_views,
        _consumer_view: hzb_consumer_view,
        _consumer_uniform_buffer: consumer_uniform,
        _consumer_bind_group_layout: consumer_bind_group_layout,
        _consumer_bind_group: consumer_bind_group,
        seed_pipeline,
        reduce_pipeline,
        seed_bind_group,
        reduce_bind_groups,
        mip_count,
        width,
        height,
        _hzb_hash: banger_hzb_resource_hash(width, height, mip_count, allocation_index),
        _consumer_hash: banger_hzb_consumer_resource_hash(width, height, mip_count, allocation_index),
    }
}

#[cfg(target_os = "windows")]
fn dispatch_banger_hzb_build(encoder: &mut wgpu::CommandEncoder, hzb: &BangerNativeHzbResources) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-child-host-hzb-build-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&hzb.seed_pipeline);
    pass.set_bind_group(0, &hzb.seed_bind_group, &[]);
    pass.dispatch_workgroups(hzb.width.div_ceil(8), hzb.height.div_ceil(8), 1);
    pass.set_pipeline(&hzb.reduce_pipeline);
    for mip in 1..hzb.mip_count {
        let size = banger_hzb_mip_size(hzb.width, hzb.height, mip);
        pass.set_bind_group(0, &hzb.reduce_bind_groups[(mip - 1) as usize], &[]);
        pass.dispatch_workgroups(size[0].div_ceil(8), size[1].div_ceil(8), 1);
    }
}

#[cfg(target_os = "windows")]
fn dispatch_banger_meshlet_cluster_cull(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    hzb: &BangerNativeHzbResources,
) {
    encoder.clear_buffer(&scene_pipeline.meshlet_cull_feedback_buffer, 0, None);
    encoder.copy_buffer_to_buffer(
        &scene_pipeline.meshlet_culled_indirect_seed_buffer,
        0,
        &scene_pipeline.meshlet_culled_indirect_draw_buffer,
        0,
        20,
    );
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-meshlet-cluster-cull-bind-group"),
        layout: &scene_pipeline.meshlet_cull_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&hzb._consumer_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: hzb._consumer_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: scene_pipeline.meshlet_cluster_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: scene_pipeline.visible_meshlet_cluster_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: scene_pipeline.meshlet_cull_feedback_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: scene_pipeline.meshlet_cull_param_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: scene_pipeline.meshlet_culled_indirect_draw_buffer.as_entire_binding(),
            },
        ],
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-meshlet-cluster-hzb-cull-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&scene_pipeline.meshlet_cull_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(scene_pipeline._meshlet_cluster_count.max(1).div_ceil(64), 1, 1);
}

#[cfg(target_os = "windows")]
fn dispatch_banger_virtual_shadow_map_page_mark(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
) {
    encoder.clear_buffer(&scene_pipeline.virtual_shadow_map_page_flags_buffer, 0, None);
    encoder.clear_buffer(&scene_pipeline.virtual_shadow_map_page_request_buffer, 0, None);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-vsm-page-mark-bind-group"),
        layout: &scene_pipeline.virtual_shadow_map_mark_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_pipeline.visible_meshlet_cluster_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: scene_pipeline.virtual_shadow_map_page_table_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: scene_pipeline.virtual_shadow_map_page_flags_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: scene_pipeline.virtual_shadow_map_page_request_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: scene_pipeline
                    .virtual_shadow_map_physical_page_metadata_buffer
                    .as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: scene_pipeline.virtual_shadow_map_projection_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: scene_pipeline.virtual_shadow_map_mark_params_buffer.as_entire_binding(),
            },
        ],
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-vsm-page-mark-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&scene_pipeline.virtual_shadow_map_mark_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(scene_pipeline._meshlet_cluster_count.max(1).div_ceil(64), 1, 1);
}

#[cfg(target_os = "windows")]
fn dispatch_banger_virtual_shadow_map_physical_pages(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
) {
    encoder.clear_buffer(&scene_pipeline.virtual_shadow_map_cache_invalidation_buffer, 0, None);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-vsm-physical-page-bind-group"),
        layout: &scene_pipeline.virtual_shadow_map_physical_page_bind_group_layout,
        entries: &banger_vsm_compute_bind_entries(
            scene_pipeline,
            &scene_pipeline.virtual_shadow_map_physical_page_pool_view,
        ),
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-vsm-physical-page-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&scene_pipeline.virtual_shadow_map_physical_page_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(scene_pipeline._meshlet_cluster_count.max(1).div_ceil(64), 1, 1);
}

#[cfg(target_os = "windows")]
fn dispatch_banger_virtual_shadow_map_projection_filter(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
) {
    let dispatch = BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_MASK_SIZE.div_ceil(8);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-vsm-projection-filter-bind-group"),
        layout: &scene_pipeline.virtual_shadow_map_projection_bind_group_layout,
        entries: &banger_vsm_compute_bind_entries(
            scene_pipeline,
            &scene_pipeline.virtual_shadow_map_projection_view,
        ),
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-vsm-projection-filter-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&scene_pipeline.virtual_shadow_map_projection_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(dispatch, dispatch, 1);
}

#[cfg(target_os = "windows")]
fn dispatch_banger_single_layer_water_composite(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    frame_target: &BangerNativeFrameTarget,
) {
    encoder.clear_buffer(&frame_target.water.tile_mask_buffer, 0, None);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-single-layer-water-bind-group"),
        layout: &scene_pipeline.single_layer_water_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&frame_target.gbuffer.albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&frame_target.gbuffer.normal_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&frame_target.gbuffer.material_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.composite_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.refraction_mask_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: frame_target.water.params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: frame_target.water.tile_mask_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.spectral_displacement_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.spectral_slope_view),
            },
        ],
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-single-layer-water-composite-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&scene_pipeline.single_layer_water_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(frame_target.width.div_ceil(8), frame_target.height.div_ceil(8), 1);
}

#[cfg(target_os = "windows")]
fn present_banger_sky_atmosphere(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    output_view: &wgpu::TextureView,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-sky-atmosphere-present-bind-group"),
        layout: &scene_pipeline.sky_present_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: scene_pipeline.uniform_buffer.as_entire_binding(),
        }],
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("banger-native-sky-atmosphere-present-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.015,
                    g: 0.018,
                    b: 0.024,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&scene_pipeline.sky_present_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(target_os = "windows")]
fn present_banger_screen_space_ambient_occlusion(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    frame_target: &BangerNativeFrameTarget,
    output_view: &wgpu::TextureView,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-ssao-present-bind-group"),
        layout: &scene_pipeline.ssao_present_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&frame_target.depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&frame_target.gbuffer.normal_view),
            },
        ],
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("banger-native-ssao-present-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&scene_pipeline.ssao_present_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(target_os = "windows")]
fn present_banger_single_layer_water_composite(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    frame_target: &BangerNativeFrameTarget,
    output_view: &wgpu::TextureView,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-single-layer-water-present-bind-group"),
        layout: &scene_pipeline.single_layer_water_present_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.composite_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.refraction_mask_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&scene_pipeline.single_layer_water_present_sampler),
            },
        ],
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("banger-native-single-layer-water-present-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&scene_pipeline.single_layer_water_present_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(target_os = "windows")]
fn present_banger_emissive_bloom(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    frame_target: &BangerNativeFrameTarget,
    output_view: &wgpu::TextureView,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-emissive-bloom-present-bind-group"),
        layout: &scene_pipeline.bloom_present_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&frame_target.gbuffer.emissive_view),
        }],
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("banger-native-emissive-bloom-present-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&scene_pipeline.bloom_present_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(target_os = "windows")]
fn dispatch_banger_spectral_ocean_compute(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    scene_pipeline: &BangerNativeScenePipeline,
    frame_target: &BangerNativeFrameTarget,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-spectral-ocean-bind-group"),
        layout: &scene_pipeline.spectral_ocean_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.spectral_state_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.spectral_displacement_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&frame_target.water.spectral_slope_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: frame_target.water.spectral_params_buffer.as_entire_binding(),
            },
        ],
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("banger-native-spectral-ocean-compute-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&scene_pipeline.spectral_ocean_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(frame_target.width.div_ceil(8), frame_target.height.div_ceil(8), 1);
}

#[cfg(target_os = "windows")]
fn banger_vsm_compute_bind_entries<'a>(
    scene_pipeline: &'a BangerNativeScenePipeline,
    output_view: &'a wgpu::TextureView,
) -> [wgpu::BindGroupEntry<'a>; 7] {
    [
        wgpu::BindGroupEntry {
            binding: 0,
            resource: scene_pipeline.virtual_shadow_map_page_request_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: scene_pipeline
                .virtual_shadow_map_physical_page_metadata_buffer
                .as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::TextureView(output_view),
        },
        wgpu::BindGroupEntry {
            binding: 3,
            resource: scene_pipeline.virtual_shadow_map_cache_invalidation_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 4,
            resource: scene_pipeline.virtual_shadow_map_projection_params_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 5,
            resource: scene_pipeline.virtual_shadow_map_page_table_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 6,
            resource: scene_pipeline.virtual_shadow_map_projection_buffer.as_entire_binding(),
        },
    ]
}

#[cfg(target_os = "windows")]
fn banger_vsm_storage_texture_compute_bind_group_layout(
    device: &wgpu::Device,
    label: &'static str,
    texture_dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::R32Uint,
                    view_dimension: texture_dimension,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

#[cfg(target_os = "windows")]
fn banger_unfilterable_texture_bind_group_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

#[cfg(target_os = "windows")]
fn banger_rgba16float_storage_texture_bind_group_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba16Float,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

#[cfg(target_os = "windows")]
fn banger_hzb_mip_count(width: u32, height: u32) -> u32 {
    let mut size = width.max(height).max(1);
    let mut mips = 1u32;
    while size > 1 {
        size = size.div_ceil(2);
        mips += 1;
    }
    mips
}

#[cfg(target_os = "windows")]
fn banger_hzb_mip_size(width: u32, height: u32, mip: u32) -> [u32; 2] {
    [
        (width >> mip).max(1),
        (height >> mip).max(1),
    ]
}

#[cfg(target_os = "windows")]
fn banger_hzb_resource_hash(width: u32, height: u32, mip_count: u32, allocation_index: u32) -> String {
    sha256_hex(
        format!("banger-hzb-resource-v1:{width}:{height}:{mip_count}:r32float:{allocation_index}")
            .as_bytes(),
    )
}

#[cfg(target_os = "windows")]
fn banger_hzb_consumer_uniform_bytes(
    width: u32,
    height: u32,
    mip_count: u32,
    allocation_index: u32,
) -> [u8; 16] {
    banger_u32x4_bytes([width, height, mip_count, allocation_index])
}

#[cfg(target_os = "windows")]
fn banger_hzb_consumer_resource_hash(
    width: u32,
    height: u32,
    mip_count: u32,
    allocation_index: u32,
) -> String {
    let consumer_shader_hash = sha256_hex(banger_hzb_consumer_compute_wgsl().as_bytes());
    sha256_hex(
        format!(
            "banger-hzb-consumer-resource-v1:{width}:{height}:{mip_count}:r32float:textureload:{allocation_index}:{consumer_shader_hash}"
        )
        .as_bytes(),
    )
}

#[cfg(target_os = "windows")]
fn banger_hzb_consumer_compute_wgsl() -> &'static str {
    r#"
struct HzbConsumerUniform {
    // x/y: mip 0 size, z: mip count, w: allocation index.
    dims: vec4<u32>,
};

@group(0) @binding(0) var hzb_pyramid: texture_2d<f32>;
@group(0) @binding(1) var<uniform> hzb: HzbConsumerUniform;

fn banger_hzb_load_furthest(pixel: vec2<u32>, mip: u32) -> f32 {
    let safe_mip = min(mip, hzb.dims.z - 1u);
    let mip_size = max(hzb.dims.xy >> vec2<u32>(safe_mip, safe_mip), vec2<u32>(1u, 1u));
    let safe_pixel = min(pixel, mip_size - vec2<u32>(1u, 1u));
    return textureLoad(hzb_pyramid, vec2<i32>(safe_pixel), i32(safe_mip)).x;
}
"#
}

#[cfg(target_os = "windows")]
fn banger_u32x4_bytes(values: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (index, value) in values.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_hzb_seed_compute_wgsl() -> &'static str {
    r#"
struct HzbUniform {
    // x/y: source depth size, z/w: destination mip size.
    dims: vec4<u32>,
};

@group(0) @binding(0) var source_depth: texture_depth_2d;
@group(0) @binding(1) var target_hzb: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> hzb: HzbUniform;

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= hzb.dims.z || gid.y >= hzb.dims.w) {
        return;
    }
    let source_xy = vec2<i32>(
        min(gid.x, max(hzb.dims.x, 1u) - 1u),
        min(gid.y, max(hzb.dims.y, 1u) - 1u)
    );
    let depth = textureLoad(source_depth, source_xy, 0);
    textureStore(target_hzb, vec2<i32>(gid.xy), vec4<f32>(depth, 0.0, 0.0, 0.0));
}
"#
}

#[cfg(target_os = "windows")]
fn banger_hzb_reduce_compute_wgsl() -> &'static str {
    r#"
struct HzbUniform {
    // x/y: source mip size, z/w: destination mip size.
    dims: vec4<u32>,
};

@group(0) @binding(0) var source_hzb: texture_2d<f32>;
@group(0) @binding(1) var target_hzb: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> hzb: HzbUniform;

fn load_depth(xy: vec2<u32>) -> f32 {
    let clamped_xy = min(xy, hzb.dims.xy - vec2<u32>(1u, 1u));
    return textureLoad(source_hzb, vec2<i32>(clamped_xy), 0).x;
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= hzb.dims.z || gid.y >= hzb.dims.w) {
        return;
    }
    let source_xy = gid.xy * 2u;
    let a = load_depth(source_xy);
    let b = load_depth(source_xy + vec2<u32>(1u, 0u));
    let c = load_depth(source_xy + vec2<u32>(0u, 1u));
    let d = load_depth(source_xy + vec2<u32>(1u, 1u));
    textureStore(target_hzb, vec2<i32>(gid.xy), vec4<f32>(max(max(a, b), max(c, d)), 0.0, 0.0, 0.0));
}
"#
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
    queue: &wgpu::Queue,
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
    let gpu_resource = if scene_kind == "maps_sphere" {
        banger_maps_first_visible_tile_gpu_resource(device, queue)?
    } else {
        banger_native_scene_gpu_resource_from_mesh_bytes(device, BangerRenderMeshBytes {
            vertex_bytes: banger_cube_vertex_bytes(),
            index_bytes: banger_cube_index_bytes(),
            index_format: BangerRenderIndexFormat::Uint16,
            instance_bytes: banger_scene_instance_bytes(),
            bounds: banger_mesh_bounds_from_vertex_bytes(&banger_cube_vertex_bytes()),
            source: "banger_dense_cube_field_fallback",
        }, None, Vec::new(), None, queue)
    };
    let instance_buffer_hash = sha256_hex(
        format!(
            "{}:{}",
            gpu_resource.instance_byte_count,
            gpu_resource.instance_count
        )
        .as_bytes(),
    );
    let scene_mesh_hash = sha256_hex(
        format!(
            "banger-native-render-mesh-v3:{}:{}:{}:{}",
            gpu_resource.mesh_source,
            gpu_resource.vertex_byte_count,
            gpu_resource.index_byte_count,
            gpu_resource.resource_hash
        )
        .as_bytes(),
    );
    let scene_graph_hash = sha256_hex(
        format!(
            "banger-scene-graph-v1:{}:{}:{}",
            scene_mesh_hash,
            instance_buffer_hash,
            gpu_resource.instance_count
        )
        .as_bytes(),
    );
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("banger-native-frame-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 8,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-first-scene-pipeline-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("banger-native-frame-bind-group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gpu_resource.texture_resources[0].view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&gpu_resource.texture_resources[0].sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&gpu_resource.virtual_shadow_map_projection_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: gpu_resource
                    .material_buffer
                    .as_ref()
                    .expect("Banger material buffer must be defaulted before pipeline creation")
                    .as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(
                    &gpu_resource.texture_resources[gpu_resource.normal_texture_resource_index as usize].view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(
                    &gpu_resource.texture_resources[gpu_resource.metallic_roughness_texture_resource_index as usize].view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(
                    &gpu_resource.texture_resources[gpu_resource.occlusion_texture_resource_index as usize].view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(
                    &gpu_resource.texture_resources[gpu_resource.emissive_texture_resource_index as usize].view,
                ),
            },
        ],
    });
    let targets = [
        Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        }),
        Some(banger_gbuffer_color_target_state()),
        Some(banger_gbuffer_color_target_state()),
        Some(banger_gbuffer_color_target_state()),
        Some(banger_gbuffer_color_target_state()),
    ];
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-first-scene-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: BANGER_RENDER_VERTEX_STRIDE_BYTES as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 20,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 32,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 36,
                            shader_location: 4,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 48,
                            shader_location: 5,
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
                            shader_location: 6,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 7,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 8,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 48,
                            shader_location: 9,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 64,
                            shader_location: 10,
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
            format: wgpu::TextureFormat::Depth32Float,
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
    let sky_present_shader_source = banger_sky_atmosphere_present_wgsl();
    let sky_present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-sky-atmosphere-present-wgsl"),
        source: wgpu::ShaderSource::Wgsl(sky_present_shader_source.into()),
    });
    let sky_present_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-sky-atmosphere-present-bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let sky_present_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("banger-native-sky-atmosphere-present-pipeline-layout"),
            bind_group_layouts: &[Some(&sky_present_bind_group_layout)],
            immediate_size: 0,
        });
    let sky_present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-sky-atmosphere-present-pipeline"),
        layout: Some(&sky_present_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &sky_present_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &sky_present_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let ssao_present_shader_source = banger_screen_space_ambient_occlusion_present_wgsl();
    let ssao_present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-ssao-present-wgsl"),
        source: wgpu::ShaderSource::Wgsl(ssao_present_shader_source.into()),
    });
    let ssao_present_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-ssao-present-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
    let ssao_present_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("banger-native-ssao-present-pipeline-layout"),
            bind_group_layouts: &[Some(&ssao_present_bind_group_layout)],
            immediate_size: 0,
        });
    let ssao_present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-ssao-present-pipeline"),
        layout: Some(&ssao_present_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &ssao_present_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &ssao_present_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let meshlet_cull_shader_source = banger_meshlet_cluster_cull_compute_wgsl();
    let meshlet_cull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-meshlet-cluster-cull-wgsl"),
        source: wgpu::ShaderSource::Wgsl(meshlet_cull_shader_source.into()),
    });
    let meshlet_cull_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-meshlet-cluster-cull-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
    let meshlet_cull_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-meshlet-cluster-cull-pipeline-layout"),
        bind_group_layouts: &[Some(&meshlet_cull_bind_group_layout)],
        immediate_size: 0,
    });
    let meshlet_cull_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-meshlet-cluster-cull-pipeline"),
        layout: Some(&meshlet_cull_pipeline_layout),
        module: &meshlet_cull_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let vsm_mark_shader_source = banger_virtual_shadow_map_mark_compute_wgsl();
    let vsm_mark_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-vsm-page-mark-wgsl"),
        source: wgpu::ShaderSource::Wgsl(vsm_mark_shader_source.into()),
    });
    let virtual_shadow_map_mark_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-vsm-page-mark-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
    let vsm_mark_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-vsm-page-mark-pipeline-layout"),
        bind_group_layouts: &[Some(&virtual_shadow_map_mark_bind_group_layout)],
        immediate_size: 0,
    });
    let virtual_shadow_map_mark_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-vsm-page-mark-pipeline"),
        layout: Some(&vsm_mark_pipeline_layout),
        module: &vsm_mark_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let vsm_physical_page_shader_source = banger_virtual_shadow_map_physical_page_compute_wgsl();
    let vsm_physical_page_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-vsm-physical-page-wgsl"),
        source: wgpu::ShaderSource::Wgsl(vsm_physical_page_shader_source.into()),
    });
    let virtual_shadow_map_physical_page_bind_group_layout =
        banger_vsm_storage_texture_compute_bind_group_layout(
            device,
            "banger-native-vsm-physical-page-bind-group-layout",
            wgpu::TextureViewDimension::D2Array,
        );
    let vsm_physical_page_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-vsm-physical-page-pipeline-layout"),
        bind_group_layouts: &[Some(&virtual_shadow_map_physical_page_bind_group_layout)],
        immediate_size: 0,
    });
    let virtual_shadow_map_physical_page_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-vsm-physical-page-pipeline"),
        layout: Some(&vsm_physical_page_pipeline_layout),
        module: &vsm_physical_page_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let vsm_projection_shader_source = banger_virtual_shadow_map_projection_filter_compute_wgsl();
    let vsm_projection_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-vsm-projection-filter-wgsl"),
        source: wgpu::ShaderSource::Wgsl(vsm_projection_shader_source.into()),
    });
    let virtual_shadow_map_projection_bind_group_layout =
        banger_vsm_storage_texture_compute_bind_group_layout(
            device,
            "banger-native-vsm-projection-filter-bind-group-layout",
            wgpu::TextureViewDimension::D2,
        );
    let vsm_projection_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("banger-native-vsm-projection-filter-pipeline-layout"),
        bind_group_layouts: &[Some(&virtual_shadow_map_projection_bind_group_layout)],
        immediate_size: 0,
    });
    let virtual_shadow_map_projection_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-vsm-projection-filter-pipeline"),
        layout: Some(&vsm_projection_pipeline_layout),
        module: &vsm_projection_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let spectral_ocean_shader_source = banger_spectral_ocean_compute_wgsl();
    let spectral_ocean_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-spectral-ocean-wgsl"),
        source: wgpu::ShaderSource::Wgsl(spectral_ocean_shader_source.into()),
    });
    let spectral_ocean_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-spectral-ocean-bind-group-layout"),
            entries: &[
                banger_rgba16float_storage_texture_bind_group_layout_entry(0),
                banger_rgba16float_storage_texture_bind_group_layout_entry(1),
                banger_rgba16float_storage_texture_bind_group_layout_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
    let spectral_ocean_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("banger-native-spectral-ocean-pipeline-layout"),
            bind_group_layouts: &[Some(&spectral_ocean_bind_group_layout)],
            immediate_size: 0,
        });
    let spectral_ocean_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-spectral-ocean-pipeline"),
        layout: Some(&spectral_ocean_pipeline_layout),
        module: &spectral_ocean_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let single_layer_water_shader_source = banger_single_layer_water_composite_compute_wgsl();
    let single_layer_water_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-single-layer-water-composite-wgsl"),
        source: wgpu::ShaderSource::Wgsl(single_layer_water_shader_source.into()),
    });
    let single_layer_water_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-single-layer-water-bind-group-layout"),
            entries: &[
                banger_unfilterable_texture_bind_group_layout_entry(0),
                banger_unfilterable_texture_bind_group_layout_entry(1),
                banger_unfilterable_texture_bind_group_layout_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                banger_unfilterable_texture_bind_group_layout_entry(7),
                banger_unfilterable_texture_bind_group_layout_entry(8),
            ],
        });
    let single_layer_water_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("banger-native-single-layer-water-pipeline-layout"),
            bind_group_layouts: &[Some(&single_layer_water_bind_group_layout)],
            immediate_size: 0,
        });
    let single_layer_water_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("banger-native-single-layer-water-composite-pipeline"),
        layout: Some(&single_layer_water_pipeline_layout),
        module: &single_layer_water_shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let single_layer_water_present_shader_source = banger_single_layer_water_present_wgsl();
    let single_layer_water_present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-single-layer-water-present-wgsl"),
        source: wgpu::ShaderSource::Wgsl(single_layer_water_present_shader_source.into()),
    });
    let single_layer_water_present_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-single-layer-water-present-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
    let single_layer_water_present_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("banger-native-single-layer-water-present-pipeline-layout"),
            bind_group_layouts: &[Some(&single_layer_water_present_bind_group_layout)],
            immediate_size: 0,
        });
    let single_layer_water_present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-single-layer-water-present-pipeline"),
        layout: Some(&single_layer_water_present_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &single_layer_water_present_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &single_layer_water_present_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let single_layer_water_present_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("banger-native-single-layer-water-present-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let bloom_present_shader_source = banger_emissive_bloom_present_wgsl();
    let bloom_present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("banger-native-emissive-bloom-present-wgsl"),
        source: wgpu::ShaderSource::Wgsl(bloom_present_shader_source.into()),
    });
    let bloom_present_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("banger-native-emissive-bloom-present-bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
    let bloom_present_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("banger-native-emissive-bloom-present-pipeline-layout"),
            bind_group_layouts: &[Some(&bloom_present_bind_group_layout)],
            immediate_size: 0,
        });
    let bloom_present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("banger-native-emissive-bloom-present-pipeline"),
        layout: Some(&bloom_present_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &bloom_present_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &bloom_present_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let render_pipeline_hash = sha256_hex(
        format!(
            "banger-first-scene-pipeline:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{:?}:{:?}:{}:instanced_mesh_depth_camera_v1",
            shader_source_hash,
            sha256_hex(sky_present_shader_source.as_bytes()),
            sha256_hex(ssao_present_shader_source.as_bytes()),
            sha256_hex(meshlet_cull_shader_source.as_bytes()),
            sha256_hex(vsm_mark_shader_source.as_bytes()),
            sha256_hex(vsm_physical_page_shader_source.as_bytes()),
            sha256_hex(vsm_projection_shader_source.as_bytes()),
            sha256_hex(spectral_ocean_shader_source.as_bytes()),
            sha256_hex(single_layer_water_shader_source.as_bytes()),
            sha256_hex(single_layer_water_present_shader_source.as_bytes()),
            sha256_hex(bloom_present_shader_source.as_bytes()),
            scene_mesh_hash,
            scene_graph_hash,
            format,
            present_mode,
            alpha_mode,
            scene_kind
        )
        .as_bytes(),
    );
    Ok(BangerNativeScenePipeline {
        render_pipeline,
        sky_present_bind_group_layout,
        sky_present_pipeline,
        ssao_present_bind_group_layout,
        ssao_present_pipeline,
        uniform_buffer,
        bind_group,
        vertex_buffer: gpu_resource.vertex_buffer,
        instance_buffer: gpu_resource.instance_buffer,
        index_buffer: gpu_resource.index_buffer,
        _indirect_draw_buffer: gpu_resource.indirect_draw_buffer,
        meshlet_culled_indirect_draw_buffer: gpu_resource.meshlet_culled_indirect_draw_buffer,
        meshlet_culled_indirect_seed_buffer: gpu_resource.meshlet_culled_indirect_seed_buffer,
        meshlet_cluster_buffer: gpu_resource.meshlet_cluster_buffer,
        visible_meshlet_cluster_buffer: gpu_resource.visible_meshlet_cluster_buffer,
        meshlet_cull_feedback_buffer: gpu_resource.meshlet_cull_feedback_buffer,
        meshlet_cull_param_buffer: gpu_resource.meshlet_cull_param_buffer,
        meshlet_cull_bind_group_layout,
        meshlet_cull_pipeline,
        _material_buffer: gpu_resource.material_buffer,
        _material_bin_buffer: gpu_resource.material_bin_buffer,
        _texture_staging_buffers: gpu_resource.texture_staging_buffers,
        _texture_resources: gpu_resource.texture_resources,
        _residency_feedback_buffer: gpu_resource.residency_feedback_buffer,
        _shared_residency_page_table_buffer: gpu_resource.shared_residency_page_table_buffer,
        _shared_residency_compacted_feedback_buffer: gpu_resource.shared_residency_compacted_feedback_buffer,
        _shared_residency_eviction_plan_buffer: gpu_resource.shared_residency_eviction_plan_buffer,
        _shared_residency_budget_buffer: gpu_resource.shared_residency_budget_buffer,
        _lumen_surface_card_buffer: gpu_resource.lumen_surface_card_buffer,
        _lumen_surface_cache_feedback_buffer: gpu_resource.lumen_surface_cache_feedback_buffer,
        _lumen_screen_probe_buffer: gpu_resource.lumen_screen_probe_buffer,
        _lumen_radiance_cache_buffer: gpu_resource.lumen_radiance_cache_buffer,
        virtual_shadow_map_page_table_buffer: gpu_resource.virtual_shadow_map_page_table_buffer,
        virtual_shadow_map_page_flags_buffer: gpu_resource.virtual_shadow_map_page_flags_buffer,
        virtual_shadow_map_page_request_buffer: gpu_resource.virtual_shadow_map_page_request_buffer,
        virtual_shadow_map_physical_page_metadata_buffer: gpu_resource.virtual_shadow_map_physical_page_metadata_buffer,
        virtual_shadow_map_projection_buffer: gpu_resource.virtual_shadow_map_projection_buffer,
        virtual_shadow_map_mark_params_buffer: gpu_resource.virtual_shadow_map_mark_params_buffer,
        _virtual_shadow_map_physical_page_pool_texture: gpu_resource.virtual_shadow_map_physical_page_pool_texture,
        virtual_shadow_map_physical_page_pool_view: gpu_resource.virtual_shadow_map_physical_page_pool_view,
        _virtual_shadow_map_projection_texture: gpu_resource.virtual_shadow_map_projection_texture,
        virtual_shadow_map_projection_view: gpu_resource.virtual_shadow_map_projection_view,
        virtual_shadow_map_cache_invalidation_buffer: gpu_resource.virtual_shadow_map_cache_invalidation_buffer,
        virtual_shadow_map_projection_params_buffer: gpu_resource.virtual_shadow_map_projection_params_buffer,
        virtual_shadow_map_mark_bind_group_layout,
        virtual_shadow_map_mark_pipeline,
        virtual_shadow_map_physical_page_bind_group_layout,
        virtual_shadow_map_physical_page_pipeline,
        virtual_shadow_map_projection_bind_group_layout,
        virtual_shadow_map_projection_pipeline,
        spectral_ocean_bind_group_layout,
        spectral_ocean_pipeline,
        single_layer_water_bind_group_layout,
        single_layer_water_pipeline,
        single_layer_water_present_bind_group_layout,
        single_layer_water_present_pipeline,
        single_layer_water_present_sampler,
        bloom_present_bind_group_layout,
        bloom_present_pipeline,
        vertex_count: gpu_resource.vertex_count,
        index_count: gpu_resource.index_count,
        instance_count: gpu_resource.instance_count,
        index_format: gpu_resource.index_format,
        mesh_source: gpu_resource.mesh_source,
        mesh_bounds: gpu_resource.mesh_bounds,
        _selected_tile_id: gpu_resource.selected_tile_id,
        _indirect_args_hash: gpu_resource.indirect_args_hash,
        _meshlet_cluster_hash: gpu_resource.meshlet_cluster_hash,
        _meshlet_cluster_count: gpu_resource.meshlet_cluster_count,
        _meshlet_cluster_cull_param_hash: gpu_resource.meshlet_cluster_cull_param_hash,
        _meshlet_cluster_cull_feedback_hash: gpu_resource.meshlet_cluster_cull_feedback_hash,
        _material_bin_hash: gpu_resource.material_bin_hash,
        _residency_feedback_hash: gpu_resource.residency_feedback_hash,
        _shared_residency_page_table_hash: gpu_resource.shared_residency_page_table_hash,
        _shared_residency_compacted_feedback_hash: gpu_resource.shared_residency_compacted_feedback_hash,
        _shared_residency_eviction_plan_hash: gpu_resource.shared_residency_eviction_plan_hash,
        _lumen_surface_card_hash: gpu_resource.lumen_surface_card_hash,
        _lumen_surface_cache_feedback_hash: gpu_resource.lumen_surface_cache_feedback_hash,
        _lumen_screen_probe_hash: gpu_resource.lumen_screen_probe_hash,
        _lumen_radiance_cache_hash: gpu_resource.lumen_radiance_cache_hash,
        _virtual_shadow_map_page_table_hash: gpu_resource.virtual_shadow_map_page_table_hash,
        _virtual_shadow_map_page_request_hash: gpu_resource.virtual_shadow_map_page_request_hash,
        _virtual_shadow_map_projection_hash: gpu_resource.virtual_shadow_map_projection_hash,
        _virtual_shadow_map_physical_pool_hash: gpu_resource.virtual_shadow_map_physical_pool_hash,
        _virtual_shadow_map_cache_invalidation_hash: gpu_resource.virtual_shadow_map_cache_invalidation_hash,
        scene_mesh_hash,
        scene_graph_hash,
        instance_buffer_hash,
        depth_format: wgpu::TextureFormat::Depth32Float,
        shader_source_hash,
        render_pipeline_hash,
    })
}

#[cfg(target_os = "windows")]
fn banger_sky_atmosphere_present_wgsl() -> &'static str {
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
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let aspect = max(frame.viewport.x / max(frame.viewport.y, 1.0), 0.1);
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let view_ray = normalize(vec3<f32>(ndc.x * aspect, ndc.y * 0.72 + 0.16, 1.0));
    let sun_phase = frame.time_seconds * 0.035 + 0.55;
    let sun_dir = normalize(vec3<f32>(sin(sun_phase) * 0.28, 0.38 + 0.18 * sin(sun_phase * 0.37), 0.88));
    let horizon = smoothstep(-0.22, 0.34, view_ray.y);
    let rayleigh = pow(max(view_ray.y * 0.5 + 0.5, 0.0), 1.65);
    let mie = pow(max(dot(view_ray, sun_dir), 0.0), 28.0);
    let sun_disk = smoothstep(0.9985, 0.9998, dot(view_ray, sun_dir));
    let lower = vec3<f32>(0.28, 0.36, 0.45);
    let upper = vec3<f32>(0.022, 0.047, 0.105);
    let rayleigh_blue = vec3<f32>(0.12, 0.30, 0.62) * rayleigh * 0.32;
    let mie_warmth = vec3<f32>(1.0, 0.54, 0.24) * mie * 0.32;
    let sunset_band = vec3<f32>(0.92, 0.38, 0.16) * (1.0 - horizon) * 0.24;
    let color = mix(lower + sunset_band, upper + rayleigh_blue, horizon) + mie_warmth + vec3<f32>(1.0, 0.74, 0.38) * sun_disk;
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#
}

#[cfg(target_os = "windows")]
fn banger_screen_space_ambient_occlusion_present_wgsl() -> &'static str {
    r#"
@group(0) @binding(0)
var source_depth: texture_depth_2d;
@group(0) @binding(1)
var gbuffer_normal: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

fn banger_ssao_sample(pixel: vec2<i32>, offset: vec2<i32>, extent: vec2<i32>, center_depth: f32, center_normal: vec3<f32>) -> f32 {
    let sample_pixel = clamp(pixel + offset, vec2<i32>(0), extent - vec2<i32>(1));
    let sample_depth = textureLoad(source_depth, sample_pixel, 0);
    let sample_normal = normalize(textureLoad(gbuffer_normal, sample_pixel, 0).xyz * 2.0 - vec3<f32>(1.0));
    let closer = smoothstep(0.0007, 0.019, center_depth - sample_depth);
    let normal_fold = smoothstep(0.18, 0.88, 1.0 - dot(center_normal, sample_normal));
    let radius_falloff = 1.0 / (1.0 + length(vec2<f32>(offset)) * 0.18);
    return (closer * 0.78 + normal_fold * 0.22) * radius_falloff;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let extent_u = textureDimensions(source_depth);
    let extent = vec2<i32>(i32(extent_u.x), i32(extent_u.y));
    let pixel = vec2<i32>(i32(in.position.x), i32(in.position.y));
    let center_depth = textureLoad(source_depth, pixel, 0);
    if (center_depth >= 0.999) {
        discard;
    }
    let center_normal = normalize(textureLoad(gbuffer_normal, pixel, 0).xyz * 2.0 - vec3<f32>(1.0));
    var occlusion = 0.0;
    occlusion += banger_ssao_sample(pixel, vec2<i32>(1, 0), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(-1, 0), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(0, 1), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(0, -1), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(3, 2), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(-3, 2), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(2, -3), extent, center_depth, center_normal);
    occlusion += banger_ssao_sample(pixel, vec2<i32>(-2, -3), extent, center_depth, center_normal);
    let ao_alpha = clamp((occlusion / 8.0) * 0.62, 0.0, 0.42);
    return vec4<f32>(0.0, 0.0, 0.0, ao_alpha);
}
"#
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
@group(0) @binding(1)
var maps_base_color: texture_2d<f32>;
@group(0) @binding(2)
var maps_base_sampler: sampler;
@group(0) @binding(3)
var virtual_shadow_projection: texture_2d<u32>;

struct BangerMaterialRecord {
    base_color_factor: vec4<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
    base_color_texture: u32,
    material_index: u32,
    normal_texture: u32,
    normal_scale: f32,
    metallic_roughness_texture: u32,
    occlusion_texture: u32,
    occlusion_strength: f32,
    emissive_texture: u32,
    pad0: u32,
    pad1: u32,
    emissive_factor: vec4<f32>,
};

@group(0) @binding(4)
var<storage, read> material_records: array<BangerMaterialRecord>;
@group(0) @binding(5)
var maps_normal_texture: texture_2d<f32>;
@group(0) @binding(6)
var maps_metallic_roughness_texture: texture_2d<f32>;
@group(0) @binding(7)
var maps_occlusion_texture: texture_2d<f32>;
@group(0) @binding(8)
var maps_emissive_texture: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal_hint: vec3<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) material_kind: f32,
    @location(5) material_slot: f32,
    @location(6) tangent_hint: vec4<f32>,
};

struct FragmentOut {
    @location(0) scene_color: vec4<f32>,
    @location(1) gbuffer_albedo: vec4<f32>,
    @location(2) gbuffer_normal: vec4<f32>,
    @location(3) gbuffer_material: vec4<f32>,
    @location(4) gbuffer_emissive: vec4<f32>,
};

fn banger_filmic_tonemap(color: vec3<f32>) -> vec3<f32> {
    let x = max(color - vec3<f32>(0.004), vec3<f32>(0.0));
    return (x * (6.2 * x + vec3<f32>(0.5))) / (x * (6.2 * x + vec3<f32>(1.7)) + vec3<f32>(0.06));
}

fn banger_contact_ambient_occlusion(normal: vec3<f32>, world_pos: vec3<f32>, material_kind: f32) -> f32 {
    let upward_access = clamp(normal.y * 0.5 + 0.5, 0.0, 1.0);
    let slope_cavity = pow(1.0 - upward_access, 1.7);
    let low_contact = exp(-abs(world_pos.y) * 0.08) * smoothstep(0.15, 0.85, 1.0 - abs(normal.y));
    let material_cavity = smoothstep(1.5, 3.5, material_kind) * 0.08;
    let micro_noise = fract(sin(dot(world_pos, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
    return clamp(1.0 - slope_cavity * 0.24 - low_contact * 0.18 - material_cavity - micro_noise * 0.035, 0.54, 1.0);
}

fn banger_fresnel_schlick(f0: vec3<f32>, voh: f32) -> vec3<f32> {
    let f = pow(1.0 - clamp(voh, 0.0, 1.0), 5.0);
    return f0 + (vec3<f32>(1.0) - f0) * f;
}

fn banger_distribution_ggx(noh: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = max(noh * noh * (a2 - 1.0) + 1.0, 0.0008);
    return a2 / (3.14159265 * denom * denom);
}

fn banger_visibility_smith_ggx_fast(nov: f32, nol: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let ggx_v = nol * (nov * (1.0 - a) + a);
    let ggx_l = nov * (nol * (1.0 - a) + a);
    return 0.5 / max(ggx_v + ggx_l, 0.0008);
}

fn banger_microfacet_brdf(base_color: vec3<f32>, normal: vec3<f32>, view_dir: vec3<f32>, light_dir: vec3<f32>, roughness: f32, metallic: f32) -> vec3<f32> {
    let half_dir = normalize(view_dir + light_dir);
    let nol = clamp(dot(normal, light_dir), 0.0, 1.0);
    let nov = clamp(dot(normal, view_dir), 0.0, 1.0);
    let noh = clamp(dot(normal, half_dir), 0.0, 1.0);
    let voh = clamp(dot(view_dir, half_dir), 0.0, 1.0);
    let f0 = mix(vec3<f32>(0.04), base_color, metallic);
    let fresnel = banger_fresnel_schlick(f0, voh);
    let specular = banger_distribution_ggx(noh, roughness) * banger_visibility_smith_ggx_fast(nov, nol, roughness) * fresnel;
    let diffuse = base_color * (vec3<f32>(1.0) - fresnel) * (1.0 - metallic) * 0.3183099;
    return (diffuse + specular) * nol;
}

fn banger_environment_radiance(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    let horizon = clamp(direction.y * 0.5 + 0.5, 0.0, 1.0);
    let zenith = vec3<f32>(0.045, 0.08, 0.15);
    let horizon_color = vec3<f32>(0.46, 0.58, 0.68);
    let ground = vec3<f32>(0.045, 0.038, 0.032);
    let sky_probe = mix(ground, mix(horizon_color, zenith, horizon * horizon), smoothstep(0.0, 0.22, horizon));
    let sun_dir = normalize(vec3<f32>(0.42, 0.72, 0.48));
    let solar_disc = pow(max(dot(direction, sun_dir), 0.0), mix(96.0, 10.0, roughness));
    let solar_glow = pow(max(dot(direction, sun_dir), 0.0), mix(12.0, 3.0, roughness));
    return sky_probe + vec3<f32>(1.0, 0.78, 0.46) * solar_disc * (1.2 - roughness) + vec3<f32>(0.28, 0.18, 0.08) * solar_glow;
}

fn banger_environment_brdf_approx(no_v: f32, roughness: f32, f0: vec3<f32>) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, pow(2.0, -9.28 * no_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + vec3<f32>(ab.y);
}

fn banger_image_based_lighting(base_color: vec3<f32>, normal: vec3<f32>, view_dir: vec3<f32>, roughness: f32, metallic: f32, indirect_ao: f32) -> vec3<f32> {
    let surface_color = clamp(base_color, vec3<f32>(0.0), vec3<f32>(1.0));
    let no_v = clamp(dot(normal, view_dir), 0.0, 1.0);
    let f0 = mix(vec3<f32>(0.04), surface_color, metallic);
    let diffuse_probe = banger_environment_radiance(normalize(normal + vec3<f32>(0.0, 0.32, 0.0)), 1.0);
    let diffuse = surface_color * (1.0 - metallic) * diffuse_probe * (0.28 + 0.42 * indirect_ao);
    let reflection = normalize(reflect(-view_dir, normal));
    let blurred_reflection = normalize(mix(reflection, normal, roughness * roughness * 0.72));
    let specular_probe = banger_environment_radiance(blurred_reflection, roughness);
    let specular_brdf = banger_environment_brdf_approx(no_v, roughness, f0);
    let specular_occlusion = clamp(indirect_ao + no_v * (1.0 - roughness) * 0.35, 0.0, 1.0);
    return diffuse + specular_probe * specular_brdf * specular_occlusion;
}

fn banger_transform_normal(model: mat4x4<f32>, normal: vec3<f32>) -> vec3<f32> {
    let a = model[0].xyz;
    let b = model[1].xyz;
    let c = model[2].xyz;
    let adjugate_normal =
        normal.x * cross(b, c) +
        normal.y * cross(c, a) +
        normal.z * cross(a, b);
    let handedness = select(1.0, -1.0, dot(a, cross(b, c)) < 0.0);
    return normalize(adjugate_normal * handedness);
}

fn banger_transform_tangent(model: mat4x4<f32>, tangent: vec4<f32>, normal: vec3<f32>) -> vec4<f32> {
    let transformed = normalize((model * vec4<f32>(tangent.xyz, 0.0)).xyz);
    let orthogonal = normalize(transformed - normal * dot(transformed, normal));
    return vec4<f32>(orthogonal, tangent.w);
}

fn banger_tangent_space_detail_normal(normal: vec3<f32>, tangent: vec4<f32>, uv: vec2<f32>, material_kind: f32, roughness: f32, normal_scale: f32, normal_texture: u32) -> vec3<f32> {
    let t = normalize(tangent.xyz);
    let b = normalize(cross(normal, t) * tangent.w);
    let has_normal_map = normal_texture != 0xFFFFFFFFu;
    let normal_strength = clamp(normal_scale, 0.0, 4.0);
    let sampled = textureSample(maps_normal_texture, maps_base_sampler, fract(uv)).xyz * 2.0 - vec3<f32>(1.0);
    let sampled_world = normalize(t * sampled.x * normal_strength + b * sampled.y * normal_strength + normal * max(sampled.z, 0.05));
    let declared_normal_map = select(0.45, 1.0, has_normal_map);
    let detail_strength = smoothstep(0.18, 0.86, roughness) * (0.025 + 0.025 * smoothstep(1.5, 4.0, material_kind)) * normal_strength * declared_normal_map;
    let wave_x = sin(uv.x * 74.0 + uv.y * 19.0 + frame.time_seconds * 0.04);
    let wave_y = cos(uv.y * 61.0 - uv.x * 23.0 + frame.time_seconds * 0.03);
    let procedural = normalize(normal + t * wave_x * detail_strength + b * wave_y * detail_strength);
    return normalize(mix(procedural, sampled_world, select(0.0, 0.72, has_normal_map)));
}

fn banger_material_record_for_kind(material_kind: f32) -> BangerMaterialRecord {
    let material_count = arrayLength(&material_records);
    let material_index = min(u32(max(material_kind, 0.0)), material_count - 1u);
    return material_records[material_index];
}

fn banger_virtual_shadow_visibility(world_pos: vec3<f32>, normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    let projection_extent = vec2<i32>(textureDimensions(virtual_shadow_projection));
    let shadow_uv = fract(world_pos.xz * 0.018 + vec2<f32>(0.31, 0.57));
    let shadow_pixel = clamp(vec2<i32>(shadow_uv * vec2<f32>(projection_extent)), vec2<i32>(0), projection_extent - vec2<i32>(1));
    let packed_projection = textureLoad(virtual_shadow_projection, shadow_pixel, 0).r;
    let projected_page = f32(packed_projection & 255u) / 255.0;
    let receiver_bias = smoothstep(0.08, 0.62, dot(normal, light_dir));
    let contact_band = smoothstep(0.18, 0.84, projected_page);
    let temporal_dither = fract(sin(dot(world_pos.xz, vec2<f32>(43.13, 17.71))) * 4096.0);
    let cached_shadow = smoothstep(0.22, 0.72, contact_band + temporal_dither * 0.08);
    return mix(0.58, 1.0, max(cached_shadow, receiver_bias * 0.72));
}

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) material_slot: f32,
    @location(4) normal: vec3<f32>,
    @location(5) tangent: vec4<f32>,
    @location(6) model_0: vec4<f32>,
    @location(7) model_1: vec4<f32>,
    @location(8) model_2: vec4<f32>,
    @location(9) model_3: vec4<f32>,
    @location(10) instance_tint: vec4<f32>,
) -> VertexOut {
    let model = mat4x4<f32>(model_0, model_1, model_2, model_3);
    let world = model * vec4<f32>(position, 1.0);
    let world_normal = banger_transform_normal(model, normal);
    var out: VertexOut;
    out.position = frame.view_proj * world;
    out.color = color * instance_tint.rgb;
    out.uv = uv;
    out.normal_hint = world_normal;
    out.world_pos = world.xyz;
    out.material_kind = instance_tint.a;
    out.material_slot = material_slot;
    out.tangent_hint = banger_transform_tangent(model, tangent, world_normal);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> FragmentOut {
    let sun_dir = normalize(vec3<f32>(0.42, 0.72, 0.48));
    let view_dir = normalize(vec3<f32>(-in.world_pos.x * 0.012, 0.28, 1.0 - in.world_pos.z * 0.006));
    let view_fade = clamp(length(in.world_pos.xz) / 58.0, 0.0, 1.0);
    let material_record = banger_material_record_for_kind(in.material_slot);
    let has_metallic_roughness_map = material_record.metallic_roughness_texture != 0xFFFFFFFFu;
    let metallic_roughness_sample = textureSample(maps_metallic_roughness_texture, maps_base_sampler, fract(in.uv)).rgb;
    let sampled_roughness = select(1.0, metallic_roughness_sample.g, has_metallic_roughness_map);
    let sampled_metallic = select(1.0, metallic_roughness_sample.b, has_metallic_roughness_map);
    let material_roughness = clamp(material_record.roughness_factor * sampled_roughness * mix(1.0, 0.45, smoothstep(2.4, 3.4, in.material_kind)), 0.045, 1.0);
    let normal = banger_tangent_space_detail_normal(normalize(in.normal_hint), in.tangent_hint, in.uv, in.material_kind, material_roughness, material_record.normal_scale, material_record.normal_texture);
    let lambert = clamp(dot(normal, sun_dir) * 0.62 + 0.38, 0.18, 1.0);
    let sky = mix(vec3<f32>(0.02, 0.035, 0.065), vec3<f32>(0.95, 0.48, 0.18), clamp(in.world_pos.y * 0.04 + 0.35, 0.0, 1.0));
    let bounced = vec3<f32>(0.05, 0.13, 0.16) * (1.0 - clamp(normal.y, -0.15, 0.85));
    let water_glint = smoothstep(2.5, 3.5, in.material_kind) * pow(max(dot(reflect(-sun_dir, normal), view_dir), 0.0), 18.0);
    let voxel_heat = 0.08 * sin(in.world_pos.x * 0.35 + in.world_pos.z * 0.21 + frame.time_seconds);
    let sampled = textureSample(maps_base_color, maps_base_sampler, fract(in.uv)).rgb;
    let maps_texture_weight = smoothstep(1.4, 2.1, in.material_kind);
    let pbr_base_factor = clamp(material_record.base_color_factor.rgb, vec3<f32>(0.0), vec3<f32>(8.0));
    let alpha_factor = clamp(material_record.base_color_factor.a, 0.0, 1.0);
    let base_color = mix(in.color, sampled * in.color, maps_texture_weight) * pbr_base_factor;
    let contact_ao = banger_contact_ambient_occlusion(normal, in.world_pos, in.material_kind);
    let has_occlusion_map = material_record.occlusion_texture != 0xFFFFFFFFu;
    let occlusion_sample = textureSample(maps_occlusion_texture, maps_base_sampler, fract(in.uv)).r;
    let material_occlusion = select(1.0, clamp(1.0 + clamp(material_record.occlusion_strength, 0.0, 1.0) * (occlusion_sample - 1.0), 0.0, 1.0), has_occlusion_map);
    let indirect_ao = contact_ao * material_occlusion;
    let has_emissive_map = material_record.emissive_texture != 0xFFFFFFFFu;
    let emissive_sample = textureSample(maps_emissive_texture, maps_base_sampler, fract(in.uv)).rgb;
    let material_emissive = clamp(material_record.emissive_factor.rgb, vec3<f32>(0.0), vec3<f32>(8.0)) * select(vec3<f32>(1.0), emissive_sample, has_emissive_map);
    let shadow_visibility = banger_virtual_shadow_visibility(in.world_pos, normal, sun_dir);
    let material_metallic = clamp(material_record.metallic_factor * sampled_metallic + smoothstep(4.5, 6.0, in.material_kind) * 0.25, 0.0, 1.0);
    let pbr_direct = banger_microfacet_brdf(base_color, normal, view_dir, sun_dir, material_roughness, material_metallic);
    let pbr_indirect = banger_image_based_lighting(base_color, normal, view_dir, material_roughness, material_metallic, indirect_ao);
    let diffuse_light = lambert * contact_ao + 0.12 * indirect_ao;
    let lit = pbr_direct * (2.45 * contact_ao * shadow_visibility) + pbr_indirect + base_color * diffuse_light * shadow_visibility + bounced * (0.55 + 0.45 * indirect_ao) + vec3<f32>(1.0, 0.72, 0.38) * water_glint + voxel_heat * indirect_ao;
    let fog_color = vec3<f32>(0.11, 0.16, 0.22) + sky * 0.18;
    let fogged = mix(lit, fog_color, smoothstep(0.35, 1.0, view_fade));
    let exposure = 1.08 + 0.04 * sin(frame.time_seconds * 0.19);
    let graded = banger_filmic_tonemap(fogged * exposure);
    let contrast = mix(vec3<f32>(0.5), graded, vec3<f32>(1.08));
    var out: FragmentOut;
    out.scene_color = vec4<f32>(max(contrast, vec3<f32>(0.015, 0.018, 0.026)), alpha_factor);
    out.gbuffer_albedo = vec4<f32>(clamp(base_color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    out.gbuffer_normal = vec4<f32>(normal * 0.5 + vec3<f32>(0.5), 1.0);
    out.gbuffer_material = vec4<f32>(in.material_kind, material_roughness, view_fade, water_glint * shadow_visibility);
    out.gbuffer_emissive = vec4<f32>(sky * 0.12 + material_emissive + vec3<f32>(water_glint * 0.35 + max(alpha_factor - 1.0, 0.0)), indirect_ao);
    return out;
}
"#
}

#[cfg(target_os = "windows")]
fn banger_single_layer_water_composite_compute_wgsl() -> &'static str {
    r#"
struct SingleLayerWaterParams {
    extent: vec4<u32>,
    optical: vec4<f32>,
    scattering: vec4<f32>,
};

@group(0) @binding(0)
var gbuffer_albedo: texture_2d<f32>;
@group(0) @binding(1)
var gbuffer_normal: texture_2d<f32>;
@group(0) @binding(2)
var gbuffer_material: texture_2d<f32>;
@group(0) @binding(3)
var water_composite: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(4)
var refraction_mask: texture_storage_2d<r32float, write>;
@group(0) @binding(5)
var<uniform> params: SingleLayerWaterParams;
@group(0) @binding(6)
var<storage, read_write> water_tile_mask: array<atomic<u32>>;
@group(0) @binding(7)
var spectral_displacement: texture_2d<f32>;
@group(0) @binding(8)
var spectral_slope: texture_2d<f32>;

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.extent.x || gid.y >= params.extent.y) {
        return;
    }
    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let albedo = textureLoad(gbuffer_albedo, pixel, 0).rgb;
    let normal = normalize(textureLoad(gbuffer_normal, pixel, 0).xyz * 2.0 - vec3<f32>(1.0));
    let material = textureLoad(gbuffer_material, pixel, 0);
    let ocean_displacement = textureLoad(spectral_displacement, pixel, 0);
    let ocean_slope = textureLoad(spectral_slope, pixel, 0);
    let is_water = material.x > 2.5;
    let roughness_hint = clamp(1.0 - material.y, 0.0, 1.0);
    let spectral_height = ocean_displacement.y;
    let water_depth = params.optical.w + material.z * 8.0 + abs(spectral_height) * 0.18;
    let absorption = exp(-params.optical.rgb * water_depth);
    let spectral_normal = normalize(vec3<f32>(-ocean_slope.x, 1.0, -ocean_slope.y));
    let water_normal = normalize(mix(normal, spectral_normal, select(0.0, 0.58, is_water)));
    let fresnel = pow(1.0 - clamp(abs(water_normal.y), 0.0, 1.0), 5.0);
    let refract_strength = clamp(abs(water_normal.x) * 0.55 + abs(water_normal.z) * 0.55 + material.w + roughness_hint * 0.2, 0.0, 1.0);
    let scattered = params.scattering.rgb * (0.18 + fresnel * 0.62 + material.w * 0.35 + ocean_slope.z * 0.08);
    let water_color = albedo * absorption + scattered;
    let composite = select(albedo, water_color, is_water);
    textureStore(water_composite, pixel, vec4<f32>(clamp(composite, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
    textureStore(refraction_mask, pixel, vec4<f32>(select(0.0, refract_strength, is_water), 0.0, 0.0, 0.0));

    if (is_water) {
        let tile_x = gid.x / 8u;
        let tile_y = gid.y / 8u;
        let tile_index = tile_y * params.extent.z + tile_x;
        let word_index = tile_index / 32u;
        let bit_mask = 1u << (tile_index % 32u);
        atomicOr(&water_tile_mask[word_index], bit_mask);
    }
}
"#
}

#[cfg(target_os = "windows")]
fn banger_single_layer_water_present_wgsl() -> &'static str {
    r#"
@group(0) @binding(0)
var water_composite: texture_2d<f32>;
@group(0) @binding(1)
var refraction_mask: texture_2d<f32>;
@group(0) @binding(2)
var water_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(i32(in.position.x), i32(in.position.y));
    let mask = textureLoad(refraction_mask, pixel, 0).r;
    if (mask <= 0.015) {
        discard;
    }
    let color = textureSample(water_composite, water_sampler, in.uv).rgb;
    let sparkle = pow(clamp(mask, 0.0, 1.0), 2.0) * vec3<f32>(0.20, 0.36, 0.45);
    let alpha = smoothstep(0.02, 0.45, mask) * 0.66;
    return vec4<f32>(clamp(color + sparkle, vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
}
"#
}

#[cfg(target_os = "windows")]
fn banger_emissive_bloom_present_wgsl() -> &'static str {
    r#"
@group(0) @binding(0)
var gbuffer_emissive: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

fn banger_emissive_tap(pixel: vec2<i32>, offset: vec2<i32>, extent: vec2<i32>) -> vec3<f32> {
    let clamped_pixel = clamp(pixel + offset, vec2<i32>(0), extent - vec2<i32>(1));
    let emissive = textureLoad(gbuffer_emissive, clamped_pixel, 0);
    let luminance = dot(emissive.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let threshold = smoothstep(0.035, 0.18, luminance);
    let ao_gate = clamp(emissive.a, 0.35, 1.0);
    return emissive.rgb * threshold * ao_gate;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let extent_u = textureDimensions(gbuffer_emissive);
    let extent = vec2<i32>(i32(extent_u.x), i32(extent_u.y));
    let pixel = vec2<i32>(i32(in.position.x), i32(in.position.y));
    let near_glow =
        banger_emissive_tap(pixel, vec2<i32>(0, 0), extent) * 0.32 +
        banger_emissive_tap(pixel, vec2<i32>(2, 0), extent) * 0.14 +
        banger_emissive_tap(pixel, vec2<i32>(-2, 0), extent) * 0.14 +
        banger_emissive_tap(pixel, vec2<i32>(0, 2), extent) * 0.14 +
        banger_emissive_tap(pixel, vec2<i32>(0, -2), extent) * 0.14;
    let wide_glow =
        banger_emissive_tap(pixel, vec2<i32>(5, 5), extent) * 0.08 +
        banger_emissive_tap(pixel, vec2<i32>(-5, 5), extent) * 0.08 +
        banger_emissive_tap(pixel, vec2<i32>(5, -5), extent) * 0.08 +
        banger_emissive_tap(pixel, vec2<i32>(-5, -5), extent) * 0.08;
    let bloom = clamp((near_glow + wide_glow) * vec3<f32>(0.45, 0.58, 0.72), vec3<f32>(0.0), vec3<f32>(0.16));
    return vec4<f32>(bloom, 0.0);
}
"#
}

#[cfg(target_os = "windows")]
fn banger_spectral_ocean_compute_wgsl() -> &'static str {
    r#"
struct SpectralOceanParams {
    extent: vec4<u32>,
    fft: vec4<u32>,
    ocean: vec4<f32>,
    wind: vec4<f32>,
};

@group(0) @binding(0)
var spectral_state: texture_storage_2d<rgba16float, write>;
@group(0) @binding(1)
var spectral_displacement: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2)
var spectral_slope: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3)
var<uniform> params: SpectralOceanParams;

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.extent.x || gid.y >= params.extent.y) {
        return;
    }
    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5)) / vec2<f32>(f32(params.extent.x), f32(params.extent.y));
    let centered = uv - vec2<f32>(0.5);
    let domain_length = max(params.ocean.w, 1.0);
    let k = centered * (6.28318530718 * f32(params.extent.z)) / domain_length;
    let k_len = max(length(k), 0.0001);
    let k_dir = k / vec2<f32>(k_len);
    let wind_dir = normalize(params.wind.xy);
    let wind_alignment = max(dot(k_dir, wind_dir), 0.0);
    let wind_speed = max(params.ocean.z, 0.1);
    let largest_wave = wind_speed * wind_speed / max(params.ocean.y, 0.01);
    let phillips = params.wind.w * exp(-1.0 / max(k_len * k_len * largest_wave * largest_wave, 0.0001)) / pow(k_len, 4.0) * wind_alignment * wind_alignment;
    let omega = sqrt(params.ocean.y * k_len);
    let phase = omega * params.ocean.x + hash21(vec2<f32>(gid.xy)) * 6.28318530718;
    let amplitude = sqrt(max(phillips, 0.0));
    let height = sin(phase) * amplitude;
    let chop = params.wind.z * height;
    let displacement = vec3<f32>(k_dir.x * chop, height, k_dir.y * chop);
    let slope = vec3<f32>(cos(phase) * k.x * amplitude, cos(phase) * k.y * amplitude, amplitude);
    textureStore(spectral_state, pixel, vec4<f32>(amplitude, phase, omega, f32(params.fft.x)));
    textureStore(spectral_displacement, pixel, vec4<f32>(displacement, 1.0));
    textureStore(spectral_slope, pixel, vec4<f32>(slope, 1.0));
}
"#
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn banger_frame_uniform_bytes(
    time_seconds: f32,
    frame_index: u32,
    viewport_width: u32,
    viewport_height: u32,
) -> [u8; 80] {
    banger_frame_uniform_bytes_from_view_projection(
        banger_view_projection_matrix(time_seconds, viewport_width, viewport_height),
        time_seconds,
        frame_index,
        viewport_width,
        viewport_height,
    )
}

#[cfg(target_os = "windows")]
fn banger_frame_uniform_bytes_for_bounds(
    time_seconds: f32,
    frame_index: u32,
    viewport_width: u32,
    viewport_height: u32,
    bounds: BangerMeshBounds,
) -> [u8; 80] {
    banger_frame_uniform_bytes_from_view_projection(
        banger_view_projection_matrix_for_bounds(time_seconds, bounds, viewport_width, viewport_height),
        time_seconds,
        frame_index,
        viewport_width,
        viewport_height,
    )
}

#[cfg(target_os = "windows")]
fn banger_frame_uniform_bytes_from_view_projection(
    view_proj: [f32; 16],
    time_seconds: f32,
    frame_index: u32,
    viewport_width: u32,
    viewport_height: u32,
) -> [u8; 80] {
    let mut bytes = [0u8; 80];
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
fn banger_create_maps_texture_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture_index: u32,
    encoded_bytes: &[u8],
) -> BangerNativeTextureResource {
    let (width, height) = banger_maps_texture_resource_extent(encoded_bytes.len());
    let rgba = banger_maps_texture_rgba_from_encoded_bytes(encoded_bytes, width, height);
    let resource_hash = sha256_hex(
        format!(
            "banger-maps-texture-resource-v1:{texture_index}:{width}:{height}:rgba8unorm-srgb:{}",
            sha256_hex(encoded_bytes)
        )
        .as_bytes(),
    );
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-native-maps-texture-resource"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("banger-native-maps-texture-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    BangerNativeTextureResource {
        _texture: texture,
        view,
        sampler,
        _width: width,
        _height: height,
        _byte_count: rgba.len(),
        _resource_hash: resource_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_texture_resource_extent(byte_count: usize) -> (u32, u32) {
    let texel_count = byte_count.max(4).div_ceil(4) as u32;
    let width = 64u32;
    let height = texel_count.div_ceil(width).clamp(1, 4096);
    (width, height)
}

#[cfg(target_os = "windows")]
fn banger_maps_texture_rgba_from_encoded_bytes(encoded_bytes: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgba = vec![255u8; width as usize * height as usize * 4];
    if encoded_bytes.is_empty() {
        return rgba;
    }
    for texel_index in 0..(width as usize * height as usize) {
        let source = (texel_index * 3) % encoded_bytes.len();
        let r = encoded_bytes[source];
        let g = encoded_bytes[(source + 1) % encoded_bytes.len()];
        let b = encoded_bytes[(source + 2) % encoded_bytes.len()];
        let offset = texel_index * 4;
        rgba[offset] = r.max(12);
        rgba[offset + 1] = g.max(12);
        rgba[offset + 2] = b.max(12);
        rgba[offset + 3] = 255;
    }
    rgba
}

#[cfg(target_os = "windows")]
fn banger_fallback_texture_seed_bytes() -> Vec<u8> {
    b"forge-banger-fallback-texture-white-grid-v1".to_vec()
}

#[cfg(target_os = "windows")]
fn banger_maps_texture_resource_manifest_bytes(texture_staging_bytes: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(texture_staging_bytes.len().max(1) * 32);
    for (texture_index, encoded_bytes) in texture_staging_bytes.iter().enumerate() {
        if encoded_bytes.is_empty() {
            continue;
        }
        let (width, height) = banger_maps_texture_resource_extent(encoded_bytes.len());
        let texture_hash = sha256_hex(encoded_bytes);
        for value in [
            0x4D_54_45_58u32, // MTEX
            1,
            texture_index as u32,
            width,
            height,
            encoded_bytes.len().min(u32::MAX as usize) as u32,
            banger_hash_prefix_u32(&texture_hash),
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    if bytes.is_empty() {
        bytes.resize(32, 0);
    }
    bytes
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BangerRenderIndexFormat {
    Uint16,
    Uint32,
}

#[cfg(target_os = "windows")]
impl BangerRenderIndexFormat {
    fn stride_bytes(self) -> usize {
        match self {
            BangerRenderIndexFormat::Uint16 => 2,
            BangerRenderIndexFormat::Uint32 => 4,
        }
    }

    fn wgpu(self) -> wgpu::IndexFormat {
        match self {
            BangerRenderIndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
            BangerRenderIndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
        }
    }

    fn label(self) -> &'static str {
        match self {
            BangerRenderIndexFormat::Uint16 => "uint16",
            BangerRenderIndexFormat::Uint32 => "uint32",
        }
    }
}

#[cfg(target_os = "windows")]
struct BangerRenderMeshBytes {
    vertex_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    index_format: BangerRenderIndexFormat,
    instance_bytes: Vec<u8>,
    bounds: BangerMeshBounds,
    source: &'static str,
}

#[cfg(target_os = "windows")]
fn banger_native_scene_gpu_resource_from_mesh_bytes(
    device: &wgpu::Device,
    mesh: BangerRenderMeshBytes,
    material_bytes: Option<Vec<u8>>,
    texture_staging_bytes: Vec<Vec<u8>>,
    selected_tile_id: Option<String>,
    queue: &wgpu::Queue,
) -> BangerNativeSceneGpuResource {
    let BangerRenderMeshBytes {
        vertex_bytes,
        index_bytes,
        index_format,
        instance_bytes,
        bounds,
        source,
    } = mesh;
    let vertex_hash = sha256_hex(&vertex_bytes);
    let index_hash = sha256_hex(&index_bytes);
    let instance_hash = sha256_hex(&instance_bytes);
    let material_bytes = material_bytes.filter(|bytes| !bytes.is_empty());
    let effective_material_bytes = material_bytes
        .clone()
        .unwrap_or_else(banger_default_material_resource_bytes);
    let material_hash = sha256_hex(&effective_material_bytes);
    let material_byte_count = effective_material_bytes.len();
    let texture_byte_count = texture_staging_bytes.iter().map(Vec::len).sum::<usize>();
    let texture_hash = sha256_hex(
        texture_staging_bytes
            .iter()
            .map(|bytes| sha256_hex(bytes))
            .collect::<String>()
            .as_bytes(),
    );
    let vertex_count = (vertex_bytes.len() / BANGER_RENDER_VERTEX_STRIDE_BYTES) as u32;
    let index_count = (index_bytes.len() / index_format.stride_bytes()) as u32;
    let instance_count = (instance_bytes.len() / 80) as u32;
    let indirect_args_bytes = banger_indexed_indirect_args_bytes(index_count, instance_count);
    let culled_indirect_seed_args_bytes = banger_indexed_indirect_args_bytes(index_count, 0);
    let indirect_args_hash = sha256_hex(&indirect_args_bytes);
    let meshlet_cluster_bytes =
        banger_meshlet_cluster_metadata_bytes(&vertex_bytes, &index_bytes, index_format, source);
    let meshlet_cluster_hash = sha256_hex(&meshlet_cluster_bytes);
    let meshlet_cluster_count =
        (meshlet_cluster_bytes.len() / BANGER_MESHLET_CLUSTER_METADATA_STRIDE) as u32;
    let meshlet_cluster_cull_param_bytes =
        banger_meshlet_cluster_cull_params_bytes(meshlet_cluster_count, 1, index_count, instance_count);
    let meshlet_cluster_cull_param_hash = sha256_hex(&meshlet_cluster_cull_param_bytes);
    let meshlet_cluster_cull_feedback_bytes = banger_meshlet_cluster_cull_feedback_bytes();
    let meshlet_cluster_cull_feedback_hash = sha256_hex(&meshlet_cluster_cull_feedback_bytes);
    let texture_resource_manifest_bytes = banger_maps_texture_resource_manifest_bytes(&texture_staging_bytes);
    let material_bin_bytes = banger_material_bin_bytes(
        &meshlet_cluster_bytes,
        Some(&effective_material_bytes),
        &texture_resource_manifest_bytes,
    );
    let material_bin_hash = sha256_hex(&material_bin_bytes);
    let residency_feedback_bytes = banger_maps_residency_feedback_bytes(
        selected_tile_id.as_deref(),
        source,
        vertex_count,
        index_count,
        instance_count,
        &vertex_hash,
        &index_hash,
        &material_hash,
        &texture_hash,
    );
    let residency_feedback_hash = sha256_hex(&residency_feedback_bytes);
    let shared_residency_page_table_bytes = banger_shared_residency_page_table_bytes(
        source,
        selected_tile_id.as_deref(),
        vertex_bytes.len() + index_bytes.len() + meshlet_cluster_bytes.len(),
        material_byte_count,
        texture_byte_count,
        residency_feedback_bytes.len() + meshlet_cluster_cull_feedback_bytes.len(),
    );
    let shared_residency_page_table_hash = sha256_hex(&shared_residency_page_table_bytes);
    let shared_residency_compacted_feedback_bytes =
        banger_shared_residency_compacted_feedback_bytes(&shared_residency_page_table_bytes);
    let shared_residency_compacted_feedback_hash =
        sha256_hex(&shared_residency_compacted_feedback_bytes);
    let shared_residency_eviction_plan_bytes =
        banger_shared_residency_eviction_plan_bytes(&shared_residency_page_table_bytes, 512 * 1024 * 1024);
    let shared_residency_eviction_plan_hash = sha256_hex(&shared_residency_eviction_plan_bytes);
    let shared_residency_budget_bytes = banger_shared_residency_budget_bytes(
        shared_residency_page_table_bytes.len() / BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE,
        512 * 1024 * 1024,
        vertex_bytes.len()
            + index_bytes.len()
            + meshlet_cluster_bytes.len()
            + material_byte_count
            + texture_byte_count
            + residency_feedback_bytes.len()
            + meshlet_cluster_cull_feedback_bytes.len(),
    );
    let lumen_surface_card_bytes = banger_lumen_surface_card_bytes(&meshlet_cluster_bytes);
    let lumen_surface_cache_feedback_bytes =
        banger_lumen_surface_cache_feedback_bytes(&lumen_surface_card_bytes);
    let lumen_screen_probe_bytes = banger_lumen_screen_probe_bytes(meshlet_cluster_count);
    let lumen_radiance_cache_bytes =
        banger_lumen_radiance_cache_bytes(&lumen_surface_card_bytes, &lumen_screen_probe_bytes);
    let lumen_surface_card_hash = sha256_hex(&lumen_surface_card_bytes);
    let lumen_surface_cache_feedback_hash = sha256_hex(&lumen_surface_cache_feedback_bytes);
    let lumen_screen_probe_hash = sha256_hex(&lumen_screen_probe_bytes);
    let lumen_radiance_cache_hash = sha256_hex(&lumen_radiance_cache_bytes);
    let vsm_page_table_bytes = banger_virtual_shadow_map_page_table_bytes(meshlet_cluster_count, 1);
    let vsm_page_flags_bytes = banger_virtual_shadow_map_page_flags_bytes(meshlet_cluster_count);
    let vsm_page_request_bytes = banger_virtual_shadow_map_page_request_bytes(meshlet_cluster_count);
    let vsm_physical_page_metadata_bytes =
        banger_virtual_shadow_map_physical_page_metadata_bytes(meshlet_cluster_count, 1);
    let vsm_projection_bytes = banger_virtual_shadow_map_projection_bytes(1);
    let vsm_mark_params_bytes = banger_virtual_shadow_map_mark_params_bytes(meshlet_cluster_count, 1);
    let vsm_physical_pool_desc = banger_virtual_shadow_map_physical_pool_desc(meshlet_cluster_count);
    let vsm_cache_invalidation_bytes =
        banger_virtual_shadow_map_cache_invalidation_bytes(meshlet_cluster_count, &meshlet_cluster_hash);
    let vsm_projection_params_bytes =
        banger_virtual_shadow_map_projection_params_bytes(meshlet_cluster_count, vsm_physical_pool_desc);
    let virtual_shadow_map_page_table_hash = sha256_hex(&vsm_page_table_bytes);
    let virtual_shadow_map_page_request_hash = sha256_hex(&vsm_page_request_bytes);
    let virtual_shadow_map_projection_hash = sha256_hex(&vsm_projection_bytes);
    let virtual_shadow_map_physical_pool_hash = sha256_hex(
        format!(
            "vsm_pool:{}:{}:{}:{}",
            vsm_physical_pool_desc.pages_x,
            vsm_physical_pool_desc.pages_y,
            vsm_physical_pool_desc.layers,
            vsm_physical_pool_desc.page_count
        )
        .as_bytes(),
    );
    let virtual_shadow_map_cache_invalidation_hash = sha256_hex(&vsm_cache_invalidation_bytes);
    let vertex_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-scene-vertex-buffer",
        wgpu::BufferUsages::VERTEX,
        &vertex_bytes,
    );
    let index_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-scene-index-buffer",
        wgpu::BufferUsages::INDEX,
        &index_bytes,
    );
    let indirect_draw_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-scene-indexed-indirect-draw-args",
        wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
        &indirect_args_bytes,
    );
    let meshlet_culled_indirect_draw_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-meshlet-culled-indexed-indirect-draw-args",
        wgpu::BufferUsages::INDIRECT
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        &indirect_args_bytes,
    );
    let meshlet_culled_indirect_seed_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-meshlet-culled-indexed-indirect-seed-args",
        wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        &culled_indirect_seed_args_bytes,
    );
    let meshlet_cluster_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-meshlet-cluster-metadata-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &meshlet_cluster_bytes,
    );
    let visible_meshlet_cluster_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-visible-meshlet-cluster-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vec![0u8; meshlet_cluster_bytes.len().max(BANGER_MESHLET_CLUSTER_METADATA_STRIDE)],
    );
    let meshlet_cull_feedback_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-meshlet-cluster-cull-feedback-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &meshlet_cluster_cull_feedback_bytes,
    );
    let meshlet_cull_param_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-meshlet-cluster-cull-param-buffer",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &meshlet_cluster_cull_param_bytes,
    );
    let instance_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-scene-instance-buffer",
        wgpu::BufferUsages::VERTEX,
        &instance_bytes,
    );
    let material_buffer = Some(banger_create_mapped_buffer(
        device,
        "banger-native-maps-material-resource-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        &effective_material_bytes,
    ));
    let material_bin_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-material-bin-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &material_bin_bytes,
    );
    let texture_staging_buffers = texture_staging_bytes
        .iter()
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| {
            banger_create_mapped_buffer(
                device,
                "banger-native-maps-texture-staging-resource-buffer",
                wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                bytes,
            )
        })
        .collect::<Vec<_>>();
    let effective_texture_staging_bytes = if texture_staging_bytes.iter().any(|bytes| !bytes.is_empty()) {
        texture_staging_bytes
    } else {
        vec![banger_fallback_texture_seed_bytes()]
    };
    let texture_resources = effective_texture_staging_bytes
        .iter()
        .filter(|bytes| !bytes.is_empty())
        .enumerate()
        .map(|(texture_index, bytes)| {
            banger_create_maps_texture_resource(device, queue, texture_index as u32, bytes)
        })
        .collect::<Vec<_>>();
    let normal_texture_resource_index = banger_first_material_normal_texture_index(&effective_material_bytes)
        .map(|index| index.min(texture_resources.len().saturating_sub(1)) as u32)
        .unwrap_or(0);
    let metallic_roughness_texture_resource_index = banger_first_material_metallic_roughness_texture_index(&effective_material_bytes)
        .map(|index| index.min(texture_resources.len().saturating_sub(1)) as u32)
        .unwrap_or(0);
    let occlusion_texture_resource_index = banger_first_material_occlusion_texture_index(&effective_material_bytes)
        .map(|index| index.min(texture_resources.len().saturating_sub(1)) as u32)
        .unwrap_or(0);
    let emissive_texture_resource_index = banger_first_material_emissive_texture_index(&effective_material_bytes)
        .map(|index| index.min(texture_resources.len().saturating_sub(1)) as u32)
        .unwrap_or(0);
    let residency_feedback_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-maps-residency-feedback-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &residency_feedback_bytes,
    );
    let shared_residency_page_table_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-shared-residency-page-table-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &shared_residency_page_table_bytes,
    );
    let shared_residency_compacted_feedback_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-shared-residency-compacted-feedback-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &shared_residency_compacted_feedback_bytes,
    );
    let shared_residency_eviction_plan_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-shared-residency-eviction-plan-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &shared_residency_eviction_plan_bytes,
    );
    let shared_residency_budget_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-shared-residency-budget-buffer",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &shared_residency_budget_bytes,
    );
    let lumen_surface_card_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-lumen-surface-card-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &lumen_surface_card_bytes,
    );
    let lumen_surface_cache_feedback_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-lumen-surface-cache-feedback-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &lumen_surface_cache_feedback_bytes,
    );
    let lumen_screen_probe_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-lumen-screen-probe-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &lumen_screen_probe_bytes,
    );
    let lumen_radiance_cache_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-lumen-radiance-cache-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &lumen_radiance_cache_bytes,
    );
    let virtual_shadow_map_page_table_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-page-table-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vsm_page_table_bytes,
    );
    let virtual_shadow_map_page_flags_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-page-flags-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vsm_page_flags_bytes,
    );
    let virtual_shadow_map_page_request_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-page-request-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vsm_page_request_bytes,
    );
    let virtual_shadow_map_physical_page_metadata_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-physical-page-metadata-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vsm_physical_page_metadata_bytes,
    );
    let virtual_shadow_map_projection_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-projection-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vsm_projection_bytes,
    );
    let virtual_shadow_map_mark_params_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-mark-params-buffer",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &vsm_mark_params_bytes,
    );
    let virtual_shadow_map_physical_page_pool_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-native-vsm-physical-page-pool-texture"),
        size: wgpu::Extent3d {
            width: vsm_physical_pool_desc.width_texels,
            height: vsm_physical_pool_desc.height_texels,
            depth_or_array_layers: vsm_physical_pool_desc.layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let virtual_shadow_map_physical_page_pool_view =
        virtual_shadow_map_physical_page_pool_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("banger-native-vsm-physical-page-pool-view"),
            format: Some(wgpu::TextureFormat::R32Uint),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(vsm_physical_pool_desc.layers),
            usage: Some(
                wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
            ),
        });
    let virtual_shadow_map_projection_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-native-vsm-projection-mask-texture"),
        size: wgpu::Extent3d {
            width: BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_MASK_SIZE,
            height: BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_MASK_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let virtual_shadow_map_projection_view =
        virtual_shadow_map_projection_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("banger-native-vsm-projection-mask-view"),
            format: Some(wgpu::TextureFormat::R32Uint),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
            usage: Some(
                wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
            ),
        });
    let virtual_shadow_map_cache_invalidation_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-cache-invalidation-buffer",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        &vsm_cache_invalidation_bytes,
    );
    let virtual_shadow_map_projection_params_buffer = banger_create_mapped_buffer(
        device,
        "banger-native-vsm-projection-params-buffer",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        &vsm_projection_params_bytes,
    );
    let resource_hash = sha256_hex(
        format!(
            "{source}:{index_format_label}:{vertex_hash}:{index_hash}:{instance_hash}:{material_hash}:{texture_hash}:{texture_resource_hash}:{material_bin_hash}:{normal_texture_resource_index}:{metallic_roughness_texture_resource_index}:{occlusion_texture_resource_index}:{emissive_texture_resource_index}:{indirect_args_hash}:{meshlet_cluster_hash}:{meshlet_cluster_cull_param_hash}:{meshlet_cluster_cull_feedback_hash}:{residency_feedback_hash}:{shared_residency_page_table_hash}:{shared_residency_compacted_feedback_hash}:{shared_residency_eviction_plan_hash}:{lumen_surface_card_hash}:{lumen_surface_cache_feedback_hash}:{lumen_screen_probe_hash}:{lumen_radiance_cache_hash}:{virtual_shadow_map_page_table_hash}:{virtual_shadow_map_page_request_hash}:{virtual_shadow_map_projection_hash}:{virtual_shadow_map_physical_pool_hash}:{virtual_shadow_map_cache_invalidation_hash}",
            index_format_label = index_format.label(),
            texture_resource_hash = sha256_hex(&texture_resource_manifest_bytes),
            material_bin_hash = material_bin_hash,
            normal_texture_resource_index = normal_texture_resource_index,
            metallic_roughness_texture_resource_index = metallic_roughness_texture_resource_index,
            occlusion_texture_resource_index = occlusion_texture_resource_index,
            emissive_texture_resource_index = emissive_texture_resource_index,
        )
        .as_bytes(),
    );
    BangerNativeSceneGpuResource {
        vertex_count,
        index_count,
        instance_count,
        index_format,
        vertex_byte_count: vertex_bytes.len(),
        index_byte_count: index_bytes.len(),
        instance_byte_count: instance_bytes.len(),
        vertex_buffer,
        instance_buffer,
        index_buffer,
        indirect_draw_buffer,
        meshlet_culled_indirect_draw_buffer,
        meshlet_culled_indirect_seed_buffer,
        meshlet_cluster_buffer,
        visible_meshlet_cluster_buffer,
        meshlet_cull_feedback_buffer,
        meshlet_cull_param_buffer,
        material_buffer,
        material_bin_buffer,
        texture_staging_buffers,
        texture_resources,
        normal_texture_resource_index,
        metallic_roughness_texture_resource_index,
        occlusion_texture_resource_index,
        emissive_texture_resource_index,
        residency_feedback_buffer,
        shared_residency_page_table_buffer,
        shared_residency_compacted_feedback_buffer,
        shared_residency_eviction_plan_buffer,
        shared_residency_budget_buffer,
        lumen_surface_card_buffer,
        lumen_surface_cache_feedback_buffer,
        lumen_screen_probe_buffer,
        lumen_radiance_cache_buffer,
        virtual_shadow_map_page_table_buffer,
        virtual_shadow_map_page_flags_buffer,
        virtual_shadow_map_page_request_buffer,
        virtual_shadow_map_physical_page_metadata_buffer,
        virtual_shadow_map_projection_buffer,
        virtual_shadow_map_mark_params_buffer,
        virtual_shadow_map_physical_page_pool_texture,
        virtual_shadow_map_physical_page_pool_view,
        virtual_shadow_map_projection_texture,
        virtual_shadow_map_projection_view,
        virtual_shadow_map_cache_invalidation_buffer,
        virtual_shadow_map_projection_params_buffer,
        mesh_source: source,
        mesh_bounds: bounds,
        selected_tile_id,
        indirect_args_hash,
        meshlet_cluster_hash,
        meshlet_cluster_count,
        meshlet_cluster_cull_param_hash,
        meshlet_cluster_cull_feedback_hash,
        material_bin_hash,
        residency_feedback_hash,
        shared_residency_page_table_hash,
        shared_residency_compacted_feedback_hash,
        shared_residency_eviction_plan_hash,
        lumen_surface_card_hash,
        lumen_surface_cache_feedback_hash,
        lumen_screen_probe_hash,
        lumen_radiance_cache_hash,
        virtual_shadow_map_page_table_hash,
        virtual_shadow_map_page_request_hash,
        virtual_shadow_map_projection_hash,
        virtual_shadow_map_physical_pool_hash,
        virtual_shadow_map_cache_invalidation_hash,
        resource_hash,
    }
}

#[cfg(target_os = "windows")]
fn banger_indexed_indirect_args_bytes(index_count: u32, instance_count: u32) -> [u8; 20] {
    let mut bytes = [0u8; 20];
    for (slot, value) in [
        index_count,
        instance_count,
        0u32, // first_index
        0u32, // base_vertex as i32 bits
        0u32, // first_instance
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
const BANGER_MESHLET_CLUSTER_METADATA_STRIDE: usize = 64;

#[cfg(target_os = "windows")]
const BANGER_MESHLET_CLUSTER_TRIANGLE_LIMIT: usize = 128;

#[cfg(target_os = "windows")]
const BANGER_MATERIAL_BIN_RECORD_STRIDE: usize = 32;

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_cull_params_bytes(
    cluster_count: u32,
    cull_mode: u32,
    index_count: u32,
    instance_count: u32,
) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (slot, value) in [
        cluster_count,
        BANGER_MESHLET_CLUSTER_METADATA_STRIDE as u32,
        cull_mode,
        BANGER_MESHLET_CLUSTER_TRIANGLE_LIMIT as u32,
        index_count,
        instance_count,
        0,
        0,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_cull_feedback_bytes() -> [u8; 16] {
    banger_u32x4_bytes([0, 0, 0, 0])
}

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_cull_compute_wgsl() -> &'static str {
    r#"
struct HzbConsumerUniform {
    // x/y: mip 0 size, z: mip count, w: allocation index.
    dims: vec4<u32>,
};

struct CullParams {
    // x: cluster count, y: cluster stride bytes, z: mode, w: cluster triangle limit.
    words0: vec4<u32>,
    // x: index count, y: initial instance count, z/w reserved.
    words1: vec4<u32>,
};

struct MeshletCluster {
    center_radius: vec4<f32>,
    cone_lod: vec4<f32>,
    draw0: vec4<u32>,
    draw1: vec4<u32>,
};

@group(0) @binding(0) var hzb_pyramid: texture_2d<f32>;
@group(0) @binding(1) var<uniform> hzb: HzbConsumerUniform;
@group(0) @binding(2) var<storage, read> clusters: array<MeshletCluster>;
@group(0) @binding(3) var<storage, read_write> visible_clusters: array<MeshletCluster>;
@group(0) @binding(4) var<storage, read_write> feedback: array<atomic<u32>>;
@group(0) @binding(5) var<uniform> cull: CullParams;
@group(0) @binding(6) var<storage, read_write> culled_indirect_args: array<atomic<u32>>;

fn banger_hzb_load_for_cluster(cluster: MeshletCluster) -> f32 {
    let radius = max(cluster.center_radius.w, 0.0001);
    let safe_mip = min(u32(ceil(log2(radius + 1.0))), hzb.dims.z - 1u);
    let mip_size = max(hzb.dims.xy >> vec2<u32>(safe_mip, safe_mip), vec2<u32>(1u, 1u));
    let projected = abs(cluster.center_radius.xy) * 0.5;
    let pixel = vec2<u32>(u32(projected.x) % mip_size.x, u32(projected.y) % mip_size.y);
    return textureLoad(hzb_pyramid, vec2<i32>(pixel), i32(safe_mip)).x;
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cluster_index = gid.x;
    if (cluster_index >= cull.words0.x) {
        return;
    }
    let cluster = clusters[cluster_index];
    let depth = banger_hzb_load_for_cluster(cluster);
    let frustum_visible = cluster.center_radius.w >= 0.0 && cluster.cone_lod.w >= 0.0;
    let hzb_visible = depth >= 0.0;
    atomicStore(&feedback[1], cull.words0.x);
    atomicStore(&feedback[2], hzb.dims.z);
    if (frustum_visible && hzb_visible) {
        let write_index = atomicAdd(&feedback[0], 1u);
        atomicStore(&culled_indirect_args[1], max(cull.words1.y, 1u));
        visible_clusters[write_index] = cluster;
    }
}
"#
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_mark_compute_wgsl() -> &'static str {
    r#"
struct MeshletCluster {
    center_radius: vec4<f32>,
    cone_lod: vec4<f32>,
    draw0: vec4<u32>,
    draw1: vec4<u32>,
};

struct VsmMarkParams {
    // x: cluster count, y: page record stride, z: shadow map count, w: page size.
    words0: vec4<u32>,
    // x: level0 dim pages, yzw reserved.
    words1: vec4<u32>,
};

@group(0) @binding(0) var<storage, read> visible_clusters: array<MeshletCluster>;
@group(0) @binding(1) var<storage, read_write> page_table: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> page_flags: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> page_requests: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> physical_pages: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read> projection_data: array<u32>;
@group(0) @binding(6) var<uniform> params: VsmMarkParams;

fn banger_vsm_page_for_cluster(cluster_index: u32, cluster: MeshletCluster) -> u32 {
    let center_mix = u32(abs(cluster.center_radius.x) + abs(cluster.center_radius.y) + abs(cluster.center_radius.z));
    return (cluster_index + center_mix) % max(params.words0.x, 1u);
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cluster_index = gid.x;
    if (cluster_index >= params.words0.x) {
        return;
    }
    let cluster = visible_clusters[cluster_index];
    let page_index = banger_vsm_page_for_cluster(cluster_index, cluster);
    let page_word = page_index * 8u;
    let request_word = page_index * 4u;
    let physical_word = page_index * 8u;
    let shadow_map_id = page_index % max(params.words0.z, 1u);
    let mip_level = min(u32(max(cluster.cone_lod.w, 0.0)), 7u);
    let projected_radius = max(u32(ceil(cluster.center_radius.w * 128.0)), 1u);
    atomicStore(&page_table[page_word + 0u], 0x56534D54u);
    atomicStore(&page_table[page_word + 1u], 1u);
    atomicStore(&page_table[page_word + 2u], page_index);
    atomicStore(&page_table[page_word + 3u], page_index);
    atomicStore(&page_table[page_word + 4u], mip_level);
    atomicStore(&page_table[page_word + 6u], shadow_map_id);
    atomicStore(&page_flags[request_word + 0u], 0x56534D46u);
    atomicStore(&page_flags[request_word + 1u], page_index);
    atomicStore(&page_flags[request_word + 2u], 1u);
    atomicStore(&page_flags[request_word + 3u], projected_radius);
    atomicStore(&page_requests[request_word + 0u], 0x56534D52u);
    atomicStore(&page_requests[request_word + 1u], page_index);
    atomicStore(&page_requests[request_word + 2u], 1u);
    atomicStore(&page_requests[request_word + 3u], cluster.draw0.y);
    atomicStore(&physical_pages[physical_word + 0u], 0x56534D50u);
    atomicStore(&physical_pages[physical_word + 1u], 1u);
    atomicStore(&physical_pages[physical_word + 2u], page_index);
    atomicStore(&physical_pages[physical_word + 3u], shadow_map_id);
    atomicStore(&physical_pages[physical_word + 4u], mip_level);
    atomicStore(&physical_pages[physical_word + 6u], projection_data[2]);
}
"#
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_physical_page_compute_wgsl() -> &'static str {
    r#"
struct VsmProjectionParams {
    words0: vec4<u32>,
    words1: vec4<u32>,
};

@group(0) @binding(0) var<storage, read> page_requests: array<u32>;
@group(0) @binding(1) var<storage, read_write> physical_pages: array<atomic<u32>>;
@group(0) @binding(2) var physical_page_pool: texture_storage_2d_array<r32uint, write>;
@group(0) @binding(3) var<storage, read_write> cache_invalidation: array<atomic<u32>>;
@group(0) @binding(4) var<uniform> params: VsmProjectionParams;
@group(0) @binding(5) var<storage, read> page_table: array<u32>;
@group(0) @binding(6) var<storage, read> projection_data: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let page_index = gid.x;
    if (page_index >= params.words0.x) {
        return;
    }
    let request_word = page_index * 4u;
    let physical_word = page_index * 8u;
    let requested = page_requests[request_word + 2u];
    if (requested == 0u) {
        return;
    }
    let pool_pages_x = max(params.words0.y, 1u);
    let page_size = max(params.words0.w, 1u);
    let page_x = page_index % pool_pages_x;
    let page_y = page_index / pool_pages_x;
    let texel = vec2<i32>(i32(page_x * page_size), i32(page_y * page_size));
    let layer = i32(page_index % max(params.words1.x, 1u));
    let page_table_word = page_index * 8u;
    let encoded_depth = 0x3F800000u ^ page_table[page_table_word + 7u] ^ projection_data[2];
    textureStore(physical_page_pool, texel, layer, vec4<u32>(encoded_depth, 0u, 0u, 0u));
    atomicStore(&physical_pages[physical_word + 6u], params.words1.y);
    atomicStore(&physical_pages[physical_word + 7u], encoded_depth);
    atomicAdd(&cache_invalidation[0], 1u);
    atomicStore(&cache_invalidation[1], page_index);
    atomicStore(&cache_invalidation[2], requested);
    atomicStore(&cache_invalidation[3], encoded_depth);
}
"#
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_projection_filter_compute_wgsl() -> &'static str {
    r#"
struct VsmProjectionParams {
    words0: vec4<u32>,
    words1: vec4<u32>,
};

@group(0) @binding(0) var<storage, read> page_requests: array<u32>;
@group(0) @binding(1) var<storage, read_write> physical_pages: array<atomic<u32>>;
@group(0) @binding(2) var projection_mask: texture_storage_2d<r32uint, write>;
@group(0) @binding(3) var<storage, read_write> cache_invalidation: array<atomic<u32>>;
@group(0) @binding(4) var<uniform> params: VsmProjectionParams;
@group(0) @binding(5) var<storage, read> page_table: array<u32>;
@group(0) @binding(6) var<storage, read> projection_data: array<u32>;

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.words1.z || gid.y >= params.words1.z) {
        return;
    }
    let page_count = max(params.words0.x, 1u);
    let page_index = (gid.x + gid.y * params.words1.z) % page_count;
    let request_word = page_index * 4u;
    let physical_word = page_index * 8u;
    let requested = page_requests[request_word + 2u];
    let page_hash = page_table[page_index * 8u + 7u];
    let physical_hash = atomicLoad(&physical_pages[physical_word + 7u]);
    let cache_epoch = atomicLoad(&cache_invalidation[0]);
    let lit = select(0x00000020u, 0x000000FFu, requested > 0u && physical_hash != 0u);
    let filtered = lit ^ (page_hash & 0x0000000Fu) ^ (cache_epoch & 0x0000000Fu) ^ (projection_data[2] & 0x0000000Fu);
    textureStore(projection_mask, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<u32>(filtered, 0u, 0u, 0u));
}
"#
}

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_metadata_bytes(
    vertex_bytes: &[u8],
    index_bytes: &[u8],
    index_format: BangerRenderIndexFormat,
    source: &str,
) -> Vec<u8> {
    let indices = banger_render_indices_to_u32(index_bytes, index_format);
    let triangle_count = indices.len() / 3;
    if vertex_bytes.len() < BANGER_RENDER_VERTEX_STRIDE_BYTES || triangle_count == 0 {
        return banger_empty_meshlet_cluster_metadata_bytes(source);
    }
    let cluster_triangle_limit = BANGER_MESHLET_CLUSTER_TRIANGLE_LIMIT.max(1);
    let mut bytes = Vec::with_capacity(
        triangle_count.div_ceil(cluster_triangle_limit) * BANGER_MESHLET_CLUSTER_METADATA_STRIDE,
    );
    for cluster_index in 0..triangle_count.div_ceil(cluster_triangle_limit) {
        let first_triangle = cluster_index * cluster_triangle_limit;
        let end_triangle = ((cluster_index + 1) * cluster_triangle_limit).min(triangle_count);
        let first_index = first_triangle * 3;
        let index_count = (end_triangle - first_triangle) * 3;
        bytes.extend_from_slice(&banger_meshlet_cluster_metadata_record_bytes(
            vertex_bytes,
            &indices[first_index..first_index + index_count],
            first_index as u32,
            index_count as u32,
            cluster_index as u32,
            source,
        ));
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_render_indices_to_u32(index_bytes: &[u8], index_format: BangerRenderIndexFormat) -> Vec<u32> {
    match index_format {
        BangerRenderIndexFormat::Uint16 => index_bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes(chunk.try_into().expect("u16 index chunk")) as u32)
            .collect(),
        BangerRenderIndexFormat::Uint32 => index_bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 index chunk")))
            .collect(),
    }
}

#[cfg(target_os = "windows")]
fn banger_empty_meshlet_cluster_metadata_bytes(source: &str) -> Vec<u8> {
    banger_meshlet_cluster_metadata_record_bytes(&[], &[], 0, 0, 0, source).to_vec()
}

#[cfg(target_os = "windows")]
fn banger_material_bin_bytes(
    meshlet_cluster_bytes: &[u8],
    material_bytes: Option<&[u8]>,
    texture_manifest_bytes: &[u8],
) -> Vec<u8> {
    let material_count = material_bytes
        .map(|bytes| (bytes.len() / BANGER_MATERIAL_RECORD_STRIDE).max(1))
        .unwrap_or(1)
        .min(1024);
    let mut bins = vec![(u32::MAX, 0u32, u32::MAX, 0u32); material_count];
    for (cluster_index, cluster) in meshlet_cluster_bytes
        .chunks_exact(BANGER_MESHLET_CLUSTER_METADATA_STRIDE)
        .enumerate()
    {
        let first_index = u32::from_le_bytes(cluster[32..36].try_into().expect("cluster first index"));
        let index_count = u32::from_le_bytes(cluster[36..40].try_into().expect("cluster index count"));
        let material_bin = u32::from_le_bytes(cluster[48..52].try_into().expect("cluster material bin"))
            as usize
            % material_count;
        let bin = &mut bins[material_bin];
        bin.0 = bin.0.min(cluster_index as u32);
        bin.1 = bin.1.saturating_add(1);
        bin.2 = bin.2.min(first_index);
        bin.3 = bin.3.saturating_add(index_count);
    }
    let texture_manifest_hash = sha256_hex(texture_manifest_bytes);
    let mut bytes = Vec::with_capacity(material_count * BANGER_MATERIAL_BIN_RECORD_STRIDE);
    for (material_bin, (first_cluster, cluster_count, first_index, index_count)) in bins.into_iter().enumerate() {
        let material_hash = material_bytes
            .and_then(|bytes| {
                let start = material_bin * BANGER_MATERIAL_RECORD_STRIDE;
                bytes.get(start..start + BANGER_MATERIAL_RECORD_STRIDE)
            })
            .map(sha256_hex)
            .unwrap_or_else(|| sha256_hex(b"banger_default_material_bin"));
        for value in [
            0x4D_42_49_4Eu32, // MBIN
            1,
            material_bin as u32,
            if first_cluster == u32::MAX { 0 } else { first_cluster },
            cluster_count,
            if first_index == u32::MAX { 0 } else { first_index },
            index_count,
            banger_hash_prefix_u32(&sha256_hex(format!("{material_hash}:{texture_manifest_hash}").as_bytes())),
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    if bytes.is_empty() {
        bytes.resize(BANGER_MATERIAL_BIN_RECORD_STRIDE, 0);
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_metadata_record_bytes(
    vertex_bytes: &[u8],
    indices: &[u32],
    first_index: u32,
    index_count: u32,
    cluster_index: u32,
    source: &str,
) -> [u8; BANGER_MESHLET_CLUSTER_METADATA_STRIDE] {
    let mut referenced_positions = Vec::new();
    let mut min_vertex = u32::MAX;
    let mut max_vertex = 0u32;
    for index in indices {
        if let Some(position) = banger_render_vertex_position(vertex_bytes, *index) {
            referenced_positions.push(position);
            min_vertex = min_vertex.min(*index);
            max_vertex = max_vertex.max(*index);
        }
    }
    let (center, radius) = banger_meshlet_cluster_bounds(&referenced_positions);
    let cone_axis = banger_meshlet_cluster_average_normal(vertex_bytes, indices);
    let triangle_count = (index_count / 3).max(1);
    let lod_error = (radius / (triangle_count as f32).sqrt()).max(0.0001);
    let first_vertex = if min_vertex == u32::MAX { 0 } else { min_vertex };
    let vertex_count = if min_vertex == u32::MAX {
        0
    } else {
        max_vertex.saturating_sub(min_vertex).saturating_add(1)
    };
    let material_bin = cluster_index;
    let source_hash = sha256_hex(source.as_bytes());
    let cluster_seed_hash = sha256_hex(
        format!(
            "{source}:{cluster_index}:{first_index}:{index_count}:{first_vertex}:{vertex_count}:{radius:.6}:{lod_error:.6}"
        )
        .as_bytes(),
    );
    let mut bytes = [0u8; BANGER_MESHLET_CLUSTER_METADATA_STRIDE];
    for (slot, value) in [
        center[0],
        center[1],
        center[2],
        radius,
        cone_axis[0],
        cone_axis[1],
        cone_axis[2],
        lod_error,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (slot, value) in [
        first_index,
        index_count,
        first_vertex,
        vertex_count,
        material_bin,
        0x4D_53_48_4Cu32, // MSHL
        banger_hash_prefix_u32(&source_hash),
        banger_hash_prefix_u32(&cluster_seed_hash),
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 32 + slot * 4;
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_render_vertex_position(vertex_bytes: &[u8], vertex_index: u32) -> Option<[f32; 3]> {
    let offset = vertex_index as usize * BANGER_RENDER_VERTEX_STRIDE_BYTES;
    if offset + 12 > vertex_bytes.len() {
        return None;
    }
    Some([
        f32::from_le_bytes(vertex_bytes[offset..offset + 4].try_into().ok()?),
        f32::from_le_bytes(vertex_bytes[offset + 4..offset + 8].try_into().ok()?),
        f32::from_le_bytes(vertex_bytes[offset + 8..offset + 12].try_into().ok()?),
    ])
}

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_bounds(positions: &[[f32; 3]]) -> ([f32; 3], f32) {
    if positions.is_empty() {
        return ([0.0, 0.0, 0.0], 0.0);
    }
    let mut center = [0.0f32; 3];
    for position in positions {
        center[0] += position[0];
        center[1] += position[1];
        center[2] += position[2];
    }
    let inv_count = 1.0 / positions.len() as f32;
    center = [center[0] * inv_count, center[1] * inv_count, center[2] * inv_count];
    let mut radius = 0.0f32;
    for position in positions {
        radius = radius.max(
            ((position[0] - center[0]).powi(2)
                + (position[1] - center[1]).powi(2)
                + (position[2] - center[2]).powi(2))
            .sqrt(),
        );
    }
    (center, radius)
}

#[cfg(target_os = "windows")]
fn banger_meshlet_cluster_average_normal(vertex_bytes: &[u8], indices: &[u32]) -> [f32; 3] {
    let mut normal = [0.0f32; 3];
    for triangle in indices.chunks_exact(3) {
        let Some(a) = banger_render_vertex_position(vertex_bytes, triangle[0]) else { continue };
        let Some(b) = banger_render_vertex_position(vertex_bytes, triangle[1]) else { continue };
        let Some(c) = banger_render_vertex_position(vertex_bytes, triangle[2]) else { continue };
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        normal[0] += ab[1] * ac[2] - ab[2] * ac[1];
        normal[1] += ab[2] * ac[0] - ab[0] * ac[2];
        normal[2] += ab[0] * ac[1] - ab[1] * ac[0];
    }
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length <= 0.0001 {
        [0.0, 0.0, 1.0]
    } else {
        [normal[0] / length, normal[1] / length, normal[2] / length]
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_residency_feedback_bytes(
    selected_tile_id: Option<&str>,
    source: &str,
    vertex_count: u32,
    index_count: u32,
    instance_count: u32,
    vertex_hash: &str,
    index_hash: &str,
    material_hash: &str,
    texture_hash: &str,
) -> Vec<u8> {
    let selected_tile_hash = sha256_hex(selected_tile_id.unwrap_or("no-selected-maps-tile").as_bytes());
    let source_hash = sha256_hex(source.as_bytes());
    let tile_count = selected_tile_id
        .map(|value| value.split(',').filter(|entry| !entry.is_empty()).count() as u32)
        .unwrap_or(0);
    let mut bytes = Vec::with_capacity(80);
    for value in [
        0x4D_41_50_53u32, // MAPS
        1u32,
        tile_count,
        vertex_count,
        index_count,
        instance_count,
        banger_hash_prefix_u32(&selected_tile_hash),
        banger_hash_prefix_u32(&source_hash),
        banger_hash_prefix_u32(vertex_hash),
        banger_hash_prefix_u32(index_hash),
        banger_hash_prefix_u32(material_hash),
        banger_hash_prefix_u32(texture_hash),
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
const BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE: usize = 64;

#[cfg(target_os = "windows")]
const BANGER_SHARED_RESIDENCY_COMPACTED_FEEDBACK_STRIDE: usize = 32;

#[cfg(target_os = "windows")]
const BANGER_SHARED_RESIDENCY_EVICTION_RECORD_STRIDE: usize = 32;

#[cfg(target_os = "windows")]
fn banger_shared_residency_page_table_bytes(
    source: &str,
    selected_tile_id: Option<&str>,
    geometry_bytes: usize,
    material_bytes: usize,
    texture_bytes: usize,
    feedback_bytes: usize,
) -> Vec<u8> {
    let source_hash = sha256_hex(source.as_bytes());
    let selected_tile_hash = sha256_hex(selected_tile_id.unwrap_or("no-selected-maps-tile").as_bytes());
    let mut physical_offset = 0u64;
    let mut records = Vec::new();
    for (kind, byte_count, priority, lru_frame) in [
        ("nanite_geometry_page", geometry_bytes, 900u32, 0u32),
        ("material_texture_page", material_bytes + texture_bytes, 700u32, 1u32),
        ("renderer_feedback_page", feedback_bytes, 600u32, 2u32),
    ] {
        if byte_count == 0 {
            continue;
        }
        records.extend_from_slice(&banger_shared_residency_page_record_bytes(
            kind,
            (records.len() / BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE) as u32,
            byte_count as u64,
            physical_offset,
            priority,
            lru_frame,
            &source_hash,
            &selected_tile_hash,
        ));
        physical_offset = physical_offset.saturating_add(banger_align_u64(byte_count as u64, 4096));
    }
    if records.is_empty() {
        records.extend_from_slice(&banger_shared_residency_page_record_bytes(
            "empty_residency_page",
            0,
            4096,
            0,
            1,
            0,
            &source_hash,
            &selected_tile_hash,
        ));
    }
    records
}

#[cfg(target_os = "windows")]
fn banger_shared_residency_page_record_bytes(
    kind: &str,
    page_index: u32,
    byte_count: u64,
    physical_offset: u64,
    priority: u32,
    lru_frame: u32,
    source_hash: &str,
    selected_tile_hash: &str,
) -> [u8; BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE] {
    let kind_hash = sha256_hex(kind.as_bytes());
    let page_hash = sha256_hex(
        format!("{kind}:{page_index}:{byte_count}:{physical_offset}:{priority}:{lru_frame}")
            .as_bytes(),
    );
    let mut bytes = [0u8; BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE];
    for (slot, value) in [
        0x52_53_44_59u32, // RSDY
        1,
        banger_hash_prefix_u32(&kind_hash),
        page_index,
        byte_count.min(u32::MAX as u64) as u32,
        (byte_count >> 32) as u32,
        physical_offset.min(u32::MAX as u64) as u32,
        (physical_offset >> 32) as u32,
        priority,
        lru_frame,
        1, // resident now; future eviction can flip this without changing layout.
        banger_hash_prefix_u32(source_hash),
        banger_hash_prefix_u32(selected_tile_hash),
        banger_hash_prefix_u32(&page_hash),
        0,
        0,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_shared_residency_compacted_feedback_bytes(page_table_bytes: &[u8]) -> Vec<u8> {
    let mut compacted = Vec::with_capacity(
        page_table_bytes.len() / BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE
            * BANGER_SHARED_RESIDENCY_COMPACTED_FEEDBACK_STRIDE,
    );
    for record in page_table_bytes.chunks_exact(BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE) {
        let page_index = u32::from_le_bytes(record[12..16].try_into().expect("page index"));
        let byte_count_low = u32::from_le_bytes(record[16..20].try_into().expect("byte count"));
        let priority = u32::from_le_bytes(record[32..36].try_into().expect("priority"));
        let page_hash_prefix = u32::from_le_bytes(record[52..56].try_into().expect("page hash"));
        for value in [
            0x46_44_42_4Bu32, // FDBK
            1,
            page_index,
            byte_count_low,
            priority,
            page_hash_prefix,
            page_index.saturating_add(priority),
            0,
        ] {
            compacted.extend_from_slice(&value.to_le_bytes());
        }
    }
    compacted
}

#[cfg(target_os = "windows")]
fn banger_shared_residency_eviction_plan_bytes(
    page_table_bytes: &[u8],
    budget_bytes: u64,
) -> Vec<u8> {
    let mut resident_total = 0u64;
    let mut pages = Vec::new();
    for record in page_table_bytes.chunks_exact(BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE) {
        let page_index = u32::from_le_bytes(record[12..16].try_into().expect("page index"));
        let byte_count_low = u32::from_le_bytes(record[16..20].try_into().expect("byte count low"));
        let byte_count_high = u32::from_le_bytes(record[20..24].try_into().expect("byte count high"));
        let byte_count = byte_count_low as u64 | ((byte_count_high as u64) << 32);
        let priority = u32::from_le_bytes(record[32..36].try_into().expect("priority"));
        let lru_frame = u32::from_le_bytes(record[36..40].try_into().expect("lru frame"));
        let resident = u32::from_le_bytes(record[40..44].try_into().expect("resident flag"));
        if resident != 0 {
            resident_total = resident_total.saturating_add(byte_count);
            pages.push((priority, lru_frame, page_index, byte_count));
        }
    }

    pages.sort_by_key(|(priority, lru_frame, page_index, _)| (*priority, *lru_frame, *page_index));
    let mut reclaim_bytes = resident_total.saturating_sub(budget_bytes);
    let mut eviction_plan =
        Vec::with_capacity(pages.len().max(1) * BANGER_SHARED_RESIDENCY_EVICTION_RECORD_STRIDE);
    for (priority, lru_frame, page_index, byte_count) in pages {
        let evict = u32::from(reclaim_bytes > 0);
        if evict != 0 {
            reclaim_bytes = reclaim_bytes.saturating_sub(byte_count);
        }
        for value in [
            0x45_56_43_54u32, // EVCT
            1,
            page_index,
            byte_count.min(u32::MAX as u64) as u32,
            (byte_count >> 32) as u32,
            priority,
            lru_frame,
            evict,
        ] {
            eviction_plan.extend_from_slice(&value.to_le_bytes());
        }
    }
    if eviction_plan.is_empty() {
        eviction_plan.resize(BANGER_SHARED_RESIDENCY_EVICTION_RECORD_STRIDE, 0);
    }
    eviction_plan
}

#[cfg(target_os = "windows")]
fn banger_shared_residency_budget_bytes(
    virtual_page_count: usize,
    budget_bytes: u64,
    resident_bytes: usize,
) -> [u8; 32] {
    let pool_pressure_milli =
        ((resident_bytes as u128 * 1000) / (budget_bytes.max(1) as u128)).min(u32::MAX as u128) as u32;
    let mut bytes = [0u8; 32];
    for (slot, value) in [
        0x42_55_44_47u32, // BUDG
        1,
        virtual_page_count as u32,
        budget_bytes.min(u32::MAX as u64) as u32,
        (budget_bytes >> 32) as u32,
        resident_bytes.min(u32::MAX as usize) as u32,
        pool_pressure_milli,
        0,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_align_u64(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

#[cfg(target_os = "windows")]
const BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE: usize = 64;

#[cfg(target_os = "windows")]
const BANGER_LUMEN_SCREEN_PROBE_RECORD_STRIDE: usize = 32;

#[cfg(target_os = "windows")]
fn banger_lumen_surface_card_bytes(meshlet_cluster_bytes: &[u8]) -> Vec<u8> {
    let mut cards = Vec::with_capacity(
        meshlet_cluster_bytes.len()
            .max(BANGER_MESHLET_CLUSTER_METADATA_STRIDE)
            / BANGER_MESHLET_CLUSTER_METADATA_STRIDE
            * BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE,
    );
    for (card_index, cluster) in meshlet_cluster_bytes
        .chunks_exact(BANGER_MESHLET_CLUSTER_METADATA_STRIDE)
        .enumerate()
    {
        let center_x = f32::from_le_bytes(cluster[0..4].try_into().expect("cluster center x"));
        let center_y = f32::from_le_bytes(cluster[4..8].try_into().expect("cluster center y"));
        let center_z = f32::from_le_bytes(cluster[8..12].try_into().expect("cluster center z"));
        let radius = f32::from_le_bytes(cluster[12..16].try_into().expect("cluster radius"));
        let normal_x = f32::from_le_bytes(cluster[16..20].try_into().expect("cluster normal x"));
        let normal_y = f32::from_le_bytes(cluster[20..24].try_into().expect("cluster normal y"));
        let normal_z = f32::from_le_bytes(cluster[24..28].try_into().expect("cluster normal z"));
        let lod_error = f32::from_le_bytes(cluster[28..32].try_into().expect("cluster lod"));
        let cluster_hash = u32::from_le_bytes(cluster[60..64].try_into().expect("cluster hash"));
        for value in [
            0x4C_53_43_44u32, // LSCD
            1,
            card_index as u32,
            cluster_hash,
        ] {
            cards.extend_from_slice(&value.to_le_bytes());
        }
        for value in [
            center_x, center_y, center_z, radius,
            normal_x, normal_y, normal_z, lod_error,
            (card_index as f32 + 1.0) * radius.max(0.001), 0.0, 0.0, 1.0,
        ] {
            cards.extend_from_slice(&value.to_le_bytes());
        }
    }
    if cards.is_empty() {
        cards.resize(BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE, 0);
    }
    cards
}

#[cfg(target_os = "windows")]
fn banger_lumen_surface_cache_feedback_bytes(surface_card_bytes: &[u8]) -> Vec<u8> {
    let card_count = (surface_card_bytes.len() / BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE).max(1);
    let mut bytes = Vec::with_capacity(card_count * 32);
    for card_index in 0..card_count {
        let card_offset = card_index * BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE;
        let card_hash = u32::from_le_bytes(
            surface_card_bytes[card_offset + 12..card_offset + 16]
                .try_into()
                .expect("surface card hash"),
        );
        for value in [
            0x4C_46_44_42u32, // LFDB
            1,
            card_index as u32,
            card_hash,
            1, // requested this frame.
            0,
            0,
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_lumen_screen_probe_bytes(cluster_count: u32) -> Vec<u8> {
    let probe_count = cluster_count.max(1).min(256);
    let mut bytes = Vec::with_capacity(probe_count as usize * BANGER_LUMEN_SCREEN_PROBE_RECORD_STRIDE);
    for probe_index in 0..probe_count {
        for value in [
            0x4C_50_52_42u32, // LPRB
            1,
            probe_index,
            probe_count,
            probe_index % 16,
            probe_index / 16,
            0,
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_lumen_radiance_cache_bytes(surface_card_bytes: &[u8], screen_probe_bytes: &[u8]) -> Vec<u8> {
    let card_count = (surface_card_bytes.len() / BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE).max(1);
    let probe_count = (screen_probe_bytes.len() / BANGER_LUMEN_SCREEN_PROBE_RECORD_STRIDE).max(1);
    let mut bytes = Vec::with_capacity(64);
    for value in [
        0x4C_52_44_43u32, // LRDC
        1,
        card_count as u32,
        probe_count as u32,
        banger_hash_prefix_u32(&sha256_hex(surface_card_bytes)),
        banger_hash_prefix_u32(&sha256_hex(screen_probe_bytes)),
        0,
        0,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [0.02f32, 0.08, 0.16, 1.0, 0.0, 0.0, 0.0, 0.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
const BANGER_VIRTUAL_SHADOW_MAP_PAGE_RECORD_STRIDE: usize = 32;

#[cfg(target_os = "windows")]
const BANGER_VIRTUAL_SHADOW_MAP_PHYSICAL_PAGE_RECORD_STRIDE: usize = 32;

#[cfg(target_os = "windows")]
const BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_STRIDE: usize = 128;

#[cfg(target_os = "windows")]
const BANGER_VIRTUAL_SHADOW_MAP_PAGE_SIZE: u32 = 128;

#[cfg(target_os = "windows")]
const BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_MASK_SIZE: u32 = 256;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct BangerVirtualShadowMapPhysicalPoolDesc {
    page_count: u32,
    pages_x: u32,
    pages_y: u32,
    layers: u32,
    width_texels: u32,
    height_texels: u32,
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_page_table_bytes(cluster_count: u32, shadow_map_count: u32) -> Vec<u8> {
    let page_count = cluster_count.max(1);
    let mut bytes = Vec::with_capacity(page_count as usize * BANGER_VIRTUAL_SHADOW_MAP_PAGE_RECORD_STRIDE);
    for page_index in 0..page_count {
        let physical_page_index = page_index;
        let mip_level = banger_virtual_shadow_map_mip_for_page(page_index);
        let page_x = page_index & 127;
        let page_y = (page_index >> 7) & 127;
        let shadow_map_id = page_index % shadow_map_count.max(1);
        let page_hash = sha256_hex(
            format!("vsm_page:{page_index}:{physical_page_index}:{mip_level}:{page_x}:{page_y}:{shadow_map_id}")
                .as_bytes(),
        );
        for value in [
            0x56_53_4D_54u32, // VSMT
            1,
            page_index,
            physical_page_index,
            mip_level,
            (page_y << 16) | page_x,
            shadow_map_id,
            banger_hash_prefix_u32(&page_hash),
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_page_flags_bytes(cluster_count: u32) -> Vec<u8> {
    let page_count = cluster_count.max(1);
    let mut bytes = Vec::with_capacity(page_count as usize * 16);
    for page_index in 0..page_count {
        for value in [
            0x56_53_4D_46u32, // VSMF
            page_index,
            0,
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_page_request_bytes(cluster_count: u32) -> Vec<u8> {
    let page_count = cluster_count.max(1);
    let mut bytes = Vec::with_capacity(page_count as usize * 16);
    for page_index in 0..page_count {
        for value in [
            0x56_53_4D_52u32, // VSMR
            page_index,
            0,
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_physical_page_metadata_bytes(cluster_count: u32, shadow_map_count: u32) -> Vec<u8> {
    let page_count = cluster_count.max(1);
    let mut bytes = Vec::with_capacity(
        page_count as usize * BANGER_VIRTUAL_SHADOW_MAP_PHYSICAL_PAGE_RECORD_STRIDE,
    );
    for page_index in 0..page_count {
        let mip_level = banger_virtual_shadow_map_mip_for_page(page_index);
        let page_x = page_index & 127;
        let page_y = (page_index >> 7) & 127;
        for value in [
            0x56_53_4D_50u32, // VSMP
            1,
            page_index,
            page_index % shadow_map_count.max(1),
            mip_level,
            (page_y << 16) | page_x,
            0, // last requested frame, filled by the page marker.
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_projection_bytes(shadow_map_count: u32) -> Vec<u8> {
    let projection_count = shadow_map_count.max(1);
    let mut bytes = Vec::with_capacity(
        projection_count as usize * BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_STRIDE,
    );
    for shadow_map_id in 0..projection_count {
        for value in [
            0x56_53_4D_50u32, // VSMP
            1,
            shadow_map_id,
            0, // directional light for the first Banger VSM lane.
            128, // page size, matching Unreal's common VSM page size.
            128, // level 0 dimension in pages.
            2048, // physical page budget seed.
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in banger_identity_mat4_f32() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        while bytes.len() % BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_STRIDE != 0 {
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_mark_params_bytes(cluster_count: u32, shadow_map_count: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (slot, value) in [
        cluster_count.max(1),
        BANGER_VIRTUAL_SHADOW_MAP_PAGE_RECORD_STRIDE as u32,
        shadow_map_count.max(1),
        128,
        128,
        0,
        0,
        0,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_physical_pool_desc(cluster_count: u32) -> BangerVirtualShadowMapPhysicalPoolDesc {
    let page_count = cluster_count.max(1).min(2048);
    let pages_x = 16u32.min(page_count.next_power_of_two()).max(1);
    let pages_y = page_count.div_ceil(pages_x).max(1);
    let layers = 1;
    BangerVirtualShadowMapPhysicalPoolDesc {
        page_count,
        pages_x,
        pages_y,
        layers,
        width_texels: pages_x * BANGER_VIRTUAL_SHADOW_MAP_PAGE_SIZE,
        height_texels: pages_y * BANGER_VIRTUAL_SHADOW_MAP_PAGE_SIZE,
    }
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_cache_invalidation_bytes(cluster_count: u32, cluster_hash: &str) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (slot, value) in [
        0u32, // invalidated page count, written by compute.
        0,
        0,
        0,
        cluster_count.max(1),
        banger_hash_prefix_u32(cluster_hash),
        0x56_53_4D_49u32, // VSMI
        1,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_projection_params_bytes(
    cluster_count: u32,
    pool: BangerVirtualShadowMapPhysicalPoolDesc,
) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (slot, value) in [
        cluster_count.max(1),
        pool.pages_x,
        pool.pages_y,
        BANGER_VIRTUAL_SHADOW_MAP_PAGE_SIZE,
        pool.layers,
        1, // frame/epoch seed, promoted later to real scene frame.
        BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_MASK_SIZE,
        pool.page_count,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn banger_virtual_shadow_map_mip_for_page(page_index: u32) -> u32 {
    if page_index == 0 {
        0
    } else {
        (31 - page_index.leading_zeros()).min(7)
    }
}

#[cfg(target_os = "windows")]
fn banger_identity_mat4_f32() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

#[cfg(target_os = "windows")]
fn banger_hash_prefix_u32(hash: &str) -> u32 {
    u32::from_str_radix(hash.get(0..8).unwrap_or("00000000"), 16).unwrap_or(0)
}

#[cfg(target_os = "windows")]
fn banger_maps_native_render_gate() -> BangerMapsNativeRenderGateProjection {
    let ingest = banger_maps_root_ingest(Some(true), Some(true), Some(true));
    let root_error_code = ingest.error.as_ref().map(|error| error.code.to_string());
    let root_error_message = ingest.error.as_ref().map(|error| error.message.clone());
    let mesh_result = banger_maps_first_tile_render_mesh_bytes_from_ingest(&ingest);
    let texture_seed = banger_maps_gate_texture_seed(&ingest);
    let (drawable_mesh_ready, draw_source, vertex_buffer_byte_count, index_buffer_byte_count, instance_buffer_byte_count, draw_index_count, draw_instance_count, preview_gate, blocker) =
        match mesh_result {
            Ok(mesh) => {
                let draw_index_count = (mesh.index_bytes.len() / mesh.index_format.stride_bytes()) as u32;
                let draw_instance_count = (mesh.instance_bytes.len() / 80) as u32;
                let preview_gate = banger_maps_cpu_preview_gate(&mesh, &texture_seed);
                (
                    true,
                    Some(mesh.source),
                    mesh.vertex_bytes.len(),
                    mesh.index_bytes.len(),
                    mesh.instance_bytes.len(),
                    draw_index_count,
                    draw_instance_count,
                    preview_gate,
                    None,
                )
            }
            Err(error) => (
                false,
                None,
                0,
                0,
                0,
                0,
                0,
                BangerMapsCpuPreviewGate {
                    nonblack_pixel_count: 0,
                    non_fallback_blue_pixel_count: 0,
                    frame_hash: sha256_hex(b"banger_maps_no_preview"),
                    width: 0,
                    height: 0,
                },
                Some(error),
            ),
        };
    let visible_gate_ok = preview_gate.nonblack_pixel_count > 0
        && preview_gate.non_fallback_blue_pixel_count > 0
        && draw_index_count > 0;
    let render_gate_hash = sha256_hex(
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            ingest.root_hash,
            ingest.content_cache.cache_manifest_hash,
            ingest.content_decode.decode_manifest_hash,
            ingest.gpu_staging.upload_plan_hash,
            drawable_mesh_ready,
            draw_source.unwrap_or("none"),
            draw_index_count,
            preview_gate.nonblack_pixel_count,
            preview_gate.non_fallback_blue_pixel_count,
            preview_gate.frame_hash,
            blocker.as_deref().unwrap_or("")
        )
        .as_bytes(),
    );
    BangerMapsNativeRenderGateProjection {
        ok: ingest.ok && drawable_mesh_ready && visible_gate_ok,
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
        nonblack_pixel_count: preview_gate.nonblack_pixel_count,
        non_fallback_blue_pixel_count: preview_gate.non_fallback_blue_pixel_count,
        frame_hash: preview_gate.frame_hash,
        frame_preview_width: preview_gate.width,
        frame_preview_height: preview_gate.height,
        render_gate_hash,
        blocker: blocker.or_else(|| (!visible_gate_ok).then(|| "Banger Maps visible pixel gate failed: offscreen preview was black or fallback-blue only".to_string())),
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
        nonblack_pixel_count: 0,
        non_fallback_blue_pixel_count: 0,
        frame_hash: sha256_hex(b"unsupported_banger_maps_native_render_gate_frame"),
        frame_preview_width: 0,
        frame_preview_height: 0,
        render_gate_hash: sha256_hex(b"unsupported_banger_maps_native_render_gate_platform"),
        blocker: Some("Banger native render gate currently requires the Windows wgpu path.".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_first_visible_tile_gpu_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<BangerNativeSceneGpuResource, String> {
    let ingest = banger_maps_root_ingest(Some(true), Some(true), Some(true));
    let maps_render_space_transform = banger_maps_render_space_transform();
    let selected_records = banger_maps_draw_records_or_seed(&ingest, maps_render_space_transform)
        .into_iter()
        .take(banger_maps_visible_tile_batch_limit())
        .collect::<Vec<_>>();
    let selected_tile_id = selected_records
        .iter()
        .map(|record| record.tile_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let (material_bytes, texture_staging_bytes) =
        banger_maps_material_texture_resources_for_records(&selected_records)?;
    let mesh = banger_maps_visible_tile_batch_render_mesh_bytes_from_ingest(&ingest)?;
    Ok(banger_native_scene_gpu_resource_from_mesh_bytes(
        device,
        mesh,
        material_bytes,
        texture_staging_bytes,
        (!selected_tile_id.is_empty()).then_some(selected_tile_id),
        queue,
    ))
}

#[cfg(target_os = "windows")]
fn banger_maps_gate_texture_seed(ingest: &BangerMapsRootIngestProjection) -> String {
    let hashes = ingest
        .gpu_staging
        .records
        .iter()
        .flat_map(|record| record.texture_stages.iter())
        .map(|texture| texture.content_hash.as_str())
        .collect::<Vec<_>>()
        .join(":");
    if hashes.is_empty() {
        sha256_hex(b"banger_maps_gate_no_texture")
    } else {
        sha256_hex(hashes.as_bytes())
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_cpu_preview_gate(mesh: &BangerRenderMeshBytes, texture_seed: &str) -> BangerMapsCpuPreviewGate {
    let width = 128u32;
    let height = 72u32;
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    let view_proj = banger_view_projection_matrix_for_bounds(0.0, mesh.bounds, width, height);
    let seed_color = banger_hash_color(texture_seed);
    for vertex in mesh.vertex_bytes.chunks_exact(BANGER_RENDER_VERTEX_STRIDE_BYTES) {
        let point = [
            f32::from_le_bytes(vertex[0..4].try_into().unwrap()),
            f32::from_le_bytes(vertex[4..8].try_into().unwrap()),
            f32::from_le_bytes(vertex[8..12].try_into().unwrap()),
        ];
        let Some(ndc) = banger_project_point(view_proj, point) else {
            continue;
        };
        if ndc[0].abs() > 1.08 || ndc[1].abs() > 1.08 || ndc[2] < -0.05 || ndc[2] > 1.05 {
            continue;
        }
        let px = (((ndc[0] * 0.5 + 0.5) * (width - 1) as f32).round() as i32).clamp(0, width as i32 - 1);
        let py = (((1.0 - (ndc[1] * 0.5 + 0.5)) * (height - 1) as f32).round() as i32).clamp(0, height as i32 - 1);
        let base = [
            (f32::from_le_bytes(vertex[20..24].try_into().unwrap()).clamp(0.0, 1.0) * 255.0) as u8,
            (f32::from_le_bytes(vertex[24..28].try_into().unwrap()).clamp(0.0, 1.0) * 255.0) as u8,
            (f32::from_le_bytes(vertex[28..32].try_into().unwrap()).clamp(0.0, 1.0) * 255.0) as u8,
        ];
        let color = [
            ((base[0] as u16 + seed_color[0] as u16) / 2) as u8,
            ((base[1] as u16 + seed_color[1] as u16) / 2) as u8,
            ((base[2] as u16 + seed_color[2] as u16) / 2) as u8,
        ];
        for oy in -1..=1 {
            for ox in -1..=1 {
                let x = (px + ox).clamp(0, width as i32 - 1) as usize;
                let y = (py + oy).clamp(0, height as i32 - 1) as usize;
                let offset = (y * width as usize + x) * 4;
                rgba[offset] = color[0].max(8);
                rgba[offset + 1] = color[1].max(8);
                rgba[offset + 2] = color[2].max(8);
                rgba[offset + 3] = 255;
            }
        }
    }
    let mut nonblack_pixel_count = 0u32;
    let mut non_fallback_blue_pixel_count = 0u32;
    for pixel in rgba.chunks_exact(4) {
        let visible = pixel[0] > 6 || pixel[1] > 6 || pixel[2] > 6;
        if visible {
            nonblack_pixel_count += 1;
            let fallback_blue = pixel[2] > pixel[0].saturating_add(32) && pixel[2] > pixel[1].saturating_add(16);
            if !fallback_blue {
                non_fallback_blue_pixel_count += 1;
            }
        }
    }
    BangerMapsCpuPreviewGate {
        nonblack_pixel_count,
        non_fallback_blue_pixel_count,
        frame_hash: sha256_hex(&rgba),
        width,
        height,
    }
}

#[cfg(target_os = "windows")]
fn banger_hash_color(seed: &str) -> [u8; 3] {
    let hash = sha256_hex(seed.as_bytes());
    [
        u8::from_str_radix(&hash[0..2], 16).unwrap_or(180),
        u8::from_str_radix(&hash[2..4], 16).unwrap_or(160),
        u8::from_str_radix(&hash[4..6], 16).unwrap_or(120),
    ]
}

#[cfg(target_os = "windows")]
fn banger_project_point(matrix: [f32; 16], point: [f32; 3]) -> Option<[f32; 3]> {
    let x = matrix[0] * point[0] + matrix[4] * point[1] + matrix[8] * point[2] + matrix[12];
    let y = matrix[1] * point[0] + matrix[5] * point[1] + matrix[9] * point[2] + matrix[13];
    let z = matrix[2] * point[0] + matrix[6] * point[1] + matrix[10] * point[2] + matrix[14];
    let w = matrix[3] * point[0] + matrix[7] * point[1] + matrix[11] * point[2] + matrix[15];
    if !w.is_finite() || w.abs() < 0.00001 {
        return None;
    }
    Some([x / w, y / w, z / w])
}

#[cfg(target_os = "windows")]
fn banger_maps_first_tile_render_mesh_bytes_from_ingest(
    ingest: &BangerMapsRootIngestProjection,
) -> Result<BangerRenderMeshBytes, String> {
    banger_maps_visible_tile_batch_render_mesh_bytes_from_ingest(ingest)
}

#[cfg(target_os = "windows")]
fn banger_maps_visible_tile_batch_render_mesh_bytes_from_ingest(
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
    let maps_render_space_transform = banger_maps_render_space_transform();
    let selected_records = banger_maps_draw_records_or_seed(ingest, maps_render_space_transform);
    let mut drawable_meshes = Vec::new();
    for record in selected_records.into_iter().take(banger_maps_visible_tile_batch_limit()) {
        match banger_maps_render_mesh_for_record(record, maps_render_space_transform) {
            Ok(mesh) => drawable_meshes.push(mesh),
            Err(error) => primitive_errors.push(format!("{}: {error}", record.tile_id)),
        }
    }
    if drawable_meshes.len() == 1 {
        return Ok(drawable_meshes.remove(0));
    }
    if drawable_meshes.len() > 1 {
        return banger_maps_concat_visible_tile_meshes(drawable_meshes);
    }
    Err(format!(
        "Banger Maps native render blocked: no drawable glTF primitive after staging ({})",
        primitive_errors.join("; ")
    ))
}

#[cfg(target_os = "windows")]
fn banger_maps_render_mesh_for_record(
    record: &BangerMapsContentDecodeRecord,
    maps_render_space_transform: [f64; 16],
) -> Result<BangerRenderMeshBytes, String> {
    let bytes = fs::read(&record.cache_path).map_err(|error| format!("read failed: {error}"))?;
    match record.container {
        "b3dm" => decode_banger_b3dm(&bytes).and_then(|(b3dm, glb_bytes)| {
            let decoded = decode_banger_glb_full(glb_bytes)?;
            banger_maps_render_mesh_from_gltf(
                &decoded.gltf_value,
                decoded.bin_chunk,
                maps_render_space_transform,
                banger_b3dm_tile_content_transform(record.tile_global_transform, b3dm.rtc_center),
            )
        }),
        "glb" => decode_banger_glb_full(&bytes).and_then(|decoded| {
            banger_maps_render_mesh_from_gltf(
                &decoded.gltf_value,
                decoded.bin_chunk,
                maps_render_space_transform,
                record.tile_global_transform,
            )
        }),
        _ => Err(format!("render mesh unsupported container {}", record.container)),
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_material_texture_resources_for_records(
    records: &[&BangerMapsContentDecodeRecord],
) -> Result<(Option<Vec<u8>>, Vec<Vec<u8>>), String> {
    let mut material_resource_bytes = Vec::new();
    let mut texture_staging_bytes = Vec::new();
    for record in records {
        let bytes = fs::read(&record.cache_path)
            .map_err(|error| format!("{} resource read failed: {error}", record.tile_id))?;
        let (gltf_value, bin_chunk) = match record.container {
            "b3dm" => {
                let (_, glb_bytes) = decode_banger_b3dm(&bytes)?;
                let decoded = decode_banger_glb_full(glb_bytes)?;
                (decoded.gltf_value, decoded.bin_chunk)
            }
            "glb" => {
                let decoded = decode_banger_glb_full(&bytes)?;
                (decoded.gltf_value, decoded.bin_chunk)
            }
            _ => continue,
        };
        let (_, materials, textures) = stage_banger_gltf_payload(&gltf_value, bin_chunk)?;
        if let Some(bytes) = banger_maps_material_resource_bytes(&materials) {
            material_resource_bytes.extend_from_slice(&bytes);
        }
        texture_staging_bytes.extend(banger_maps_texture_staging_resource_bytes(
            &gltf_value,
            bin_chunk,
            &textures,
        )?);
    }
    Ok((
        (!material_resource_bytes.is_empty()).then_some(material_resource_bytes),
        texture_staging_bytes,
    ))
}

#[cfg(target_os = "windows")]
fn banger_maps_visible_tile_batch_limit() -> usize {
    env::var("FORGE_BANGER_MAPS_VISIBLE_DRAW_BATCH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8)
        .min(64)
}

#[cfg(target_os = "windows")]
fn banger_maps_concat_visible_tile_meshes(
    meshes: Vec<BangerRenderMeshBytes>,
) -> Result<BangerRenderMeshBytes, String> {
    let mut vertex_bytes = Vec::new();
    let mut index_bytes = Vec::new();
    let total_vertex_count = meshes
        .iter()
        .map(|mesh| mesh.vertex_bytes.len() / BANGER_RENDER_VERTEX_STRIDE_BYTES)
        .sum::<usize>();
    let mut bounds = BangerMeshBounds::empty();
    let target_index_format = if total_vertex_count > u16::MAX as usize + 1
        || meshes.iter().any(|mesh| mesh.index_format == BangerRenderIndexFormat::Uint32)
    {
        BangerRenderIndexFormat::Uint32
    } else {
        BangerRenderIndexFormat::Uint16
    };
    for mesh in meshes {
        let vertex_base = vertex_bytes.len() / BANGER_RENDER_VERTEX_STRIDE_BYTES;
        if mesh.bounds.valid() {
            bounds.include(mesh.bounds.min);
            bounds.include(mesh.bounds.max);
        }
        vertex_bytes.extend_from_slice(&mesh.vertex_bytes);
        for local_index in banger_render_indices_to_u32(&mesh.index_bytes, mesh.index_format) {
            let global_index = local_index as usize + vertex_base;
            match target_index_format {
                BangerRenderIndexFormat::Uint16 => {
                    if global_index > u16::MAX as usize {
                        return Err(format!("visible tile batch u16 index overflow at vertex {global_index}"));
                    }
                    index_bytes.extend_from_slice(&(global_index as u16).to_le_bytes());
                }
                BangerRenderIndexFormat::Uint32 => {
                    index_bytes.extend_from_slice(&(global_index as u32).to_le_bytes());
                }
            }
        }
    }
    Ok(BangerRenderMeshBytes {
        bounds,
        vertex_bytes,
        index_bytes,
        index_format: target_index_format,
        instance_bytes: banger_maps_tile_instance_bytes(),
        source: "banger_maps_3d_tiles_visible_tile_batch",
    })
}

#[cfg(target_os = "windows")]
fn banger_maps_draw_records_or_seed<'a>(
    ingest: &'a BangerMapsRootIngestProjection,
    maps_render_space_transform: [f64; 16],
) -> Vec<&'a BangerMapsContentDecodeRecord> {
    let visible = banger_maps_visible_draw_records(ingest, maps_render_space_transform);
    if !visible.is_empty() {
        return visible;
    }
    ingest
        .content_decode
        .records
        .iter()
        .filter(|record| record.error.is_none())
        .filter(|record| matches!(record.container, "b3dm" | "glb"))
        .take(banger_maps_visible_tile_batch_limit())
        .collect()
}

#[cfg(target_os = "windows")]
fn banger_maps_visible_draw_records<'a>(
    ingest: &'a BangerMapsRootIngestProjection,
    maps_render_space_transform: [f64; 16],
) -> Vec<&'a BangerMapsContentDecodeRecord> {
    let camera = banger_maps_cpu_camera();
    let mut scored = ingest
        .content_decode
        .records
        .iter()
        .filter(|record| record.error.is_none())
        .filter(|record| matches!(record.container, "b3dm" | "glb"))
        .map(|record| {
            let score = banger_maps_visible_tile_score(record, ingest, maps_render_space_transform, &camera);
            (record, score)
        })
        .filter(|(_, score)| *score > 0.0)
        .collect::<Vec<_>>();
    scored.sort_by(|(_, left), (_, right)| {
        right
            .partial_cmp(left)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.into_iter().map(|(record, _)| record).collect()
}

#[cfg(target_os = "windows")]
fn banger_maps_visible_tile_score(
    record: &BangerMapsContentDecodeRecord,
    ingest: &BangerMapsRootIngestProjection,
    maps_render_space_transform: [f64; 16],
    camera: &BangerMapsCpuCamera,
) -> f64 {
    let tile = ingest
        .traversal_seed
        .tiles
        .iter()
        .find(|tile| tile.tile_id == record.tile_id);
    let world_transform =
        banger_mat4_mul_f64(maps_render_space_transform, record.tile_global_transform);
    let center = banger_transform_point64_f64(world_transform, [0.0, 0.0, 0.0]);
    let to_center = [
        center[0] - camera.eye[0],
        center[1] - camera.eye[1],
        center[2] - camera.eye[2],
    ];
    let geometric_error = tile.and_then(|tile| tile.geometric_error).unwrap_or(1.0).max(0.001);
    let radius = banger_maps_estimated_tile_radius(tile, geometric_error);
    let has_bounding_volume = tile
        .map(|tile| tile.bounding_volume_kind.as_str() != "none")
        .unwrap_or(false);
    if has_bounding_volume && !banger_maps_sphere_intersects_cpu_frustum(to_center, radius, camera) {
        return 0.0;
    }
    let distance = if has_bounding_volume {
        banger_vec3_dot_f64(to_center, camera.forward).max(camera.near)
    } else {
        banger_vec3_length_f64([
            camera.target[0] - camera.eye[0],
            camera.target[1] - camera.eye[1],
            camera.target[2] - camera.eye[2],
        ])
    };
    let screen_space_error =
        geometric_error * camera.viewport_height / (2.0 * distance * (camera.fovy_radians * 0.5).tan());
    let depth_bias = tile.map(|tile| tile.depth as f64 * 0.001).unwrap_or(0.0);
    screen_space_error + depth_bias
}

#[cfg(target_os = "windows")]
struct BangerMapsCpuCamera {
    eye: [f64; 3],
    target: [f64; 3],
    forward: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    fovy_radians: f64,
    aspect: f64,
    near: f64,
    far: f64,
    viewport_height: f64,
}

#[cfg(target_os = "windows")]
fn banger_maps_cpu_camera() -> BangerMapsCpuCamera {
    let viewport_width = env::var("FORGE_BANGER_VIEWPORT_WIDTH")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1920.0);
    let viewport_height = env::var("FORGE_BANGER_VIEWPORT_HEIGHT")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1080.0);
    let eye = [19.5, 9.6, 26.0];
    let target = [0.0_f64, -0.35, 2.5];
    let forward = banger_vec3_normalize_f64([
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    ]);
    let right = banger_vec3_normalize_f64(banger_vec3_cross_f64(forward, [0.0, 1.0, 0.0]));
    let up = banger_vec3_cross_f64(right, forward);
    BangerMapsCpuCamera {
        eye,
        target,
        forward,
        right,
        up,
        fovy_radians: 58.0_f64.to_radians(),
        aspect: (viewport_width / viewport_height).clamp(0.25, 4.0),
        near: 0.05,
        far: 280.0,
        viewport_height,
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_estimated_tile_radius(tile: Option<&BangerMapsTraversalTile>, geometric_error: f64) -> f64 {
    let depth_scale = tile.map(|tile| 1.0 / ((tile.depth + 1) as f64)).unwrap_or(1.0);
    (geometric_error.max(1.0) * depth_scale * 2.0).clamp(1.0, 4096.0)
}

#[cfg(target_os = "windows")]
fn banger_maps_sphere_intersects_cpu_frustum(
    to_center: [f64; 3],
    radius: f64,
    camera: &BangerMapsCpuCamera,
) -> bool {
    let depth = banger_vec3_dot_f64(to_center, camera.forward);
    if depth + radius < camera.near || depth - radius > camera.far {
        return false;
    }
    let tan_y = (camera.fovy_radians * 0.5).tan();
    let tan_x = tan_y * camera.aspect;
    let x = banger_vec3_dot_f64(to_center, camera.right).abs();
    let y = banger_vec3_dot_f64(to_center, camera.up).abs();
    x <= depth.max(camera.near) * tan_x + radius && y <= depth.max(camera.near) * tan_y + radius
}

#[cfg(target_os = "windows")]
fn banger_vec3_length_f64(value: [f64; 3]) -> f64 {
    banger_vec3_dot_f64(value, value).sqrt()
}

#[cfg(target_os = "windows")]
fn banger_vec3_dot_f64(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(target_os = "windows")]
fn banger_vec3_cross_f64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(target_os = "windows")]
fn banger_vec3_normalize_f64(value: [f64; 3]) -> [f64; 3] {
    let length = banger_vec3_length_f64(value).max(0.0001);
    [value[0] / length, value[1] / length, value[2] / length]
}

#[cfg(target_os = "windows")]
fn banger_maps_render_mesh_from_gltf(
    gltf: &Value,
    bin_chunk: &[u8],
    maps_render_space_transform: [f64; 16],
    tile_global_transform: [f64; 16],
) -> Result<BangerRenderMeshBytes, String> {
    let meshes = gltf
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or_else(|| "gltf meshes array missing".to_string())?;
    let mesh_node_transforms = banger_gltf_mesh_node_global_transforms(gltf);
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let primitives = mesh.get("primitives").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        let gltf_node_transform = mesh_node_transforms
            .get(&mesh_index)
            .copied()
            .unwrap_or_else(banger_identity_mat4_f64);
        let render_transform = banger_mat4_mul_f64(
            banger_mat4_mul_f64(
                banger_mat4_mul_f64(maps_render_space_transform, tile_global_transform),
                banger_gltf_y_up_to_z_up_matrix_f64(),
            ),
            gltf_node_transform,
        );
        let mut drawable_primitives = Vec::new();
        let mut primitive_errors = Vec::new();
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            match banger_maps_render_mesh_from_primitive(gltf, bin_chunk, primitive, render_transform) {
                Ok(mesh) => drawable_primitives.push(mesh),
                Err(error) => {
                    primitive_errors.push(format!("mesh {mesh_index} primitive {primitive_index}: {error}"));
                }
            }
        }
        if drawable_primitives.len() == 1 {
            return Ok(drawable_primitives.remove(0));
        }
        if drawable_primitives.len() > 1 {
            return banger_maps_concat_visible_tile_meshes(drawable_primitives);
        }
        let _ = primitive_errors;
    }
    Err("no drawable glTF primitive could be converted to Banger render mesh".to_string())
}

#[cfg(target_os = "windows")]
fn banger_maps_render_mesh_from_primitive(
    gltf: &Value,
    bin_chunk: &[u8],
    primitive: &Value,
    render_transform: [f64; 16],
) -> Result<BangerRenderMeshBytes, String> {
    let mode = primitive.get("mode").and_then(Value::as_u64).unwrap_or(4);
    if mode != 4 {
        return Err(format!("render primitive mode {mode} is not TRIANGLES"));
    }
    if let Some(draco) = primitive
        .get("extensions")
        .and_then(|extensions| extensions.get("KHR_draco_mesh_compression"))
    {
        return banger_maps_render_mesh_from_draco_primitive(gltf, bin_chunk, primitive, draco, render_transform);
    }
    let attributes = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| "primitive missing attributes".to_string())?;
    let position_accessor = attributes
        .get("POSITION")
        .and_then(Value::as_u64)
        .ok_or_else(|| "primitive missing POSITION accessor".to_string())? as usize;
    let position = banger_gltf_accessor_stage(gltf, bin_chunk, position_accessor)?;
    banger_maps_float_vec3_accessor_values(&position, "POSITION")?;
    let texcoords = attributes
        .get("TEXCOORD_0")
        .and_then(Value::as_u64)
        .map(|accessor| banger_gltf_accessor_stage(gltf, bin_chunk, accessor as usize))
        .transpose()?
        .map(|stage| banger_maps_float_vec2_accessor_values(&stage, "TEXCOORD_0"))
        .transpose()?;
    let normals = attributes
        .get("NORMAL")
        .and_then(Value::as_u64)
        .map(|accessor| banger_gltf_accessor_stage(gltf, bin_chunk, accessor as usize))
        .transpose()?
        .map(|stage| banger_maps_float_vec3_accessor_values(&stage, "NORMAL"))
        .transpose()?;
    let tangents = attributes
        .get("TANGENT")
        .and_then(Value::as_u64)
        .map(|accessor| banger_gltf_accessor_stage(gltf, bin_chunk, accessor as usize))
        .transpose()?
        .map(|stage| banger_maps_float_vec4_accessor_values(&stage, "TANGENT"))
        .transpose()?;
    let material_color = primitive
        .get("material")
        .and_then(Value::as_u64)
        .and_then(|index| banger_gltf_material_base_color(gltf, index as usize))
        .unwrap_or([0.54, 0.78, 0.92, 1.0]);
    let material_index = primitive.get("material").and_then(Value::as_u64).unwrap_or(0) as f32;
    let vertex_bytes = banger_maps_position_accessor_to_render_vertices(
        &position,
        texcoords.as_deref(),
        normals.as_deref(),
        tangents.as_deref(),
        material_color,
        material_index,
        render_transform,
    )?;
    let (index_bytes, index_format) = match primitive.get("indices").and_then(Value::as_u64).map(|value| value as usize) {
        Some(index_accessor) => banger_maps_index_bytes_from_accessor(gltf, bin_chunk, index_accessor)?,
        None => banger_maps_generated_index_bytes(position.count),
    };
    Ok(BangerRenderMeshBytes {
        bounds: banger_mesh_bounds_from_vertex_bytes(&vertex_bytes),
        vertex_bytes,
        index_bytes,
        index_format,
        instance_bytes: banger_maps_tile_instance_bytes(),
        source: "banger_maps_3d_tiles_gltf_first_primitive",
    })
}

#[cfg(target_os = "windows")]
fn banger_maps_render_mesh_from_draco_primitive(
    gltf: &Value,
    bin_chunk: &[u8],
    primitive: &Value,
    draco: &Value,
    render_transform: [f64; 16],
) -> Result<BangerRenderMeshBytes, String> {
    let decoded = banger_decode_draco_primitive(gltf, bin_chunk, primitive, draco)?;
    let position = decoded
        .attributes
        .get("POSITION")
        .ok_or_else(|| "Draco render primitive missing POSITION".to_string())?;
    banger_maps_float_vec3_accessor_values(position, "POSITION")?;
    let texcoords = decoded
        .attributes
        .get("TEXCOORD_0")
        .map(|stage| banger_maps_float_vec2_accessor_values(stage, "TEXCOORD_0"))
        .transpose()?;
    let normals = decoded
        .attributes
        .get("NORMAL")
        .map(|stage| banger_maps_float_vec3_accessor_values(stage, "NORMAL"))
        .transpose()?;
    let tangents = decoded
        .attributes
        .get("TANGENT")
        .map(|stage| banger_maps_float_vec4_accessor_values(stage, "TANGENT"))
        .transpose()?;
    let material_color = primitive
        .get("material")
        .and_then(Value::as_u64)
        .and_then(|index| banger_gltf_material_base_color(gltf, index as usize))
        .unwrap_or([0.54, 0.78, 0.92, 1.0]);
    let material_index = primitive.get("material").and_then(Value::as_u64).unwrap_or(0) as f32;
    let vertex_bytes = banger_maps_position_accessor_to_render_vertices(
        position,
        texcoords.as_deref(),
        normals.as_deref(),
        tangents.as_deref(),
        material_color,
        material_index,
        render_transform,
    )?;
    let (index_bytes, index_format) = banger_maps_index_bytes_from_draco(&decoded)?;
    Ok(BangerRenderMeshBytes {
        bounds: banger_mesh_bounds_from_vertex_bytes(&vertex_bytes),
        vertex_bytes,
        index_bytes,
        index_format,
        instance_bytes: banger_maps_tile_instance_bytes(),
        source: "banger_maps_3d_tiles_draco_first_primitive",
    })
}

#[cfg(target_os = "windows")]
fn banger_maps_position_accessor_to_render_vertices(
    position: &BangerGltfAccessorStage,
    texcoords: Option<&[[f32; 2]]>,
    normals: Option<&[[f32; 3]]>,
    tangents: Option<&[[f32; 4]]>,
    material_color: [f32; 4],
    material_index: f32,
    render_transform: [f64; 16],
) -> Result<Vec<u8>, String> {
    let positions = banger_maps_float_vec3_accessor_values(position, "POSITION")?
        .into_iter()
        .map(|position| banger_transform_point_f64(render_transform, position))
        .collect::<Vec<_>>();
    if texcoords.as_ref().is_some_and(|values| values.len() != positions.len()) {
        return Err("render TEXCOORD_0 count must match POSITION count".to_string());
    }
    if normals.as_ref().is_some_and(|values| values.len() != positions.len()) {
        return Err("render NORMAL count must match POSITION count".to_string());
    }
    if tangents.as_ref().is_some_and(|values| values.len() != positions.len()) {
        return Err("render TANGENT count must match POSITION count".to_string());
    }
    let mut bytes = Vec::with_capacity(positions.len() * BANGER_RENDER_VERTEX_STRIDE_BYTES);
    for (index, position) in positions.into_iter().enumerate() {
        if !position.iter().all(|value| value.is_finite()) {
            return Err("render position became non-finite after ECEF/ENU transform".to_string());
        }
        let mapped = [position[0] as f32, position[1] as f32, position[2] as f32];
        if !mapped.iter().all(|value| value.is_finite()) {
            return Err("render position exceeded f32 range after ECEF/ENU transform".to_string());
        }
        let uv = texcoords
            .and_then(|values| values.get(index).copied())
            .unwrap_or([
                (mapped[0] * 0.0125).fract().abs(),
                (mapped[2] * 0.0125).fract().abs(),
            ]);
        let normal = normals
            .and_then(|values| values.get(index).copied())
            .map(|normal| banger_transform_normal_f64(render_transform, normal))
            .unwrap_or_else(|| banger_fallback_render_normal(mapped));
        let tangent = tangents
            .and_then(|values| values.get(index).copied())
            .map(|tangent| banger_transform_tangent_f64(render_transform, tangent, normal))
            .unwrap_or_else(|| banger_fallback_render_tangent(normal));
        for value in [
            mapped[0],
            mapped[1],
            mapped[2],
            uv[0],
            uv[1],
            material_color[0],
            material_color[1],
            material_color[2],
            material_index,
            normal[0],
            normal[1],
            normal[2],
            tangent[0],
            tangent[1],
            tangent[2],
            tangent[3],
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn banger_maps_index_bytes_from_draco(
    decoded: &BangerDecodedDracoPrimitive,
) -> Result<(Vec<u8>, BangerRenderIndexFormat), String> {
    match decoded.index_format {
        "uint16" => Ok((decoded.index_bytes.clone(), BangerRenderIndexFormat::Uint16)),
        "uint32" => Ok((decoded.index_bytes.clone(), BangerRenderIndexFormat::Uint32)),
        other => Err(format!("unsupported Draco render index format {other}")),
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_index_bytes_from_accessor(
    gltf: &Value,
    bin_chunk: &[u8],
    accessor_index: usize,
) -> Result<(Vec<u8>, BangerRenderIndexFormat), String> {
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
            Ok((bytes, BangerRenderIndexFormat::Uint16))
        }
        5123 => Ok((indices.bytes, BangerRenderIndexFormat::Uint16)),
        5125 => Ok((indices.bytes, BangerRenderIndexFormat::Uint32)),
        other => Err(format!("unsupported render index component type {other}")),
    }
}

#[cfg(target_os = "windows")]
fn banger_maps_generated_index_bytes(vertex_count: usize) -> (Vec<u8>, BangerRenderIndexFormat) {
    if vertex_count <= u16::MAX as usize + 1 {
        let mut bytes = Vec::with_capacity(vertex_count * 2);
        for index in 0..vertex_count {
            bytes.extend_from_slice(&(index as u16).to_le_bytes());
        }
        return (bytes, BangerRenderIndexFormat::Uint16);
    }
    let mut bytes = Vec::with_capacity(vertex_count * 4);
    for index in 0..vertex_count {
        bytes.extend_from_slice(&(index as u32).to_le_bytes());
    }
    (bytes, BangerRenderIndexFormat::Uint32)
}

#[cfg(target_os = "windows")]
fn banger_mesh_bounds_from_vertex_bytes(vertex_bytes: &[u8]) -> BangerMeshBounds {
    let mut bounds = BangerMeshBounds::empty();
    for vertex in vertex_bytes.chunks_exact(BANGER_RENDER_VERTEX_STRIDE_BYTES) {
        let x = f32::from_le_bytes(vertex[0..4].try_into().unwrap());
        let y = f32::from_le_bytes(vertex[4..8].try_into().unwrap());
        let z = f32::from_le_bytes(vertex[8..12].try_into().unwrap());
        if x.is_finite() && y.is_finite() && z.is_finite() {
            bounds.include([x, y, z]);
        }
    }
    if bounds.valid() {
        bounds
    } else {
        BangerMeshBounds {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        }
    }
}

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

fn banger_identity_mat4_f64() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn banger_tile_transform_matrix(tile: &Value) -> [f64; 16] {
    let Some(values) = tile.get("transform").and_then(Value::as_array) else {
        return banger_identity_mat4_f64();
    };
    if values.len() != 16 {
        return banger_identity_mat4_f64();
    }
    let mut matrix = [0.0; 16];
    for (index, value) in values.iter().enumerate() {
        let Some(number) = value.as_f64().filter(|number| number.is_finite()) else {
            return banger_identity_mat4_f64();
        };
        matrix[index] = number;
    }
    matrix
}

fn banger_transform_hash(transform: &[f64; 16]) -> String {
    sha256_hex(
        transform
            .iter()
            .map(|value| format!("{value:.9};"))
            .collect::<String>()
            .as_bytes(),
    )
}

fn banger_env_f64(name: &str) -> Option<f64> {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn banger_maps_render_space_transform() -> [f64; 16] {
    let contract = BangerMapsTilesetContract::google_photorealistic_default();
    let meters_to_world =
        banger_env_f64("FORGE_BANGER_MAPS_METERS_TO_WORLD_SCALE").unwrap_or(0.04);
    banger_mat4_mul_f64(
        banger_scale_mat4_f64([meters_to_world, meters_to_world, meters_to_world]),
        banger_ecef_to_enu_matrix(&contract.georeference),
    )
}

fn banger_b3dm_tile_content_transform(
    tile_global_transform: [f64; 16],
    rtc_center: Option<[f64; 3]>,
) -> [f64; 16] {
    match rtc_center {
        Some(center) => banger_mat4_mul_f64(tile_global_transform, banger_translation_mat4_f64(center)),
        None => tile_global_transform,
    }
}

fn banger_wgs84_geodetic_to_ecef(
    latitude_degrees: f64,
    longitude_degrees: f64,
    height_meters: f64,
) -> [f64; 3] {
    let semi_major_axis = 6_378_137.0_f64;
    let flattening = 1.0 / 298.257_223_563_f64;
    let eccentricity_squared = flattening * (2.0 - flattening);
    let latitude = latitude_degrees.to_radians();
    let longitude = longitude_degrees.to_radians();
    let sin_latitude = latitude.sin();
    let cos_latitude = latitude.cos();
    let sin_longitude = longitude.sin();
    let cos_longitude = longitude.cos();
    let prime_vertical_radius =
        semi_major_axis / (1.0 - eccentricity_squared * sin_latitude * sin_latitude).sqrt();
    [
        (prime_vertical_radius + height_meters) * cos_latitude * cos_longitude,
        (prime_vertical_radius + height_meters) * cos_latitude * sin_longitude,
        (prime_vertical_radius * (1.0 - eccentricity_squared) + height_meters) * sin_latitude,
    ]
}

fn banger_ecef_to_enu_matrix(georeference: &BangerMapsGeoreference) -> [f64; 16] {
    let origin = banger_wgs84_geodetic_to_ecef(
        georeference.origin_latitude,
        georeference.origin_longitude,
        georeference.origin_height_meters as f64,
    );
    let latitude = georeference.origin_latitude.to_radians();
    let longitude = georeference.origin_longitude.to_radians();
    let sin_latitude = latitude.sin();
    let cos_latitude = latitude.cos();
    let sin_longitude = longitude.sin();
    let cos_longitude = longitude.cos();
    let east = [-sin_longitude, cos_longitude, 0.0];
    let north = [
        -sin_latitude * cos_longitude,
        -sin_latitude * sin_longitude,
        cos_latitude,
    ];
    let up = [
        cos_latitude * cos_longitude,
        cos_latitude * sin_longitude,
        sin_latitude,
    ];
    [
        east[0],
        north[0],
        up[0],
        0.0,
        east[1],
        north[1],
        up[1],
        0.0,
        east[2],
        north[2],
        up[2],
        0.0,
        -dot3(east, origin),
        -dot3(north, origin),
        -dot3(up, origin),
        1.0,
    ]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn banger_gltf_y_up_to_z_up_matrix_f64() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, -1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn banger_mat4_mul_f64(a: [f64; 16], b: [f64; 16]) -> [f64; 16] {
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

fn banger_transform_point_f64(matrix: [f64; 16], point: [f32; 3]) -> [f64; 3] {
    banger_transform_point64_f64(matrix, [point[0] as f64, point[1] as f64, point[2] as f64])
}

fn banger_transform_normal_f64(matrix: [f64; 16], normal: [f32; 3]) -> [f32; 3] {
    let a = [matrix[0], matrix[1], matrix[2]];
    let b = [matrix[4], matrix[5], matrix[6]];
    let c = [matrix[8], matrix[9], matrix[10]];
    let cross_bc = cross3(b, c);
    let cross_ca = cross3(c, a);
    let cross_ab = cross3(a, b);
    let handedness = if dot3(a, cross_bc) < 0.0 { -1.0 } else { 1.0 };
    banger_normalize_vec3_f64([
        (normal[0] as f64 * cross_bc[0] + normal[1] as f64 * cross_ca[0] + normal[2] as f64 * cross_ab[0]) * handedness,
        (normal[0] as f64 * cross_bc[1] + normal[1] as f64 * cross_ca[1] + normal[2] as f64 * cross_ab[1]) * handedness,
        (normal[0] as f64 * cross_bc[2] + normal[1] as f64 * cross_ca[2] + normal[2] as f64 * cross_ab[2]) * handedness,
    ])
}

fn banger_transform_tangent_f64(matrix: [f64; 16], tangent: [f32; 4], normal: [f32; 3]) -> [f32; 4] {
    let transformed = [
        matrix[0] * tangent[0] as f64 + matrix[4] * tangent[1] as f64 + matrix[8] * tangent[2] as f64,
        matrix[1] * tangent[0] as f64 + matrix[5] * tangent[1] as f64 + matrix[9] * tangent[2] as f64,
        matrix[2] * tangent[0] as f64 + matrix[6] * tangent[1] as f64 + matrix[10] * tangent[2] as f64,
    ];
    let n = [normal[0] as f64, normal[1] as f64, normal[2] as f64];
    let projected = [
        transformed[0] - n[0] * dot3(transformed, n),
        transformed[1] - n[1] * dot3(transformed, n),
        transformed[2] - n[2] * dot3(transformed, n),
    ];
    let normalized = banger_normalize_vec3_f64(projected);
    let determinant_sign = if dot3([matrix[0], matrix[1], matrix[2]], cross3([matrix[4], matrix[5], matrix[6]], [matrix[8], matrix[9], matrix[10]])) < 0.0 {
        -1.0
    } else {
        1.0
    };
    let handedness = if tangent[3] < 0.0 { -1.0 } else { 1.0 } * determinant_sign;
    [normalized[0], normalized[1], normalized[2], handedness as f32]
}

fn banger_fallback_render_normal(position: [f32; 3]) -> [f32; 3] {
    banger_normalize_vec3_f64([position[0] as f64, position[1] as f64, position[2] as f64])
}

fn banger_fallback_render_tangent(normal: [f32; 3]) -> [f32; 4] {
    let n = [normal[0] as f64, normal[1] as f64, normal[2] as f64];
    let reference = if normal[1].abs() < 0.92 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let tangent = banger_normalize_vec3_f64(cross3(reference, n));
    [tangent[0], tangent[1], tangent[2], 1.0]
}

fn banger_normalize_vec3_f64(vector: [f64; 3]) -> [f32; 3] {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if !length.is_finite() || length <= 0.000001 {
        return [0.0, 1.0, 0.0];
    }
    [
        (vector[0] / length) as f32,
        (vector[1] / length) as f32,
        (vector[2] / length) as f32,
    ]
}

fn banger_transform_point64_f64(matrix: [f64; 16], point: [f64; 3]) -> [f64; 3] {
    let x = point[0];
    let y = point[1];
    let z = point[2];
    let w = matrix[3] * x + matrix[7] * y + matrix[11] * z + matrix[15];
    let inv_w = if w.abs() > f64::EPSILON { 1.0 / w } else { 1.0 };
    [
        (matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12]) * inv_w,
        (matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13]) * inv_w,
        (matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14]) * inv_w,
    ]
}

#[cfg(target_os = "windows")]
fn banger_gltf_mesh_node_global_transforms(gltf: &Value) -> HashMap<usize, [f64; 16]> {
    let nodes = gltf.get("nodes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut transforms = HashMap::new();
    let scene_index = gltf.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
    let scene_roots = gltf
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| scenes.get(scene_index))
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .map(Vec::as_slice);
    if let Some(scene_roots) = scene_roots {
        for node_index in scene_roots.iter().filter_map(Value::as_u64).map(|value| value as usize) {
            banger_collect_gltf_node_transforms(nodes, node_index, banger_identity_mat4_f64(), &mut transforms);
        }
    } else {
        for node_index in 0..nodes.len() {
            banger_collect_gltf_node_transforms(nodes, node_index, banger_identity_mat4_f64(), &mut transforms);
        }
    }
    transforms
}

#[cfg(target_os = "windows")]
fn banger_collect_gltf_node_transforms(
    nodes: &[Value],
    node_index: usize,
    parent_global: [f64; 16],
    transforms: &mut HashMap<usize, [f64; 16]>,
) {
    let Some(node) = nodes.get(node_index) else {
        return;
    };
    let global = banger_mat4_mul_f64(parent_global, banger_gltf_node_local_transform(node));
    if let Some(mesh_index) = node.get("mesh").and_then(Value::as_u64).map(|value| value as usize) {
        transforms.entry(mesh_index).or_insert(global);
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child_index in children.iter().filter_map(Value::as_u64).map(|value| value as usize) {
            banger_collect_gltf_node_transforms(nodes, child_index, global, transforms);
        }
    }
}

fn banger_gltf_node_local_transform(node: &Value) -> [f64; 16] {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array) {
        if matrix.len() == 16 {
            let mut out = [0.0; 16];
            for (index, value) in matrix.iter().enumerate() {
                let Some(number) = value.as_f64().filter(|number| number.is_finite()) else {
                    return banger_identity_mat4_f64();
                };
                out[index] = number;
            }
            return out;
        }
    }
    let translation = banger_json_vec3(node.get("translation"), [0.0, 0.0, 0.0]);
    let rotation = banger_json_vec4(node.get("rotation"), [0.0, 0.0, 0.0, 1.0]);
    let scale = banger_json_vec3(node.get("scale"), [1.0, 1.0, 1.0]);
    banger_mat4_mul_f64(
        banger_translation_mat4_f64(translation),
        banger_mat4_mul_f64(banger_quaternion_mat4_f64(rotation), banger_scale_mat4_f64(scale)),
    )
}

fn banger_json_vec3(value: Option<&Value>, fallback: [f64; 3]) -> [f64; 3] {
    let Some(values) = value.and_then(Value::as_array) else {
        return fallback;
    };
    if values.len() != 3 {
        return fallback;
    }
    [
        values[0].as_f64().unwrap_or(fallback[0]),
        values[1].as_f64().unwrap_or(fallback[1]),
        values[2].as_f64().unwrap_or(fallback[2]),
    ]
}

fn banger_json_vec4(value: Option<&Value>, fallback: [f64; 4]) -> [f64; 4] {
    let Some(values) = value.and_then(Value::as_array) else {
        return fallback;
    };
    if values.len() != 4 {
        return fallback;
    }
    [
        values[0].as_f64().unwrap_or(fallback[0]),
        values[1].as_f64().unwrap_or(fallback[1]),
        values[2].as_f64().unwrap_or(fallback[2]),
        values[3].as_f64().unwrap_or(fallback[3]),
    ]
}

fn banger_translation_mat4_f64(translation: [f64; 3]) -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        translation[0], translation[1], translation[2], 1.0,
    ]
}

fn banger_scale_mat4_f64(scale: [f64; 3]) -> [f64; 16] {
    [
        scale[0], 0.0, 0.0, 0.0,
        0.0, scale[1], 0.0, 0.0,
        0.0, 0.0, scale[2], 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn banger_quaternion_mat4_f64(rotation: [f64; 4]) -> [f64; 16] {
    let [x, y, z, w] = rotation;
    let len = (x * x + y * y + z * z + w * w).sqrt();
    if len <= f64::EPSILON {
        return banger_identity_mat4_f64();
    }
    let (x, y, z, w) = (x / len, y / len, z / len, w / len);
    [
        1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w), 0.0,
        2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w), 0.0,
        2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y), 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

#[cfg(target_os = "windows")]
fn banger_cube_vertex_bytes() -> Vec<u8> {
    let vertices: [[f32; 16]; 8] = [
        [-0.75, -0.75, 0.75, 0.0, 0.0, 0.95, 0.18, 0.12, 0.0, -0.577, -0.577, 0.577, 1.0, 0.0, 0.0, 1.0],
        [0.75, -0.75, 0.75, 1.0, 0.0, 0.12, 0.82, 0.42, 0.0, 0.577, -0.577, 0.577, 1.0, 0.0, 0.0, 1.0],
        [0.75, 0.75, 0.75, 1.0, 1.0, 0.18, 0.44, 1.00, 0.0, 0.577, 0.577, 0.577, 1.0, 0.0, 0.0, 1.0],
        [-0.75, 0.75, 0.75, 0.0, 1.0, 0.98, 0.78, 0.16, 0.0, -0.577, 0.577, 0.577, 1.0, 0.0, 0.0, 1.0],
        [-0.75, -0.75, -0.75, 0.0, 0.0, 0.84, 0.26, 0.92, 0.0, -0.577, -0.577, -0.577, 1.0, 0.0, 0.0, 1.0],
        [0.75, -0.75, -0.75, 1.0, 0.0, 0.10, 0.72, 0.82, 0.0, 0.577, -0.577, -0.577, 1.0, 0.0, 0.0, 1.0],
        [0.75, 0.75, -0.75, 1.0, 1.0, 0.96, 0.42, 0.21, 0.0, 0.577, 0.577, -0.577, 1.0, 0.0, 0.0, 1.0],
        [-0.75, 0.75, -0.75, 0.0, 1.0, 0.66, 0.92, 0.24, 0.0, -0.577, 0.577, -0.577, 1.0, 0.0, 0.0, 1.0],
    ];
    let mut bytes = Vec::with_capacity(vertices.len() * BANGER_RENDER_VERTEX_STRIDE_BYTES);
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
fn banger_view_projection_matrix_for_bounds(
    time_seconds: f32,
    bounds: BangerMeshBounds,
    viewport_width: u32,
    viewport_height: u32,
) -> [f32; 16] {
    if !bounds.valid() {
        return banger_view_projection_matrix(time_seconds, viewport_width, viewport_height);
    }
    let aspect = (viewport_width as f32 / viewport_height.max(1) as f32).clamp(0.25, 4.0);
    let center = bounds.center();
    let radius = bounds.radius();
    let fovy = 55.0_f32.to_radians();
    let distance = (radius / (fovy * 0.5).tan()).max(radius * 2.4).max(8.0);
    let orbit = time_seconds * 0.055;
    let lateral = radius * 0.38 + distance * 0.08;
    let dolly = 1.0 + 0.035 * (time_seconds * 0.17).sin();
    let eye = [
        center[0] + lateral * orbit.cos(),
        center[1] + radius * 0.42 + 2.0 + radius * 0.04 * (time_seconds * 0.11).sin(),
        center[2] + distance * dolly + lateral * orbit.sin(),
    ];
    let near = (distance - radius * 1.8).max(0.02);
    let far = (distance + radius * 4.0).max(256.0);
    if env::var("FORGE_BANGER_MAPS_CAMERA_DEBUG").ok().as_deref() == Some("1")
        && !BANGER_MAPS_CAMERA_DEBUG_LOGGED.swap(true, Ordering::Relaxed)
    {
        eprintln!(
            "Banger Maps camera bounds center={center:?} radius={radius:.3} eye={eye:?} near={near:.3} far={far:.3}"
        );
    }
    let view = banger_look_at_rh(eye, center, [0.0, 1.0, 0.0]);
    let projection = banger_perspective_rh_zo(fovy, aspect, near, far);
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

#[cfg(target_os = "windows")]
fn parse_banger_native_host_command(line: &str) -> Option<BangerNativeHostCommand> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "resize" => {
            let x = parts.next()?.parse::<i32>().ok()?;
            let y = parts.next()?.parse::<i32>().ok()?;
            let width = parts.next()?.parse::<u32>().ok()?.clamp(64, 16384);
            let height = parts.next()?.parse::<u32>().ok()?.clamp(64, 16384);
            Some(BangerNativeHostCommand::Resize { x, y, width, height })
        }
        "shutdown" => Some(BangerNativeHostCommand::Shutdown),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn spawn_banger_native_host_command_reader() -> std::sync::mpsc::Receiver<BangerNativeHostCommand> {
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            let Some(command) = parse_banger_native_host_command(&line) else {
                continue;
            };
            let shutdown = matches!(command, BangerNativeHostCommand::Shutdown);
            if sender.send(command).is_err() || shutdown {
                break;
            }
        }
    });
    receiver
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

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_banger_native_host_control_commands() {
        match parse_banger_native_host_command("resize 12 34 800 450") {
            Some(BangerNativeHostCommand::Resize { x, y, width, height }) => {
                assert_eq!((x, y, width, height), (12, 34, 800, 450));
            }
            other => panic!("unexpected resize command: {other:?}"),
        }
        assert!(matches!(
            parse_banger_native_host_command("shutdown"),
            Some(BangerNativeHostCommand::Shutdown)
        ));
        assert!(matches!(
            parse_banger_native_host_command("resize 0 0 1 2"),
            Some(BangerNativeHostCommand::Resize { width: 64, height: 64, .. })
        ));
        assert!(parse_banger_native_host_command("noise").is_none());
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
        let sky_source = banger_sky_atmosphere_present_wgsl();
        assert!(sky_source.contains("rayleigh"));
        assert!(sky_source.contains("mie"));
        assert!(sky_source.contains("sun_disk"));
        assert!(sky_source.contains("FrameUniform"));
        let ssao_source = banger_screen_space_ambient_occlusion_present_wgsl();
        assert!(ssao_source.contains("texture_depth_2d"));
        assert!(ssao_source.contains("gbuffer_normal"));
        assert!(ssao_source.contains("banger_ssao_sample"));
        assert!(ssao_source.contains("ao_alpha"));
        let source = banger_native_first_scene_wgsl();
        assert!(source.contains("@vertex"));
        assert!(source.contains("@fragment"));
        assert!(source.contains("view_proj"));
        assert!(source.contains("@location(0) position"));
        assert!(source.contains("@location(1) uv"));
        assert!(source.contains("@location(2) color"));
        assert!(source.contains("@location(3) material_slot"));
        assert!(source.contains("@location(4) normal"));
        assert!(source.contains("@location(5) tangent"));
        assert!(source.contains("@location(6) model_0"));
        assert!(source.contains("@location(10) instance_tint"));
        assert!(source.contains("world_pos"));
        assert!(source.contains("material_kind"));
        assert!(source.contains("material_slot"));
        assert!(source.contains("banger_transform_normal"));
        assert!(source.contains("banger_transform_tangent"));
        assert!(source.contains("banger_tangent_space_detail_normal"));
        assert!(source.contains("out.normal_hint = world_normal"));
        assert!(source.contains("water_glint"));
        assert!(source.contains("banger_filmic_tonemap"));
        assert!(source.contains("banger_contact_ambient_occlusion"));
        assert!(source.contains("banger_microfacet_brdf"));
        assert!(source.contains("banger_distribution_ggx"));
        assert!(source.contains("banger_visibility_smith_ggx_fast"));
        assert!(source.contains("banger_fresnel_schlick"));
        assert!(source.contains("banger_environment_radiance"));
        assert!(source.contains("banger_environment_brdf_approx"));
        assert!(source.contains("banger_image_based_lighting"));
        assert!(source.contains("pbr_indirect"));
        assert!(source.contains("virtual_shadow_projection"));
        assert!(source.contains("banger_virtual_shadow_visibility"));
        assert!(source.contains("shadow_visibility"));
        assert!(source.contains("BangerMaterialRecord"));
        assert!(source.contains("normal_texture"));
        assert!(source.contains("normal_scale"));
        assert!(source.contains("maps_normal_texture"));
        assert!(source.contains("@group(0) @binding(5)"));
        assert!(source.contains("textureSample(maps_normal_texture"));
        assert!(source.contains("metallic_roughness_texture"));
        assert!(source.contains("maps_metallic_roughness_texture"));
        assert!(source.contains("@group(0) @binding(6)"));
        assert!(source.contains("metallic_roughness_sample.g"));
        assert!(source.contains("metallic_roughness_sample.b"));
        assert!(source.contains("occlusion_texture"));
        assert!(source.contains("occlusion_strength"));
        assert!(source.contains("maps_occlusion_texture"));
        assert!(source.contains("@group(0) @binding(7)"));
        assert!(source.contains("occlusion_sample"));
        assert!(source.contains("indirect_ao"));
        assert!(source.contains("emissive_texture"));
        assert!(source.contains("emissive_factor"));
        assert!(source.contains("maps_emissive_texture"));
        assert!(source.contains("@group(0) @binding(8)"));
        assert!(source.contains("textureSample(maps_emissive_texture"));
        assert!(source.contains("material_emissive"));
        assert!(source.contains("material_records"));
        assert!(source.contains("banger_material_record_for_kind"));
        assert!(source.contains("contact_ao"));
        assert!(source.contains("material_roughness"));
        assert!(source.contains("exposure"));
        assert!(source.contains("FrameUniform"));
        assert!(source.contains("@group(0) @binding(0)"));
        assert_eq!(sha256_hex(source.as_bytes()).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn animates_banger_bounds_camera_over_time() {
        fn matrix_bytes(matrix: &[f32; 16]) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(16 * 4);
            for value in matrix {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            bytes
        }
        let mut bounds = BangerMeshBounds::empty();
        bounds.include([-10.0, -2.0, -8.0]);
        bounds.include([14.0, 6.0, 18.0]);
        let first = banger_view_projection_matrix_for_bounds(0.0, bounds, 1280, 720);
        let later = banger_view_projection_matrix_for_bounds(12.0, bounds, 1280, 720);
        assert_ne!(sha256_hex(&matrix_bytes(&first)), sha256_hex(&matrix_bytes(&later)));
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
        assert_eq!(vertex_bytes.len(), 8 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
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
        let mesh = banger_maps_render_mesh_from_gltf(
            &decoded.gltf_value,
            decoded.bin_chunk,
            banger_identity_mat4_f64(),
            banger_identity_mat4_f64(),
        )
        .unwrap();
        assert_eq!(mesh.source, "banger_maps_3d_tiles_gltf_first_primitive");
        assert_eq!(mesh.index_format, BangerRenderIndexFormat::Uint16);
        assert_eq!(mesh.vertex_bytes.len(), 3 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
        assert_eq!(mesh.index_bytes.len(), 3 * 2);
        assert_eq!(mesh.instance_bytes.len(), 80);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[20..24].try_into().unwrap()), 0.7);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[24..28].try_into().unwrap()), 0.82);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[28..32].try_into().unwrap()), 0.9);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[32..36].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[36..40].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[40..44].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[44..48].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[48..52].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[52..56].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[56..60].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(mesh.vertex_bytes[60..64].try_into().unwrap()), 1.0);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[0..2].try_into().unwrap()), 0);
        assert_eq!(sha256_hex(&mesh.vertex_bytes).len(), 64);
        assert_eq!(sha256_hex(&mesh.index_bytes).len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn converts_all_tile_gltf_primitives_into_one_native_draw_mesh() {
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"material":0,"mode":4},{"attributes":{"POSITION":0},"indices":1,"material":1,"mode":4}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.7,0.82,0.9,1.0]}},{"pbrMetallicRoughness":{"baseColorFactor":[0.2,0.4,0.6,1.0]}}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36,"target":34962},{"buffer":0,"byteOffset":36,"byteLength":6,"target":34963}],"buffers":[{"byteLength":42}]}"#;
        let mut bin_chunk = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin_chunk.extend_from_slice(&value.to_le_bytes());
        }
        for index in [0u16, 1, 2] {
            bin_chunk.extend_from_slice(&index.to_le_bytes());
        }
        let glb = test_glb_with_json_bin(json, &bin_chunk);
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let mesh = banger_maps_render_mesh_from_gltf(
            &decoded.gltf_value,
            decoded.bin_chunk,
            banger_identity_mat4_f64(),
            banger_identity_mat4_f64(),
        )
        .unwrap();
        assert_eq!(mesh.source, "banger_maps_3d_tiles_visible_tile_batch");
        assert_eq!(mesh.index_format, BangerRenderIndexFormat::Uint16);
        assert_eq!(mesh.vertex_bytes.len(), 6 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
        assert_eq!(mesh.index_bytes.len(), 6 * 2);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[0..2].try_into().unwrap()), 0);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[6..8].try_into().unwrap()), 3);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[10..12].try_into().unwrap()), 5);
        let second_primitive_vertex = 3 * BANGER_RENDER_VERTEX_STRIDE_BYTES;
        assert_eq!(
            f32::from_le_bytes(mesh.vertex_bytes[second_primitive_vertex + 32..second_primitive_vertex + 36].try_into().unwrap()),
            1.0
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn keeps_uint32_tile_indices_for_native_draw_and_meshlets() {
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"material":0,"mode":4}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.7,0.82,0.9,1.0]}}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":1,"componentType":5125,"count":3,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36,"target":34962},{"buffer":0,"byteOffset":36,"byteLength":12,"target":34963}],"buffers":[{"byteLength":48}]}"#;
        let mut bin_chunk = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin_chunk.extend_from_slice(&value.to_le_bytes());
        }
        for index in [0u32, 1, 2] {
            bin_chunk.extend_from_slice(&index.to_le_bytes());
        }
        let glb = test_glb_with_json_bin(json, &bin_chunk);
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let mesh = banger_maps_render_mesh_from_gltf(
            &decoded.gltf_value,
            decoded.bin_chunk,
            banger_identity_mat4_f64(),
            banger_identity_mat4_f64(),
        )
        .unwrap();
        assert_eq!(mesh.index_format, BangerRenderIndexFormat::Uint32);
        assert_eq!(mesh.index_bytes.len(), 3 * 4);
        assert_eq!(u32::from_le_bytes(mesh.index_bytes[8..12].try_into().unwrap()), 2);
        let clusters = banger_meshlet_cluster_metadata_bytes(
            &mesh.vertex_bytes,
            &mesh.index_bytes,
            mesh.index_format,
            mesh.source,
        );
        assert_eq!(u32::from_le_bytes(clusters[36..40].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(clusters[44..48].try_into().unwrap()), 3);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolves_maps_root_url_from_direct_url_or_cesium_ion_endpoint() {
        let direct = banger_maps_root_url_from_values(
            Some(" https://example.test/tileset.json "),
            Some("https://broker.example.test/api/banger/cesium-ion-token"),
            None,
            "2275207",
        )
        .unwrap();
        assert_eq!(direct, "https://example.test/tileset.json");

        let cesium_endpoint = banger_cesium_ion_asset_endpoint_url(" 2275207 ", " cesium-token ");
        assert_eq!(
            cesium_endpoint,
            "https://api.cesium.com/v1/assets/2275207/endpoint?access_token=cesium-token"
        );
        assert_eq!(
            redact_url_secret(&cesium_endpoint),
            "https://api.cesium.com/v1/assets/2275207/endpoint?access_token=redacted"
        );

        let endpoint = serde_json::json!({
            "url": "https://assets.cesium.com/2275207/root.json?v=1",
            "accessToken": "tileset-session-token"
        });
        let root = banger_maps_cesium_root_url_from_endpoint_value(&endpoint, "2275207").unwrap();
        assert_eq!(
            root,
            "https://assets.cesium.com/2275207/root.json?v=1&access_token=tileset-session-token"
        );
        assert_eq!(
            redact_url_secret(&root),
            "https://assets.cesium.com/2275207/root.json?v=1&access_token=redacted"
        );

        let cesium_ion_options_endpoint = serde_json::json!({
            "externalType": "3DTILES",
            "type": "3DTILES",
            "options": {
                "url": "https://tile.googleapis.com/v1/3dtiles/root.json?key=cesium-session-key"
            }
        });
        let root = banger_maps_cesium_root_url_from_endpoint_value(&cesium_ion_options_endpoint, "2275207").unwrap();
        assert_eq!(
            redact_url_secret(&root),
            "https://tile.googleapis.com/v1/3dtiles/root.json?key=redacted"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn propagates_cesium_google_root_key_to_absolute_tile_content_urls() {
        let resolved = resolve_banger_tile_content_url(
            "https://tile.googleapis.com/v1/3dtiles/root.json?key=cesium-session-key",
            "https://tile.googleapis.com/v1/3dtiles/datasets/google/tileset/tile.b3dm",
        );
        assert_eq!(
            redact_url_secret(&resolved),
            "https://tile.googleapis.com/v1/3dtiles/datasets/google/tileset/tile.b3dm?key=redacted"
        );

        let with_existing_query = resolve_banger_tile_content_url(
            "https://tile.googleapis.com/v1/3dtiles/root.json?key=cesium-session-key",
            "https://tile.googleapis.com/v1/3dtiles/datasets/google/tileset/tile.b3dm?alt=media",
        );
        assert_eq!(
            redact_url_secret(&with_existing_query),
            "https://tile.googleapis.com/v1/3dtiles/datasets/google/tileset/tile.b3dm?alt=media&key=redacted"
        );
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
            "https://assets.cesium.com/2275207/root.json?access_token=secret",
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
        assert_eq!(mesh.vertex_bytes.len(), 3 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
        assert_eq!(mesh.index_bytes.len(), 3 * 2);
        assert_eq!(mesh.instance_bytes.len(), 80);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn batches_visible_maps_tiles_into_one_indexed_draw_mesh() {
        let cache_dir = env::temp_dir().join(format!(
            "forge-banger-render-batch-test-{}",
            sha256_hex(format!("{:?}", SystemTime::now()).as_bytes())
        ));
        let source_dir = cache_dir.join("source");
        fs::create_dir_all(&source_dir).unwrap();
        let root_path = source_dir.join("tileset.json");
        fs::write(source_dir.join("a.glb"), test_glb_bytes()).unwrap();
        fs::write(source_dir.join("b.glb"), test_glb_bytes()).unwrap();
        fs::write(
            &root_path,
            br#"{"asset":{"version":"1.1"},"root":{"geometricError":10,"children":[{"geometricError":10,"content":{"uri":"a.glb"}},{"geometricError":9,"content":{"uri":"b.glb"}}]}}"#,
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
        let mesh = banger_maps_visible_tile_batch_render_mesh_bytes_from_ingest(&projection).unwrap();
        assert_eq!(mesh.source, "banger_maps_3d_tiles_visible_tile_batch");
        assert_eq!(mesh.vertex_bytes.len(), 6 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
        assert_eq!(mesh.index_bytes.len(), 6 * 2);
        assert_eq!(mesh.instance_bytes.len(), 80);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[0..2].try_into().unwrap()), 0);
        assert_eq!(u16::from_le_bytes(mesh.index_bytes[6..8].try_into().unwrap()), 3);
        let selected_records = banger_maps_visible_draw_records(
            &projection,
            banger_maps_render_space_transform(),
        );
        let (material_bytes, texture_staging_bytes) =
            banger_maps_material_texture_resources_for_records(&selected_records).unwrap();
        assert_eq!(material_bytes.unwrap().len(), 2 * 32);
        assert_eq!(texture_staging_bytes.len(), 2);
        assert_eq!(texture_staging_bytes[0], vec![137, 80, 78, 71]);
        let texture_manifest = banger_maps_texture_resource_manifest_bytes(&texture_staging_bytes);
        assert_eq!(u32::from_le_bytes(texture_manifest[0..4].try_into().unwrap()), 0x4D_54_45_58);
        assert_eq!(u32::from_le_bytes(texture_manifest[8..12].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(texture_manifest[20..24].try_into().unwrap()), 4);
        assert_ne!(u32::from_le_bytes(texture_manifest[24..28].try_into().unwrap()), 0);
    }

    #[test]
    fn maps_contract_reports_first_native_indexed_tile_draw_ready() {
        let contract = BangerMapsTilesetContract::google_photorealistic_default();
        assert_eq!(
            contract.native_streamer.status,
            "native_visible_tile_batch_draw_ready_direct_tiles_required"
        );
        assert_eq!(
            contract.native_streamer.gpu_submission_stage,
            "visible_tile_batch_indexed_mesh_wgpu_draw_ready"
        );
        assert_eq!(
            contract.native_streamer.blocker,
            "screen_space_error_traversal_material_texture_streaming_required_for_full_cesium_parity"
        );
        assert_eq!(
            contract.native_streamer.georeference_stage,
            "wgs84_ecef_to_enu_floating_origin_live"
        );
        assert_eq!(contract.native_streamer.visual_fallback, "none_direct_tiles_required");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rejects_non_triangle_gltf_primitives_for_first_maps_draw_path() {
        let json = br#"{
            "asset": {"version": "2.0"},
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "mode": 1}]}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3"}],
            "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 24}],
            "buffers": [{"byteLength": 24}]
        }"#;
        let mut bin = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0] {
            bin.extend_from_slice(&value.to_le_bytes());
        }
        let glb = test_glb_with_json_bin(json, &bin);
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let error = match banger_maps_render_mesh_from_gltf(
            &decoded.gltf_value,
            decoded.bin_chunk,
            banger_identity_mat4_f64(),
            banger_identity_mat4_f64(),
        ) {
            Ok(_) => panic!("line primitive unexpectedly entered triangle draw path"),
            Err(error) => error,
        };
        assert!(error.contains("no drawable glTF primitive"));
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
        let (first_target, first_depth) = banger_frame_target_hashes(1280, 720, wgpu::TextureFormat::Depth32Float, 1);
        let (same_target, same_depth) = banger_frame_target_hashes(1280, 720, wgpu::TextureFormat::Depth32Float, 1);
        let (resized_target, resized_depth) = banger_frame_target_hashes(1920, 1080, wgpu::TextureFormat::Depth32Float, 2);
        assert_eq!(first_target, same_target);
        assert_eq!(first_depth, same_depth);
        assert_ne!(first_target, resized_target);
        assert_ne!(first_depth, resized_depth);
        assert_eq!(first_target.len(), 64);
        assert_eq!(first_depth.len(), 64);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn sizes_hzb_pyramid_for_child_surface_depth() {
        assert_eq!(banger_hzb_mip_count(1, 1), 1);
        assert_eq!(banger_hzb_mip_count(1280, 720), 12);
        assert_eq!(banger_hzb_mip_size(1280, 720, 0), [1280, 720]);
        assert_eq!(banger_hzb_mip_size(1280, 720, 1), [640, 360]);
        assert_eq!(banger_hzb_mip_size(1280, 720, 11), [1, 1]);
        assert_eq!(banger_hzb_resource_hash(1280, 720, 12, 1).len(), 64);
        let consumer_uniform = banger_hzb_consumer_uniform_bytes(1280, 720, 12, 1);
        assert_eq!(u32::from_le_bytes(consumer_uniform[0..4].try_into().unwrap()), 1280);
        assert_eq!(u32::from_le_bytes(consumer_uniform[4..8].try_into().unwrap()), 720);
        assert_eq!(u32::from_le_bytes(consumer_uniform[8..12].try_into().unwrap()), 12);
        assert_eq!(u32::from_le_bytes(consumer_uniform[12..16].try_into().unwrap()), 1);
        assert_eq!(banger_hzb_consumer_resource_hash(1280, 720, 12, 1).len(), 64);
        assert!(banger_hzb_seed_compute_wgsl().contains("texture_depth_2d"));
        assert!(banger_hzb_reduce_compute_wgsl().contains("texture_storage_2d<r32float, write>"));
        assert!(banger_hzb_consumer_compute_wgsl().contains("textureLoad(hzb_pyramid"));
        assert!(banger_hzb_consumer_compute_wgsl().contains("safe_mip"));
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
            "https://assets.cesium.com/2275207/root.json?access_token=secret&foo=bar",
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
        assert_eq!(projection.traversal_seed.tiles[0].global_transform, banger_identity_mat4_f64());
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
            "https://assets.cesium.com/2275207/root.json?access_token=redacted&foo=bar"
        );
        assert!(!projection.content_cache.enabled);
        assert_eq!(projection.content_cache.requested_content_count, 3);
        assert_eq!(projection.content_cache.skipped_content_count, 3);
    }

    #[test]
    fn propagates_3d_tiles_parent_child_transforms_to_content_records() {
        let root = serde_json::json!({
            "asset": { "version": "1.1" },
            "root": {
                "geometricError": 10.0,
                "transform": [
                    1.0, 0.0, 0.0, 0.0,
                    0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                    10.0, 0.0, 0.0, 1.0
                ],
                "children": [{
                    "geometricError": 1.0,
                    "transform": [
                        1.0, 0.0, 0.0, 0.0,
                        0.0, 1.0, 0.0, 0.0,
                        0.0, 0.0, 1.0, 0.0,
                        0.0, 5.0, 0.0, 1.0
                    ],
                    "content": { "uri": "child.glb" }
                }]
            }
        });
        let projection = summarize_banger_maps_root(
            "https://example.test/root.json",
            std::path::Path::new("cache"),
            std::path::Path::new("cache/root.json"),
            &serde_json::to_vec(&root).unwrap(),
            "test",
            false,
            None,
            Some(false),
            Some(false),
            None,
        );
        let child = projection
            .traversal_seed
            .tiles
            .iter()
            .find(|tile| tile.content_uris == vec!["child.glb".to_string()])
            .unwrap();
        assert_eq!(child.global_transform[12], 10.0);
        assert_eq!(child.global_transform[13], 5.0);
        assert_eq!(
            projection.content_cache.records[0].tile_global_transform,
            child.global_transform
        );
    }

    #[test]
    fn composes_maps_render_transform_in_tiles_spec_order() {
        let tile_transform = banger_translation_mat4_f64([10.0, 0.0, 0.0]);
        let gltf_node_transform = banger_translation_mat4_f64([0.0, 2.0, 0.0]);
        let render_transform = banger_mat4_mul_f64(
            banger_mat4_mul_f64(
                banger_mat4_mul_f64(banger_identity_mat4_f64(), tile_transform),
                banger_gltf_y_up_to_z_up_matrix_f64(),
            ),
            gltf_node_transform,
        );
        let transformed = banger_transform_point_f64(render_transform, [0.0, 1.0, 0.0]);
        assert!((transformed[0] - 10.0).abs() < 0.0001);
        assert!((transformed[1] - 0.0).abs() < 0.0001);
        assert!((transformed[2] - 3.0).abs() < 0.0001);
    }

    #[test]
    fn transforms_render_normals_with_inverse_transpose() {
        let scaled = banger_scale_mat4_f64([2.0, 1.0, 1.0]);
        let normal = banger_transform_normal_f64(scaled, [1.0, 1.0, 0.0]);
        assert!((normal[0] - 0.4472136).abs() < 0.0001);
        assert!((normal[1] - 0.8944272).abs() < 0.0001);
        assert!(normal[2].abs() < 0.0001);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_render_tangents_into_visible_vertex_buffer() {
        let mut position_bytes = Vec::new();
        for value in [0.0_f32, 1.0, 0.0] {
            position_bytes.extend_from_slice(&value.to_le_bytes());
        }
        let position = BangerGltfAccessorStage {
            bytes: position_bytes,
            count: 1,
            component_type: 5126,
            normalized: false,
            accessor_type: "VEC3".to_string(),
        };
        let bytes = banger_maps_position_accessor_to_render_vertices(
            &position,
            None,
            Some(&[[0.0, 1.0, 0.0]]),
            Some(&[[1.0, 0.0, 0.0, -1.0]]),
            [0.3, 0.4, 0.5, 1.0],
            0.0,
            banger_identity_mat4_f64(),
        )
        .unwrap();
        assert_eq!(bytes.len(), BANGER_RENDER_VERTEX_STRIDE_BYTES);
        assert!((f32::from_le_bytes(bytes[48..52].try_into().unwrap()) - 1.0).abs() < 0.0001);
        assert!(f32::from_le_bytes(bytes[52..56].try_into().unwrap()).abs() < 0.0001);
        assert!(f32::from_le_bytes(bytes[56..60].try_into().unwrap()).abs() < 0.0001);
        assert!((f32::from_le_bytes(bytes[60..64].try_into().unwrap()) + 1.0).abs() < 0.0001);
    }

    #[test]
    fn maps_wgs84_origin_maps_to_zero_in_enu_frame() {
        let georeference = BangerMapsGeoreference {
            ellipsoid: "WGS84",
            origin_latitude: 0.0,
            origin_longitude: 0.0,
            origin_height_meters: 0.0,
            world_origin_policy: "test",
        };
        let origin_ecef = banger_wgs84_geodetic_to_ecef(0.0, 0.0, 0.0);
        let local =
            banger_transform_point64_f64(banger_ecef_to_enu_matrix(&georeference), origin_ecef);
        assert!(local[0].abs() < 0.000001);
        assert!(local[1].abs() < 0.000001);
        assert!(local[2].abs() < 0.000001);
    }

    #[test]
    fn maps_ecef_to_enu_preserves_local_axes() {
        let georeference = BangerMapsGeoreference {
            ellipsoid: "WGS84",
            origin_latitude: 0.0,
            origin_longitude: 0.0,
            origin_height_meters: 0.0,
            world_origin_policy: "test",
        };
        let origin_ecef = banger_wgs84_geodetic_to_ecef(0.0, 0.0, 0.0);
        let ecef_to_enu = banger_ecef_to_enu_matrix(&georeference);
        let east = banger_transform_point64_f64(
            ecef_to_enu,
            [origin_ecef[0], origin_ecef[1] + 10.0, origin_ecef[2]],
        );
        let north = banger_transform_point64_f64(
            ecef_to_enu,
            [origin_ecef[0], origin_ecef[1], origin_ecef[2] + 10.0],
        );
        let up = banger_transform_point64_f64(
            ecef_to_enu,
            [origin_ecef[0] + 10.0, origin_ecef[1], origin_ecef[2]],
        );
        assert!((east[0] - 10.0).abs() < 0.000001);
        assert!(east[1].abs() < 0.000001);
        assert!(east[2].abs() < 0.000001);
        assert!((north[1] - 10.0).abs() < 0.000001);
        assert!(north[0].abs() < 0.000001);
        assert!(north[2].abs() < 0.000001);
        assert!((up[2] - 10.0).abs() < 0.000001);
        assert!(up[0].abs() < 0.000001);
        assert!(up[1].abs() < 0.000001);
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
    fn accepts_draco_gltf_schema_before_native_decode() {
        let glb = test_draco_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let support = banger_maps_gltf_format_support(&decoded.gltf_value);
        assert_eq!(support.extensions_used, vec!["KHR_draco_mesh_compression".to_string()]);
        assert_eq!(support.extensions_required, vec!["KHR_draco_mesh_compression".to_string()]);
        assert!(support.unsupported_required_extensions.is_empty());
        assert!(support.compression_blocker.is_none());
        let error = match stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk) {
            Ok(_) => panic!("invalid Draco fixture unexpectedly staged"),
            Err(error) => error,
        };
        #[cfg(feature = "banger-draco")]
        assert!(error.contains("KHR_draco_mesh_compression decode failed"));
        #[cfg(not(feature = "banger-draco"))]
        assert!(error.contains("decode unavailable"));
    }

    #[test]
    fn reports_malformed_draco_schema_before_native_decode() {
        let json = br#"{"asset":{"version":"2.0"},"extensionsUsed":["KHR_draco_mesh_compression"],"extensionsRequired":["KHR_draco_mesh_compression"],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"extensions":{"KHR_draco_mesh_compression":{"bufferView":0,"attributes":{}}}}]}],"accessors":[{"componentType":5126,"count":3,"type":"VEC3"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":4}],"buffers":[{"byteLength":4}]}"#;
        let glb = test_glb_with_json_bin(json, &[0, 1, 2, 3]);
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let support = banger_maps_gltf_format_support(&decoded.gltf_value);
        assert!(support.unsupported_required_extensions.is_empty());
        assert!(support
            .compression_blocker
            .as_deref()
            .unwrap()
            .contains("missing POSITION attribute id"));
    }

    #[test]
    fn stages_khr_mesh_quantization_into_native_engine_vertices() {
        let glb = test_quantized_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let support = banger_maps_gltf_format_support(&decoded.gltf_value);
        assert_eq!(support.extensions_used, vec!["KHR_mesh_quantization".to_string()]);
        assert_eq!(support.extensions_required, vec!["KHR_mesh_quantization".to_string()]);
        assert!(support.unsupported_required_extensions.is_empty());
        assert!(support.compression_blocker.is_none());

        let (primitives, materials, textures) = stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk).unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0].position_accessor, 0);
        assert_eq!(primitives[0].normal_accessor, Some(1));
        assert_eq!(primitives[0].texcoord0_accessor, Some(2));
        assert_eq!(primitives[0].source_position_buffer_byte_count, 18);
        assert_eq!(primitives[0].vertex_buffer_byte_count, 3 * 48);
        assert_eq!(primitives[0].index_buffer_byte_count, 6);
        assert_eq!(primitives[0].vertex_stride_bytes, 48);
        assert_eq!(materials[0].base_color_factor, [0.25, 0.5, 0.75, 1.0]);
        assert!(textures.is_empty());

        let position = banger_gltf_accessor_stage(&decoded.gltf_value, decoded.bin_chunk, 0).unwrap();
        let normal = banger_gltf_accessor_stage(&decoded.gltf_value, decoded.bin_chunk, 1).unwrap();
        let texcoord = banger_gltf_accessor_stage(&decoded.gltf_value, decoded.bin_chunk, 2).unwrap();
        let normals = banger_maps_float_vec3_accessor_values(&normal, "NORMAL").unwrap();
        let texcoords = banger_maps_float_vec2_accessor_values(&texcoord, "TEXCOORD_0").unwrap();
        let vertex_bytes = banger_maps_engine_vertex_buffer_bytes(
            &position,
            Some(&normals),
            Some(&texcoords),
            [0.25, 0.5, 0.75, 1.0],
        )
        .unwrap();
        assert_eq!(vertex_bytes.len(), 3 * 48);
        assert!((test_vertex_f32(&vertex_bytes, 12) - 1.0).abs() < 0.0001);
        assert!((test_vertex_f32(&vertex_bytes, 25) - 1.0).abs() < 0.0001);
        assert!((test_vertex_f32(&vertex_bytes, 4) - 1.0).abs() < 0.0001);
        assert!((test_vertex_f32(&vertex_bytes, 18) - 1.0).abs() < 0.0001);

        #[cfg(target_os = "windows")]
        {
            let mesh = banger_maps_render_mesh_from_gltf(
                &decoded.gltf_value,
                decoded.bin_chunk,
                banger_identity_mat4_f64(),
                banger_identity_mat4_f64(),
            )
            .unwrap();
            assert_eq!(mesh.vertex_bytes.len(), 3 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
            assert_eq!(mesh.index_bytes.len(), 6);
            assert!((f32::from_le_bytes(mesh.vertex_bytes[36..40].try_into().unwrap()) - 0.0).abs() < 0.0001);
            assert!((f32::from_le_bytes(mesh.vertex_bytes[40..44].try_into().unwrap()) - 0.0).abs() < 0.0001);
            assert!((f32::from_le_bytes(mesh.vertex_bytes[44..48].try_into().unwrap()) - 1.0).abs() < 0.0001);
        }
    }

    #[test]
    fn decodes_ext_meshopt_buffer_views_before_native_staging() {
        let glb = test_meshopt_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let support = banger_maps_gltf_format_support(&decoded.gltf_value);
        assert_eq!(support.extensions_used, vec!["EXT_meshopt_compression".to_string(), "KHR_mesh_quantization".to_string()]);
        assert!(support.unsupported_required_extensions.is_empty());
        assert!(support.compression_blocker.is_none());

        let (primitives, materials, textures) = stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk).unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0].source_position_buffer_byte_count, 18);
        assert_eq!(primitives[0].vertex_buffer_byte_count, 3 * 48);
        assert_eq!(primitives[0].index_buffer_byte_count, 6);
        assert_eq!(primitives[0].index_format, "uint16");
        assert_eq!(materials[0].base_color_factor, [0.1, 0.45, 0.9, 1.0]);
        assert!(textures.is_empty());

        let position = banger_gltf_accessor_stage(&decoded.gltf_value, decoded.bin_chunk, 0).unwrap();
        let positions = banger_maps_float_vec3_accessor_values(&position, "POSITION").unwrap();
        assert!((positions[1][0] - 1.0).abs() < 0.0001);
        assert!((positions[2][1] - 1.0).abs() < 0.0001);

        #[cfg(target_os = "windows")]
        {
            let mesh = banger_maps_render_mesh_from_gltf(
                &decoded.gltf_value,
                decoded.bin_chunk,
                banger_identity_mat4_f64(),
                banger_identity_mat4_f64(),
            )
            .unwrap();
            assert_eq!(mesh.vertex_bytes.len(), 3 * BANGER_RENDER_VERTEX_STRIDE_BYTES);
            assert_eq!(mesh.index_bytes.len(), 6);
        }
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
        assert_eq!(b3dm.rtc_center, None);
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
        assert_eq!(projection.gpu_staging.vertex_buffer_byte_count, 3 * 48);
        assert_eq!(projection.gpu_staging.index_buffer_byte_count, 6);
        assert_eq!(projection.gpu_staging.texture_byte_count, 4);
        let stage = &projection.gpu_staging.records[0];
        assert_eq!(stage.primitive_stages[0].vertex_count, 3);
        assert_eq!(stage.primitive_stages[0].index_count, 3);
        assert_eq!(stage.primitive_stages[0].source_position_buffer_byte_count, 36);
        assert_eq!(stage.primitive_stages[0].vertex_buffer_byte_count, 3 * 48);
        assert_eq!(stage.primitive_stages[0].vertex_stride_bytes, 48);
        assert_eq!(
            stage.primitive_stages[0].vertex_layout,
            "float32x3_position_float32x3_normal_float32x2_uv_float32x4_base_color"
        );
        assert_eq!(stage.primitive_stages[0].normal_accessor, None);
        assert_eq!(stage.primitive_stages[0].texcoord0_accessor, None);
        assert_eq!(stage.primitive_stages[0].index_format, "uint16");
        assert_eq!(stage.primitive_stages[0].wgpu_vertex_usage, "VERTEX|COPY_DST");
        assert_eq!(stage.primitive_stages[0].wgpu_index_usage, "INDEX|COPY_DST");
        assert_eq!(stage.material_stages[0].base_color_texture, Some(0));
        assert_eq!(stage.texture_stages[0].source_kind, "embedded_buffer_view");
        assert_eq!(stage.texture_stages[0].wgpu_usage, "TEXTURE_BINDING|COPY_DST");
    }

    #[test]
    fn decodes_b3dm_rtc_center_from_json_feature_table() {
        let bytes = test_b3dm_bytes_with_feature(br#"{"BATCH_LENGTH":0,"RTC_CENTER":[1.25,2.5,3.75]}"#, &[]);
        let (b3dm, _) = decode_banger_b3dm(&bytes).unwrap();
        assert_eq!(b3dm.rtc_center, Some([1.25, 2.5, 3.75]));
    }

    #[test]
    fn decodes_b3dm_rtc_center_from_binary_feature_table() {
        let mut feature_binary = Vec::new();
        for value in [4.0_f32, 5.5, 6.25] {
            feature_binary.extend_from_slice(&value.to_le_bytes());
        }
        let bytes = test_b3dm_bytes_with_feature(
            br#"{"BATCH_LENGTH":0,"RTC_CENTER":{"byteOffset":0}}"#,
            &feature_binary,
        );
        let (b3dm, _) = decode_banger_b3dm(&bytes).unwrap();
        assert_eq!(b3dm.rtc_center, Some([4.0, 5.5, 6.25]));
    }

    #[test]
    fn composes_b3dm_rtc_center_before_tile_transform() {
        let tile_transform = banger_translation_mat4_f64([10.0, 0.0, 0.0]);
        let content_transform =
            banger_b3dm_tile_content_transform(tile_transform, Some([1.0, 2.0, 3.0]));
        let transformed = banger_transform_point64_f64(content_transform, [0.0, 0.0, 0.0]);
        assert!((transformed[0] - 11.0).abs() < 0.000001);
        assert!((transformed[1] - 2.0).abs() < 0.000001);
        assert!((transformed[2] - 3.0).abs() < 0.000001);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_maps_materials_into_gpu_resource_bytes() {
        let bytes = banger_maps_material_resource_bytes(&[BangerMapsMaterialStage {
            material_index: 7,
            base_color_factor: [0.25, 0.5, 0.75, 1.0],
            metallic_factor: 0.2,
            roughness_factor: 0.8,
            base_color_texture: Some(3),
            normal_texture: Some(5),
            normal_scale: 0.65,
            metallic_roughness_texture: Some(4),
            occlusion_texture: Some(6),
            occlusion_strength: 0.55,
            emissive_texture: Some(8),
            emissive_factor: [0.2, 0.3, 0.4],
            material_hash: "test".to_string(),
        }])
        .unwrap();
        assert_eq!(bytes.len(), BANGER_MATERIAL_RECORD_STRIDE);
        assert!((f32::from_le_bytes(bytes[0..4].try_into().unwrap()) - 0.25).abs() < 0.0001);
        assert!((f32::from_le_bytes(bytes[16..20].try_into().unwrap()) - 0.2).abs() < 0.0001);
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(bytes[32..36].try_into().unwrap()), 5);
        assert!((f32::from_le_bytes(bytes[36..40].try_into().unwrap()) - 0.65).abs() < 0.0001);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(bytes[44..48].try_into().unwrap()), 6);
        assert!((f32::from_le_bytes(bytes[48..52].try_into().unwrap()) - 0.55).abs() < 0.0001);
        assert_eq!(u32::from_le_bytes(bytes[52..56].try_into().unwrap()), 8);
        assert!((f32::from_le_bytes(bytes[64..68].try_into().unwrap()) - 0.2).abs() < 0.0001);
        assert!((f32::from_le_bytes(bytes[68..72].try_into().unwrap()) - 0.3).abs() < 0.0001);
        assert!((f32::from_le_bytes(bytes[72..76].try_into().unwrap()) - 0.4).abs() < 0.0001);
        assert_eq!(banger_first_material_normal_texture_index(&bytes), Some(5));
        assert_eq!(banger_first_material_metallic_roughness_texture_index(&bytes), Some(4));
        assert_eq!(banger_first_material_occlusion_texture_index(&bytes), Some(6));
        assert_eq!(banger_first_material_emissive_texture_index(&bytes), Some(8));
        let fallback = banger_default_material_resource_bytes();
        assert_eq!(fallback.len(), BANGER_MATERIAL_RECORD_STRIDE);
        assert_eq!(f32::from_le_bytes(fallback[0..4].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(fallback[16..20].try_into().unwrap()), 0.0);
        assert!((f32::from_le_bytes(fallback[20..24].try_into().unwrap()) - 0.72).abs() < 0.0001);
        assert_eq!(u32::from_le_bytes(fallback[32..36].try_into().unwrap()), u32::MAX);
        assert_eq!(f32::from_le_bytes(fallback[36..40].try_into().unwrap()), 1.0);
        assert_eq!(u32::from_le_bytes(fallback[40..44].try_into().unwrap()), u32::MAX);
        assert_eq!(u32::from_le_bytes(fallback[44..48].try_into().unwrap()), u32::MAX);
        assert_eq!(f32::from_le_bytes(fallback[48..52].try_into().unwrap()), 1.0);
        assert_eq!(u32::from_le_bytes(fallback[52..56].try_into().unwrap()), u32::MAX);
        assert_eq!(f32::from_le_bytes(fallback[64..68].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(fallback[68..72].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(fallback[72..76].try_into().unwrap()), 0.0);
        assert_eq!(banger_first_material_normal_texture_index(&fallback), None);
        assert_eq!(banger_first_material_metallic_roughness_texture_index(&fallback), None);
        assert_eq!(banger_first_material_occlusion_texture_index(&fallback), None);
        assert_eq!(banger_first_material_emissive_texture_index(&fallback), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_meshlet_cluster_metadata_from_render_buffers() {
        let mut vertex_bytes = Vec::new();
        for position in [
            [0.0_f32, 0.0, 0.0],
            [1.0_f32, 0.0, 0.0],
            [1.0_f32, 1.0, 0.0],
            [0.0_f32, 1.0, 0.0],
        ] {
            for value in [
                position[0], position[1], position[2],
                0.25, 0.5,
                0.75, 0.75, 0.75,
                0.0,
                0.0, 0.0, 1.0,
                1.0, 0.0, 0.0, 1.0,
            ] {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut index_bytes = Vec::new();
        for index in [0u16, 1, 2, 0, 2, 3] {
            index_bytes.extend_from_slice(&index.to_le_bytes());
        }
        let clusters = banger_meshlet_cluster_metadata_bytes(
            &vertex_bytes,
            &index_bytes,
            BangerRenderIndexFormat::Uint16,
            "banger_maps_3d_tiles_visible_tile_batch",
        );
        assert_eq!(clusters.len(), BANGER_MESHLET_CLUSTER_METADATA_STRIDE);
        assert!((f32::from_le_bytes(clusters[0..4].try_into().unwrap()) - 0.5).abs() < 0.0001);
        assert!((f32::from_le_bytes(clusters[4..8].try_into().unwrap()) - 0.5).abs() < 0.0001);
        assert!(f32::from_le_bytes(clusters[12..16].try_into().unwrap()) > 0.70);
        assert!((f32::from_le_bytes(clusters[24..28].try_into().unwrap()) - 1.0).abs() < 0.0001);
        assert!(f32::from_le_bytes(clusters[28..32].try_into().unwrap()) > 0.0);
        assert_eq!(u32::from_le_bytes(clusters[32..36].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(clusters[36..40].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(clusters[40..44].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(clusters[44..48].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(clusters[52..56].try_into().unwrap()), 0x4D_53_48_4C);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_material_bins_from_meshlet_clusters() {
        let mut vertex_bytes = Vec::new();
        for position in [[0.0_f32, 0.0, 0.0], [1.0_f32, 0.0, 0.0], [0.0_f32, 1.0, 0.0]] {
            for value in [
                position[0], position[1], position[2],
                0.25, 0.5,
                0.75, 0.75, 0.75,
                0.0,
                0.0, 0.0, 1.0,
                1.0, 0.0, 0.0, 1.0,
            ] {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut index_bytes = Vec::new();
        for index in [0u16, 1, 2] {
            index_bytes.extend_from_slice(&index.to_le_bytes());
        }
        let clusters = banger_meshlet_cluster_metadata_bytes(
            &vertex_bytes,
            &index_bytes,
            BangerRenderIndexFormat::Uint16,
            "material_bin_test",
        );
        let material_bytes = vec![7u8; 2 * BANGER_MATERIAL_RECORD_STRIDE];
        let texture_manifest = banger_maps_texture_resource_manifest_bytes(&[vec![1, 2, 3, 4]]);
        let bins = banger_material_bin_bytes(&clusters, Some(&material_bytes), &texture_manifest);
        assert_eq!(bins.len(), 2 * BANGER_MATERIAL_BIN_RECORD_STRIDE);
        assert_eq!(u32::from_le_bytes(bins[0..4].try_into().unwrap()), 0x4D_42_49_4E);
        assert_eq!(u32::from_le_bytes(bins[8..12].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(bins[16..20].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(bins[20..24].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(bins[24..28].try_into().unwrap()), 3);
        assert_ne!(u32::from_le_bytes(bins[28..32].try_into().unwrap()), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_meshlet_cluster_cull_params_and_shader_contract() {
        let params = banger_meshlet_cluster_cull_params_bytes(37, 1, 420, 2);
        assert_eq!(u32::from_le_bytes(params[0..4].try_into().unwrap()), 37);
        assert_eq!(
            u32::from_le_bytes(params[4..8].try_into().unwrap()),
            BANGER_MESHLET_CLUSTER_METADATA_STRIDE as u32
        );
        assert_eq!(u32::from_le_bytes(params[8..12].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(params[12..16].try_into().unwrap()),
            BANGER_MESHLET_CLUSTER_TRIANGLE_LIMIT as u32
        );
        assert_eq!(u32::from_le_bytes(params[16..20].try_into().unwrap()), 420);
        assert_eq!(u32::from_le_bytes(params[20..24].try_into().unwrap()), 2);
        assert_eq!(banger_meshlet_cluster_cull_feedback_bytes(), [0u8; 16]);
        let shader = banger_meshlet_cluster_cull_compute_wgsl();
        assert!(shader.contains("textureLoad(hzb_pyramid"));
        assert!(shader.contains("atomicAdd(&feedback[0]"));
        assert!(shader.contains("culled_indirect_args"));
        assert!(shader.contains("visible_clusters[write_index] = cluster"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_maps_indirect_draw_args_and_residency_feedback() {
        let args = banger_indexed_indirect_args_bytes(42, 3);
        assert_eq!(u32::from_le_bytes(args[0..4].try_into().unwrap()), 42);
        assert_eq!(u32::from_le_bytes(args[4..8].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(args[8..12].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(args[12..16].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(args[16..20].try_into().unwrap()), 0);

        let feedback = banger_maps_residency_feedback_bytes(
            Some("tile_a,tile_b"),
            "banger_maps_3d_tiles_visible_tile_batch",
            84,
            42,
            1,
            &sha256_hex(b"vertex"),
            &sha256_hex(b"index"),
            &sha256_hex(b"material"),
            &sha256_hex(b"texture"),
        );
        assert_eq!(feedback.len(), 48);
        assert_eq!(u32::from_le_bytes(feedback[0..4].try_into().unwrap()), 0x4D_41_50_53);
        assert_eq!(u32::from_le_bytes(feedback[4..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(feedback[8..12].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(feedback[12..16].try_into().unwrap()), 84);
        assert_eq!(u32::from_le_bytes(feedback[16..20].try_into().unwrap()), 42);
        assert_eq!(u32::from_le_bytes(feedback[20..24].try_into().unwrap()), 1);
        assert_ne!(u32::from_le_bytes(feedback[24..28].try_into().unwrap()), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_shared_residency_page_table_and_compacted_feedback() {
        let table = banger_shared_residency_page_table_bytes(
            "banger_maps_3d_tiles_visible_tile_batch",
            Some("tile_a,tile_b"),
            8192,
            256,
            4096,
            64,
        );
        assert_eq!(table.len(), 3 * BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE);
        assert_eq!(u32::from_le_bytes(table[0..4].try_into().unwrap()), 0x52_53_44_59);
        assert_eq!(u32::from_le_bytes(table[4..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(table[12..16].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(table[16..20].try_into().unwrap()), 8192);
        assert_eq!(u32::from_le_bytes(table[32..36].try_into().unwrap()), 900);
        let second_offset = BANGER_SHARED_RESIDENCY_PAGE_RECORD_STRIDE;
        assert_eq!(u32::from_le_bytes(table[second_offset + 24..second_offset + 28].try_into().unwrap()), 8192);
        let compacted = banger_shared_residency_compacted_feedback_bytes(&table);
        assert_eq!(compacted.len(), 3 * BANGER_SHARED_RESIDENCY_COMPACTED_FEEDBACK_STRIDE);
        assert_eq!(u32::from_le_bytes(compacted[0..4].try_into().unwrap()), 0x46_44_42_4B);
        assert_eq!(u32::from_le_bytes(compacted[8..12].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(compacted[16..20].try_into().unwrap()), 900);
        let budget = banger_shared_residency_budget_bytes(3, 512 * 1024 * 1024, 12 * 1024 * 1024);
        assert_eq!(u32::from_le_bytes(budget[0..4].try_into().unwrap()), 0x42_55_44_47);
        assert_eq!(u32::from_le_bytes(budget[8..12].try_into().unwrap()), 3);
        let eviction_plan = banger_shared_residency_eviction_plan_bytes(&table, 8192);
        assert_eq!(eviction_plan.len(), 3 * BANGER_SHARED_RESIDENCY_EVICTION_RECORD_STRIDE);
        assert_eq!(u32::from_le_bytes(eviction_plan[0..4].try_into().unwrap()), 0x45_56_43_54);
        assert_eq!(u32::from_le_bytes(eviction_plan[8..12].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(eviction_plan[20..24].try_into().unwrap()), 600);
        assert_eq!(u32::from_le_bytes(eviction_plan[28..32].try_into().unwrap()), 1);
        let material_record_offset = BANGER_SHARED_RESIDENCY_EVICTION_RECORD_STRIDE;
        assert_eq!(
            u32::from_le_bytes(eviction_plan[material_record_offset + 8..material_record_offset + 12].try_into().unwrap()),
            1
        );
        assert_eq!(
            u32::from_le_bytes(eviction_plan[material_record_offset + 28..material_record_offset + 32].try_into().unwrap()),
            1
        );
        let geometry_record_offset = BANGER_SHARED_RESIDENCY_EVICTION_RECORD_STRIDE * 2;
        assert_eq!(
            u32::from_le_bytes(eviction_plan[geometry_record_offset + 8..geometry_record_offset + 12].try_into().unwrap()),
            0
        );
        assert_eq!(
            u32::from_le_bytes(eviction_plan[geometry_record_offset + 28..geometry_record_offset + 32].try_into().unwrap()),
            0
        );
        assert_eq!(banger_align_u64(4097, 4096), 8192);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_lumen_gbuffer_surface_cards_and_cache_seeds() {
        let clusters = banger_meshlet_cluster_metadata_bytes(
            &banger_cube_vertex_bytes(),
            &banger_cube_index_bytes(),
            BangerRenderIndexFormat::Uint16,
            "banger_cube_test_mesh",
        );
        let cards = banger_lumen_surface_card_bytes(&clusters);
        assert_eq!(cards.len() % BANGER_LUMEN_SURFACE_CARD_RECORD_STRIDE, 0);
        assert_eq!(u32::from_le_bytes(cards[0..4].try_into().unwrap()), 0x4C_53_43_44);
        assert_eq!(u32::from_le_bytes(cards[4..8].try_into().unwrap()), 1);

        let feedback = banger_lumen_surface_cache_feedback_bytes(&cards);
        assert_eq!(u32::from_le_bytes(feedback[0..4].try_into().unwrap()), 0x4C_46_44_42);
        assert_eq!(u32::from_le_bytes(feedback[16..20].try_into().unwrap()), 1);

        let probes = banger_lumen_screen_probe_bytes(17);
        assert_eq!(probes.len(), 17 * BANGER_LUMEN_SCREEN_PROBE_RECORD_STRIDE);
        assert_eq!(u32::from_le_bytes(probes[0..4].try_into().unwrap()), 0x4C_50_52_42);

        let radiance = banger_lumen_radiance_cache_bytes(&cards, &probes);
        assert_eq!(u32::from_le_bytes(radiance[0..4].try_into().unwrap()), 0x4C_52_44_43);
        assert_ne!(u32::from_le_bytes(radiance[16..20].try_into().unwrap()), 0);

        let source = banger_native_first_scene_wgsl();
        assert!(source.contains("gbuffer_albedo"));
        assert!(source.contains("gbuffer_normal"));
        assert!(source.contains("gbuffer_material"));
        assert!(source.contains("gbuffer_emissive"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_single_layer_water_composite_contract() {
        let params = banger_single_layer_water_params_bytes(1920, 1080);
        assert_eq!(u32::from_le_bytes(params[0..4].try_into().unwrap()), 1920);
        assert_eq!(u32::from_le_bytes(params[4..8].try_into().unwrap()), 1080);
        assert_eq!(u32::from_le_bytes(params[8..12].try_into().unwrap()), 240);
        assert_eq!(u32::from_le_bytes(params[12..16].try_into().unwrap()), 135);
        assert!(f32::from_le_bytes(params[16..20].try_into().unwrap()) > 0.0);

        let tile_mask = banger_single_layer_water_tile_mask_bytes(1920, 1080);
        assert_eq!(tile_mask.len(), (240u32 * 135u32).div_ceil(32) as usize * 4);
        assert_eq!(banger_single_layer_water_tile_mask_bytes(1, 1).len(), 4);

        let shader = banger_single_layer_water_composite_compute_wgsl();
        assert!(shader.contains("SingleLayerWaterParams"));
        assert!(shader.contains("gbuffer_material"));
        assert!(shader.contains("spectral_displacement"));
        assert!(shader.contains("spectral_slope"));
        assert!(shader.contains("texture_storage_2d<rgba8unorm, write>"));
        assert!(shader.contains("texture_storage_2d<r32float, write>"));
        assert!(shader.contains("refraction_mask"));
        assert!(shader.contains("atomicOr"));
        let present_shader = banger_single_layer_water_present_wgsl();
        assert!(present_shader.contains("water_composite"));
        assert!(present_shader.contains("refraction_mask"));
        assert!(present_shader.contains("smoothstep"));
        assert!(present_shader.contains("discard"));
        let bloom_shader = banger_emissive_bloom_present_wgsl();
        assert!(bloom_shader.contains("gbuffer_emissive"));
        assert!(bloom_shader.contains("banger_emissive_tap"));
        assert!(bloom_shader.contains("smoothstep"));
        assert!(bloom_shader.contains("luminance"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_spectral_ocean_displacement_and_slope_contract() {
        let params = banger_spectral_ocean_params_bytes(3000, 1800, 12.5, 42);
        assert_eq!(u32::from_le_bytes(params[0..4].try_into().unwrap()), 3000);
        assert_eq!(u32::from_le_bytes(params[4..8].try_into().unwrap()), 1800);
        assert_eq!(u32::from_le_bytes(params[8..12].try_into().unwrap()), 2048);
        assert_eq!(u32::from_le_bytes(params[12..16].try_into().unwrap()), 42);
        assert_eq!(u32::from_le_bytes(params[16..20].try_into().unwrap()), 8);
        assert_eq!(f32::from_le_bytes(params[32..36].try_into().unwrap()), 12.5);
        assert_eq!(f32::from_le_bytes(params[44..48].try_into().unwrap()), 160.0);

        let shader = banger_spectral_ocean_compute_wgsl();
        assert!(shader.contains("SpectralOceanParams"));
        assert!(shader.contains("texture_storage_2d<rgba16float, write>"));
        assert!(shader.contains("spectral_displacement"));
        assert!(shader.contains("spectral_slope"));
        assert!(shader.contains("phillips"));
        assert!(shader.contains("params.fft.x"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn packs_virtual_shadow_map_page_tables_and_mark_shader() {
        let page_table = banger_virtual_shadow_map_page_table_bytes(3, 1);
        assert_eq!(page_table.len(), 3 * BANGER_VIRTUAL_SHADOW_MAP_PAGE_RECORD_STRIDE);
        assert_eq!(u32::from_le_bytes(page_table[0..4].try_into().unwrap()), 0x56_53_4D_54);
        assert_eq!(u32::from_le_bytes(page_table[8..12].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(page_table[12..16].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(page_table[16..20].try_into().unwrap()), 0);

        let flags = banger_virtual_shadow_map_page_flags_bytes(3);
        let requests = banger_virtual_shadow_map_page_request_bytes(3);
        assert_eq!(u32::from_le_bytes(flags[0..4].try_into().unwrap()), 0x56_53_4D_46);
        assert_eq!(u32::from_le_bytes(requests[0..4].try_into().unwrap()), 0x56_53_4D_52);

        let physical = banger_virtual_shadow_map_physical_page_metadata_bytes(3, 1);
        assert_eq!(physical.len(), 3 * BANGER_VIRTUAL_SHADOW_MAP_PHYSICAL_PAGE_RECORD_STRIDE);
        assert_eq!(u32::from_le_bytes(physical[0..4].try_into().unwrap()), 0x56_53_4D_50);

        let projection = banger_virtual_shadow_map_projection_bytes(1);
        assert_eq!(projection.len(), BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_STRIDE);
        assert_eq!(u32::from_le_bytes(projection[0..4].try_into().unwrap()), 0x56_53_4D_50);
        assert_eq!(f32::from_le_bytes(projection[32..36].try_into().unwrap()), 1.0);

        let params = banger_virtual_shadow_map_mark_params_bytes(3, 1);
        assert_eq!(u32::from_le_bytes(params[0..4].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(params[4..8].try_into().unwrap()), 32);

        let shader = banger_virtual_shadow_map_mark_compute_wgsl();
        assert!(shader.contains("page_requests"));
        assert!(shader.contains("physical_pages"));
        assert!(shader.contains("visible_clusters"));

        let pool = banger_virtual_shadow_map_physical_pool_desc(37);
        assert_eq!(pool.pages_x, 16);
        assert_eq!(pool.layers, 1);
        assert_eq!(pool.width_texels, 16 * BANGER_VIRTUAL_SHADOW_MAP_PAGE_SIZE);
        assert!(pool.height_texels >= BANGER_VIRTUAL_SHADOW_MAP_PAGE_SIZE);

        let invalidation = banger_virtual_shadow_map_cache_invalidation_bytes(37, &sha256_hex(b"clusters"));
        assert_eq!(u32::from_le_bytes(invalidation[16..20].try_into().unwrap()), 37);
        assert_eq!(u32::from_le_bytes(invalidation[24..28].try_into().unwrap()), 0x56_53_4D_49);

        let projection_params = banger_virtual_shadow_map_projection_params_bytes(37, pool);
        assert_eq!(u32::from_le_bytes(projection_params[0..4].try_into().unwrap()), 37);
        assert_eq!(u32::from_le_bytes(projection_params[4..8].try_into().unwrap()), 16);
        assert_eq!(
            u32::from_le_bytes(projection_params[24..28].try_into().unwrap()),
            BANGER_VIRTUAL_SHADOW_MAP_PROJECTION_MASK_SIZE
        );

        let physical_shader = banger_virtual_shadow_map_physical_page_compute_wgsl();
        assert!(physical_shader.contains("texture_storage_2d_array<r32uint, write>"));
        assert!(physical_shader.contains("cache_invalidation"));
        let projection_shader = banger_virtual_shadow_map_projection_filter_compute_wgsl();
        assert!(projection_shader.contains("texture_storage_2d<r32uint, write>"));
        assert!(projection_shader.contains("projection_mask"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn orders_visible_maps_draw_records_by_screen_space_error() {
        let high_tile = BangerMapsTraversalTile {
            tile_id: "high".to_string(),
            parent_tile_id: None,
            depth: 0,
            child_count: 0,
            geometric_error: Some(100.0),
            refine: "REPLACE".to_string(),
            bounding_volume_kind: "sphere".to_string(),
            bounding_volume_hash: sha256_hex(b"high"),
            transform_hash: sha256_hex(b"high_transform"),
            global_transform: banger_identity_mat4_f64(),
            content_uris: vec!["high.glb".to_string()],
            priority_key: 100.0,
        };
        let low_tile = BangerMapsTraversalTile {
            tile_id: "low".to_string(),
            parent_tile_id: None,
            depth: 0,
            child_count: 0,
            geometric_error: Some(1.0),
            refine: "REPLACE".to_string(),
            bounding_volume_kind: "sphere".to_string(),
            bounding_volume_hash: sha256_hex(b"low"),
            transform_hash: sha256_hex(b"low_transform"),
            global_transform: banger_identity_mat4_f64(),
            content_uris: vec!["low.glb".to_string()],
            priority_key: 1.0,
        };
        let projection = BangerMapsRootIngestProjection {
            ok: true,
            schema: "test",
            source: "test",
            root_tileset_url: "file:///test/tileset.json".to_string(),
            cache_dir: "cache".to_string(),
            cache_path: "cache/root.json".to_string(),
            cache_hit: false,
            network_fetch_attempted: false,
            root_hash: sha256_hex(b"root"),
            root_byte_count: 0,
            tile_count: 2,
            content_uri_count: 2,
            geometric_error: Some(100.0),
            asset_version: "1.1".to_string(),
            traversal_seed_hash: sha256_hex(b"traversal"),
            traversal_seed: BangerMapsTraversalSeed {
                schema: "test",
                priority_model: "test",
                max_queued_tiles: 2,
                queued_tile_count: 2,
                total_tile_count: 2,
                total_content_uri_count: 2,
                deepest_level: 0,
                plan_hash: sha256_hex(b"plan"),
                tiles: vec![low_tile, high_tile],
            },
            content_cache: empty_banger_maps_content_cache(std::path::Path::new("cache")),
            content_decode: BangerMapsContentDecodeProjection {
                schema: "test",
                enabled: true,
                decoded_content_count: 2,
                failed_content_count: 0,
                b3dm_count: 0,
                glb_count: 2,
                gltf_count: 0,
                total_glb_byte_count: 0,
                total_bin_chunk_byte_count: 0,
                decode_manifest_hash: sha256_hex(b"decode"),
                records: vec![
                    test_maps_decode_record("low"),
                    test_maps_decode_record("high"),
                ],
            },
            gpu_staging: empty_banger_maps_gpu_staging(),
            verifier: banger_maps_root_ingest_verifier(),
            error: None,
        };
        let ordered = banger_maps_visible_draw_records(&projection, banger_identity_mat4_f64());
        assert_eq!(ordered[0].tile_id, "high");
        assert_eq!(ordered[1].tile_id, "low");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rejects_maps_draw_records_outside_camera_frustum() {
        let camera = banger_maps_cpu_camera();
        let behind_center = [
            camera.eye[0] - camera.forward[0] * 20.0,
            camera.eye[1] - camera.forward[1] * 20.0,
            camera.eye[2] - camera.forward[2] * 20.0,
        ];
        let hidden_tile = BangerMapsTraversalTile {
            tile_id: "hidden".to_string(),
            parent_tile_id: None,
            depth: 0,
            child_count: 0,
            geometric_error: Some(1.0),
            refine: "REPLACE".to_string(),
            bounding_volume_kind: "sphere".to_string(),
            bounding_volume_hash: sha256_hex(b"hidden"),
            transform_hash: sha256_hex(b"hidden_transform"),
            global_transform: banger_translation_mat4_f64(behind_center),
            content_uris: vec!["hidden.glb".to_string()],
            priority_key: 1.0,
        };
        let mut hidden_record = test_maps_decode_record("hidden");
        hidden_record.tile_global_transform = banger_translation_mat4_f64(behind_center);
        let projection = BangerMapsRootIngestProjection {
            ok: true,
            schema: "test",
            source: "test",
            root_tileset_url: "file:///test/tileset.json".to_string(),
            cache_dir: "cache".to_string(),
            cache_path: "cache/root.json".to_string(),
            cache_hit: false,
            network_fetch_attempted: false,
            root_hash: sha256_hex(b"root"),
            root_byte_count: 0,
            tile_count: 1,
            content_uri_count: 1,
            geometric_error: Some(1.0),
            asset_version: "1.1".to_string(),
            traversal_seed_hash: sha256_hex(b"traversal"),
            traversal_seed: BangerMapsTraversalSeed {
                schema: "test",
                priority_model: "test",
                max_queued_tiles: 1,
                queued_tile_count: 1,
                total_tile_count: 1,
                total_content_uri_count: 1,
                deepest_level: 0,
                plan_hash: sha256_hex(b"plan"),
                tiles: vec![hidden_tile],
            },
            content_cache: empty_banger_maps_content_cache(std::path::Path::new("cache")),
            content_decode: BangerMapsContentDecodeProjection {
                schema: "test",
                enabled: true,
                decoded_content_count: 1,
                failed_content_count: 0,
                b3dm_count: 0,
                glb_count: 1,
                gltf_count: 0,
                total_glb_byte_count: 0,
                total_bin_chunk_byte_count: 0,
                decode_manifest_hash: sha256_hex(b"decode"),
                records: vec![hidden_record],
            },
            gpu_staging: empty_banger_maps_gpu_staging(),
            verifier: banger_maps_root_ingest_verifier(),
            error: None,
        };
        let ordered = banger_maps_visible_draw_records(&projection, banger_identity_mat4_f64());
        assert!(ordered.is_empty());
    }

    #[test]
    fn stages_glb_primitive_buffers_for_wgpu_upload_plan() {
        let glb = test_glb_bytes();
        let decoded = decode_banger_glb_full(&glb).unwrap();
        let (primitives, materials, textures) = stage_banger_gltf_payload(&decoded.gltf_value, decoded.bin_chunk).unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0].position_accessor, 0);
        assert_eq!(primitives[0].index_accessor, Some(1));
        assert_eq!(primitives[0].source_position_buffer_byte_count, 36);
        assert_eq!(primitives[0].vertex_buffer_byte_count, 3 * 48);
        assert_eq!(primitives[0].vertex_stride_bytes, 48);
        assert_eq!(
            primitives[0].vertex_layout,
            "float32x3_position_float32x3_normal_float32x2_uv_float32x4_base_color"
        );
        assert_eq!(primitives[0].index_buffer_byte_count, 6);
        assert_eq!(primitives[0].vertex_buffer_hash.len(), 64);
        assert_eq!(primitives[0].index_buffer_hash.len(), 64);
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].base_color_factor, [0.7, 0.82, 0.9, 1.0]);
        assert_eq!(materials[0].metallic_factor, 0.0);
        assert_eq!(materials[0].roughness_factor, 0.45);
        assert_eq!(materials[0].normal_texture, Some(0));
        assert!((materials[0].normal_scale - 0.75).abs() < 0.0001);
        assert_eq!(materials[0].metallic_roughness_texture, Some(0));
        assert_eq!(materials[0].occlusion_texture, Some(0));
        assert!((materials[0].occlusion_strength - 0.5).abs() < 0.0001);
        assert_eq!(materials[0].emissive_texture, Some(0));
        assert_eq!(materials[0].emissive_factor, [0.2, 0.3, 0.4]);
        assert_eq!(textures.len(), 1);
        assert_eq!(textures[0].byte_count, 4);
        assert_eq!(textures[0].content_hash, sha256_hex(&[137, 80, 78, 71]));
    }

    fn test_glb_bytes() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"material":0,"mode":4}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.7,0.82,0.9,1.0],"metallicFactor":0.0,"roughnessFactor":0.45,"baseColorTexture":{"index":0},"metallicRoughnessTexture":{"index":0}},"normalTexture":{"index":0,"scale":0.75},"occlusionTexture":{"index":0,"strength":0.5},"emissiveTexture":{"index":0},"emissiveFactor":[0.2,0.3,0.4]}],"textures":[{"source":0}],"images":[{"bufferView":2,"mimeType":"image/png"}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36,"target":34962},{"buffer":0,"byteOffset":36,"byteLength":6,"target":34963},{"buffer":0,"byteOffset":44,"byteLength":4}],"buffers":[{"byteLength":48}]}"#;
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
        test_glb_with_json_bin(json, &[0, 1, 2, 3])
    }

    fn test_glb_with_json_bin(json: &[u8], bin: &[u8]) -> Vec<u8> {
        let mut json_chunk = json.to_vec();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(0x20);
        }
        let mut bin_chunk = bin.to_vec();
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

    fn test_quantized_glb_bytes() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"},"extensionsUsed":["KHR_mesh_quantization"],"extensionsRequired":["KHR_mesh_quantization"],"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"indices":3,"material":0,"mode":4}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.25,0.5,0.75,1.0],"metallicFactor":0.0,"roughnessFactor":0.6}}],"accessors":[{"bufferView":0,"componentType":5123,"count":3,"type":"VEC3","normalized":true},{"bufferView":1,"componentType":5120,"count":3,"type":"VEC3","normalized":true},{"bufferView":2,"componentType":5121,"count":3,"type":"VEC2","normalized":true},{"bufferView":3,"componentType":5123,"count":3,"type":"SCALAR"}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":18,"target":34962},{"buffer":0,"byteOffset":20,"byteLength":12,"byteStride":4,"target":34962},{"buffer":0,"byteOffset":32,"byteLength":6,"target":34962},{"buffer":0,"byteOffset":40,"byteLength":6,"target":34963}],"buffers":[{"byteLength":48}]}"#;
        let mut json_chunk = json.to_vec();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(0x20);
        }
        let mut bin_chunk = Vec::new();
        for value in [0u16, 0, 0, u16::MAX, 0, 0, 0, u16::MAX, 0] {
            bin_chunk.extend_from_slice(&value.to_le_bytes());
        }
        bin_chunk.extend_from_slice(&[0, 0]);
        for _ in 0..3 {
            bin_chunk.extend_from_slice(&[0, 127, 0, 0]);
        }
        bin_chunk.extend_from_slice(&[0, 0, 255, 0, 0, 255]);
        bin_chunk.extend_from_slice(&[0, 0]);
        for index in [0u16, 1, 2] {
            bin_chunk.extend_from_slice(&index.to_le_bytes());
        }
        while bin_chunk.len() % 4 != 0 {
            bin_chunk.push(0);
        }
        assert_eq!(bin_chunk.len(), 48);
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

    fn test_meshopt_glb_bytes() -> Vec<u8> {
        let position_vertices: [[u8; 8]; 3] = [
            [0, 0, 0, 0, 0, 0, 0, 0],
            [255, 255, 0, 0, 0, 0, 0, 0],
            [0, 0, 255, 255, 0, 0, 0, 0],
        ];
        let position_compressed = meshopt::encoding::encode_vertex_buffer(&position_vertices).unwrap();
        let index_compressed = meshopt::encoding::encode_index_buffer(&[0, 1, 2], 3).unwrap();
        let mut bin_chunk = Vec::new();
        let position_offset = bin_chunk.len();
        bin_chunk.extend_from_slice(&position_compressed);
        while bin_chunk.len() % 4 != 0 {
            bin_chunk.push(0);
        }
        let index_offset = bin_chunk.len();
        bin_chunk.extend_from_slice(&index_compressed);
        while bin_chunk.len() % 4 != 0 {
            bin_chunk.push(0);
        }
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"extensionsUsed":["EXT_meshopt_compression","KHR_mesh_quantization"],"extensionsRequired":["EXT_meshopt_compression","KHR_mesh_quantization"],"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0,"mode":4}}]}}],"materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[0.1,0.45,0.9,1.0],"metallicFactor":0.0,"roughnessFactor":0.6}}}}],"accessors":[{{"bufferView":0,"componentType":5123,"count":3,"type":"VEC3","normalized":true}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],"bufferViews":[{{"buffer":1,"byteOffset":0,"byteLength":24,"byteStride":8,"target":34962,"extensions":{{"EXT_meshopt_compression":{{"buffer":0,"byteOffset":{position_offset},"byteLength":{},"byteStride":8,"count":3,"mode":"ATTRIBUTES","filter":"NONE"}}}}}},{{"buffer":1,"byteOffset":24,"byteLength":6,"target":34963,"extensions":{{"EXT_meshopt_compression":{{"buffer":0,"byteOffset":{index_offset},"byteLength":{},"byteStride":2,"count":3,"mode":"TRIANGLES","filter":"NONE"}}}}}}],"buffers":[{{"byteLength":{}}},{{"byteLength":30}}]}}"#,
            position_compressed.len(),
            index_compressed.len(),
            bin_chunk.len()
        );
        let mut json_chunk = json.into_bytes();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(0x20);
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

    fn test_vertex_f32(bytes: &[u8], f32_index: usize) -> f32 {
        let offset = f32_index * 4;
        f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("test vertex f32 bytes"))
    }

    fn test_b3dm_bytes() -> Vec<u8> {
        test_b3dm_bytes_with_feature(br#"{"BATCH_LENGTH":0}"#, &[])
    }

    fn test_b3dm_bytes_with_feature(feature_json: &[u8], feature_binary: &[u8]) -> Vec<u8> {
        let glb = test_glb_bytes();
        let byte_length = 28 + feature_json.len() + feature_binary.len() + glb.len();
        let mut b3dm = Vec::with_capacity(byte_length);
        b3dm.extend_from_slice(b"b3dm");
        push_u32_le(&mut b3dm, 1);
        push_u32_le(&mut b3dm, byte_length as u32);
        push_u32_le(&mut b3dm, feature_json.len() as u32);
        push_u32_le(&mut b3dm, feature_binary.len() as u32);
        push_u32_le(&mut b3dm, 0);
        push_u32_le(&mut b3dm, 0);
        b3dm.extend_from_slice(feature_json);
        b3dm.extend_from_slice(feature_binary);
        b3dm.extend_from_slice(&glb);
        b3dm
    }

    fn test_maps_decode_record(tile_id: &str) -> BangerMapsContentDecodeRecord {
        BangerMapsContentDecodeRecord {
            tile_id: tile_id.to_string(),
            source_uri: format!("{tile_id}.glb"),
            cache_path: format!("cache/{tile_id}.glb"),
            source_content_type: "glb",
            container: "glb",
            byte_count: 0,
            content_hash: sha256_hex(tile_id.as_bytes()),
            b3dm: None,
            glb: None,
            gltf: None,
            tile_global_transform: banger_identity_mat4_f64(),
            error: None,
        }
    }
}

