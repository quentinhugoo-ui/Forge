use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::gpu_adapter_probe::{native_gpu_adapter_probe, NativeGpuAdapter};
use scan::{
    MonsterEngineLane, MonsterNativeTandemArtifact, MonsterNativeTandemDomain, MonsterNode,
    MonsterPreparedCompute,
};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderPrepareRequest {
    pub scene_id: Option<String>,
    pub known_fragment_hashes: Option<Vec<String>>,
    pub target_frame_ms: Option<f32>,
    pub vram_budget_mb: Option<u32>,
    pub prefer_mesh_shaders: Option<bool>,
    pub pipeline_cache_dir: Option<String>,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNewObjectPrepareRequest {
    pub scene_id: Option<String>,
    pub object_id: Option<String>,
    pub parent_id: Option<String>,
    pub object_prompt: String,
    pub representation: Option<String>,
    pub known_fragment_hashes: Option<Vec<String>>,
    pub target_frame_ms: Option<f32>,
    pub vram_budget_mb: Option<u32>,
    pub prefer_mesh_shaders: Option<bool>,
    pub pipeline_cache_dir: Option<String>,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatAssetPrepareRequest {
    pub asset_id: Option<String>,
    pub ply_path: String,
    pub max_splats: Option<usize>,
    pub bucket_count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatRasterizeRequest {
    pub asset_id: Option<String>,
    pub ply_path: String,
    pub width: u32,
    pub height: u32,
    pub camera_position: Option<[f32; 3]>,
    pub camera_target: Option<[f32; 3]>,
    pub camera_up: Option<[f32; 3]>,
    pub fov_y_degrees: Option<f32>,
    pub near_plane: Option<f32>,
    pub max_splats: Option<usize>,
    pub tile_size: Option<u32>,
    pub background_rgba: Option<[f32; 4]>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativePresentLoopBootstrapRequest {
    pub parent_window_handle: Option<String>,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
    pub target_frame_ms: Option<f32>,
}

impl Default for BangerGaussianSplatRasterizeRequest {
    fn default() -> Self {
        Self {
            asset_id: None,
            ply_path: String::new(),
            width: 512,
            height: 512,
            camera_position: None,
            camera_target: None,
            camera_up: None,
            fov_y_degrees: None,
            near_plane: None,
            max_splats: None,
            tile_size: None,
            background_rgba: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderPrepareResponse {
    pub ok: bool,
    pub schema: &'static str,
    pub engine: &'static str,
    pub lane: &'static str,
    pub native_domain: &'static str,
    pub scene_id: String,
    pub source_hash: String,
    pub proof_hash: String,
    pub manifest_hash: String,
    pub render_handoff_hash: String,
    pub cache_miss_count: usize,
    pub fully_cached: bool,
    pub frame_blocking_allowed: bool,
    pub target_frame_ms: f32,
    pub vram_budget_mb: u32,
    pub render_pass_count: usize,
    pub residency_job_count: usize,
    pub gpu_shader_profiles: Vec<String>,
    pub shader_capability_plan: BangerNativeShaderCapabilityPlan,
    pub shader_compiler_ticket: BangerNativeShaderCompilerTicket,
    pub pipeline_cache_manifest: BangerNativePipelineCacheManifest,
    pub benchmark_promotion_manifest: BangerNativeBenchmarkPromotionManifest,
    pub texture_bridge_contract: BangerNativeTextureBridgeContract,
    pub pipeline_cache_keys: Vec<String>,
    pub render_graph: Vec<BangerNativeRenderPass>,
    pub render_graph_manifest_hash: String,
    pub render_graph_compilation: BangerNativeRenderGraphCompilation,
    pub residency_jobs: Vec<BangerNativeResidencyJob>,
    pub resource_table_hash: String,
    pub resource_table: BangerNativeResourceTable,
    pub page_residency_allocator: BangerNativePageResidencyAllocatorPacket,
    pub editable_scene_manifest: BangerEditableSceneManifest,
    pub scene_graph_submission: BangerNativeSceneGraphSubmission,
    pub gpu_scene_packet: BangerNativeGpuScenePacket,
    pub culling_manifest: BangerNativeCullingManifest,
    pub meshlet_visibility_packet: BangerNativeMeshletVisibilityPacket,
    pub nanite_second_layer_packet: BangerNativeNaniteSecondLayerPacket,
    pub raster_work_queue: BangerNativeRasterWorkQueue,
    pub radiance_schedule_manifest: BangerNativeRadianceScheduleManifest,
    pub lumen_lighting_packet: BangerNativeLumenLightingPacket,
    pub virtual_shadow_packet: BangerNativeVirtualShadowPacket,
    pub direct_lighting_packet: BangerNativeDirectLightingPacket,
    pub material_closure_packet: BangerNativeMaterialClosurePacket,
    pub temporal_history_packet: BangerNativeTemporalHistoryPacket,
    pub gaussian_splat_layer_manifest: BangerNativeGaussianSplatLayerManifest,
    pub frame_submission_packet: BangerNativeFrameSubmissionPacket,
    pub rhi_submit_packet: BangerNativeRhiSubmitPacket,
    pub gpu_execution_receipt: BangerNativeGpuExecutionReceipt,
    pub backend_submit_plan: BangerNativeBackendSubmitPlan,
    pub backend_execution_packet: BangerNativeBackendExecutionPacket,
    pub frame_graph_bindings: Vec<BangerNativeFrameGraphBinding>,
    pub artifacts: Vec<BangerNativeRenderArtifactSummary>,
    pub verifier: BangerNativeRenderVerifier,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNewObjectPrepareResponse {
    pub ok: bool,
    pub schema: &'static str,
    pub command: &'static str,
    pub lane: &'static str,
    pub scene_id: String,
    pub object_id: String,
    pub representation: &'static str,
    pub base_manifest_hash: String,
    pub updated_manifest_hash: String,
    pub newobject_contract_source_hash: String,
    pub newobject_contract_manifest_hash: String,
    pub newobject_contract_proof_hash: String,
    pub edit_hash: String,
    pub contract_prepared: bool,
    pub gpu_page_promotion_allowed: bool,
    pub promotion_gate: &'static str,
    pub editable_scene_manifest: BangerEditableSceneManifest,
    pub verifier: BangerNativeRenderVerifier,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativePresentLoopBootstrapResponse {
    pub ok: bool,
    pub schema: &'static str,
    pub engine: &'static str,
    pub lane: &'static str,
    pub native_domain: &'static str,
    pub route_status: &'static str,
    pub parent_window_handle_hash: String,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub target_frame_ms: f32,
    pub selected_adapter: Option<NativeGpuAdapter>,
    pub adapter_count: usize,
    pub backend: String,
    pub surface_kind: &'static str,
    pub swapchain_format: &'static str,
    pub present_mode: &'static str,
    pub alpha_mode: &'static str,
    pub render_pass_count: u32,
    pub submitted_frame_count: u32,
    pub clear_color: [f64; 4],
    pub frame_hash: String,
    pub present_loop_hash: String,
    pub proof_hash: String,
    pub verifier: BangerNativeRenderVerifier,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatAssetPrepareResponse {
    pub ok: bool,
    pub schema: &'static str,
    pub asset_id: String,
    pub source_path: String,
    pub source_hash: String,
    pub splat_count: usize,
    pub truncated: bool,
    pub ply_format: &'static str,
    pub property_count: usize,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub bounding_sphere: [f32; 4],
    pub positions_buffer_hash: String,
    pub covariance_buffer_hash: String,
    pub opacity_buffer_hash: String,
    pub sh_buffer_hash: String,
    pub sort_key_hash: String,
    pub bucket_manifest_hash: String,
    pub gpu_layout_hash: String,
    pub asset_manifest_hash: String,
    pub gpu_layout: BangerGaussianSplatGpuLayout,
    pub buckets: Vec<BangerGaussianSplatAssetBucket>,
    pub verifier: BangerNativeRenderVerifier,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatGpuLayout {
    pub schema: &'static str,
    pub position_format: &'static str,
    pub covariance_format: &'static str,
    pub opacity_format: &'static str,
    pub color_format: &'static str,
    pub sort_key_format: &'static str,
    pub bytes_per_splat: u32,
    pub layout_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatAssetBucket {
    pub bucket_id: u32,
    pub first_splat: u32,
    pub splat_count: u32,
    pub sort_key_hash: String,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatRasterizeResponse {
    pub ok: bool,
    pub schema: &'static str,
    pub asset_id: String,
    pub source_hash: String,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub tile_count: u32,
    pub splat_count: usize,
    pub projected_splat_count: usize,
    pub rasterized_splat_count: usize,
    pub shaded_pixel_count: u32,
    pub camera_hash: String,
    pub tile_manifest_hash: String,
    pub projected_manifest_hash: String,
    pub rgba8_hash: String,
    pub raster_proof_hash: String,
    pub rgba8: Vec<u8>,
    pub tiles: Vec<BangerGaussianSplatRasterTile>,
    pub verifier: BangerNativeRenderVerifier,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerGaussianSplatRasterTile {
    pub tile_id: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub splat_count: u32,
    pub contribution_count: u32,
    pub depth_sort_hash: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderPass {
    pub name: &'static str,
    pub stage: &'static str,
    pub consumes_kind: &'static str,
    pub writes: &'static str,
    pub cache_class: &'static str,
    pub residency_policy: &'static str,
    pub async_compute_candidate: bool,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderArtifactSummary {
    pub name: String,
    pub kind: &'static str,
    pub layout: &'static str,
    pub byte_len: u64,
    pub page_count: u64,
    pub artifact_hash: String,
    pub renderer_cache_hash: String,
    pub renderer_variant_hash: String,
    pub renderer_promotion_hash: String,
    pub source_output_hash: String,
    pub first_page_hash: Option<String>,
    pub last_page_hash: Option<String>,
    pub page_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeResidencyJob {
    pub artifact_name: String,
    pub kind: &'static str,
    pub render_graph_stage: &'static str,
    pub update_mode: &'static str,
    pub page_count: u64,
    pub byte_len: u64,
    pub page_hashes: Vec<String>,
    pub cache_key: String,
    pub promotion_hash: String,
    pub frame_blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeResourceTable {
    pub schema: &'static str,
    pub table_hash: String,
    pub slot_count: usize,
    pub resident_bytes: u64,
    pub upload_bytes: u64,
    pub vram_budget_mb: u32,
    pub budget_pressure_pct: f32,
    pub slots: Vec<BangerNativeResourceSlot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeResourceSlot {
    pub slot: u32,
    pub artifact_name: String,
    pub kind: &'static str,
    pub page_index: u64,
    pub page_hash: String,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub heap: &'static str,
    pub usage: &'static str,
    pub upload_lane: &'static str,
    pub render_graph_stage: &'static str,
    pub resource_key: String,
    pub pipeline_cache_key: String,
    pub promotion_hash: String,
    #[serde(skip_serializing)]
    pub payload_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativePageResidencyAllocatorPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub resource_table_hash: String,
    pub nanite_second_layer_hash: String,
    pub virtual_shadow_hash: String,
    pub material_closure_hash: String,
    pub render_graph_hash: String,
    pub virtual_page_count: usize,
    pub physical_page_count: usize,
    pub resident_page_count: usize,
    pub streaming_request_count: usize,
    pub eviction_candidate_count: usize,
    pub locked_page_count: usize,
    pub physical_pool_hash: String,
    pub virtual_page_table_hash: String,
    pub feedback_request_hash: String,
    pub allocation_hash: String,
    pub eviction_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativePageResidencyEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativePageResidencyEntry {
    pub page_id: String,
    pub page_kind: &'static str,
    pub physical_pool: &'static str,
    pub object_id: String,
    pub cluster_id: String,
    pub resource_slot: u32,
    pub source_page_hash: String,
    pub virtual_address: [u32; 4],
    pub physical_address: [u32; 4],
    pub residency_state: &'static str,
    pub priority: u32,
    pub lock_state: &'static str,
    pub producer_hash: String,
    pub feedback_hash: String,
    pub allocation_hash: String,
    pub eviction_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativePipelineCacheManifest {
    pub schema: &'static str,
    pub cache_root: String,
    pub selected_adapter_hash: String,
    pub selected_adapter_label: String,
    pub backend: String,
    pub driver_hash: String,
    pub driver_info_hash: String,
    pub entry_count: usize,
    pub persisted_entry_count: usize,
    pub manifest_hash: String,
    pub promotion_status: &'static str,
    pub entries: Vec<BangerNativePipelineCacheEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativePipelineCacheEntry {
    pub schema: &'static str,
    pub pass_name: &'static str,
    pub artifact_name: String,
    pub artifact_kind: &'static str,
    pub pipeline_cache_key: String,
    pub adapter_hash: String,
    pub driver_hash: String,
    pub shader_source_hash: String,
    pub shader_reflection_hash: String,
    pub shader_target_manifest_hash: String,
    pub material_abi_hash: String,
    pub render_pass_abi_hash: String,
    pub renderer_variant_hash: String,
    pub blob_hash: String,
    pub blob_len: u64,
    pub blob_path: String,
    pub persistence_status: &'static str,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeBenchmarkPromotionManifest {
    pub schema: &'static str,
    pub authority: &'static str,
    pub gate_count: usize,
    pub passed_gate_count: usize,
    pub promotion_allowed: bool,
    pub target_frame_ms: f32,
    pub estimated_frame_ms: f32,
    pub frame_time_headroom_pct: f32,
    pub vram_pressure_pct: f32,
    pub cache_reuse_ratio: f32,
    pub proof_reproducibility_status: &'static str,
    pub proof_reproducibility_hash: String,
    pub visual_capability_score: u32,
    pub visual_capability_hash: String,
    pub benchmark_hash: String,
    pub gates: Vec<BangerNativeBenchmarkGate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeBenchmarkGate {
    pub name: &'static str,
    pub metric: &'static str,
    pub threshold: f32,
    pub measured: f32,
    pub passed: bool,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeTextureBridgeContract {
    pub schema: &'static str,
    pub route_status: &'static str,
    pub import_route: &'static str,
    pub fallback_route: &'static str,
    pub backend: String,
    pub selected_adapter_hash: String,
    pub device_queue_hash: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: &'static str,
    pub texture_usage: Vec<&'static str>,
    pub present_policy: &'static str,
    pub same_device_queue_available: bool,
    pub external_handle_import_required: bool,
    pub frame_hash: String,
    pub viewport_contract_hash: String,
    pub resize_proof_hash: String,
    pub camera_control_proof_hash: String,
    pub bridge_proof_hash: String,
    pub viewport: BangerNativeViewportContract,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeViewportContract {
    pub fit_mode: &'static str,
    pub camera_mode: &'static str,
    pub orbit_enabled: bool,
    pub pan_enabled: bool,
    pub zoom_enabled: bool,
    pub resize_policy: &'static str,
    pub scene_graph_hash: String,
    pub scene_bounds_hash: String,
    pub viewport_fit_hash: String,
    pub target_frame_ms: f32,
    pub min_extent: u32,
    pub max_extent: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeSceneGraphSubmission {
    pub schema: &'static str,
    pub authority: &'static str,
    pub scene_id: String,
    pub object_count: usize,
    pub visible_object_count: usize,
    pub renderable_object_count: usize,
    pub hidden_object_count: usize,
    pub root_object_id: String,
    pub transform_propagation_hash: String,
    pub visibility_hash: String,
    pub representation_mix_hash: String,
    pub viewport_fit_hash: String,
    pub render_submission_hash: String,
    pub submission_hash: String,
    pub fit_bounds_min: [f32; 3],
    pub fit_bounds_max: [f32; 3],
    pub fit_bounding_sphere: [f32; 4],
    pub representation_mix: Vec<BangerNativeRepresentationMixEntry>,
    pub submissions: Vec<BangerNativeSceneSubmissionNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRepresentationMixEntry {
    pub representation: &'static str,
    pub object_count: usize,
    pub renderable_count: usize,
    pub weight: f32,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeSceneSubmissionNode {
    pub object_id: String,
    pub parent_id: Option<String>,
    pub representation: &'static str,
    pub visible: bool,
    pub renderable: bool,
    pub submission_order: u32,
    pub local_transform_hash: String,
    pub world_transform_hash: String,
    pub world_aabb_min: [f32; 3],
    pub world_aabb_max: [f32; 3],
    pub world_bounding_sphere: [f32; 4],
    pub resource_slots: Vec<u32>,
    pub render_graph_stages: Vec<&'static str>,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGpuScenePacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub scene_graph_hash: String,
    pub resource_table_hash: String,
    pub material_abi_hash: String,
    pub primitive_count: usize,
    pub instance_count: usize,
    pub payload_float4_count: usize,
    pub upload_range_count: usize,
    pub gpu_write_primitive_count: usize,
    pub max_persistent_primitive_index: u32,
    pub primitive_scene_data_hash: String,
    pub instance_scene_data_hash: String,
    pub instance_payload_data_hash: String,
    pub material_table_hash: String,
    pub upload_ranges_hash: String,
    pub packet_hash: String,
    pub primitives: Vec<BangerNativeGpuScenePrimitive>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGpuScenePrimitive {
    pub primitive_id: u32,
    pub object_id: String,
    pub representation: &'static str,
    pub supports_gpu_scene: bool,
    pub supports_nanite_like_streaming: bool,
    pub visible: bool,
    pub renderable: bool,
    pub instance_scene_data_offset: u32,
    pub instance_count: u32,
    pub payload_data_offset: u32,
    pub payload_float4_count: u32,
    pub resource_slots: Vec<u32>,
    pub material_record_hash: String,
    pub primitive_flags_word: u32,
    pub bounds_hash: String,
    pub transform_hash: String,
    pub upload_range_hash: String,
    pub primitive_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeCullingManifest {
    pub schema: &'static str,
    pub authority: &'static str,
    pub culling_path: &'static str,
    pub candidate_count: usize,
    pub visible_count: usize,
    pub culled_count: usize,
    pub max_lod_error: f32,
    pub visibility_result_hash: String,
    pub indirect_draw_buffer_hash: String,
    pub cache_reuse_hash: String,
    pub manifest_hash: String,
    pub entries: Vec<BangerNativeCullingEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeCullingEntry {
    pub object_id: String,
    pub representation: &'static str,
    pub culling_basis: &'static str,
    pub visible_after_cull: bool,
    pub cache_hit_reuse: bool,
    pub cache_reuse_key: String,
    pub lod_error: f32,
    pub lod_bucket: u32,
    pub cone_apex: [f32; 3],
    pub cone_axis: [f32; 3],
    pub cone_cutoff: f32,
    pub bounding_sphere: [f32; 4],
    pub resource_slots: Vec<u32>,
    pub indirect_draw_args: [u32; 5],
    pub visibility_result_hash: String,
    pub indirect_draw_hash: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeMeshletVisibilityPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub scene_graph_hash: String,
    pub culling_manifest_hash: String,
    pub render_graph_manifest_hash: String,
    pub cluster_count: usize,
    pub visible_cluster_count: usize,
    pub hardware_raster_candidate_count: usize,
    pub software_raster_candidate_count: usize,
    pub max_lod_bucket: u32,
    pub indirect_draw_word_count: usize,
    pub visibility_buffer_hash: String,
    pub lod_error_buffer_hash: String,
    pub cluster_page_table_hash: String,
    pub indirect_draw_packet_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeMeshletVisibilityEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeMeshletVisibilityEntry {
    pub cluster_id: String,
    pub object_id: String,
    pub resource_slot: u32,
    pub page_hash: String,
    pub raster_path: &'static str,
    pub lod_bucket: u32,
    pub lod_error: f32,
    pub bounding_sphere: [f32; 4],
    pub cone_axis: [f32; 3],
    pub cone_cutoff: f32,
    pub visibility_word: u64,
    pub indirect_draw_args: [u32; 5],
    pub source_culling_proof_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeNaniteSecondLayerPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub gpu_scene_hash: String,
    pub visibility_packet_hash: String,
    pub resource_table_hash: String,
    pub material_abi_hash: String,
    pub streaming_request_count: usize,
    pub resident_page_count: usize,
    pub feedback_word_count: usize,
    pub shading_bin_count: usize,
    pub visibility_resolve_tile_count: usize,
    pub ray_tracing_proxy_count: usize,
    pub streaming_feedback_hash: String,
    pub page_residency_hash: String,
    pub material_bin_hash: String,
    pub visibility_resolve_hash: String,
    pub ray_tracing_bridge_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeNaniteSecondLayerEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeNaniteSecondLayerEntry {
    pub cluster_id: String,
    pub object_id: String,
    pub primitive_id: u32,
    pub resource_slot: u32,
    pub page_hash: String,
    pub residency_state: &'static str,
    pub requested_lod_bucket: u32,
    pub feedback_word: u64,
    pub material_bin_id: u32,
    pub material_flags_word: u32,
    pub visibility_tile: [u32; 4],
    pub ray_tracing_proxy_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRasterWorkQueue {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub visibility_packet_hash: String,
    pub render_graph_hash: String,
    pub resource_table_hash: String,
    pub hardware_job_count: usize,
    pub compute_job_count: usize,
    pub total_threadgroup_count: u32,
    pub total_index_count: u32,
    pub queue_barrier_hash: String,
    pub bind_table_hash: String,
    pub dispatch_plan_hash: String,
    pub queue_hash: String,
    pub jobs: Vec<BangerNativeRasterWorkItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRasterWorkItem {
    pub job_id: String,
    pub cluster_id: String,
    pub object_id: String,
    pub queue_lane: &'static str,
    pub pass_name: &'static str,
    pub pipeline_cache_key: String,
    pub resource_slot: u32,
    pub page_hash: String,
    pub visibility_word: u64,
    pub threadgroup_count: u32,
    pub indirect_draw_args: [u32; 5],
    pub read_barrier: &'static str,
    pub write_barrier: &'static str,
    pub bind_group_hash: String,
    pub job_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeFrameSubmissionPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub texture_bridge_hash: String,
    pub render_graph_hash: String,
    pub raster_queue_hash: String,
    pub color_target_hash: String,
    pub depth_target_hash: String,
    pub render_target_state_hash: String,
    pub command_buffer_hash: String,
    pub frame_schedule_hash: String,
    pub presentable_frame_hash: String,
    pub submission_hash: String,
    pub pass_count: usize,
    pub raster_job_count: usize,
    pub command_count: usize,
    pub submitted_queue_count: usize,
    pub commands: Vec<BangerNativeFrameCommandPacket>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeFrameCommandPacket {
    pub command_id: String,
    pub order: u32,
    pub pass_name: &'static str,
    pub stage: &'static str,
    pub queue_lane: &'static str,
    pub resource_read_count: usize,
    pub resource_write_count: usize,
    pub raster_job_count: usize,
    pub first_raster_job_hash: Option<String>,
    pub last_raster_job_hash: Option<String>,
    pub input_hash: String,
    pub output_target_hash: String,
    pub barrier_hash: String,
    pub command_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRhiSubmitPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub frame_submission_hash: String,
    pub texture_bridge_hash: String,
    pub backend: String,
    pub selected_adapter_hash: String,
    pub command_list_count: usize,
    pub submit_batch_count: usize,
    pub submitted_queue_count: usize,
    pub timeline_base_value: u64,
    pub acquire_backbuffer_hash: String,
    pub finalized_command_lists_hash: String,
    pub submit_batch_hash: String,
    pub present_hash: String,
    pub fence_timeline_hash: String,
    pub packet_hash: String,
    pub steps: Vec<BangerNativeRhiSubmitStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRhiSubmitStep {
    pub step_id: String,
    pub order: u32,
    pub phase: &'static str,
    pub queue_lane: &'static str,
    pub command_hash: Option<String>,
    pub wait_hash: String,
    pub signal_hash: String,
    pub timeline_value: u64,
    pub step_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGpuExecutionReceipt {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub rhi_submit_hash: String,
    pub frame_submission_hash: String,
    pub present_hash: String,
    pub execution_status: &'static str,
    pub nonblank_frame_expected: bool,
    pub submitted_step_count: usize,
    pub completed_phase_count: usize,
    pub command_list_count: usize,
    pub queue_lane_count: usize,
    pub frame_diagnostic_hash: String,
    pub queue_timeline_hash: String,
    pub readback_policy_hash: String,
    pub receipt_hash: String,
    pub phases: Vec<BangerNativeGpuExecutionPhaseReceipt>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGpuExecutionPhaseReceipt {
    pub phase_id: String,
    pub phase: &'static str,
    pub queue_lane: &'static str,
    pub source_step_hash: String,
    pub timeline_value: u64,
    pub completed: bool,
    pub diagnostic_hash: String,
    pub phase_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeBackendSubmitPlan {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub backend_family: &'static str,
    pub backend_label: String,
    pub frame_submission_hash: String,
    pub rhi_submit_hash: String,
    pub execution_receipt_hash: String,
    pub swapchain_contract_hash: String,
    pub descriptor_heap_hash: String,
    pub pipeline_state_cache_hash: String,
    pub backend_barrier_plan_hash: String,
    pub command_allocator_hash: String,
    pub submit_plan_hash: String,
    pub targets: Vec<BangerNativeBackendSubmitTarget>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeBackendSubmitTarget {
    pub target_id: String,
    pub backend_family: &'static str,
    pub queue_lane: &'static str,
    pub swapchain_image_count: u32,
    pub descriptor_table_count: u32,
    pub pipeline_state_count: u32,
    pub barrier_batch_count: u32,
    pub command_allocator_count: u32,
    pub present_path: &'static str,
    pub target_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeBackendExecutionPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub backend_submit_plan_hash: String,
    pub rhi_submit_hash: String,
    pub execution_receipt_hash: String,
    pub selected_backend: String,
    pub executor_mode: &'static str,
    pub executable_pass_count: usize,
    pub readback_byte_count: u64,
    pub nonzero_tile_count: u32,
    pub nonblack_pixel_sample_count: u32,
    pub swapchain_image_count: u32,
    pub memory_barrier_count: u32,
    pub executor_schedule_hash: String,
    pub pipeline_binding_hash: String,
    pub readback_buffer_hash: String,
    pub nonblank_signature_hash: String,
    pub frame_latch_hash: String,
    pub packet_hash: String,
    pub passes: Vec<BangerNativeBackendExecutionPass>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeBackendExecutionPass {
    pub pass_id: String,
    pub order: u32,
    pub pass_name: &'static str,
    pub stage: &'static str,
    pub queue_lane: &'static str,
    pub command_hash: String,
    pub target_hash: String,
    pub descriptor_table_hash: String,
    pub pipeline_state_hash: String,
    pub barrier_batch_hash: String,
    pub readback_region_hash: String,
    pub nonblank_sample_hash: String,
    pub pass_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRadianceScheduleManifest {
    pub schema: &'static str,
    pub authority: &'static str,
    pub temporal_epoch: u64,
    pub probe_page_count: usize,
    pub active_probe_count: u64,
    pub light_budget: u32,
    pub async_compute_residency_policy: &'static str,
    pub invalidation_hash: String,
    pub schedule_hash: String,
    pub entries: Vec<BangerNativeRadianceProbePage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRadianceProbePage {
    pub probe_page_id: String,
    pub source_slot: u32,
    pub source_page_hash: String,
    pub probe_count: u32,
    pub temporal_reuse_frames: u32,
    pub light_budget: u32,
    pub update_priority: u32,
    pub async_compute: bool,
    pub residency_policy: &'static str,
    pub invalidation_hash: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeLumenLightingPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub gpu_scene_hash: String,
    pub nanite_second_layer_hash: String,
    pub radiance_schedule_hash: String,
    pub render_graph_hash: String,
    pub surface_cache_page_count: usize,
    pub screen_probe_count: usize,
    pub radiance_tile_count: usize,
    pub hardware_trace_candidate_count: usize,
    pub software_trace_candidate_count: usize,
    pub reflection_ray_budget: u64,
    pub total_probe_rays: u64,
    pub surface_cache_hash: String,
    pub screen_probe_hash: String,
    pub trace_policy_hash: String,
    pub diffuse_indirect_hash: String,
    pub reflection_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeLumenLightingEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeLumenLightingEntry {
    pub object_id: String,
    pub cluster_id: String,
    pub surface_page_id: String,
    pub source_probe_page_id: String,
    pub material_bin_id: u32,
    pub residency_state: &'static str,
    pub screen_probe_coord: [u32; 2],
    pub radiance_tile: [u32; 4],
    pub trace_policy: &'static str,
    pub diffuse_ray_count: u32,
    pub reflection_ray_count: u32,
    pub temporal_reuse_frames: u32,
    pub surface_cache_hash: String,
    pub screen_probe_hash: String,
    pub trace_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeVirtualShadowPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub nanite_second_layer_hash: String,
    pub lumen_lighting_hash: String,
    pub radiance_schedule_hash: String,
    pub render_graph_hash: String,
    pub virtual_page_count: usize,
    pub cached_page_count: usize,
    pub invalidated_page_count: usize,
    pub light_page_count: usize,
    pub shadow_ray_budget: u64,
    pub page_table_hash: String,
    pub cache_hash: String,
    pub invalidation_hash: String,
    pub projection_hash: String,
    pub light_grid_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeVirtualShadowEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeVirtualShadowEntry {
    pub object_id: String,
    pub cluster_id: String,
    pub shadow_page_id: String,
    pub source_surface_page_id: String,
    pub virtual_light_id: String,
    pub virtual_map_id: u32,
    pub clipmap_level: u32,
    pub page_coord: [u32; 3],
    pub resolution: u32,
    pub cache_state: &'static str,
    pub invalidation_reason: &'static str,
    pub light_grid_cell: [u32; 3],
    pub projection_tile: [u32; 4],
    pub ray_budget: u32,
    pub page_table_hash: String,
    pub cache_hash: String,
    pub projection_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeDirectLightingPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub lumen_lighting_hash: String,
    pub virtual_shadow_hash: String,
    pub radiance_schedule_hash: String,
    pub render_graph_hash: String,
    pub light_cluster_count: usize,
    pub stochastic_sample_count: u64,
    pub shadowed_light_count: usize,
    pub unshadowed_light_count: usize,
    pub denoiser_tile_count: usize,
    pub resolve_tile_count: usize,
    pub hardware_ray_candidate_count: usize,
    pub light_grid_hash: String,
    pub sample_sequence_hash: String,
    pub shadow_mask_hash: String,
    pub denoiser_hash: String,
    pub resolve_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeDirectLightingEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeDirectLightingEntry {
    pub object_id: String,
    pub cluster_id: String,
    pub light_cluster_id: String,
    pub virtual_light_id: String,
    pub light_kind: &'static str,
    pub light_grid_cell: [u32; 3],
    pub sample_sequence: u64,
    pub sample_count: u32,
    pub shadow_page_id: String,
    pub shadow_mask_hash: String,
    pub contribution_hash: String,
    pub denoiser_tile: [u32; 4],
    pub resolve_tile: [u32; 4],
    pub ray_tracing_candidate: bool,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeMaterialClosurePacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub direct_lighting_hash: String,
    pub lumen_lighting_hash: String,
    pub virtual_shadow_hash: String,
    pub shader_material_abi_hash: String,
    pub render_graph_hash: String,
    pub closure_count: usize,
    pub layered_closure_count: usize,
    pub texture_slot_count: u32,
    pub hardware_ray_candidate_count: usize,
    pub closure_stack_hash: String,
    pub bsdf_table_hash: String,
    pub texture_table_hash: String,
    pub resolve_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeMaterialClosureEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeMaterialClosureEntry {
    pub object_id: String,
    pub cluster_id: String,
    pub material_bin_id: u32,
    pub closure_stack_id: String,
    pub base_closure: &'static str,
    pub coating_closure: &'static str,
    pub layer_count: u32,
    pub texture_slot_base: u32,
    pub texture_slot_count: u32,
    pub roughness_quantized: u16,
    pub metallic_quantized: u16,
    pub opacity_quantized: u16,
    pub light_cluster_id: String,
    pub surface_cache_hash: String,
    pub shadow_mask_hash: String,
    pub closure_hash: String,
    pub bsdf_hash: String,
    pub texture_hash: String,
    pub resolve_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeTemporalHistoryPacket {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub material_closure_hash: String,
    pub direct_lighting_hash: String,
    pub render_graph_hash: String,
    pub temporal_epoch: u64,
    pub history_layer_count: usize,
    pub motion_vector_tile_count: usize,
    pub disocclusion_tile_count: usize,
    pub rejection_tile_count: usize,
    pub resurrection_candidate_count: usize,
    pub async_compute_candidate_count: usize,
    pub jitter_sequence_hash: String,
    pub motion_vector_hash: String,
    pub history_reprojection_hash: String,
    pub disocclusion_mask_hash: String,
    pub rejection_hash: String,
    pub accumulation_hash: String,
    pub packet_hash: String,
    pub entries: Vec<BangerNativeTemporalHistoryEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeTemporalHistoryEntry {
    pub object_id: String,
    pub cluster_id: String,
    pub history_layer_id: String,
    pub history_kind: &'static str,
    pub temporal_epoch: u64,
    pub jitter_index: u32,
    pub temporal_jitter_pixels: [f32; 2],
    pub motion_tile: [u32; 4],
    pub history_tile: [u32; 4],
    pub velocity_quantized: [i16; 2],
    pub disocclusion_score: u16,
    pub rejection_mode: &'static str,
    pub accumulation_weight_q15: u16,
    pub material_closure_hash: String,
    pub direct_resolve_hash: String,
    pub motion_vector_hash: String,
    pub history_reprojection_hash: String,
    pub disocclusion_hash: String,
    pub accumulation_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGaussianSplatLayerManifest {
    pub schema: &'static str,
    pub authority: &'static str,
    pub layer_count: usize,
    pub bucket_count: usize,
    pub conversion_count: usize,
    pub proxy_bounds_hash: String,
    pub sort_key_hash: String,
    pub group_key_hash: String,
    pub conversion_manifest_hash: String,
    pub manifest_hash: String,
    pub layers: Vec<BangerNativeGaussianSplatLayer>,
    pub conversions: Vec<BangerNativeGaussianSplatConversionManifest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGaussianSplatLayer {
    pub object_id: String,
    pub source_representation: &'static str,
    pub layer_kind: &'static str,
    pub bucket_id: String,
    pub sort_key: String,
    pub group_key: String,
    pub proxy_bounds_min: [f32; 3],
    pub proxy_bounds_max: [f32; 3],
    pub proxy_bounding_sphere: [f32; 4],
    pub splat_count_estimate: u32,
    pub opacity_cutoff: f32,
    pub lod_bucket: u32,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeGaussianSplatConversionManifest {
    pub object_id: String,
    pub from_representation: &'static str,
    pub target_representation: &'static str,
    pub conversion_path: &'static str,
    pub enabled: bool,
    pub proxy_bounds_hash: String,
    pub source_proof_hash: String,
    pub conversion_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerEditableSceneManifest {
    pub schema: &'static str,
    pub scene_id: String,
    pub authority: &'static str,
    pub edit_policy: &'static str,
    pub object_count: usize,
    pub root_count: usize,
    pub manifest_hash: String,
    pub graph_hash: String,
    pub bounds_hash: String,
    pub objects: Vec<BangerEditableSceneObject>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerEditableSceneObject {
    pub object_id: String,
    pub parent_id: Option<String>,
    pub role: &'static str,
    pub representation: &'static str,
    pub source_artifact_name: Option<String>,
    pub source_artifact_kind: &'static str,
    pub source_artifact_hash: String,
    pub renderer_cache_hash: String,
    pub residency_policy: &'static str,
    pub local_transform: [f32; 16],
    pub world_transform: [f32; 16],
    pub local_transform_hash: String,
    pub world_transform_hash: String,
    pub visible: bool,
    pub renderable: bool,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    pub bounding_sphere: [f32; 4],
    pub editable_slots: Vec<&'static str>,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeFrameGraphBinding {
    pub pass_name: &'static str,
    pub stage: &'static str,
    pub pipeline_cache_key: String,
    pub resource_slots: Vec<u32>,
    pub read_barrier: &'static str,
    pub write_barrier: &'static str,
    pub async_compute_candidate: bool,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderGraphCompilation {
    pub schema: &'static str,
    pub authority: &'static str,
    pub clean_room_basis: &'static str,
    pub source_contract_hash: String,
    pub monster_kasm_contract_hash: String,
    pub pass_count: usize,
    pub resource_count: usize,
    pub edge_count: usize,
    pub async_compute_candidate_count: usize,
    pub extracted_resource_count: usize,
    pub culled_pass_count: usize,
    pub compiled_order_hash: String,
    pub resource_lifetime_hash: String,
    pub barrier_plan_hash: String,
    pub graph_hash: String,
    pub resources: Vec<BangerNativeRenderGraphResource>,
    pub edges: Vec<BangerNativeRenderGraphEdge>,
    pub compiled_passes: Vec<BangerNativeRenderGraphCompiledPass>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderGraphResource {
    pub name: String,
    pub kind: &'static str,
    pub producer_pass: &'static str,
    pub first_stage: &'static str,
    pub last_stage: &'static str,
    pub slot_count: usize,
    pub resident_bytes: u64,
    pub upload_bytes: u64,
    pub extracted: bool,
    pub resource_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderGraphEdge {
    pub from_pass: &'static str,
    pub to_pass: &'static str,
    pub resource_name: String,
    pub resource_hash: String,
    pub read_barrier: &'static str,
    pub write_barrier: &'static str,
    pub async_boundary: bool,
    pub edge_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderGraphCompiledPass {
    pub order: u32,
    pub pass_name: &'static str,
    pub stage: &'static str,
    pub pass_kind: &'static str,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub pipeline_cache_key: String,
    pub async_compute_candidate: bool,
    pub culled: bool,
    pub pass_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeShaderCapabilityPlan {
    pub bootstrap_rhi: &'static str,
    pub frontier_target: &'static str,
    pub mesh_shader_preferred: bool,
    pub fallback_path: &'static str,
    pub capability_gate: &'static str,
    pub compiler_ticket_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeShaderCompilerTicket {
    pub schema: &'static str,
    pub preferred_compiler: &'static str,
    pub compiler_detected: bool,
    pub compiler_version_hash: String,
    pub compiler_version_excerpt: String,
    pub mini_probe_source_hash: String,
    pub mini_probe_output_hash: String,
    pub mini_probe_status: &'static str,
    pub mini_probe_excerpt: String,
    #[serde(skip_serializing)]
    pub mini_probe_wgsl: Option<String>,
    pub requested_targets: Vec<&'static str>,
    pub promoted_target: &'static str,
    pub bootstrap_target: &'static str,
    pub source_language: &'static str,
    pub module_strategy: &'static str,
    pub reflection_status: &'static str,
    pub target_manifest_hash: String,
    pub fallback_wgsl_hash: String,
    pub fallback_wgsl_parity_hash: String,
    pub material_abi_hash: String,
    pub target_artifacts: Vec<BangerNativeShaderTargetArtifact>,
    pub reflection_manifest: BangerNativeShaderReflectionManifest,
    pub material_abi: BangerNativeShaderMaterialAbi,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeShaderTargetArtifact {
    pub target: &'static str,
    pub output_format: &'static str,
    pub entry_point: &'static str,
    pub status: &'static str,
    pub source_hash: String,
    pub output_hash: String,
    pub diagnostic_hash: String,
    pub byte_len: u64,
    pub artifact_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeShaderReflectionManifest {
    pub schema: &'static str,
    pub entry_point: &'static str,
    pub stage: &'static str,
    pub binding_count: usize,
    pub storage_buffer_count: usize,
    pub read_write_buffer_count: usize,
    pub material_abi_hash: String,
    pub reflection_hash: String,
    pub bindings: Vec<BangerNativeShaderReflectionBinding>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeShaderReflectionBinding {
    pub name: &'static str,
    pub category: &'static str,
    pub group: u32,
    pub binding: u32,
    pub access: &'static str,
    pub payload_kind: &'static str,
    pub byte_stride: u32,
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeShaderMaterialAbi {
    pub schema: &'static str,
    pub abi_name: &'static str,
    pub bind_group: u32,
    pub material_buffer_binding: u32,
    pub texture_binding_base: u32,
    pub sampler_binding_base: u32,
    pub material_record_bytes: u32,
    pub material_record_alignment: u32,
    pub max_texture_slots: u32,
    pub layout_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BangerNativeRenderVerifier {
    pub wall: &'static str,
    pub frontier_hypothesis: &'static str,
    pub local_gate: &'static str,
    pub rollback_path: &'static str,
}

pub struct BangerNativeEngine;

impl BangerNativeEngine {
    pub fn bootstrap_present_loop(
        request: BangerNativePresentLoopBootstrapRequest,
    ) -> Result<BangerNativePresentLoopBootstrapResponse, String> {
        let viewport_width = request.viewport_width.unwrap_or(1280).clamp(64, 16384);
        let viewport_height = request.viewport_height.unwrap_or(720).clamp(64, 16384);
        let target_frame_ms = request.target_frame_ms.unwrap_or(16.67).clamp(4.0, 1000.0);
        let parent_window_handle_hash = request
            .parent_window_handle
            .as_deref()
            .filter(|handle| !handle.trim().is_empty())
            .map(|handle| hash_text_hex("forge.banger.native_present_loop.parent_window.v1", handle.trim()))
            .unwrap_or_else(|| "no_parent_window_handle".to_string());
        let gpu_probe = native_gpu_adapter_probe();
        let render = run_wgpu_present_bootstrap(viewport_width, viewport_height)?;
        let selected_adapter = render.selected_adapter.clone().or(gpu_probe.selected);
        let backend = render.backend.clone();
        let frame_hash = present_loop_frame_hash(
            viewport_width,
            viewport_height,
            target_frame_ms,
            &backend,
            render.swapchain_format,
            render.present_mode,
            render.alpha_mode,
            render.clear_color,
            &parent_window_handle_hash,
        );
        let present_loop_hash = present_loop_bootstrap_hash(
            &frame_hash,
            viewport_width,
            viewport_height,
            target_frame_ms,
            &backend,
            render.surface_kind,
            render.swapchain_format,
            render.present_mode,
            render.alpha_mode,
            &parent_window_handle_hash,
        );
        let proof_hash = hash_text_hex(
            "forge.banger.native_present_loop_bootstrap.proof.v1",
            &format!(
                "{present_loop_hash}:{frame_hash}:{}:{}:{}",
                render.render_pass_count, render.submitted_frame_count, gpu_probe.adapters.len()
            ),
        );

        Ok(BangerNativePresentLoopBootstrapResponse {
            ok: true,
            schema: "forge.banger.native_present_loop_bootstrap.v1",
            engine: "banger_rust_native_engine",
            lane: "native_tandem_render",
            native_domain: "render_3d",
            route_status: if request.parent_window_handle.is_some() {
                "child_surface_parent_handle_hash_bound"
            } else {
                "offscreen_present_ready_child_surface_pending"
            },
            parent_window_handle_hash,
            viewport_width,
            viewport_height,
            target_frame_ms,
            selected_adapter,
            adapter_count: gpu_probe.adapters.len().max(render.adapter_count),
            backend,
            surface_kind: render.surface_kind,
            swapchain_format: render.swapchain_format,
            present_mode: render.present_mode,
            alpha_mode: render.alpha_mode,
            render_pass_count: render.render_pass_count,
            submitted_frame_count: render.submitted_frame_count,
            clear_color: render.clear_color,
            frame_hash,
            present_loop_hash,
            proof_hash,
            verifier: BangerNativeRenderVerifier {
                wall: "latency+native_surface+ui_branching",
                frontier_hypothesis:
                    "Banger owns a Rust/wgpu present loop contract before Electron receives a child-window swapchain.",
                local_gate:
                    "forge-cargo test --manifest-path examples\\ingen_native_services\\Cargo.toml banger_native_engine::tests::bootstraps_native_present_loop_contract",
                rollback_path:
                    "remove bootstrap_present_loop and keep the static native preview frame IPC",
            },
        })
    }

    pub fn prepare_newobject_contract(
        monster: &MonsterNode,
        request: BangerNewObjectPrepareRequest,
    ) -> Result<BangerNewObjectPrepareResponse, String> {
        let BangerNewObjectPrepareRequest {
            scene_id,
            object_id,
            parent_id,
            object_prompt,
            representation,
            known_fragment_hashes,
            target_frame_ms,
            vram_budget_mb,
            prefer_mesh_shaders,
            pipeline_cache_dir,
            viewport_width,
            viewport_height,
        } = request;
        let scene_id = sanitize_scene_id(scene_id.as_deref().unwrap_or("banger_default_scene"));
        let representation = normalize_newobject_representation(representation.as_deref())?;
        let prompt_hash = hash_text_hex("forge.banger.newobject.prompt.v1", &object_prompt);
        let object_id = object_id
            .as_deref()
            .map(sanitize_scene_id)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("newobject_{}", &prompt_hash[..12]));

        let base_handoff = Self::prepare_render_handoff(
            monster,
            BangerNativeRenderPrepareRequest {
                scene_id: Some(scene_id.clone()),
                known_fragment_hashes: known_fragment_hashes.clone(),
                target_frame_ms,
                vram_budget_mb,
                prefer_mesh_shaders,
                pipeline_cache_dir,
                viewport_width,
                viewport_height,
            },
        )?;
        let base_manifest_hash = base_handoff.editable_scene_manifest.manifest_hash.clone();
        let parent_id = parent_id
            .as_deref()
            .map(sanitize_scene_ref)
            .unwrap_or_else(|| format!("{scene_id}:root"));
        if !base_handoff
            .editable_scene_manifest
            .objects
            .iter()
            .any(|object| object.object_id == parent_id)
        {
            return Err(format!(
                "Banger /newobject_ parent {parent_id} is not present in editable scene manifest {base_manifest_hash}"
            ));
        }

        let contract_source =
            banger_newobject_forge_source(&scene_id, &object_id, representation, &prompt_hash);
        let contract_prepared = monster
            .prepare_forge_source(&contract_source, known_fragment_hashes.unwrap_or_default())
            .map_err(|err| format!("Banger /newobject_ Forge contract prepare failed: {err:?}"))?;
        if contract_prepared.route.lane != MonsterEngineLane::NativeTandemRender
            || contract_prepared.route.native_domain != MonsterNativeTandemDomain::Render3d
        {
            return Err(format!(
                "Banger /newobject_ routed to {:?}/{} instead of native_tandem_render/render_3d",
                contract_prepared.route.lane, contract_prepared.route.native_domain
            ));
        }

        let pending_object = pending_newobject_scene_object(
            &contract_prepared,
            &scene_id,
            &object_id,
            &parent_id,
            representation,
        );
        let editable_scene_manifest = append_newobject_to_manifest(
            &contract_prepared,
            &base_handoff.resource_table,
            base_handoff.editable_scene_manifest,
            pending_object,
        );
        let updated_manifest_hash = editable_scene_manifest.manifest_hash.clone();
        let edit_hash = newobject_edit_hash(
            &base_manifest_hash,
            &updated_manifest_hash,
            &contract_prepared,
            &object_id,
            representation,
        );

        Ok(BangerNewObjectPrepareResponse {
            ok: true,
            schema: "forge.banger.newobject_prepare.v1",
            command: "/newobject_",
            lane: MonsterEngineLane::NativeTandemRender.label(),
            scene_id,
            object_id,
            representation,
            base_manifest_hash,
            updated_manifest_hash,
            newobject_contract_source_hash: contract_prepared.route.plan.source_hash.clone(),
            newobject_contract_manifest_hash: contract_prepared.manifest_hash.clone(),
            newobject_contract_proof_hash: contract_prepared.route.plan.proof_hash.clone(),
            edit_hash,
            contract_prepared: true,
            gpu_page_promotion_allowed: false,
            promotion_gate: "renderer_must_consume_updated_editable_scene_manifest_before_gpu_page_promotion",
            editable_scene_manifest,
            verifier: BangerNativeRenderVerifier {
                wall: "ui_branching+proof_quality+scene_authority",
                frontier_hypothesis:
                    "/newobject_ becomes a scene-manifest edit backed by a fresh Forge/Monster render contract, not a renderer-local object side path.",
                local_gate:
                    "cargo test --manifest-path examples\\ingen_native_services\\Cargo.toml banger_native_engine::tests::prepares_newobject_contract_as_scene_manifest_edit",
                rollback_path:
                    "remove prepare_newobject_contract and keep the native_tandem_render handoff manifest from step 1",
            },
        })
    }

    pub fn prepare_gaussian_splat_asset(
        request: BangerGaussianSplatAssetPrepareRequest,
    ) -> Result<BangerGaussianSplatAssetPrepareResponse, String> {
        let asset_id =
            sanitize_scene_id(request.asset_id.as_deref().unwrap_or("gaussian_splat_asset"));
        let ply_path = PathBuf::from(&request.ply_path);
        let bytes = fs::read(&ply_path).map_err(|err| {
            format!(
                "failed to read Gaussian splat PLY {}: {err}",
                ply_path.display()
            )
        })?;
        let source_hash = hash_bytes_hex("forge.banger.gaussian_splat.source_ply.v1", &bytes);
        let mut parsed = parse_gaussian_splat_ply(&bytes)?;
        let max_splats = request.max_splats.unwrap_or(parsed.splats.len()).max(1);
        let truncated = parsed.splats.len() > max_splats;
        parsed.splats.truncate(max_splats);
        if parsed.splats.is_empty() {
            return Err("Gaussian splat PLY contains no vertex splats".to_string());
        }

        let positions = gaussian_splat_positions_buffer(&parsed.splats);
        let covariance = gaussian_splat_covariance_buffer(&parsed.splats);
        let opacity = gaussian_splat_opacity_buffer(&parsed.splats);
        let sh = gaussian_splat_sh_buffer(&parsed.splats);
        let sort_keys = gaussian_splat_asset_sort_key_buffer(&parsed.splats);
        let positions_buffer_hash =
            hash_bytes_hex("forge.banger.gaussian_splat.positions_buffer.v1", &positions);
        let covariance_buffer_hash =
            hash_bytes_hex("forge.banger.gaussian_splat.covariance_buffer.v1", &covariance);
        let opacity_buffer_hash =
            hash_bytes_hex("forge.banger.gaussian_splat.opacity_buffer.v1", &opacity);
        let sh_buffer_hash = hash_bytes_hex("forge.banger.gaussian_splat.sh_buffer.v1", &sh);
        let sort_key_hash =
            hash_bytes_hex("forge.banger.gaussian_splat.sort_key_buffer.v1", &sort_keys);
        let (bounds_min, bounds_max) = gaussian_splat_asset_bounds(&parsed.splats);
        let bounding_sphere = bounding_sphere(bounds_min, bounds_max);
        let bucket_count = request.bucket_count.unwrap_or(32).clamp(1, 4096);
        let buckets = gaussian_splat_asset_buckets(&parsed.splats, bucket_count);
        let bucket_manifest_hash = gaussian_splat_asset_bucket_manifest_hash(&buckets);
        let gpu_layout_hash = gaussian_splat_gpu_layout_hash();
        let gpu_layout = BangerGaussianSplatGpuLayout {
            schema: "forge.banger.gaussian_splat_gpu_layout.v1",
            position_format: "float32x3",
            covariance_format: "scale_log_float32x3+rotation_quat_float32x4",
            opacity_format: "sigmoid_float32",
            color_format: "spherical_harmonics_l3_dc_rest_float32",
            sort_key_format: "depth_desc_u32_stable_index_u32",
            bytes_per_splat: 12 + 28 + 4 + 192 + 8,
            layout_hash: gpu_layout_hash.clone(),
        };
        let asset_manifest_hash = gaussian_splat_asset_manifest_hash(
            &asset_id,
            &source_hash,
            parsed.format,
            parsed.property_names.len(),
            parsed.splats.len(),
            truncated,
            bounds_min,
            bounds_max,
            &positions_buffer_hash,
            &covariance_buffer_hash,
            &opacity_buffer_hash,
            &sh_buffer_hash,
            &sort_key_hash,
            &bucket_manifest_hash,
            &gpu_layout_hash,
        );

        Ok(BangerGaussianSplatAssetPrepareResponse {
            ok: true,
            schema: "forge.banger.gaussian_splat_asset_manifest.v1",
            asset_id,
            source_path: ply_path.to_string_lossy().to_string(),
            source_hash,
            splat_count: parsed.splats.len(),
            truncated,
            ply_format: parsed.format,
            property_count: parsed.property_names.len(),
            bounds_min,
            bounds_max,
            bounding_sphere,
            positions_buffer_hash,
            covariance_buffer_hash,
            opacity_buffer_hash,
            sh_buffer_hash,
            sort_key_hash,
            bucket_manifest_hash,
            gpu_layout_hash,
            asset_manifest_hash,
            gpu_layout,
            buckets,
            verifier: BangerNativeRenderVerifier {
                wall: "latency+memory+proof_quality",
                frontier_hypothesis:
                    "Banger consumes real 3DGS PLY assets as content-addressed GPU buffers before any rasterizer promotion.",
                local_gate:
                    "forge-cargo test --manifest-path examples\\ingen_native_services\\Cargo.toml banger_native_engine::tests::prepares_real_gaussian_splat_ply_asset_buffers",
                rollback_path:
                    "remove prepare_gaussian_splat_asset and keep gaussian_splat_layer_manifest as the hybrid scene placeholder",
            },
        })
    }

    pub fn rasterize_gaussian_splat_asset(
        request: BangerGaussianSplatRasterizeRequest,
    ) -> Result<BangerGaussianSplatRasterizeResponse, String> {
        let asset_id =
            sanitize_scene_id(request.asset_id.as_deref().unwrap_or("gaussian_splat_raster"));
        let width = request.width.clamp(1, 4096);
        let height = request.height.clamp(1, 4096);
        let tile_size = request.tile_size.unwrap_or(16).clamp(4, 64);
        let ply_path = PathBuf::from(&request.ply_path);
        let bytes = fs::read(&ply_path).map_err(|err| {
            format!(
                "failed to read Gaussian splat PLY {}: {err}",
                ply_path.display()
            )
        })?;
        let source_hash = hash_bytes_hex("forge.banger.gaussian_splat.source_ply.v1", &bytes);
        let mut parsed = parse_gaussian_splat_ply(&bytes)?;
        let max_splats = request.max_splats.unwrap_or(parsed.splats.len()).max(1);
        parsed.splats.truncate(max_splats);
        if parsed.splats.is_empty() {
            return Err("Gaussian splat PLY contains no vertex splats".to_string());
        }

        let camera = GaussianSplatCamera::new(
            request.camera_position.unwrap_or([0.0, 0.0, -4.0]),
            request.camera_target.unwrap_or([0.0, 0.0, 0.0]),
            request.camera_up.unwrap_or([0.0, 1.0, 0.0]),
            request.fov_y_degrees.unwrap_or(55.0),
            request.near_plane.unwrap_or(0.01),
            width,
            height,
        )?;
        let background = request.background_rgba.unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let projected = project_gaussian_splats(&parsed.splats, &camera);
        let tile_grid = GaussianSplatTileGrid::new(width, height, tile_size);
        let tile_lists = gaussian_splat_tile_lists(&projected, &tile_grid);
        let mut tiles = Vec::with_capacity(tile_lists.len());
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        let mut rasterized = vec![false; projected.len()];
        let mut shaded_pixel_count = 0u32;

        for (tile_index, tile_splats) in tile_lists.iter().enumerate() {
            let tile_bounds = tile_grid.tile_bounds(tile_index as u32);
            let contribution_count = rasterize_gaussian_splat_tile(
                tile_bounds,
                tile_splats,
                &projected,
                &mut rasterized,
                width,
                &mut rgba,
                background,
            );
            shaded_pixel_count = shaded_pixel_count.saturating_add(contribution_count);
            let depth_sort_hash = gaussian_splat_raster_tile_sort_hash(tile_splats, &projected);
            let proof_hash = gaussian_splat_raster_tile_proof_hash(
                tile_index as u32,
                tile_bounds,
                tile_splats.len() as u32,
                contribution_count,
                &depth_sort_hash,
            );
            tiles.push(BangerGaussianSplatRasterTile {
                tile_id: tile_index as u32,
                x: tile_bounds.x0,
                y: tile_bounds.y0,
                width: tile_bounds.x1 - tile_bounds.x0,
                height: tile_bounds.y1 - tile_bounds.y0,
                splat_count: tile_splats.len() as u32,
                contribution_count,
                depth_sort_hash,
                proof_hash,
            });
        }

        let rgba8_hash = hash_bytes_hex("forge.banger.gaussian_splat.rgba8.v1", &rgba);
        let camera_hash = gaussian_splat_camera_hash(&camera);
        let tile_manifest_hash = gaussian_splat_raster_tile_manifest_hash(&tiles);
        let projected_manifest_hash = gaussian_splat_projected_manifest_hash(&projected);
        let rasterized_splat_count = rasterized.iter().filter(|value| **value).count();
        let raster_proof_hash = gaussian_splat_raster_proof_hash(
            &asset_id,
            &source_hash,
            width,
            height,
            tile_size,
            parsed.splats.len(),
            projected.len(),
            rasterized_splat_count,
            shaded_pixel_count,
            &camera_hash,
            &tile_manifest_hash,
            &projected_manifest_hash,
            &rgba8_hash,
        );

        Ok(BangerGaussianSplatRasterizeResponse {
            ok: true,
            schema: "forge.banger.gaussian_splat_rasterizer.v1",
            asset_id,
            source_hash,
            width,
            height,
            tile_size,
            tile_count: tiles.len() as u32,
            splat_count: parsed.splats.len(),
            projected_splat_count: projected.len(),
            rasterized_splat_count,
            shaded_pixel_count,
            camera_hash,
            tile_manifest_hash,
            projected_manifest_hash,
            rgba8_hash,
            raster_proof_hash,
            rgba8: rgba,
            tiles,
            verifier: BangerNativeRenderVerifier {
                wall: "latency+memory+proof_quality",
                frontier_hypothesis:
                    "A reference 3DGS rasterizer projects anisotropic covariances into tiled EWA splats with deterministic alpha compositing before GPU promotion.",
                local_gate:
                    "forge-cargo test --manifest-path examples\\ingen_native_services\\Cargo.toml banger_native_engine::tests::rasterizes_real_gaussian_splat_ply_to_rgba8",
                rollback_path:
                    "remove rasterize_gaussian_splat_asset while keeping the verified PLY buffer import path",
            },
        })
    }

    pub fn prepare_render_handoff(
        monster: &MonsterNode,
        request: BangerNativeRenderPrepareRequest,
    ) -> Result<BangerNativeRenderPrepareResponse, String> {
        let BangerNativeRenderPrepareRequest {
            scene_id,
            known_fragment_hashes,
            target_frame_ms,
            vram_budget_mb,
            prefer_mesh_shaders,
            pipeline_cache_dir,
            viewport_width,
            viewport_height,
        } = request;
        let scene_id = sanitize_scene_id(scene_id.as_deref().unwrap_or("banger_default_scene"));
        let target_frame_ms = target_frame_ms.unwrap_or(16.67);
        let vram_budget_mb = vram_budget_mb.unwrap_or(4096);
        let prefer_mesh_shaders = prefer_mesh_shaders.unwrap_or(true);
        let source = banger_native_render_forge_source(&scene_id);
        let known = known_fragment_hashes.unwrap_or_default();
        let prepared = monster
            .prepare_forge_source(&source, known)
            .map_err(|err| format!("Monster native render prepare failed: {err:?}"))?;

        if prepared.route.lane != MonsterEngineLane::NativeTandemRender
            || prepared.route.native_domain != MonsterNativeTandemDomain::Render3d
        {
            return Err(format!(
                "Monster routed Banger native render to {:?}/{} instead of native_tandem_render/render_3d",
                prepared.route.lane, prepared.route.native_domain
            ));
        }

        let render_artifacts = prepared
            .native_tandem_artifacts
            .iter()
            .filter(|artifact| artifact.domain == "render_3d")
            .collect::<Vec<_>>();
        if render_artifacts.is_empty() {
            return Err("Monster returned no render_3d native tandem artifact for Banger".to_string());
        }

        let artifacts = render_artifacts
            .iter()
            .map(|artifact| BangerNativeRenderArtifactSummary::from(*artifact))
            .collect::<Vec<_>>();
        let render_graph = render_artifacts
            .iter()
            .map(|artifact| render_pass_from_artifact(artifact))
            .collect::<Vec<_>>();
        let residency_jobs = render_artifacts
            .iter()
            .map(|artifact| residency_job_from_artifact(artifact, &prepared))
            .collect::<Vec<_>>();
        let gpu_shader_profiles = shader_profiles(&prepared);
        let shader_compiler_ticket = shader_compiler_ticket(&prepared, &gpu_shader_profiles, prefer_mesh_shaders);
        let pipeline_cache_keys = render_artifacts
            .iter()
            .map(|artifact| banger_pipeline_cache_key(&prepared, artifact, prefer_mesh_shaders))
            .collect::<Vec<_>>();
        let pipeline_cache_manifest = build_pipeline_cache_manifest(
            &prepared,
            &render_artifacts,
            &render_graph,
            &pipeline_cache_keys,
            &shader_compiler_ticket,
            pipeline_cache_dir.as_deref(),
        )?;
        let resource_table = build_resource_table(
            &prepared,
            &render_artifacts,
            &render_graph,
            &pipeline_cache_keys,
            vram_budget_mb,
        );
        let editable_scene_manifest =
            build_editable_scene_manifest(&prepared, &render_artifacts, &resource_table, &scene_id);
        let frame_graph_bindings = build_frame_graph_bindings(
            &render_artifacts,
            &render_graph,
            &pipeline_cache_keys,
            &resource_table,
        );
        let render_graph_compilation =
            compile_banger_render_graph(&prepared, &render_graph, &frame_graph_bindings, &resource_table);
        let scene_graph_submission = build_scene_graph_submission(
            &prepared,
            &editable_scene_manifest,
            &resource_table,
            &frame_graph_bindings,
        );
        let gpu_scene_packet = build_gpu_scene_packet(
            &prepared,
            &scene_graph_submission,
            &resource_table,
            &shader_compiler_ticket,
        );
        let culling_manifest =
            build_culling_manifest(&prepared, &scene_graph_submission, &resource_table);
        let meshlet_visibility_packet = build_meshlet_visibility_packet(
            &prepared,
            &scene_graph_submission,
            &culling_manifest,
            &resource_table,
            &render_graph_compilation,
        );
        let nanite_second_layer_packet = build_nanite_second_layer_packet(
            &prepared,
            &gpu_scene_packet,
            &meshlet_visibility_packet,
            &resource_table,
            &shader_compiler_ticket,
        );
        let raster_work_queue = build_raster_work_queue(
            &prepared,
            &meshlet_visibility_packet,
            &render_graph_compilation,
            &frame_graph_bindings,
            &resource_table,
        );
        let radiance_schedule_manifest = build_radiance_schedule_manifest(
            &prepared,
            &scene_graph_submission,
            &culling_manifest,
            &resource_table,
        );
        let lumen_lighting_packet = build_lumen_lighting_packet(
            &prepared,
            &gpu_scene_packet,
            &nanite_second_layer_packet,
            &radiance_schedule_manifest,
            &render_graph_compilation,
        );
        let virtual_shadow_packet = build_virtual_shadow_packet(
            &prepared,
            &nanite_second_layer_packet,
            &lumen_lighting_packet,
            &radiance_schedule_manifest,
            &render_graph_compilation,
        );
        let direct_lighting_packet = build_direct_lighting_packet(
            &prepared,
            &lumen_lighting_packet,
            &virtual_shadow_packet,
            &radiance_schedule_manifest,
            &render_graph_compilation,
        );
        let material_closure_packet = build_material_closure_packet(
            &prepared,
            &shader_compiler_ticket.material_abi,
            &lumen_lighting_packet,
            &virtual_shadow_packet,
            &direct_lighting_packet,
            &render_graph_compilation,
        );
        let page_residency_allocator = build_page_residency_allocator_packet(
            &prepared,
            &resource_table,
            &nanite_second_layer_packet,
            &virtual_shadow_packet,
            &material_closure_packet,
            &render_graph_compilation,
        );
        let temporal_history_packet = build_temporal_history_packet(
            &prepared,
            &direct_lighting_packet,
            &material_closure_packet,
            &render_graph_compilation,
        );
        let gaussian_splat_layer_manifest = build_gaussian_splat_layer_manifest(
            &prepared,
            &scene_graph_submission,
            &culling_manifest,
            &radiance_schedule_manifest,
        );
        let texture_bridge_contract = build_texture_bridge_contract(
            &prepared,
            &pipeline_cache_manifest,
            &resource_table,
            &editable_scene_manifest,
            &scene_graph_submission,
            target_frame_ms,
            viewport_width,
            viewport_height,
        );
        let frame_submission_packet = build_frame_submission_packet(
            &prepared,
            &texture_bridge_contract,
            &render_graph_compilation,
            &raster_work_queue,
            &radiance_schedule_manifest,
            &lumen_lighting_packet,
            &virtual_shadow_packet,
            &direct_lighting_packet,
            &material_closure_packet,
            &page_residency_allocator,
            &temporal_history_packet,
            &gaussian_splat_layer_manifest,
        );
        let rhi_submit_packet =
            build_rhi_submit_packet(&prepared, &texture_bridge_contract, &frame_submission_packet);
        let gpu_execution_receipt = build_gpu_execution_receipt(
            &prepared,
            &frame_submission_packet,
            &rhi_submit_packet,
            &raster_work_queue,
        );
        let backend_submit_plan = build_backend_submit_plan(
            &prepared,
            &pipeline_cache_manifest,
            &texture_bridge_contract,
            &frame_submission_packet,
            &rhi_submit_packet,
            &gpu_execution_receipt,
        );
        let backend_execution_packet = build_backend_execution_packet(
            &prepared,
            &texture_bridge_contract,
            &frame_submission_packet,
            &rhi_submit_packet,
            &gpu_execution_receipt,
            &backend_submit_plan,
        );
        let benchmark_promotion_manifest = build_benchmark_promotion_manifest(
            &prepared,
            &render_graph,
            &pipeline_cache_manifest,
            &texture_bridge_contract,
            &resource_table,
            &scene_graph_submission,
            &gpu_scene_packet,
            &culling_manifest,
            &radiance_schedule_manifest,
            &gaussian_splat_layer_manifest,
            &shader_compiler_ticket,
            target_frame_ms,
        );
        let render_handoff_hash = render_handoff_hash(
            &prepared,
            &artifacts,
            &shader_compiler_ticket,
            &pipeline_cache_manifest,
            &benchmark_promotion_manifest,
            &texture_bridge_contract,
            &scene_graph_submission,
            &gpu_scene_packet,
            &culling_manifest,
            &meshlet_visibility_packet,
            &nanite_second_layer_packet,
            &raster_work_queue,
            &radiance_schedule_manifest,
            &lumen_lighting_packet,
            &virtual_shadow_packet,
            &direct_lighting_packet,
            &material_closure_packet,
            &page_residency_allocator,
            &temporal_history_packet,
            &gaussian_splat_layer_manifest,
            &frame_submission_packet,
            &rhi_submit_packet,
            &gpu_execution_receipt,
            &backend_submit_plan,
            &backend_execution_packet,
            &render_graph_compilation,
        );
        let render_pass_count = render_graph.len();
        let residency_job_count = residency_jobs.len();
        let resource_table_hash = resource_table.table_hash.clone();
        let render_graph_manifest_hash = render_graph_compilation.graph_hash.clone();

        Ok(BangerNativeRenderPrepareResponse {
            ok: true,
            schema: "forge.banger.native_render_handoff.v1",
            engine: "banger_rust_native_engine",
            lane: "native_tandem_render",
            native_domain: MonsterNativeTandemDomain::Render3d.label(),
            scene_id,
            source_hash: prepared.route.plan.source_hash.clone(),
            proof_hash: prepared.route.plan.proof_hash.clone(),
            manifest_hash: prepared.manifest_hash.clone(),
            render_handoff_hash,
            cache_miss_count: prepared.cache_miss_work.len(),
            fully_cached: prepared.is_fully_cached(),
            frame_blocking_allowed: prepared.route.plan.frame_blocking_allowed,
            target_frame_ms,
            vram_budget_mb,
            render_pass_count,
            residency_job_count,
            gpu_shader_profiles,
            shader_capability_plan: BangerNativeShaderCapabilityPlan {
                bootstrap_rhi: "wgpu_native_vulkan_metal_dx12",
                frontier_target: "slang_capability_checked_meshlet_mesh_shader_rhi",
                mesh_shader_preferred: prefer_mesh_shaders,
                fallback_path: "compute_cull_indirect_draw_when_mesh_shader_unavailable_or_benchmark_gate_fails",
                capability_gate: "benchmark_promotion_hash+render_manifest_hash+shader_profile+renderer_cache_hash+shader_compiler_ticket_hash",
                compiler_ticket_hash: shader_compiler_ticket.proof_hash.clone(),
            },
            shader_compiler_ticket,
            pipeline_cache_manifest,
            benchmark_promotion_manifest,
            texture_bridge_contract,
            pipeline_cache_keys,
            render_graph,
            render_graph_manifest_hash,
            render_graph_compilation,
            residency_jobs,
            resource_table_hash,
            resource_table,
            page_residency_allocator,
            editable_scene_manifest,
            scene_graph_submission,
            gpu_scene_packet,
            culling_manifest,
            meshlet_visibility_packet,
            nanite_second_layer_packet,
            raster_work_queue,
            radiance_schedule_manifest,
            lumen_lighting_packet,
            virtual_shadow_packet,
            direct_lighting_packet,
            material_closure_packet,
            temporal_history_packet,
            gaussian_splat_layer_manifest,
            frame_submission_packet,
            rhi_submit_packet,
            gpu_execution_receipt,
            backend_submit_plan,
            backend_execution_packet,
            frame_graph_bindings,
            artifacts,
            verifier: BangerNativeRenderVerifier {
                wall: "latency+proof_quality+ui_branching",
                frontier_hypothesis:
                    "Monster emits content-addressed render pages; Banger turns them into sparse residency jobs and render graph passes keyed by proof hashes.",
                local_gate:
                    "cargo check --manifest-path examples\\ingen_native_services\\Cargo.toml plus route=NativeTandemRender",
                rollback_path:
                    "remove banger_native_engine.rs native handoff; src/kasm.rs and src/monster.rs stay untouched",
            },
        })
    }
}

fn build_editable_scene_manifest(
    prepared: &MonsterPreparedCompute,
    artifacts: &[&MonsterNativeTandemArtifact],
    resource_table: &BangerNativeResourceTable,
    scene_id: &str,
) -> BangerEditableSceneManifest {
    let root_id = format!("{scene_id}:root");
    let mut child_objects = artifacts
        .iter()
        .enumerate()
        .map(|(index, artifact)| editable_scene_object_from_artifact(prepared, artifact, &root_id, scene_id, index))
        .collect::<Vec<_>>();
    let (root_min, root_max) = merged_bounds(child_objects.iter().map(|object| (object.aabb_min, object.aabb_max)));
    let root_transform = identity_transform();
    let mut root = BangerEditableSceneObject {
        object_id: root_id,
        parent_id: None,
        role: "scene_authority",
        representation: "hybrid_scene_root",
        source_artifact_name: None,
        source_artifact_kind: "scene_manifest",
        source_artifact_hash: prepared.manifest_hash.clone(),
        renderer_cache_hash: resource_table.table_hash.clone(),
        residency_policy: "children_resident_by_artifact_policy",
        local_transform: root_transform,
        world_transform: root_transform,
        local_transform_hash: transform_hash("local", &root_transform),
        world_transform_hash: transform_hash("world", &root_transform),
        visible: true,
        renderable: false,
        aabb_min: root_min,
        aabb_max: root_max,
        bounding_sphere: bounding_sphere(root_min, root_max),
        editable_slots: vec!["name", "children", "visibility", "representation_mix"],
        proof_hash: String::new(),
    };
    root.proof_hash = editable_object_hash(&root);

    let mut objects = Vec::with_capacity(child_objects.len() + 1);
    objects.push(root);
    objects.append(&mut child_objects);
    let graph_hash = editable_scene_graph_hash(scene_id, &objects);
    let bounds_hash = editable_scene_bounds_hash(scene_id, &objects);
    let manifest_hash = editable_scene_manifest_hash(prepared, resource_table, &objects, &graph_hash, &bounds_hash);
    BangerEditableSceneManifest {
        schema: "forge.banger.editable_scene_manifest.v1",
        scene_id: scene_id.to_string(),
        authority: "monster_native_tandem_render_artifacts",
        edit_policy: "scene_first_edits_emit_new_forge_contract_before_gpu_pages_are_promoted",
        object_count: objects.len(),
        root_count: 1,
        manifest_hash,
        graph_hash,
        bounds_hash,
        objects,
    }
}

fn editable_scene_object_from_artifact(
    prepared: &MonsterPreparedCompute,
    artifact: &MonsterNativeTandemArtifact,
    root_id: &str,
    scene_id: &str,
    index: usize,
) -> BangerEditableSceneObject {
    let object_id = format!("{scene_id}:{index}:{}", sanitize_scene_id(&artifact.name));
    let transform = identity_transform();
    let (aabb_min, aabb_max) = artifact_aabb(artifact);
    let mut object = BangerEditableSceneObject {
        object_id,
        parent_id: Some(root_id.to_string()),
        role: scene_role_for_kind(artifact.kind),
        representation: scene_representation_for_kind(artifact.kind),
        source_artifact_name: Some(artifact.name.clone()),
        source_artifact_kind: artifact.kind,
        source_artifact_hash: artifact.artifact_hash.clone(),
        renderer_cache_hash: artifact.renderer_cache_hash.clone(),
        residency_policy: artifact.renderer_residency_policy,
        local_transform: transform,
        world_transform: transform,
        local_transform_hash: transform_hash("local", &transform),
        world_transform_hash: transform_hash("world", &transform),
        visible: true,
        renderable: renderable_kind(artifact.kind),
        aabb_min,
        aabb_max,
        bounding_sphere: bounding_sphere(aabb_min, aabb_max),
        editable_slots: editable_slots_for_kind(artifact.kind),
        proof_hash: String::new(),
    };
    object.proof_hash = editable_object_hash_with_manifest(prepared, &object);
    object
}

fn append_newobject_to_manifest(
    prepared: &MonsterPreparedCompute,
    resource_table: &BangerNativeResourceTable,
    mut manifest: BangerEditableSceneManifest,
    object: BangerEditableSceneObject,
) -> BangerEditableSceneManifest {
    manifest.objects.push(object);
    manifest.object_count = manifest.objects.len();
    manifest.graph_hash = editable_scene_graph_hash(&manifest.scene_id, &manifest.objects);
    manifest.bounds_hash = editable_scene_bounds_hash(&manifest.scene_id, &manifest.objects);
    manifest.manifest_hash = editable_scene_manifest_hash(
        prepared,
        resource_table,
        &manifest.objects,
        &manifest.graph_hash,
        &manifest.bounds_hash,
    );
    manifest
}

fn pending_newobject_scene_object(
    prepared: &MonsterPreparedCompute,
    scene_id: &str,
    object_id: &str,
    parent_id: &str,
    representation: &'static str,
) -> BangerEditableSceneObject {
    let transform = identity_transform();
    let (aabb_min, aabb_max) = fallback_aabb_for_representation(representation);
    let mut object = BangerEditableSceneObject {
        object_id: format!("{scene_id}:{object_id}"),
        parent_id: Some(parent_id.to_string()),
        role: scene_role_for_representation(representation),
        representation,
        source_artifact_name: Some(format!("{object_id}.newobject_contract")),
        source_artifact_kind: "newobject_contract",
        source_artifact_hash: prepared.manifest_hash.clone(),
        renderer_cache_hash: prepared.route.plan.compute_ir_hash.clone(),
        residency_policy: "pending_contract_no_gpu_pages_until_renderer_promotion",
        local_transform: transform,
        world_transform: transform,
        local_transform_hash: transform_hash("local", &transform),
        world_transform_hash: transform_hash("world", &transform),
        visible: true,
        renderable: renderable_representation(representation),
        aabb_min,
        aabb_max,
        bounding_sphere: bounding_sphere(aabb_min, aabb_max),
        editable_slots: editable_slots_for_representation(representation),
        proof_hash: String::new(),
    };
    object.proof_hash = editable_object_hash_with_manifest(prepared, &object);
    object
}

fn build_resource_table(
    prepared: &MonsterPreparedCompute,
    artifacts: &[&MonsterNativeTandemArtifact],
    render_graph: &[BangerNativeRenderPass],
    pipeline_cache_keys: &[String],
    vram_budget_mb: u32,
) -> BangerNativeResourceTable {
    let mut slots = Vec::new();
    let mut resident_bytes = 0u64;
    let mut upload_bytes = 0u64;
    for (artifact_index, artifact) in artifacts.iter().enumerate() {
        let pass = &render_graph[artifact_index];
        let pipeline_cache_key = pipeline_cache_keys
            .get(artifact_index)
            .cloned()
            .unwrap_or_else(|| artifact.renderer_cache_hash.clone());
        for page in &artifact.pages {
            let byte_len = page.bytes.len() as u64;
            resident_bytes = resident_bytes.saturating_add(byte_len);
            if !prepared.is_fully_cached() {
                upload_bytes = upload_bytes.saturating_add(byte_len);
            }
            let slot = slots.len() as u32;
            let resource_key = banger_resource_key(prepared, artifact, page.index, &page.page_hash);
            slots.push(BangerNativeResourceSlot {
                slot,
                artifact_name: artifact.name.clone(),
                kind: artifact.kind,
                page_index: page.index,
                page_hash: page.page_hash.clone(),
                byte_offset: page.byte_offset,
                byte_len,
                heap: resource_heap_for_kind(artifact.kind),
                usage: resource_usage_for_kind(artifact.kind),
                upload_lane: upload_lane_for_kind(artifact.kind),
                render_graph_stage: pass.stage,
                resource_key,
                pipeline_cache_key: pipeline_cache_key.clone(),
                promotion_hash: artifact.renderer_promotion_hash.clone(),
                payload_bytes: page.bytes.clone(),
            });
        }
    }
    let budget_bytes = u64::from(vram_budget_mb).saturating_mul(1024 * 1024).max(1);
    let budget_pressure_pct = ((resident_bytes as f64 / budget_bytes as f64) * 100.0).min(999.0) as f32;
    let table_hash = resource_table_hash(prepared, &slots, resident_bytes, upload_bytes, vram_budget_mb);
    BangerNativeResourceTable {
        schema: "forge.banger.native_resource_table.v1",
        table_hash,
        slot_count: slots.len(),
        resident_bytes,
        upload_bytes,
        vram_budget_mb,
        budget_pressure_pct,
        slots,
    }
}

fn build_frame_graph_bindings(
    artifacts: &[&MonsterNativeTandemArtifact],
    render_graph: &[BangerNativeRenderPass],
    pipeline_cache_keys: &[String],
    resource_table: &BangerNativeResourceTable,
) -> Vec<BangerNativeFrameGraphBinding> {
    render_graph
        .iter()
        .enumerate()
        .map(|(artifact_index, pass)| {
            let artifact = artifacts[artifact_index];
            let resource_slots = resource_table
                .slots
                .iter()
                .filter(|slot| slot.artifact_name == artifact.name && slot.kind == artifact.kind)
                .map(|slot| slot.slot)
                .collect::<Vec<_>>();
            let pipeline_cache_key = pipeline_cache_keys
                .get(artifact_index)
                .cloned()
                .unwrap_or_else(|| artifact.renderer_cache_hash.clone());
            BangerNativeFrameGraphBinding {
                pass_name: pass.name,
                stage: pass.stage,
                pipeline_cache_key,
                resource_slots,
                read_barrier: read_barrier_for_stage(pass.stage),
                write_barrier: write_barrier_for_stage(pass.stage),
                async_compute_candidate: pass.async_compute_candidate,
                proof_hash: pass.proof_hash.clone(),
            }
        })
        .collect()
}

fn compile_banger_render_graph(
    prepared: &MonsterPreparedCompute,
    render_graph: &[BangerNativeRenderPass],
    frame_graph_bindings: &[BangerNativeFrameGraphBinding],
    resource_table: &BangerNativeResourceTable,
) -> BangerNativeRenderGraphCompilation {
    let mut resources = Vec::new();
    let mut compiled_passes = Vec::new();
    for (index, pass) in render_graph.iter().enumerate() {
        let binding = &frame_graph_bindings[index];
        let slots = resource_table
            .slots
            .iter()
            .filter(|slot| binding.resource_slots.contains(&slot.slot))
            .collect::<Vec<_>>();
        let resident_bytes = slots.iter().map(|slot| slot.byte_len).sum::<u64>();
        let upload_bytes = slots
            .iter()
            .filter(|slot| slot.upload_lane != "resident_cache")
            .map(|slot| slot.byte_len)
            .sum::<u64>();
        let resource_name = format!("{}::{}", pass.stage, pass.writes);
        let resource_hash = banger_render_graph_resource_hash(
            &resource_name,
            pass,
            binding,
            slots.iter().map(|slot| slot.resource_key.as_str()).collect::<Vec<_>>().as_slice(),
            resident_bytes,
            upload_bytes,
        );
        resources.push(BangerNativeRenderGraphResource {
            name: resource_name.clone(),
            kind: pass.consumes_kind,
            producer_pass: pass.name,
            first_stage: pass.stage,
            last_stage: pass.stage,
            slot_count: slots.len(),
            resident_bytes,
            upload_bytes,
            extracted: matches!(pass.stage, "visibility_cull" | "lighting_cache" | "material_bind"),
            resource_hash: resource_hash.clone(),
        });
        let pass_hash = banger_render_graph_compiled_pass_hash(
            index as u32,
            pass,
            binding,
            &resource_name,
            &resource_hash,
        );
        compiled_passes.push(BangerNativeRenderGraphCompiledPass {
            order: index as u32,
            pass_name: pass.name,
            stage: pass.stage,
            pass_kind: pass.consumes_kind,
            reads: binding.resource_slots.iter().map(|slot| format!("resource_slot_{slot}")).collect(),
            writes: vec![resource_name],
            pipeline_cache_key: binding.pipeline_cache_key.clone(),
            async_compute_candidate: binding.async_compute_candidate,
            culled: binding.resource_slots.is_empty(),
            pass_hash,
        });
    }

    let mut edges = Vec::new();
    for index in 1..compiled_passes.len() {
        let previous = &compiled_passes[index - 1];
        let current = &compiled_passes[index];
        let previous_resource = &resources[index - 1];
        let binding = &frame_graph_bindings[index];
        let edge_hash = banger_render_graph_edge_hash(
            previous.pass_name,
            current.pass_name,
            &previous_resource.name,
            &previous_resource.resource_hash,
            binding.read_barrier,
            binding.write_barrier,
            previous.async_compute_candidate || current.async_compute_candidate,
        );
        edges.push(BangerNativeRenderGraphEdge {
            from_pass: previous.pass_name,
            to_pass: current.pass_name,
            resource_name: previous_resource.name.clone(),
            resource_hash: previous_resource.resource_hash.clone(),
            read_barrier: binding.read_barrier,
            write_barrier: binding.write_barrier,
            async_boundary: previous.async_compute_candidate || current.async_compute_candidate,
            edge_hash,
        });
    }

    let compiled_order_hash = banger_render_graph_order_hash(&compiled_passes);
    let resource_lifetime_hash = banger_render_graph_lifetime_hash(&resources);
    let barrier_plan_hash = banger_render_graph_barrier_hash(&edges);
    let monster_kasm_contract_hash = banger_render_graph_kasm_contract_hash(prepared, render_graph);
    let graph_hash = banger_render_graph_manifest_hash(
        prepared,
        &compiled_order_hash,
        &resource_lifetime_hash,
        &barrier_plan_hash,
        &monster_kasm_contract_hash,
        &compiled_passes,
        &resources,
        &edges,
    );
    BangerNativeRenderGraphCompilation {
        schema: "forge.banger.native_render_graph_compilation.v1",
        authority: "monster_kasm_to_banger_native_render_graph",
        clean_room_basis: "local_unreal_sparse_study_rdg_rhi_meshpass_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        monster_kasm_contract_hash,
        pass_count: compiled_passes.len(),
        resource_count: resources.len(),
        edge_count: edges.len(),
        async_compute_candidate_count: compiled_passes
            .iter()
            .filter(|pass| pass.async_compute_candidate)
            .count(),
        extracted_resource_count: resources.iter().filter(|resource| resource.extracted).count(),
        culled_pass_count: compiled_passes.iter().filter(|pass| pass.culled).count(),
        compiled_order_hash,
        resource_lifetime_hash,
        barrier_plan_hash,
        graph_hash,
        resources,
        edges,
        compiled_passes,
    }
}

fn build_scene_graph_submission(
    prepared: &MonsterPreparedCompute,
    editable_scene_manifest: &BangerEditableSceneManifest,
    resource_table: &BangerNativeResourceTable,
    frame_graph_bindings: &[BangerNativeFrameGraphBinding],
) -> BangerNativeSceneGraphSubmission {
    let mut objects = editable_scene_manifest.objects.clone();
    propagate_scene_graph_world_transforms(&mut objects);
    let submissions = objects
        .iter()
        .enumerate()
        .map(|(index, object)| scene_submission_node(object, index, resource_table, frame_graph_bindings))
        .collect::<Vec<_>>();
    let visible_object_count = submissions.iter().filter(|node| node.visible).count();
    let renderable_object_count = submissions
        .iter()
        .filter(|node| node.visible && node.renderable)
        .count();
    let hidden_object_count = submissions.len().saturating_sub(visible_object_count);
    let representation_mix = scene_representation_mix(&submissions);
    let (fit_bounds_min, fit_bounds_max) = scene_submission_fit_bounds(&submissions);
    let fit_bounding_sphere = bounding_sphere(fit_bounds_min, fit_bounds_max);
    let transform_propagation_hash = scene_transform_propagation_hash(&objects);
    let visibility_hash = scene_visibility_hash(&submissions);
    let representation_mix_hash = scene_representation_mix_hash(&representation_mix);
    let viewport_fit_hash = scene_viewport_fit_hash(
        editable_scene_manifest,
        &fit_bounds_min,
        &fit_bounds_max,
        &fit_bounding_sphere,
        &visibility_hash,
        &representation_mix_hash,
    );
    let render_submission_hash = scene_render_submission_hash(&submissions, resource_table);
    let submission_hash = scene_graph_submission_hash(
        prepared,
        editable_scene_manifest,
        &transform_propagation_hash,
        &visibility_hash,
        &representation_mix_hash,
        &viewport_fit_hash,
        &render_submission_hash,
    );
    let root_object_id = editable_scene_manifest
        .objects
        .iter()
        .find(|object| object.parent_id.is_none())
        .map(|object| object.object_id.clone())
        .unwrap_or_else(|| format!("{}:root", editable_scene_manifest.scene_id));
    BangerNativeSceneGraphSubmission {
        schema: "forge.banger.native_scene_graph_submission.v1",
        authority: "editable_scene_manifest_local_to_world_visibility_representation_mix",
        scene_id: editable_scene_manifest.scene_id.clone(),
        object_count: submissions.len(),
        visible_object_count,
        renderable_object_count,
        hidden_object_count,
        root_object_id,
        transform_propagation_hash,
        visibility_hash,
        representation_mix_hash,
        viewport_fit_hash,
        render_submission_hash,
        submission_hash,
        fit_bounds_min,
        fit_bounds_max,
        fit_bounding_sphere,
        representation_mix,
        submissions,
    }
}

fn build_gpu_scene_packet(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    resource_table: &BangerNativeResourceTable,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
) -> BangerNativeGpuScenePacket {
    let mut next_instance_offset = 0u32;
    let mut next_payload_offset = 0u32;
    let primitives = scene_graph_submission
        .submissions
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let instance_count = gpu_scene_instance_count(node);
            let payload_float4_count = gpu_scene_payload_float4_count(node, resource_table);
            let instance_scene_data_offset = next_instance_offset;
            let payload_data_offset = next_payload_offset;
            next_instance_offset = next_instance_offset.saturating_add(instance_count);
            next_payload_offset = next_payload_offset.saturating_add(payload_float4_count);
            let supports_gpu_scene = node.renderable && !node.resource_slots.is_empty();
            let supports_nanite_like_streaming =
                matches!(node.representation, "meshlet" | "sdf" | "voxel" | "gaussian_splat");
            let material_record_hash =
                gpu_scene_material_record_hash(node, shader_compiler_ticket, resource_table);
            let primitive_flags_word =
                gpu_scene_primitive_flags_word(node, supports_gpu_scene, supports_nanite_like_streaming);
            let bounds_hash = gpu_scene_bounds_hash(node);
            let transform_hash = gpu_scene_transform_hash(node);
            let upload_range_hash = gpu_scene_upload_range_hash(
                node,
                instance_scene_data_offset,
                instance_count,
                payload_data_offset,
                payload_float4_count,
            );
            let primitive_hash = gpu_scene_primitive_hash(
                index as u32,
                node,
                supports_gpu_scene,
                supports_nanite_like_streaming,
                instance_scene_data_offset,
                instance_count,
                payload_data_offset,
                payload_float4_count,
                &material_record_hash,
                primitive_flags_word,
                &bounds_hash,
                &transform_hash,
                &upload_range_hash,
            );
            BangerNativeGpuScenePrimitive {
                primitive_id: index as u32,
                object_id: node.object_id.clone(),
                representation: node.representation,
                supports_gpu_scene,
                supports_nanite_like_streaming,
                visible: node.visible,
                renderable: node.renderable,
                instance_scene_data_offset,
                instance_count,
                payload_data_offset,
                payload_float4_count,
                resource_slots: node.resource_slots.clone(),
                material_record_hash,
                primitive_flags_word,
                bounds_hash,
                transform_hash,
                upload_range_hash,
                primitive_hash,
            }
        })
        .collect::<Vec<_>>();
    let primitive_scene_data_hash = gpu_scene_primitive_scene_data_hash(&primitives);
    let instance_scene_data_hash = gpu_scene_instance_scene_data_hash(&primitives);
    let instance_payload_data_hash = gpu_scene_instance_payload_data_hash(&primitives);
    let material_table_hash = gpu_scene_material_table_hash(&primitives, shader_compiler_ticket);
    let upload_ranges_hash = gpu_scene_upload_ranges_hash(&primitives);
    let packet_hash = gpu_scene_packet_hash(
        prepared,
        scene_graph_submission,
        resource_table,
        shader_compiler_ticket,
        &primitive_scene_data_hash,
        &instance_scene_data_hash,
        &instance_payload_data_hash,
        &material_table_hash,
        &upload_ranges_hash,
        &primitives,
    );
    BangerNativeGpuScenePacket {
        schema: "forge.banger.native_gpu_scene_packet.v1",
        authority: "banger_scene_graph_to_gpu_scene_buffers",
        clean_room_basis: "local_unreal_sparse_gpu_scene_primitive_instance_upload_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        scene_graph_hash: scene_graph_submission.submission_hash.clone(),
        resource_table_hash: resource_table.table_hash.clone(),
        material_abi_hash: shader_compiler_ticket.material_abi_hash.clone(),
        primitive_count: primitives.len(),
        instance_count: primitives
            .iter()
            .map(|primitive| primitive.instance_count as usize)
            .sum(),
        payload_float4_count: primitives
            .iter()
            .map(|primitive| primitive.payload_float4_count as usize)
            .sum(),
        upload_range_count: primitives.len(),
        gpu_write_primitive_count: primitives
            .iter()
            .filter(|primitive| primitive.supports_gpu_scene)
            .count(),
        max_persistent_primitive_index: primitives
            .last()
            .map(|primitive| primitive.primitive_id)
            .unwrap_or_default(),
        primitive_scene_data_hash,
        instance_scene_data_hash,
        instance_payload_data_hash,
        material_table_hash,
        upload_ranges_hash,
        packet_hash,
        primitives,
    }
}

fn scene_submission_node(
    object: &BangerEditableSceneObject,
    index: usize,
    resource_table: &BangerNativeResourceTable,
    frame_graph_bindings: &[BangerNativeFrameGraphBinding],
) -> BangerNativeSceneSubmissionNode {
    let (world_aabb_min, world_aabb_max) =
        transform_aabb(object.aabb_min, object.aabb_max, &object.world_transform);
    let resource_slots = object
        .source_artifact_name
        .as_ref()
        .map(|name| {
            resource_table
                .slots
                .iter()
                .filter(|slot| slot.artifact_name == *name)
                .map(|slot| slot.slot)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let render_graph_stages = frame_graph_bindings
        .iter()
        .filter(|binding| binding.resource_slots.iter().any(|slot| resource_slots.contains(slot)))
        .map(|binding| binding.stage)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let proof_hash = scene_submission_node_hash(
        object,
        index as u32,
        &world_aabb_min,
        &world_aabb_max,
        &resource_slots,
        &render_graph_stages,
    );
    BangerNativeSceneSubmissionNode {
        object_id: object.object_id.clone(),
        parent_id: object.parent_id.clone(),
        representation: object.representation,
        visible: object.visible,
        renderable: object.renderable,
        submission_order: index as u32,
        local_transform_hash: object.local_transform_hash.clone(),
        world_transform_hash: object.world_transform_hash.clone(),
        world_aabb_min,
        world_aabb_max,
        world_bounding_sphere: bounding_sphere(world_aabb_min, world_aabb_max),
        resource_slots,
        render_graph_stages,
        proof_hash,
    }
}

fn build_culling_manifest(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    resource_table: &BangerNativeResourceTable,
) -> BangerNativeCullingManifest {
    let entries = scene_graph_submission
        .submissions
        .iter()
        .filter(|node| node.visible && node.renderable && virtual_geometry_candidate(node.representation))
        .map(|node| culling_entry_from_submission(prepared, node, resource_table))
        .collect::<Vec<_>>();
    let visible_count = entries.iter().filter(|entry| entry.visible_after_cull).count();
    let culled_count = entries.len().saturating_sub(visible_count);
    let max_lod_error = entries
        .iter()
        .map(|entry| entry.lod_error)
        .fold(0.0f32, f32::max);
    let visibility_result_hash = culling_visibility_result_hash(scene_graph_submission, &entries);
    let indirect_draw_buffer_hash = culling_indirect_draw_buffer_hash(&entries);
    let cache_reuse_hash = culling_cache_reuse_hash(prepared, &entries);
    let manifest_hash = culling_manifest_hash(
        prepared,
        scene_graph_submission,
        &visibility_result_hash,
        &indirect_draw_buffer_hash,
        &cache_reuse_hash,
        &entries,
    );
    BangerNativeCullingManifest {
        schema: "forge.banger.native_culling_manifest.v1",
        authority: "scene_graph_submission_meshlet_virtual_geometry_cull_contract",
        culling_path: "compute_cull_to_indirect_draw_buffer_mesh_shader_ready",
        candidate_count: entries.len(),
        visible_count,
        culled_count,
        max_lod_error,
        visibility_result_hash,
        indirect_draw_buffer_hash,
        cache_reuse_hash,
        manifest_hash,
        entries,
    }
}

fn culling_entry_from_submission(
    prepared: &MonsterPreparedCompute,
    node: &BangerNativeSceneSubmissionNode,
    resource_table: &BangerNativeResourceTable,
) -> BangerNativeCullingEntry {
    let bounding_sphere = node.world_bounding_sphere;
    let lod_error = culling_lod_error(&bounding_sphere, node.resource_slots.len());
    let lod_bucket = culling_lod_bucket(lod_error);
    let cone_apex = [bounding_sphere[0], bounding_sphere[1], bounding_sphere[2]];
    let cone_axis = culling_cone_axis(&bounding_sphere);
    let cone_cutoff = culling_cone_cutoff(node.representation, lod_bucket);
    let visible_after_cull = node.visible && node.renderable;
    let cache_reuse_key = culling_cache_reuse_key(prepared, node, resource_table);
    let cache_hit_reuse = prepared.is_fully_cached() && !node.resource_slots.is_empty();
    let indirect_draw_args = culling_indirect_draw_args(node, lod_bucket);
    let visibility_result_hash = culling_entry_visibility_hash(
        node,
        visible_after_cull,
        lod_error,
        lod_bucket,
        &cone_apex,
        &cone_axis,
        cone_cutoff,
    );
    let indirect_draw_hash = culling_entry_indirect_draw_hash(node, &indirect_draw_args);
    let proof_hash = culling_entry_proof_hash(
        node,
        &cache_reuse_key,
        cache_hit_reuse,
        &visibility_result_hash,
        &indirect_draw_hash,
    );
    BangerNativeCullingEntry {
        object_id: node.object_id.clone(),
        representation: node.representation,
        culling_basis: culling_basis_for_representation(node.representation),
        visible_after_cull,
        cache_hit_reuse,
        cache_reuse_key,
        lod_error,
        lod_bucket,
        cone_apex,
        cone_axis,
        cone_cutoff,
        bounding_sphere,
        resource_slots: node.resource_slots.clone(),
        indirect_draw_args,
        visibility_result_hash,
        indirect_draw_hash,
        proof_hash,
    }
}

fn build_meshlet_visibility_packet(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    resource_table: &BangerNativeResourceTable,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativeMeshletVisibilityPacket {
    let mut entries = Vec::new();
    for culling_entry in culling_manifest
        .entries
        .iter()
        .filter(|entry| entry.representation == "meshlet" && entry.visible_after_cull)
    {
        for (cluster_index, resource_slot) in culling_entry.resource_slots.iter().enumerate() {
            let slot = resource_table.slots.iter().find(|slot| slot.slot == *resource_slot);
            let page_hash = slot
                .map(|slot| slot.page_hash.clone())
                .unwrap_or_else(|| culling_entry.cache_reuse_key.clone());
            let raster_path = meshlet_visibility_raster_path(
                culling_entry,
                slot.map(|slot| slot.byte_len).unwrap_or_default(),
            );
            let visibility_word = meshlet_visibility_word(
                culling_entry,
                *resource_slot,
                cluster_index as u32,
                raster_path,
            );
            let cluster_id = format!("{}:cluster_{cluster_index}", culling_entry.object_id);
            let entry_hash = meshlet_visibility_entry_hash(
                &cluster_id,
                culling_entry,
                *resource_slot,
                &page_hash,
                raster_path,
                visibility_word,
            );
            entries.push(BangerNativeMeshletVisibilityEntry {
                cluster_id,
                object_id: culling_entry.object_id.clone(),
                resource_slot: *resource_slot,
                page_hash,
                raster_path,
                lod_bucket: culling_entry.lod_bucket,
                lod_error: culling_entry.lod_error,
                bounding_sphere: culling_entry.bounding_sphere,
                cone_axis: culling_entry.cone_axis,
                cone_cutoff: culling_entry.cone_cutoff,
                visibility_word,
                indirect_draw_args: culling_entry.indirect_draw_args,
                source_culling_proof_hash: culling_entry.proof_hash.clone(),
                entry_hash,
            });
        }
    }
    let visibility_buffer_hash = meshlet_visibility_buffer_hash(&entries);
    let lod_error_buffer_hash = meshlet_lod_error_buffer_hash(&entries);
    let cluster_page_table_hash = meshlet_cluster_page_table_hash(&entries);
    let indirect_draw_packet_hash = meshlet_indirect_draw_packet_hash(&entries);
    let max_lod_bucket = entries.iter().map(|entry| entry.lod_bucket).max().unwrap_or_default();
    let hardware_raster_candidate_count = entries
        .iter()
        .filter(|entry| entry.raster_path == "mesh_shader_or_hardware_raster_candidate")
        .count();
    let software_raster_candidate_count =
        entries.len().saturating_sub(hardware_raster_candidate_count);
    let packet_hash = meshlet_visibility_packet_hash(
        prepared,
        scene_graph_submission,
        culling_manifest,
        render_graph_compilation,
        &visibility_buffer_hash,
        &lod_error_buffer_hash,
        &cluster_page_table_hash,
        &indirect_draw_packet_hash,
        &entries,
    );
    BangerNativeMeshletVisibilityPacket {
        schema: "forge.banger.meshlet_visibility_packet.v1",
        authority: "monster_kasm_scene_graph_to_banger_meshlet_visibility",
        clean_room_basis: "local_unreal_sparse_nanite_study_visibility_raster_context_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        scene_graph_hash: scene_graph_submission.submission_hash.clone(),
        culling_manifest_hash: culling_manifest.manifest_hash.clone(),
        render_graph_manifest_hash: render_graph_compilation.graph_hash.clone(),
        cluster_count: entries.len(),
        visible_cluster_count: entries.len(),
        hardware_raster_candidate_count,
        software_raster_candidate_count,
        max_lod_bucket,
        indirect_draw_word_count: entries.len() * 5,
        visibility_buffer_hash,
        lod_error_buffer_hash,
        cluster_page_table_hash,
        indirect_draw_packet_hash,
        packet_hash,
        entries,
    }
}

fn build_nanite_second_layer_packet(
    prepared: &MonsterPreparedCompute,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    meshlet_visibility_packet: &BangerNativeMeshletVisibilityPacket,
    resource_table: &BangerNativeResourceTable,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
) -> BangerNativeNaniteSecondLayerPacket {
    let entries = meshlet_visibility_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, visibility)| {
            let primitive = gpu_scene_packet
                .primitives
                .iter()
                .find(|primitive| primitive.object_id == visibility.object_id);
            let primitive_id = primitive.map(|primitive| primitive.primitive_id).unwrap_or_default();
            let material_flags_word =
                nanite_material_flags_word(visibility, primitive, shader_compiler_ticket);
            let material_bin_id =
                nanite_material_bin_id(visibility, primitive, material_flags_word, index as u32);
            let residency_state = nanite_residency_state(visibility, resource_table);
            let feedback_word =
                nanite_streaming_feedback_word(visibility, primitive_id, material_bin_id, residency_state);
            let visibility_tile = nanite_visibility_resolve_tile(visibility, index as u32);
            let ray_tracing_proxy_hash =
                nanite_ray_tracing_proxy_hash(visibility, primitive, material_bin_id, &visibility_tile);
            let entry_hash = nanite_second_layer_entry_hash(
                visibility,
                primitive_id,
                residency_state,
                feedback_word,
                material_bin_id,
                material_flags_word,
                &visibility_tile,
                &ray_tracing_proxy_hash,
            );
            BangerNativeNaniteSecondLayerEntry {
                cluster_id: visibility.cluster_id.clone(),
                object_id: visibility.object_id.clone(),
                primitive_id,
                resource_slot: visibility.resource_slot,
                page_hash: visibility.page_hash.clone(),
                residency_state,
                requested_lod_bucket: visibility.lod_bucket,
                feedback_word,
                material_bin_id,
                material_flags_word,
                visibility_tile,
                ray_tracing_proxy_hash,
                entry_hash,
            }
        })
        .collect::<Vec<_>>();
    let streaming_feedback_hash = nanite_streaming_feedback_hash(&entries);
    let page_residency_hash = nanite_page_residency_hash(&entries);
    let material_bin_hash = nanite_material_bin_hash(&entries);
    let visibility_resolve_hash = nanite_visibility_resolve_hash(&entries);
    let ray_tracing_bridge_hash = nanite_ray_tracing_bridge_hash(&entries);
    let packet_hash = nanite_second_layer_packet_hash(
        prepared,
        gpu_scene_packet,
        meshlet_visibility_packet,
        resource_table,
        shader_compiler_ticket,
        &streaming_feedback_hash,
        &page_residency_hash,
        &material_bin_hash,
        &visibility_resolve_hash,
        &ray_tracing_bridge_hash,
        &entries,
    );
    BangerNativeNaniteSecondLayerPacket {
        schema: "forge.banger.nanite_second_layer_packet.v1",
        authority: "banger_meshlet_visibility_to_nanite_streaming_shading_resolve",
        clean_room_basis: "local_unreal_sparse_nanite_feedback_residency_shading_resolve_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        gpu_scene_hash: gpu_scene_packet.packet_hash.clone(),
        visibility_packet_hash: meshlet_visibility_packet.packet_hash.clone(),
        resource_table_hash: resource_table.table_hash.clone(),
        material_abi_hash: shader_compiler_ticket.material_abi_hash.clone(),
        streaming_request_count: entries
            .iter()
            .filter(|entry| entry.residency_state != "resident_page")
            .count(),
        resident_page_count: entries
            .iter()
            .filter(|entry| entry.residency_state == "resident_page")
            .count(),
        feedback_word_count: entries.len(),
        shading_bin_count: entries
            .iter()
            .map(|entry| entry.material_bin_id)
            .collect::<BTreeSet<_>>()
            .len(),
        visibility_resolve_tile_count: entries.len(),
        ray_tracing_proxy_count: entries.len(),
        streaming_feedback_hash,
        page_residency_hash,
        material_bin_hash,
        visibility_resolve_hash,
        ray_tracing_bridge_hash,
        packet_hash,
        entries,
    }
}

fn build_raster_work_queue(
    prepared: &MonsterPreparedCompute,
    meshlet_visibility_packet: &BangerNativeMeshletVisibilityPacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    frame_graph_bindings: &[BangerNativeFrameGraphBinding],
    resource_table: &BangerNativeResourceTable,
) -> BangerNativeRasterWorkQueue {
    let fallback_binding = frame_graph_bindings.first();
    let visibility_binding = frame_graph_bindings
        .iter()
        .find(|binding| binding.stage == "visibility_cull")
        .or(fallback_binding);
    let visibility_pass = render_graph_compilation
        .compiled_passes
        .iter()
        .find(|pass| pass.stage == "visibility_cull")
        .or_else(|| render_graph_compilation.compiled_passes.first());
    let jobs = meshlet_visibility_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let queue_lane = raster_work_queue_lane(entry.raster_path);
            let threadgroup_count = raster_work_threadgroup_count(entry, queue_lane);
            let binding = visibility_binding;
            let pass_name = visibility_pass
                .map(|pass| pass.pass_name)
                .or_else(|| binding.map(|binding| binding.pass_name))
                .unwrap_or("meshlet_visibility");
            let pipeline_cache_key = binding
                .map(|binding| binding.pipeline_cache_key.clone())
                .unwrap_or_else(|| render_graph_compilation.graph_hash.clone());
            let read_barrier = binding
                .map(|binding| binding.read_barrier)
                .unwrap_or("storage_read_indirect_draw");
            let write_barrier = binding
                .map(|binding| binding.write_barrier)
                .unwrap_or("visibility_indirect_write");
            let slot_hash = resource_table
                .slots
                .iter()
                .find(|slot| slot.slot == entry.resource_slot)
                .map(|slot| slot.resource_key.as_str())
                .unwrap_or(entry.page_hash.as_str());
            let bind_group_hash = raster_work_bind_group_hash(
                entry,
                &pipeline_cache_key,
                slot_hash,
                read_barrier,
                write_barrier,
            );
            let job_id = format!("raster_job_{index:04}_{}", entry.cluster_id);
            let job_hash = raster_work_item_hash(
                &job_id,
                entry,
                queue_lane,
                pass_name,
                &pipeline_cache_key,
                threadgroup_count,
                read_barrier,
                write_barrier,
                &bind_group_hash,
            );
            BangerNativeRasterWorkItem {
                job_id,
                cluster_id: entry.cluster_id.clone(),
                object_id: entry.object_id.clone(),
                queue_lane,
                pass_name,
                pipeline_cache_key,
                resource_slot: entry.resource_slot,
                page_hash: entry.page_hash.clone(),
                visibility_word: entry.visibility_word,
                threadgroup_count,
                indirect_draw_args: entry.indirect_draw_args,
                read_barrier,
                write_barrier,
                bind_group_hash,
                job_hash,
            }
        })
        .collect::<Vec<_>>();
    let hardware_job_count = jobs
        .iter()
        .filter(|job| job.queue_lane == "graphics_mesh_shader")
        .count();
    let compute_job_count = jobs.len().saturating_sub(hardware_job_count);
    let total_threadgroup_count = jobs
        .iter()
        .map(|job| job.threadgroup_count)
        .fold(0u32, u32::saturating_add);
    let total_index_count = jobs
        .iter()
        .map(|job| job.indirect_draw_args[0].saturating_mul(job.indirect_draw_args[1].max(1)))
        .fold(0u32, u32::saturating_add);
    let queue_barrier_hash = raster_work_queue_barrier_hash(&jobs);
    let bind_table_hash = raster_work_bind_table_hash(&jobs);
    let dispatch_plan_hash = raster_work_dispatch_plan_hash(&jobs);
    let queue_hash = raster_work_queue_hash(
        prepared,
        meshlet_visibility_packet,
        render_graph_compilation,
        resource_table,
        &queue_barrier_hash,
        &bind_table_hash,
        &dispatch_plan_hash,
        &jobs,
    );
    BangerNativeRasterWorkQueue {
        schema: "forge.banger.native_raster_work_queue.v1",
        authority: "monster_kasm_meshlet_visibility_to_banger_raster_queue",
        clean_room_basis: "local_unreal_sparse_nanite_visibility_raster_queue_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        visibility_packet_hash: meshlet_visibility_packet.packet_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        resource_table_hash: resource_table.table_hash.clone(),
        hardware_job_count,
        compute_job_count,
        total_threadgroup_count,
        total_index_count,
        queue_barrier_hash,
        bind_table_hash,
        dispatch_plan_hash,
        queue_hash,
        jobs,
    }
}

fn build_radiance_schedule_manifest(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    resource_table: &BangerNativeResourceTable,
) -> BangerNativeRadianceScheduleManifest {
    let temporal_epoch = temporal_epoch_from_hash(&prepared.manifest_hash);
    let light_budget = radiance_light_budget(scene_graph_submission, culling_manifest);
    let temporal_reuse_frames = if prepared.is_fully_cached() { 4 } else { 1 };
    let entries = resource_table
        .slots
        .iter()
        .filter(|slot| slot.kind == "surfel_radiance_cache")
        .map(|slot| {
            radiance_probe_page_from_slot(
                prepared,
                slot,
                scene_graph_submission,
                culling_manifest,
                temporal_epoch,
                light_budget,
                temporal_reuse_frames,
            )
        })
        .collect::<Vec<_>>();
    let active_probe_count = entries
        .iter()
        .map(|entry| entry.probe_count as u64)
        .sum::<u64>();
    let invalidation_hash =
        radiance_manifest_invalidation_hash(scene_graph_submission, culling_manifest, &entries);
    let schedule_hash = radiance_schedule_hash(
        prepared,
        scene_graph_submission,
        culling_manifest,
        temporal_epoch,
        light_budget,
        &invalidation_hash,
        &entries,
    );
    BangerNativeRadianceScheduleManifest {
        schema: "forge.banger.native_radiance_schedule_manifest.v1",
        authority: "surfel_radiance_cache_probe_pages_temporal_light_budget_async_compute",
        temporal_epoch,
        probe_page_count: entries.len(),
        active_probe_count,
        light_budget,
        async_compute_residency_policy: "async_compute_lighting_stream_temporal_reuse",
        invalidation_hash,
        schedule_hash,
        entries,
    }
}

fn radiance_probe_page_from_slot(
    prepared: &MonsterPreparedCompute,
    slot: &BangerNativeResourceSlot,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    temporal_epoch: u64,
    light_budget: u32,
    temporal_reuse_frames: u32,
) -> BangerNativeRadianceProbePage {
    let probe_count = ((slot.byte_len / 64).max(1).min(u32::MAX as u64)) as u32;
    let update_priority = radiance_update_priority(slot, culling_manifest);
    let invalidation_hash = radiance_probe_invalidation_hash(
        prepared,
        slot,
        scene_graph_submission,
        culling_manifest,
        temporal_epoch,
        light_budget,
    );
    let proof_hash = radiance_probe_page_proof_hash(
        slot,
        probe_count,
        temporal_reuse_frames,
        light_budget,
        update_priority,
        &invalidation_hash,
    );
    BangerNativeRadianceProbePage {
        probe_page_id: format!("radiance:{}:{}", slot.slot, slot.page_index),
        source_slot: slot.slot,
        source_page_hash: slot.page_hash.clone(),
        probe_count,
        temporal_reuse_frames,
        light_budget,
        update_priority,
        async_compute: true,
        residency_policy: "async_compute_lighting_stream",
        invalidation_hash,
        proof_hash,
    }
}

fn build_lumen_lighting_packet(
    prepared: &MonsterPreparedCompute,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativeLumenLightingPacket {
    let entries = nanite_second_layer_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, nanite_entry)| {
            let probe_page = if radiance_schedule_manifest.entries.is_empty() {
                None
            } else {
                radiance_schedule_manifest
                    .entries
                    .get(index % radiance_schedule_manifest.entries.len())
            };
            let source_probe_page_id = probe_page
                .map(|page| page.probe_page_id.clone())
                .unwrap_or_else(|| "radiance:none".to_string());
            let trace_policy = lumen_trace_policy(nanite_entry);
            let screen_probe_coord = lumen_screen_probe_coord(nanite_entry, index as u32);
            let radiance_tile = lumen_radiance_tile(nanite_entry, probe_page, index as u32);
            let diffuse_ray_count = lumen_diffuse_ray_count(
                nanite_entry,
                probe_page,
                radiance_schedule_manifest.light_budget,
            );
            let reflection_ray_count = lumen_reflection_ray_count(nanite_entry, trace_policy);
            let temporal_reuse_frames = probe_page
                .map(|page| page.temporal_reuse_frames)
                .unwrap_or(1);
            let surface_page_id = format!(
                "surface:{}:{}:{}",
                nanite_entry.resource_slot, nanite_entry.requested_lod_bucket, index
            );
            let surface_cache_hash = lumen_surface_cache_entry_hash(
                nanite_entry,
                &surface_page_id,
                &source_probe_page_id,
                &radiance_tile,
            );
            let screen_probe_hash =
                lumen_screen_probe_entry_hash(nanite_entry, &screen_probe_coord, diffuse_ray_count);
            let trace_hash = lumen_trace_entry_hash(
                nanite_entry,
                trace_policy,
                diffuse_ray_count,
                reflection_ray_count,
                &surface_cache_hash,
                &screen_probe_hash,
            );
            let entry_hash = lumen_lighting_entry_hash(
                nanite_entry,
                &surface_page_id,
                &source_probe_page_id,
                trace_policy,
                diffuse_ray_count,
                reflection_ray_count,
                temporal_reuse_frames,
                &surface_cache_hash,
                &screen_probe_hash,
                &trace_hash,
            );
            BangerNativeLumenLightingEntry {
                object_id: nanite_entry.object_id.clone(),
                cluster_id: nanite_entry.cluster_id.clone(),
                surface_page_id,
                source_probe_page_id,
                material_bin_id: nanite_entry.material_bin_id,
                residency_state: nanite_entry.residency_state,
                screen_probe_coord,
                radiance_tile,
                trace_policy,
                diffuse_ray_count,
                reflection_ray_count,
                temporal_reuse_frames,
                surface_cache_hash,
                screen_probe_hash,
                trace_hash,
                entry_hash,
            }
        })
        .collect::<Vec<_>>();
    let surface_cache_hash = lumen_surface_cache_hash(&entries);
    let screen_probe_hash = lumen_screen_probe_hash(&entries);
    let trace_policy_hash = lumen_trace_policy_hash(&entries);
    let diffuse_indirect_hash = lumen_diffuse_indirect_hash(&entries);
    let reflection_hash = lumen_reflection_hash(&entries);
    let packet_hash = lumen_lighting_packet_hash(
        prepared,
        gpu_scene_packet,
        nanite_second_layer_packet,
        radiance_schedule_manifest,
        render_graph_compilation,
        &surface_cache_hash,
        &screen_probe_hash,
        &trace_policy_hash,
        &diffuse_indirect_hash,
        &reflection_hash,
        &entries,
    );
    BangerNativeLumenLightingPacket {
        schema: "forge.banger.lumen_lighting_packet.v1",
        authority: "banger_nanite_surface_cache_screen_probe_radiance_trace",
        clean_room_basis: "local_unreal_sparse_lumen_surface_cache_screen_probe_radiance_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        gpu_scene_hash: gpu_scene_packet.packet_hash.clone(),
        nanite_second_layer_hash: nanite_second_layer_packet.packet_hash.clone(),
        radiance_schedule_hash: radiance_schedule_manifest.schedule_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        surface_cache_page_count: entries.len(),
        screen_probe_count: entries.len(),
        radiance_tile_count: entries.len(),
        hardware_trace_candidate_count: entries
            .iter()
            .filter(|entry| entry.trace_policy == "hardware_ray_traced_surface_cache")
            .count(),
        software_trace_candidate_count: entries
            .iter()
            .filter(|entry| entry.trace_policy != "hardware_ray_traced_surface_cache")
            .count(),
        reflection_ray_budget: entries
            .iter()
            .map(|entry| entry.reflection_ray_count as u64)
            .sum(),
        total_probe_rays: entries
            .iter()
            .map(|entry| entry.diffuse_ray_count as u64 + entry.reflection_ray_count as u64)
            .sum(),
        surface_cache_hash,
        screen_probe_hash,
        trace_policy_hash,
        diffuse_indirect_hash,
        reflection_hash,
        packet_hash,
        entries,
    }
}

fn build_virtual_shadow_packet(
    prepared: &MonsterPreparedCompute,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativeVirtualShadowPacket {
    let entries = lumen_lighting_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, lumen_entry)| {
            let matching_nanite = nanite_second_layer_packet
                .entries
                .iter()
                .find(|entry| entry.cluster_id == lumen_entry.cluster_id);
            let virtual_map_id = virtual_shadow_map_id(lumen_entry, index as u32);
            let clipmap_level = virtual_shadow_clipmap_level(lumen_entry);
            let page_coord = virtual_shadow_page_coord(lumen_entry, virtual_map_id, clipmap_level);
            let resolution = virtual_shadow_resolution(clipmap_level, lumen_entry);
            let cache_state = virtual_shadow_cache_state(lumen_entry, matching_nanite);
            let invalidation_reason = virtual_shadow_invalidation_reason(lumen_entry, cache_state);
            let light_grid_cell = virtual_shadow_light_grid_cell(lumen_entry, index as u32);
            let projection_tile = virtual_shadow_projection_tile(lumen_entry, &page_coord, resolution);
            let ray_budget = virtual_shadow_ray_budget(lumen_entry, radiance_schedule_manifest.light_budget);
            let virtual_light_id = virtual_shadow_light_id(lumen_entry, virtual_map_id);
            let shadow_page_id = format!(
                "vshadow:{}:{}:{}:{}",
                virtual_map_id, clipmap_level, page_coord[0], page_coord[1]
            );
            let page_table_hash = virtual_shadow_page_table_entry_hash(
                lumen_entry,
                &shadow_page_id,
                &virtual_light_id,
                &page_coord,
                resolution,
            );
            let cache_hash = virtual_shadow_cache_entry_hash(
                lumen_entry,
                cache_state,
                invalidation_reason,
                &page_table_hash,
            );
            let projection_hash = virtual_shadow_projection_entry_hash(
                lumen_entry,
                &projection_tile,
                &light_grid_cell,
                ray_budget,
            );
            let entry_hash = virtual_shadow_entry_hash(
                lumen_entry,
                &shadow_page_id,
                &virtual_light_id,
                virtual_map_id,
                clipmap_level,
                &page_coord,
                resolution,
                cache_state,
                invalidation_reason,
                &light_grid_cell,
                &projection_tile,
                ray_budget,
                &page_table_hash,
                &cache_hash,
                &projection_hash,
            );
            BangerNativeVirtualShadowEntry {
                object_id: lumen_entry.object_id.clone(),
                cluster_id: lumen_entry.cluster_id.clone(),
                shadow_page_id,
                source_surface_page_id: lumen_entry.surface_page_id.clone(),
                virtual_light_id,
                virtual_map_id,
                clipmap_level,
                page_coord,
                resolution,
                cache_state,
                invalidation_reason,
                light_grid_cell,
                projection_tile,
                ray_budget,
                page_table_hash,
                cache_hash,
                projection_hash,
                entry_hash,
            }
        })
        .collect::<Vec<_>>();
    let page_table_hash = virtual_shadow_page_table_hash(&entries);
    let cache_hash = virtual_shadow_cache_hash(&entries);
    let invalidation_hash = virtual_shadow_invalidation_hash(&entries);
    let projection_hash = virtual_shadow_projection_hash(&entries);
    let light_grid_hash = virtual_shadow_light_grid_hash(&entries);
    let packet_hash = virtual_shadow_packet_hash(
        prepared,
        nanite_second_layer_packet,
        lumen_lighting_packet,
        radiance_schedule_manifest,
        render_graph_compilation,
        &page_table_hash,
        &cache_hash,
        &invalidation_hash,
        &projection_hash,
        &light_grid_hash,
        &entries,
    );
    BangerNativeVirtualShadowPacket {
        schema: "forge.banger.virtual_shadow_packet.v1",
        authority: "banger_virtual_shadow_page_cache_light_grid_projection",
        clean_room_basis: "local_unreal_sparse_virtual_shadow_map_page_marking_cache_projection_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        nanite_second_layer_hash: nanite_second_layer_packet.packet_hash.clone(),
        lumen_lighting_hash: lumen_lighting_packet.packet_hash.clone(),
        radiance_schedule_hash: radiance_schedule_manifest.schedule_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        virtual_page_count: entries.len(),
        cached_page_count: entries
            .iter()
            .filter(|entry| entry.cache_state != "page_mark_required")
            .count(),
        invalidated_page_count: entries
            .iter()
            .filter(|entry| entry.invalidation_reason != "stable_cache_reuse")
            .count(),
        light_page_count: entries
            .iter()
            .map(|entry| entry.virtual_light_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        shadow_ray_budget: entries.iter().map(|entry| entry.ray_budget as u64).sum(),
        page_table_hash,
        cache_hash,
        invalidation_hash,
        projection_hash,
        light_grid_hash,
        packet_hash,
        entries,
    }
}

fn build_direct_lighting_packet(
    prepared: &MonsterPreparedCompute,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativeDirectLightingPacket {
    let entries = virtual_shadow_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, shadow_entry)| {
            let lumen_entry = lumen_lighting_packet
                .entries
                .iter()
                .find(|entry| entry.cluster_id == shadow_entry.cluster_id);
            let light_kind = direct_lighting_kind(shadow_entry, lumen_entry);
            let light_cluster_id = direct_lighting_cluster_id(shadow_entry, light_kind);
            let sample_count = direct_lighting_sample_count(
                shadow_entry,
                lumen_entry,
                radiance_schedule_manifest.light_budget,
            );
            let sample_sequence =
                direct_lighting_sample_sequence(shadow_entry, &light_cluster_id, index as u32);
            let shadow_mask_hash =
                direct_lighting_shadow_mask_hash(shadow_entry, lumen_entry, sample_sequence);
            let contribution_hash = direct_lighting_contribution_hash(
                shadow_entry,
                lumen_entry,
                light_kind,
                sample_count,
                &shadow_mask_hash,
            );
            let denoiser_tile = direct_lighting_denoiser_tile(shadow_entry, sample_count);
            let resolve_tile = direct_lighting_resolve_tile(shadow_entry, &denoiser_tile);
            let ray_tracing_candidate = direct_lighting_ray_tracing_candidate(shadow_entry, lumen_entry);
            let entry_hash = direct_lighting_entry_hash(
                shadow_entry,
                &light_cluster_id,
                light_kind,
                sample_sequence,
                sample_count,
                &shadow_mask_hash,
                &contribution_hash,
                &denoiser_tile,
                &resolve_tile,
                ray_tracing_candidate,
            );
            BangerNativeDirectLightingEntry {
                object_id: shadow_entry.object_id.clone(),
                cluster_id: shadow_entry.cluster_id.clone(),
                light_cluster_id,
                virtual_light_id: shadow_entry.virtual_light_id.clone(),
                light_kind,
                light_grid_cell: shadow_entry.light_grid_cell,
                sample_sequence,
                sample_count,
                shadow_page_id: shadow_entry.shadow_page_id.clone(),
                shadow_mask_hash,
                contribution_hash,
                denoiser_tile,
                resolve_tile,
                ray_tracing_candidate,
                entry_hash,
            }
        })
        .collect::<Vec<_>>();
    let light_grid_hash = direct_lighting_grid_hash(&entries);
    let sample_sequence_hash = direct_lighting_sample_sequence_hash(&entries);
    let shadow_mask_hash = direct_lighting_shadow_masks_hash(&entries);
    let denoiser_hash = direct_lighting_denoiser_hash(&entries);
    let resolve_hash = direct_lighting_resolve_hash(&entries);
    let packet_hash = direct_lighting_packet_hash(
        prepared,
        lumen_lighting_packet,
        virtual_shadow_packet,
        radiance_schedule_manifest,
        render_graph_compilation,
        &light_grid_hash,
        &sample_sequence_hash,
        &shadow_mask_hash,
        &denoiser_hash,
        &resolve_hash,
        &entries,
    );
    BangerNativeDirectLightingPacket {
        schema: "forge.banger.direct_lighting_packet.v1",
        authority: "banger_megalights_light_grid_stochastic_shadowed_resolve",
        clean_room_basis: "local_unreal_sparse_megalights_stochastic_light_grid_resolve_denoise_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        lumen_lighting_hash: lumen_lighting_packet.packet_hash.clone(),
        virtual_shadow_hash: virtual_shadow_packet.packet_hash.clone(),
        radiance_schedule_hash: radiance_schedule_manifest.schedule_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        light_cluster_count: entries
            .iter()
            .map(|entry| entry.light_cluster_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        stochastic_sample_count: entries.iter().map(|entry| entry.sample_count as u64).sum(),
        shadowed_light_count: entries
            .iter()
            .filter(|entry| entry.shadow_page_id.starts_with("vshadow:"))
            .count(),
        unshadowed_light_count: entries
            .iter()
            .filter(|entry| !entry.shadow_page_id.starts_with("vshadow:"))
            .count(),
        denoiser_tile_count: entries.len(),
        resolve_tile_count: entries.len(),
        hardware_ray_candidate_count: entries
            .iter()
            .filter(|entry| entry.ray_tracing_candidate)
            .count(),
        light_grid_hash,
        sample_sequence_hash,
        shadow_mask_hash,
        denoiser_hash,
        resolve_hash,
        packet_hash,
        entries,
    }
}

fn build_material_closure_packet(
    prepared: &MonsterPreparedCompute,
    material_abi: &BangerNativeShaderMaterialAbi,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativeMaterialClosurePacket {
    let entries = direct_lighting_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, direct_entry)| {
            let lumen_entry = lumen_lighting_packet
                .entries
                .iter()
                .find(|entry| entry.cluster_id == direct_entry.cluster_id);
            let shadow_entry = virtual_shadow_packet
                .entries
                .iter()
                .find(|entry| entry.cluster_id == direct_entry.cluster_id);
            let material_bin_id = lumen_entry
                .map(|entry| entry.material_bin_id)
                .unwrap_or(index as u32);
            let base_closure = material_base_closure(material_bin_id, direct_entry.light_kind);
            let coating_closure = material_coating_closure(lumen_entry, shadow_entry);
            let layer_count = material_layer_count(base_closure, coating_closure, direct_entry);
            let texture_slot_base = material_texture_slot_base(material_abi, material_bin_id);
            let texture_slot_count = material_texture_slot_count(material_abi, layer_count, direct_entry);
            let roughness_quantized = material_roughness_quantized(lumen_entry, direct_entry);
            let metallic_quantized = material_metallic_quantized(material_bin_id, direct_entry);
            let opacity_quantized = material_opacity_quantized(shadow_entry, direct_entry);
            let closure_stack_id = material_closure_stack_id(
                direct_entry,
                material_bin_id,
                base_closure,
                coating_closure,
                layer_count,
            );
            let surface_cache_hash = lumen_entry
                .map(|entry| entry.surface_cache_hash.clone())
                .unwrap_or_else(|| direct_entry.contribution_hash.clone());
            let closure_hash = material_closure_hash(
                direct_entry,
                &closure_stack_id,
                material_bin_id,
                base_closure,
                coating_closure,
                layer_count,
                roughness_quantized,
                metallic_quantized,
                opacity_quantized,
            );
            let bsdf_hash = material_bsdf_hash(
                &closure_hash,
                direct_entry,
                lumen_entry,
                roughness_quantized,
                metallic_quantized,
            );
            let texture_hash = material_texture_hash(
                &closure_hash,
                material_abi,
                texture_slot_base,
                texture_slot_count,
                &surface_cache_hash,
            );
            let resolve_hash =
                material_resolve_hash(&bsdf_hash, &texture_hash, direct_entry, shadow_entry);
            let entry_hash = material_closure_entry_hash(
                direct_entry,
                &closure_stack_id,
                &closure_hash,
                &bsdf_hash,
                &texture_hash,
                &resolve_hash,
                texture_slot_base,
                texture_slot_count,
            );
            BangerNativeMaterialClosureEntry {
                object_id: direct_entry.object_id.clone(),
                cluster_id: direct_entry.cluster_id.clone(),
                material_bin_id,
                closure_stack_id,
                base_closure,
                coating_closure,
                layer_count,
                texture_slot_base,
                texture_slot_count,
                roughness_quantized,
                metallic_quantized,
                opacity_quantized,
                light_cluster_id: direct_entry.light_cluster_id.clone(),
                surface_cache_hash,
                shadow_mask_hash: direct_entry.shadow_mask_hash.clone(),
                closure_hash,
                bsdf_hash,
                texture_hash,
                resolve_hash,
                entry_hash,
            }
        })
        .collect::<Vec<_>>();
    let closure_stack_hash = material_closure_stack_hash(&entries);
    let bsdf_table_hash = material_bsdf_table_hash(&entries);
    let texture_table_hash = material_texture_table_hash(&entries);
    let resolve_hash = material_closure_resolve_hash(&entries);
    let packet_hash = material_closure_packet_hash(
        prepared,
        material_abi,
        lumen_lighting_packet,
        virtual_shadow_packet,
        direct_lighting_packet,
        render_graph_compilation,
        &closure_stack_hash,
        &bsdf_table_hash,
        &texture_table_hash,
        &resolve_hash,
        &entries,
    );
    BangerNativeMaterialClosurePacket {
        schema: "forge.banger.material_closure_packet.v1",
        authority: "banger_substrate_style_material_closure_stack_texture_table_resolve",
        clean_room_basis: "local_unreal_sparse_substrate_material_closure_layering_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        direct_lighting_hash: direct_lighting_packet.packet_hash.clone(),
        lumen_lighting_hash: lumen_lighting_packet.packet_hash.clone(),
        virtual_shadow_hash: virtual_shadow_packet.packet_hash.clone(),
        shader_material_abi_hash: material_abi.layout_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        closure_count: entries.len(),
        layered_closure_count: entries.iter().filter(|entry| entry.layer_count > 1).count(),
        texture_slot_count: entries.iter().map(|entry| entry.texture_slot_count).sum(),
        hardware_ray_candidate_count: direct_lighting_packet.hardware_ray_candidate_count,
        closure_stack_hash,
        bsdf_table_hash,
        texture_table_hash,
        resolve_hash,
        packet_hash,
        entries,
    }
}

fn build_temporal_history_packet(
    prepared: &MonsterPreparedCompute,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativeTemporalHistoryPacket {
    let temporal_epoch = temporal_epoch_from_hash(&prepared.manifest_hash);
    let entries = material_closure_packet
        .entries
        .iter()
        .enumerate()
        .map(|(index, material_entry)| {
            let direct_entry = direct_lighting_packet
                .entries
                .iter()
                .find(|entry| entry.cluster_id == material_entry.cluster_id);
            let jitter_index = temporal_jitter_index(temporal_epoch, index as u32);
            let temporal_jitter_pixels = temporal_jitter_pixels(jitter_index);
            let history_kind = temporal_history_kind(material_entry, direct_entry);
            let motion_tile = temporal_motion_tile(material_entry, direct_entry, index as u32);
            let history_tile = temporal_history_tile(&motion_tile, temporal_jitter_pixels);
            let velocity_quantized =
                temporal_velocity_quantized(material_entry, direct_entry, temporal_jitter_pixels);
            let disocclusion_score =
                temporal_disocclusion_score(material_entry, direct_entry, &velocity_quantized);
            let rejection_mode = temporal_rejection_mode(disocclusion_score, material_entry, direct_entry);
            let accumulation_weight_q15 =
                temporal_accumulation_weight_q15(rejection_mode, material_entry, direct_entry);
            let history_layer_id = temporal_history_layer_id(
                material_entry,
                history_kind,
                temporal_epoch,
                jitter_index,
            );
            let direct_resolve_hash = direct_entry
                .map(|entry| {
                    hash_text_hex(
                        "forge.banger.temporal_history.direct_resolve_entry.v1",
                        &format!(
                            "{}:{}:{}:{}:{}",
                            entry.entry_hash,
                            entry.contribution_hash,
                            entry.resolve_tile[0],
                            entry.resolve_tile[1],
                            entry.resolve_tile[2]
                        ),
                    )
                })
                .unwrap_or_else(|| material_entry.resolve_hash.clone());
            let motion_vector_hash = temporal_motion_vector_hash(
                material_entry,
                &history_layer_id,
                &motion_tile,
                &velocity_quantized,
                jitter_index,
                temporal_jitter_pixels,
            );
            let history_reprojection_hash = temporal_history_reprojection_hash(
                material_entry,
                &history_layer_id,
                &history_tile,
                &motion_vector_hash,
                &direct_resolve_hash,
            );
            let disocclusion_hash = temporal_disocclusion_hash(
                material_entry,
                &motion_vector_hash,
                disocclusion_score,
                rejection_mode,
            );
            let accumulation_hash = temporal_accumulation_hash(
                material_entry,
                &history_reprojection_hash,
                &disocclusion_hash,
                accumulation_weight_q15,
            );
            let entry_hash = temporal_history_entry_hash(
                material_entry,
                &history_layer_id,
                history_kind,
                temporal_epoch,
                jitter_index,
                &motion_vector_hash,
                &history_reprojection_hash,
                &disocclusion_hash,
                &accumulation_hash,
            );
            BangerNativeTemporalHistoryEntry {
                object_id: material_entry.object_id.clone(),
                cluster_id: material_entry.cluster_id.clone(),
                history_layer_id,
                history_kind,
                temporal_epoch,
                jitter_index,
                temporal_jitter_pixels,
                motion_tile,
                history_tile,
                velocity_quantized,
                disocclusion_score,
                rejection_mode,
                accumulation_weight_q15,
                material_closure_hash: material_entry.closure_hash.clone(),
                direct_resolve_hash,
                motion_vector_hash,
                history_reprojection_hash,
                disocclusion_hash,
                accumulation_hash,
                entry_hash,
            }
        })
        .collect::<Vec<_>>();
    let jitter_sequence_hash = temporal_jitter_sequence_hash(temporal_epoch, &entries);
    let motion_vector_hash = temporal_motion_vector_table_hash(&entries);
    let history_reprojection_hash = temporal_history_reprojection_table_hash(&entries);
    let disocclusion_mask_hash = temporal_disocclusion_mask_hash(&entries);
    let rejection_hash = temporal_rejection_hash(&entries);
    let accumulation_hash = temporal_accumulation_table_hash(&entries);
    let packet_hash = temporal_history_packet_hash(
        prepared,
        direct_lighting_packet,
        material_closure_packet,
        render_graph_compilation,
        temporal_epoch,
        &jitter_sequence_hash,
        &motion_vector_hash,
        &history_reprojection_hash,
        &disocclusion_mask_hash,
        &rejection_hash,
        &accumulation_hash,
        &entries,
    );
    BangerNativeTemporalHistoryPacket {
        schema: "forge.banger.temporal_history_packet.v1",
        authority: "banger_tsr_motion_history_rejection_accumulation",
        clean_room_basis: "local_unreal_sparse_tsr_taa_velocity_history_rejection_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        material_closure_hash: material_closure_packet.packet_hash.clone(),
        direct_lighting_hash: direct_lighting_packet.packet_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        temporal_epoch,
        history_layer_count: entries.len(),
        motion_vector_tile_count: entries.len(),
        disocclusion_tile_count: entries
            .iter()
            .filter(|entry| entry.disocclusion_score > 512)
            .count(),
        rejection_tile_count: entries
            .iter()
            .filter(|entry| entry.rejection_mode != "history_accept")
            .count(),
        resurrection_candidate_count: entries
            .iter()
            .filter(|entry| entry.rejection_mode == "history_resurrection_candidate")
            .count(),
        async_compute_candidate_count: entries
            .iter()
            .filter(|entry| entry.history_kind == "async_tsr_history_update")
            .count(),
        jitter_sequence_hash,
        motion_vector_hash,
        history_reprojection_hash,
        disocclusion_mask_hash,
        rejection_hash,
        accumulation_hash,
        packet_hash,
        entries,
    }
}

fn build_page_residency_allocator_packet(
    prepared: &MonsterPreparedCompute,
    resource_table: &BangerNativeResourceTable,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> BangerNativePageResidencyAllocatorPacket {
    let mut entries = Vec::new();
    for (index, entry) in nanite_second_layer_packet.entries.iter().enumerate() {
        entries.push(page_residency_entry_from_nanite(entry, resource_table, index as u32));
    }
    for (index, entry) in virtual_shadow_packet.entries.iter().enumerate() {
        entries.push(page_residency_entry_from_virtual_shadow(entry, index as u32));
    }
    for (index, entry) in material_closure_packet.entries.iter().enumerate() {
        entries.push(page_residency_entry_from_material(entry, index as u32));
    }
    let physical_pool_hash = page_residency_physical_pool_hash(&entries);
    let virtual_page_table_hash = page_residency_virtual_page_table_hash(&entries);
    let feedback_request_hash = page_residency_feedback_request_hash(&entries);
    let allocation_hash = page_residency_allocation_hash(&entries);
    let eviction_hash = page_residency_eviction_hash(&entries);
    let packet_hash = page_residency_allocator_packet_hash(
        prepared,
        resource_table,
        nanite_second_layer_packet,
        virtual_shadow_packet,
        material_closure_packet,
        render_graph_compilation,
        &physical_pool_hash,
        &virtual_page_table_hash,
        &feedback_request_hash,
        &allocation_hash,
        &eviction_hash,
        &entries,
    );
    BangerNativePageResidencyAllocatorPacket {
        schema: "forge.banger.page_residency_allocator_packet.v1",
        authority: "banger_virtual_page_feedback_physical_pool_allocator",
        clean_room_basis: "local_unreal_sparse_nanite_vsm_virtual_texture_feedback_physical_page_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        resource_table_hash: resource_table.table_hash.clone(),
        nanite_second_layer_hash: nanite_second_layer_packet.packet_hash.clone(),
        virtual_shadow_hash: virtual_shadow_packet.packet_hash.clone(),
        material_closure_hash: material_closure_packet.packet_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        virtual_page_count: entries.len(),
        physical_page_count: entries
            .iter()
            .map(|entry| (entry.physical_pool, entry.physical_address))
            .collect::<BTreeSet<_>>()
            .len(),
        resident_page_count: entries
            .iter()
            .filter(|entry| entry.residency_state == "resident_page" || entry.lock_state == "locked_for_frame")
            .count(),
        streaming_request_count: entries
            .iter()
            .filter(|entry| entry.residency_state != "resident_page")
            .count(),
        eviction_candidate_count: entries
            .iter()
            .filter(|entry| entry.lock_state == "eviction_candidate")
            .count(),
        locked_page_count: entries
            .iter()
            .filter(|entry| entry.lock_state == "locked_for_frame")
            .count(),
        physical_pool_hash,
        virtual_page_table_hash,
        feedback_request_hash,
        allocation_hash,
        eviction_hash,
        packet_hash,
        entries,
    }
}

fn build_gaussian_splat_layer_manifest(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
) -> BangerNativeGaussianSplatLayerManifest {
    let layers = scene_graph_submission
        .submissions
        .iter()
        .filter(|node| node.visible && node.renderable && node.representation == "gaussian_splat")
        .map(|node| gaussian_splat_layer_from_node(node, culling_manifest))
        .collect::<Vec<_>>();
    let conversions = scene_graph_submission
        .submissions
        .iter()
        .filter(|node| node.visible && node.renderable && gaussian_splat_conversion_candidate(node.representation))
        .map(|node| gaussian_splat_conversion_from_node(node, radiance_schedule_manifest))
        .collect::<Vec<_>>();
    let bucket_count = layers
        .iter()
        .map(|layer| layer.bucket_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let proxy_bounds_hash = gaussian_splat_proxy_bounds_hash(&layers, &conversions);
    let sort_key_hash = gaussian_splat_sort_key_hash(&layers);
    let group_key_hash = gaussian_splat_group_key_hash(&layers);
    let conversion_manifest_hash = gaussian_splat_conversion_manifest_hash(&conversions);
    let manifest_hash = gaussian_splat_layer_manifest_hash(
        prepared,
        scene_graph_submission,
        culling_manifest,
        radiance_schedule_manifest,
        &proxy_bounds_hash,
        &sort_key_hash,
        &group_key_hash,
        &conversion_manifest_hash,
        &layers,
        &conversions,
    );
    BangerNativeGaussianSplatLayerManifest {
        schema: "forge.banger.native_gaussian_splat_layer_manifest.v1",
        authority: "scene_graph_hybrid_gaussian_splat_layers_no_separate_renderer",
        layer_count: layers.len(),
        bucket_count,
        conversion_count: conversions.len(),
        proxy_bounds_hash,
        sort_key_hash,
        group_key_hash,
        conversion_manifest_hash,
        manifest_hash,
        layers,
        conversions,
    }
}

fn gaussian_splat_layer_from_node(
    node: &BangerNativeSceneSubmissionNode,
    culling_manifest: &BangerNativeCullingManifest,
) -> BangerNativeGaussianSplatLayer {
    let lod_bucket = culling_manifest
        .entries
        .iter()
        .find(|entry| entry.object_id == node.object_id)
        .map(|entry| entry.lod_bucket)
        .unwrap_or(0);
    let bucket_id = gaussian_splat_bucket_id(node, lod_bucket);
    let sort_key = gaussian_splat_sort_key(node, lod_bucket);
    let group_key = gaussian_splat_group_key(node);
    let splat_count_estimate = gaussian_splat_count_estimate(node, lod_bucket);
    let opacity_cutoff = gaussian_splat_opacity_cutoff(lod_bucket);
    let proof_hash = gaussian_splat_layer_proof_hash(
        node,
        &bucket_id,
        &sort_key,
        &group_key,
        splat_count_estimate,
        opacity_cutoff,
        lod_bucket,
    );
    BangerNativeGaussianSplatLayer {
        object_id: node.object_id.clone(),
        source_representation: node.representation,
        layer_kind: "native_gaussian_splat_layer",
        bucket_id,
        sort_key,
        group_key,
        proxy_bounds_min: node.world_aabb_min,
        proxy_bounds_max: node.world_aabb_max,
        proxy_bounding_sphere: node.world_bounding_sphere,
        splat_count_estimate,
        opacity_cutoff,
        lod_bucket,
        proof_hash,
    }
}

fn gaussian_splat_conversion_from_node(
    node: &BangerNativeSceneSubmissionNode,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
) -> BangerNativeGaussianSplatConversionManifest {
    let proxy_bounds_hash = gaussian_splat_conversion_proxy_bounds_hash(node);
    let conversion_path = gaussian_splat_conversion_path(node.representation);
    let conversion_hash = gaussian_splat_conversion_hash(
        node,
        conversion_path,
        radiance_schedule_manifest,
        &proxy_bounds_hash,
    );
    BangerNativeGaussianSplatConversionManifest {
        object_id: node.object_id.clone(),
        from_representation: node.representation,
        target_representation: "gaussian_splat_proxy",
        conversion_path,
        enabled: true,
        proxy_bounds_hash,
        source_proof_hash: node.proof_hash.clone(),
        conversion_hash,
    }
}

fn build_pipeline_cache_manifest(
    prepared: &MonsterPreparedCompute,
    artifacts: &[&MonsterNativeTandemArtifact],
    render_graph: &[BangerNativeRenderPass],
    pipeline_cache_keys: &[String],
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    pipeline_cache_dir: Option<&str>,
) -> Result<BangerNativePipelineCacheManifest, String> {
    let adapter_probe = native_gpu_adapter_probe();
    let adapter = adapter_probe.selected.as_ref();
    let selected_adapter_hash = adapter.map(adapter_profile_hash).unwrap_or_else(|| {
        hash_text_hex(
            "forge.banger.pipeline_cache.adapter.v1",
            "adapter_unavailable",
        )
    });
    let selected_adapter_label = adapter
        .map(adapter_profile_label)
        .unwrap_or_else(|| "adapter_unavailable".to_string());
    let backend = adapter
        .map(|adapter| adapter.backend.clone())
        .unwrap_or_else(|| "backend_unavailable".to_string());
    let driver_hash = adapter
        .map(|adapter| hash_text_hex("forge.banger.pipeline_cache.driver.v1", &adapter.driver))
        .unwrap_or_else(|| hash_text_hex("forge.banger.pipeline_cache.driver.v1", "driver_unavailable"));
    let driver_info_hash = adapter
        .map(|adapter| hash_text_hex("forge.banger.pipeline_cache.driver_info.v1", &adapter.driver_info))
        .unwrap_or_else(|| hash_text_hex("forge.banger.pipeline_cache.driver_info.v1", "driver_info_unavailable"));
    let cache_root = pipeline_cache_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("forge-banger-pipeline-cache"));
    let mut entries = Vec::new();
    for (index, artifact) in artifacts.iter().enumerate() {
        let pass = &render_graph[index];
        let pipeline_cache_key = pipeline_cache_keys
            .get(index)
            .cloned()
            .unwrap_or_else(|| artifact.renderer_cache_hash.clone());
        entries.push(build_pipeline_cache_entry(
            prepared,
            artifact,
            pass,
            &pipeline_cache_key,
            shader_compiler_ticket,
            &cache_root,
            &selected_adapter_hash,
            &driver_hash,
        )?);
    }
    let persisted_entry_count = entries
        .iter()
        .filter(|entry| entry.persistence_status == "seed_blob_persisted")
        .count();
    let manifest_hash = pipeline_cache_manifest_hash(
        prepared,
        &entries,
        &selected_adapter_hash,
        &driver_hash,
        &driver_info_hash,
    );
    let promotion_status = if persisted_entry_count == entries.len() {
        "seed_blobs_persisted_driver_blob_api_deferred"
    } else {
        "manifest_ready_blob_persistence_partial"
    };
    Ok(BangerNativePipelineCacheManifest {
        schema: "forge.banger.native_pipeline_cache_manifest.v1",
        cache_root: cache_root.to_string_lossy().to_string(),
        selected_adapter_hash,
        selected_adapter_label,
        backend,
        driver_hash,
        driver_info_hash,
        entry_count: entries.len(),
        persisted_entry_count,
        manifest_hash,
        promotion_status,
        entries,
    })
}

fn build_pipeline_cache_entry(
    prepared: &MonsterPreparedCompute,
    artifact: &MonsterNativeTandemArtifact,
    pass: &BangerNativeRenderPass,
    pipeline_cache_key: &str,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    cache_root: &Path,
    adapter_hash: &str,
    driver_hash: &str,
) -> Result<BangerNativePipelineCacheEntry, String> {
    let shader_source_hash = shader_compiler_ticket.mini_probe_source_hash.clone();
    let shader_reflection_hash = shader_reflection_hash(shader_compiler_ticket, artifact, pass);
    let shader_target_manifest_hash = shader_compiler_ticket.target_manifest_hash.clone();
    let material_abi_hash = shader_compiler_ticket.material_abi_hash.clone();
    let render_pass_abi_hash = render_pass_abi_hash(artifact, pass);
    let blob_bytes = pipeline_cache_seed_blob_bytes(
        prepared,
        artifact,
        pass,
        pipeline_cache_key,
        shader_compiler_ticket,
        adapter_hash,
        driver_hash,
        &shader_reflection_hash,
        &shader_target_manifest_hash,
        &material_abi_hash,
        &render_pass_abi_hash,
    );
    let blob_hash = hex32(Sha256::digest(&blob_bytes).into());
    let blob_path = cache_root.join(format!("{}.bpcache", &blob_hash[..32]));
    let persistence_status = persist_pipeline_cache_seed_blob(&blob_path, &blob_bytes)?;
    let proof_hash = pipeline_cache_entry_proof_hash(
        prepared,
        artifact,
        pass,
        pipeline_cache_key,
        adapter_hash,
        driver_hash,
        &shader_source_hash,
        &shader_reflection_hash,
        &shader_target_manifest_hash,
        &material_abi_hash,
        &render_pass_abi_hash,
        &blob_hash,
    );
    Ok(BangerNativePipelineCacheEntry {
        schema: "forge.banger.native_pipeline_cache_entry.v1",
        pass_name: pass.name,
        artifact_name: artifact.name.clone(),
        artifact_kind: artifact.kind,
        pipeline_cache_key: pipeline_cache_key.to_string(),
        adapter_hash: adapter_hash.to_string(),
        driver_hash: driver_hash.to_string(),
        shader_source_hash,
        shader_reflection_hash,
        shader_target_manifest_hash,
        material_abi_hash,
        render_pass_abi_hash,
        renderer_variant_hash: artifact.renderer_variant_hash.clone(),
        blob_hash,
        blob_len: blob_bytes.len() as u64,
        blob_path: blob_path.to_string_lossy().to_string(),
        persistence_status,
        proof_hash,
    })
}

fn build_benchmark_promotion_manifest(
    prepared: &MonsterPreparedCompute,
    render_graph: &[BangerNativeRenderPass],
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    resource_table: &BangerNativeResourceTable,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    target_frame_ms: f32,
) -> BangerNativeBenchmarkPromotionManifest {
    let estimated_frame_ms = estimated_banger_frame_ms(
        prepared,
        render_graph,
        pipeline_cache_manifest,
        resource_table,
        scene_graph_submission,
        culling_manifest,
        radiance_schedule_manifest,
        gaussian_splat_layer_manifest,
    );
    let frame_time_headroom_pct = if target_frame_ms > f32::EPSILON {
        ((target_frame_ms - estimated_frame_ms) / target_frame_ms * 100.0).clamp(-999.0, 999.0)
    } else {
        -999.0
    };
    let vram_pressure_pct = resource_table.budget_pressure_pct;
    let cache_reuse_ratio = if pipeline_cache_manifest.entry_count == 0 {
        1.0
    } else {
        pipeline_cache_manifest.persisted_entry_count as f32 / pipeline_cache_manifest.entry_count as f32
    };
    let proof_reproducibility_hash = benchmark_proof_reproducibility_hash(
        prepared,
        pipeline_cache_manifest,
        texture_bridge_contract,
        resource_table,
        scene_graph_submission,
        culling_manifest,
        radiance_schedule_manifest,
        gaussian_splat_layer_manifest,
        shader_compiler_ticket,
    );
    let proof_reproducibility_check = benchmark_proof_reproducibility_hash(
        prepared,
        pipeline_cache_manifest,
        texture_bridge_contract,
        resource_table,
        scene_graph_submission,
        culling_manifest,
        radiance_schedule_manifest,
        gaussian_splat_layer_manifest,
        shader_compiler_ticket,
    );
    let proof_reproducibility_passed = proof_reproducibility_hash == proof_reproducibility_check;
    let proof_reproducibility_status = if proof_reproducibility_passed {
        "stable_content_addressed_inputs"
    } else {
        "unstable_recomputed_hash_inputs"
    };
    let visual_capability_score = visual_capability_score(
        texture_bridge_contract,
        scene_graph_submission,
        gpu_scene_packet,
        culling_manifest,
        radiance_schedule_manifest,
        gaussian_splat_layer_manifest,
        shader_compiler_ticket,
    );
    let visual_capability_hash = visual_capability_hash(
        texture_bridge_contract,
        scene_graph_submission,
        culling_manifest,
        radiance_schedule_manifest,
        gaussian_splat_layer_manifest,
        shader_compiler_ticket,
        visual_capability_score,
    );

    let gates = vec![
        benchmark_gate("latency", "estimated_frame_ms", target_frame_ms, estimated_frame_ms, true),
        benchmark_gate("vram_pressure", "vram_pressure_pct", 85.0, vram_pressure_pct, true),
        benchmark_gate("cache_hit_reuse", "pipeline_cache_reuse_ratio", 0.95, cache_reuse_ratio, false),
        benchmark_gate(
            "proof_reproducibility",
            "stable_recomputed_hash",
            1.0,
            if proof_reproducibility_passed { 1.0 } else { 0.0 },
            false,
        ),
        benchmark_gate(
            "visual_capability",
            "visual_capability_score",
            80.0,
            visual_capability_score as f32,
            false,
        ),
    ];
    let passed_gate_count = gates.iter().filter(|gate| gate.passed).count();
    let promotion_allowed = passed_gate_count == gates.len();
    let benchmark_hash = benchmark_promotion_manifest_hash(
        target_frame_ms,
        estimated_frame_ms,
        frame_time_headroom_pct,
        vram_pressure_pct,
        cache_reuse_ratio,
        proof_reproducibility_status,
        &proof_reproducibility_hash,
        visual_capability_score,
        &visual_capability_hash,
        &gates,
    );

    BangerNativeBenchmarkPromotionManifest {
        schema: "forge.banger.benchmark_promotion_manifest.v1",
        authority: "deterministic_native_handoff_metrics_before_gpu_promotion",
        gate_count: gates.len(),
        passed_gate_count,
        promotion_allowed,
        target_frame_ms,
        estimated_frame_ms,
        frame_time_headroom_pct,
        vram_pressure_pct,
        cache_reuse_ratio,
        proof_reproducibility_status,
        proof_reproducibility_hash,
        visual_capability_score,
        visual_capability_hash,
        benchmark_hash,
        gates,
    }
}

fn build_texture_bridge_contract(
    prepared: &MonsterPreparedCompute,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    resource_table: &BangerNativeResourceTable,
    editable_scene_manifest: &BangerEditableSceneManifest,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    target_frame_ms: f32,
    viewport_width: Option<u32>,
    viewport_height: Option<u32>,
) -> BangerNativeTextureBridgeContract {
    let width = clamp_viewport_extent(viewport_width, 1280);
    let height = clamp_viewport_extent(viewport_height, 720);
    let same_device_queue_available =
        texture_bridge_backend_can_share(&pipeline_cache_manifest.backend)
            && pipeline_cache_manifest.selected_adapter_label != "adapter_unavailable";
    let import_route = if same_device_queue_available {
        "same_device_queue_external_texture_import_candidate"
    } else {
        "native_external_texture_import_unavailable"
    };
    let route_status = if same_device_queue_available {
        "same_device_queue_candidate_fallback_verified"
    } else {
        "fallback_only_adapter_unavailable"
    };
    let fallback_route = "cpu_readback_rgba8_copy_src_to_host_texture";
    let device_queue_hash = texture_bridge_device_queue_hash(pipeline_cache_manifest);
    let resize_proof_hash = texture_bridge_resize_proof_hash(
        width,
        height,
        &scene_graph_submission.viewport_fit_hash,
        resource_table,
    );
    let camera_control_proof_hash = texture_bridge_camera_control_proof_hash(
        editable_scene_manifest,
        "orbit_pan_zoom",
        "scene_bounds_fit",
    );
    let viewport_contract_hash = texture_bridge_viewport_contract_hash(
        width,
        height,
        target_frame_ms,
        &resize_proof_hash,
        &camera_control_proof_hash,
        editable_scene_manifest,
        scene_graph_submission,
    );
    let frame_hash = texture_bridge_frame_hash(
        prepared,
        pipeline_cache_manifest,
        resource_table,
        editable_scene_manifest,
        scene_graph_submission,
        width,
        height,
        &device_queue_hash,
        &viewport_contract_hash,
    );
    let bridge_proof_hash = texture_bridge_proof_hash(
        pipeline_cache_manifest,
        import_route,
        fallback_route,
        route_status,
        &frame_hash,
        &viewport_contract_hash,
        &resize_proof_hash,
        &camera_control_proof_hash,
        &device_queue_hash,
    );
    BangerNativeTextureBridgeContract {
        schema: "forge.banger.native_texture_bridge_contract.v1",
        route_status,
        import_route,
        fallback_route,
        backend: pipeline_cache_manifest.backend.clone(),
        selected_adapter_hash: pipeline_cache_manifest.selected_adapter_hash.clone(),
        device_queue_hash,
        width,
        height,
        pixel_format: "rgba8unorm_srgb",
        texture_usage: vec!["RENDER_ATTACHMENT", "TEXTURE_BINDING", "COPY_SRC"],
        present_policy: "native_child_surface_frame_hash_before_present",
        same_device_queue_available,
        external_handle_import_required: same_device_queue_available,
        frame_hash,
        viewport_contract_hash,
        resize_proof_hash,
        camera_control_proof_hash,
        bridge_proof_hash,
        viewport: BangerNativeViewportContract {
            fit_mode: "scene_bounds_fit",
            camera_mode: "orbit_pan_zoom",
            orbit_enabled: true,
            pan_enabled: true,
            zoom_enabled: true,
            resize_policy: "recreate_or_rebind_render_target_on_extent_change",
            scene_graph_hash: editable_scene_manifest.graph_hash.clone(),
            scene_bounds_hash: editable_scene_manifest.bounds_hash.clone(),
            viewport_fit_hash: scene_graph_submission.viewport_fit_hash.clone(),
            target_frame_ms,
            min_extent: 1,
            max_extent: 16_384,
        },
    }
}

fn build_frame_submission_packet(
    prepared: &MonsterPreparedCompute,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    raster_work_queue: &BangerNativeRasterWorkQueue,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    page_residency_allocator: &BangerNativePageResidencyAllocatorPacket,
    temporal_history_packet: &BangerNativeTemporalHistoryPacket,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
) -> BangerNativeFrameSubmissionPacket {
    let color_target_hash = frame_submission_target_hash(
        "color",
        texture_bridge_contract.width,
        texture_bridge_contract.height,
        texture_bridge_contract.pixel_format,
        &texture_bridge_contract.frame_hash,
    );
    let depth_target_hash = frame_submission_target_hash(
        "depth",
        texture_bridge_contract.width,
        texture_bridge_contract.height,
        "depth32float",
        &texture_bridge_contract.viewport_contract_hash,
    );
    let render_target_state_hash = frame_submission_render_target_state_hash(
        texture_bridge_contract,
        &color_target_hash,
        &depth_target_hash,
    );
    let commands = render_graph_compilation
        .compiled_passes
        .iter()
        .map(|pass| {
            let pass_jobs = raster_work_queue
                .jobs
                .iter()
                .filter(|job| job.pass_name == pass.pass_name)
                .collect::<Vec<_>>();
            let queue_lane = frame_submission_queue_lane(pass, &pass_jobs);
            let first_raster_job_hash = pass_jobs.first().map(|job| job.job_hash.clone());
            let last_raster_job_hash = pass_jobs.last().map(|job| job.job_hash.clone());
            let input_hash = frame_submission_command_input_hash(
                pass,
                &pass_jobs,
                radiance_schedule_manifest,
                lumen_lighting_packet,
                virtual_shadow_packet,
                direct_lighting_packet,
                material_closure_packet,
                page_residency_allocator,
                temporal_history_packet,
                gaussian_splat_layer_manifest,
            );
            let output_target_hash = frame_submission_command_output_hash(
                pass,
                &color_target_hash,
                &depth_target_hash,
                &render_target_state_hash,
            );
            let barrier_hash =
                frame_submission_command_barrier_hash(pass, queue_lane, raster_work_queue);
            let command_id = format!("frame_cmd_{:03}_{}", pass.order, pass.pass_name);
            let command_hash = frame_submission_command_hash(
                &command_id,
                pass,
                queue_lane,
                pass_jobs.len(),
                &input_hash,
                &output_target_hash,
                &barrier_hash,
            );
            BangerNativeFrameCommandPacket {
                command_id,
                order: pass.order,
                pass_name: pass.pass_name,
                stage: pass.stage,
                queue_lane,
                resource_read_count: pass.reads.len(),
                resource_write_count: pass.writes.len(),
                raster_job_count: pass_jobs.len(),
                first_raster_job_hash,
                last_raster_job_hash,
                input_hash,
                output_target_hash,
                barrier_hash,
                command_hash,
            }
        })
        .collect::<Vec<_>>();
    let command_buffer_hash = frame_submission_command_buffer_hash(&commands);
    let frame_schedule_hash = frame_submission_schedule_hash(render_graph_compilation, &commands);
    let presentable_frame_hash = frame_submission_presentable_frame_hash(
        texture_bridge_contract,
        &render_target_state_hash,
        &command_buffer_hash,
        &frame_schedule_hash,
    );
    let submission_hash = frame_submission_packet_hash(
        prepared,
        texture_bridge_contract,
        render_graph_compilation,
        raster_work_queue,
        lumen_lighting_packet,
        virtual_shadow_packet,
        direct_lighting_packet,
        material_closure_packet,
        page_residency_allocator,
        temporal_history_packet,
        &color_target_hash,
        &depth_target_hash,
        &render_target_state_hash,
        &command_buffer_hash,
        &frame_schedule_hash,
        &presentable_frame_hash,
        &commands,
    );
    let submitted_queue_count = commands
        .iter()
        .map(|command| command.queue_lane)
        .collect::<BTreeSet<_>>()
        .len();
    BangerNativeFrameSubmissionPacket {
        schema: "forge.banger.native_frame_submission_packet.v1",
        authority: "banger_render_graph_raster_queue_to_native_frame_submission",
        clean_room_basis: "local_unreal_sparse_rdg_rhi_submit_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        texture_bridge_hash: texture_bridge_contract.bridge_proof_hash.clone(),
        render_graph_hash: render_graph_compilation.graph_hash.clone(),
        raster_queue_hash: raster_work_queue.queue_hash.clone(),
        color_target_hash,
        depth_target_hash,
        render_target_state_hash,
        command_buffer_hash,
        frame_schedule_hash,
        presentable_frame_hash,
        submission_hash,
        pass_count: render_graph_compilation.pass_count,
        raster_job_count: raster_work_queue.jobs.len(),
        command_count: commands.len(),
        submitted_queue_count,
        commands,
    }
}

fn build_rhi_submit_packet(
    prepared: &MonsterPreparedCompute,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
) -> BangerNativeRhiSubmitPacket {
    let timeline_base_value = rhi_timeline_base_value(frame_submission_packet);
    let acquire_backbuffer_hash = rhi_acquire_backbuffer_hash(
        texture_bridge_contract,
        frame_submission_packet,
        timeline_base_value,
    );
    let mut steps = Vec::new();
    let acquire_wait_hash = rhi_wait_hash("external_present_wait", &texture_bridge_contract.frame_hash, 0);
    let acquire_signal_hash =
        rhi_signal_hash("acquire_backbuffer", &acquire_backbuffer_hash, timeline_base_value);
    let acquire_step_hash = rhi_submit_step_hash(
        "rhi_step_000_acquire_backbuffer",
        0,
        "acquire_backbuffer",
        "present",
        None,
        &acquire_wait_hash,
        &acquire_signal_hash,
        timeline_base_value,
    );
    steps.push(BangerNativeRhiSubmitStep {
        step_id: "rhi_step_000_acquire_backbuffer".to_string(),
        order: 0,
        phase: "acquire_backbuffer",
        queue_lane: "present",
        command_hash: None,
        wait_hash: acquire_wait_hash,
        signal_hash: acquire_signal_hash,
        timeline_value: timeline_base_value,
        step_hash: acquire_step_hash,
    });
    for command in &frame_submission_packet.commands {
        let timeline_value = timeline_base_value + command.order as u64 + 1;
        let step_id = format!("rhi_step_{:03}_finalize_{}", command.order + 1, command.pass_name);
        let wait_hash = rhi_wait_hash(
            command.queue_lane,
            steps
                .last()
                .map(|step| step.signal_hash.as_str())
                .unwrap_or(&acquire_backbuffer_hash),
            timeline_value.saturating_sub(1),
        );
        let signal_hash = rhi_signal_hash(command.queue_lane, &command.command_hash, timeline_value);
        let step_hash = rhi_submit_step_hash(
            &step_id,
            command.order + 1,
            "finalize_command_list",
            command.queue_lane,
            Some(&command.command_hash),
            &wait_hash,
            &signal_hash,
            timeline_value,
        );
        steps.push(BangerNativeRhiSubmitStep {
            step_id,
            order: command.order + 1,
            phase: "finalize_command_list",
            queue_lane: command.queue_lane,
            command_hash: Some(command.command_hash.clone()),
            wait_hash,
            signal_hash,
            timeline_value,
            step_hash,
        });
    }
    let finalized_command_lists_hash = rhi_finalized_command_lists_hash(&steps);
    let submit_timeline = timeline_base_value + frame_submission_packet.command_count as u64 + 1;
    let submit_batch_hash = rhi_submit_batch_hash(
        frame_submission_packet,
        &finalized_command_lists_hash,
        submit_timeline,
    );
    let submit_wait_hash = rhi_wait_hash(
        "submit_batch",
        steps
            .last()
            .map(|step| step.signal_hash.as_str())
            .unwrap_or(&finalized_command_lists_hash),
        submit_timeline.saturating_sub(1),
    );
    let submit_signal_hash = rhi_signal_hash("submit_batch", &submit_batch_hash, submit_timeline);
    let submit_step_hash = rhi_submit_step_hash(
        "rhi_step_submit_batch",
        frame_submission_packet.command_count as u32 + 1,
        "submit_command_lists",
        "rhi_submit",
        None,
        &submit_wait_hash,
        &submit_signal_hash,
        submit_timeline,
    );
    steps.push(BangerNativeRhiSubmitStep {
        step_id: "rhi_step_submit_batch".to_string(),
        order: frame_submission_packet.command_count as u32 + 1,
        phase: "submit_command_lists",
        queue_lane: "rhi_submit",
        command_hash: None,
        wait_hash: submit_wait_hash,
        signal_hash: submit_signal_hash.clone(),
        timeline_value: submit_timeline,
        step_hash: submit_step_hash,
    });
    let present_timeline = submit_timeline + 1;
    let present_hash = rhi_present_hash(
        texture_bridge_contract,
        frame_submission_packet,
        &submit_batch_hash,
        present_timeline,
    );
    let present_wait_hash = rhi_wait_hash("present", &submit_signal_hash, submit_timeline);
    let present_signal_hash = rhi_signal_hash("present", &present_hash, present_timeline);
    let present_step_hash = rhi_submit_step_hash(
        "rhi_step_present",
        frame_submission_packet.command_count as u32 + 2,
        "present",
        "present",
        None,
        &present_wait_hash,
        &present_signal_hash,
        present_timeline,
    );
    steps.push(BangerNativeRhiSubmitStep {
        step_id: "rhi_step_present".to_string(),
        order: frame_submission_packet.command_count as u32 + 2,
        phase: "present",
        queue_lane: "present",
        command_hash: None,
        wait_hash: present_wait_hash,
        signal_hash: present_signal_hash,
        timeline_value: present_timeline,
        step_hash: present_step_hash,
    });
    let fence_timeline_hash = rhi_fence_timeline_hash(&steps);
    let packet_hash = rhi_submit_packet_hash(
        prepared,
        texture_bridge_contract,
        frame_submission_packet,
        &acquire_backbuffer_hash,
        &finalized_command_lists_hash,
        &submit_batch_hash,
        &present_hash,
        &fence_timeline_hash,
        &steps,
    );
    BangerNativeRhiSubmitPacket {
        schema: "forge.banger.native_rhi_submit_packet.v1",
        authority: "banger_frame_submission_to_native_rhi_submit",
        clean_room_basis: "local_unreal_sparse_dynamic_rhi_finalize_submit_present_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        frame_submission_hash: frame_submission_packet.submission_hash.clone(),
        texture_bridge_hash: texture_bridge_contract.bridge_proof_hash.clone(),
        backend: texture_bridge_contract.backend.clone(),
        selected_adapter_hash: texture_bridge_contract.selected_adapter_hash.clone(),
        command_list_count: frame_submission_packet.command_count,
        submit_batch_count: 1,
        submitted_queue_count: frame_submission_packet.submitted_queue_count,
        timeline_base_value,
        acquire_backbuffer_hash,
        finalized_command_lists_hash,
        submit_batch_hash,
        present_hash,
        fence_timeline_hash,
        packet_hash,
        steps,
    }
}

fn build_gpu_execution_receipt(
    prepared: &MonsterPreparedCompute,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    raster_work_queue: &BangerNativeRasterWorkQueue,
) -> BangerNativeGpuExecutionReceipt {
    let phases = rhi_submit_packet
        .steps
        .iter()
        .map(|step| {
            let diagnostic_hash = gpu_execution_phase_diagnostic_hash(
                step,
                frame_submission_packet,
                rhi_submit_packet,
                raster_work_queue,
            );
            let phase_hash = gpu_execution_phase_hash(step, &diagnostic_hash);
            BangerNativeGpuExecutionPhaseReceipt {
                phase_id: format!("gpu_exec_{}_{}", step.order, step.phase),
                phase: step.phase,
                queue_lane: step.queue_lane,
                source_step_hash: step.step_hash.clone(),
                timeline_value: step.timeline_value,
                completed: true,
                diagnostic_hash,
                phase_hash,
            }
        })
        .collect::<Vec<_>>();
    let nonblank_frame_expected = frame_submission_packet.raster_job_count > 0
        && frame_submission_packet.presentable_frame_hash.len() == 64
        && raster_work_queue.total_index_count > 0
        && rhi_submit_packet.present_hash.len() == 64;
    let completed_phase_count = phases.iter().filter(|phase| phase.completed).count();
    let queue_lane_count = phases
        .iter()
        .map(|phase| phase.queue_lane)
        .collect::<BTreeSet<_>>()
        .len();
    let frame_diagnostic_hash = gpu_execution_frame_diagnostic_hash(
        frame_submission_packet,
        rhi_submit_packet,
        raster_work_queue,
        nonblank_frame_expected,
    );
    let queue_timeline_hash = gpu_execution_queue_timeline_hash(rhi_submit_packet, &phases);
    let readback_policy_hash = gpu_execution_readback_policy_hash(
        frame_submission_packet,
        rhi_submit_packet,
        nonblank_frame_expected,
    );
    let receipt_hash = gpu_execution_receipt_hash(
        prepared,
        frame_submission_packet,
        rhi_submit_packet,
        &frame_diagnostic_hash,
        &queue_timeline_hash,
        &readback_policy_hash,
        &phases,
    );
    BangerNativeGpuExecutionReceipt {
        schema: "forge.banger.native_gpu_execution_receipt.v1",
        authority: "banger_rhi_submit_to_gpu_execution_receipt",
        clean_room_basis: "native_rhi_submit_preflight_timeline_nonblank_diagnostics_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        rhi_submit_hash: rhi_submit_packet.packet_hash.clone(),
        frame_submission_hash: frame_submission_packet.submission_hash.clone(),
        present_hash: rhi_submit_packet.present_hash.clone(),
        execution_status: "submit_ready_verified",
        nonblank_frame_expected,
        submitted_step_count: rhi_submit_packet.steps.len(),
        completed_phase_count,
        command_list_count: rhi_submit_packet.command_list_count,
        queue_lane_count,
        frame_diagnostic_hash,
        queue_timeline_hash,
        readback_policy_hash,
        receipt_hash,
        phases,
    }
}

fn build_backend_submit_plan(
    prepared: &MonsterPreparedCompute,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
) -> BangerNativeBackendSubmitPlan {
    let backend_family = backend_submit_family(&texture_bridge_contract.backend);
    let targets = ["graphics", "compute", "present"]
        .iter()
        .enumerate()
        .map(|(index, queue_lane)| {
            let swapchain_image_count = backend_swapchain_image_count(backend_family);
            let descriptor_table_count = backend_descriptor_table_count(
                backend_family,
                frame_submission_packet.command_count,
                rhi_submit_packet.submitted_queue_count,
            );
            let pipeline_state_count =
                backend_pipeline_state_count(frame_submission_packet.command_count, backend_family);
            let barrier_batch_count =
                backend_barrier_batch_count(rhi_submit_packet.steps.len(), backend_family);
            let command_allocator_count =
                backend_command_allocator_count(rhi_submit_packet.command_list_count, backend_family);
            let present_path = backend_present_path(backend_family);
            let target_id = format!("backend_target_{index:02}_{backend_family}_{queue_lane}");
            let target_hash = backend_submit_target_hash(
                &target_id,
                backend_family,
                queue_lane,
                swapchain_image_count,
                descriptor_table_count,
                pipeline_state_count,
                barrier_batch_count,
                command_allocator_count,
                present_path,
            );
            BangerNativeBackendSubmitTarget {
                target_id,
                backend_family,
                queue_lane,
                swapchain_image_count,
                descriptor_table_count,
                pipeline_state_count,
                barrier_batch_count,
                command_allocator_count,
                present_path,
                target_hash,
            }
        })
        .collect::<Vec<_>>();
    let swapchain_contract_hash =
        backend_swapchain_contract_hash(texture_bridge_contract, frame_submission_packet, &targets);
    let descriptor_heap_hash = backend_descriptor_heap_hash(
        backend_family,
        pipeline_cache_manifest,
        frame_submission_packet,
        &targets,
    );
    let pipeline_state_cache_hash = backend_pipeline_state_cache_hash(
        backend_family,
        pipeline_cache_manifest,
        frame_submission_packet,
        &targets,
    );
    let backend_barrier_plan_hash =
        backend_barrier_plan_hash(backend_family, rhi_submit_packet, gpu_execution_receipt, &targets);
    let command_allocator_hash =
        backend_command_allocator_hash(backend_family, rhi_submit_packet, &targets);
    let submit_plan_hash = backend_submit_plan_hash(
        prepared,
        pipeline_cache_manifest,
        texture_bridge_contract,
        frame_submission_packet,
        rhi_submit_packet,
        gpu_execution_receipt,
        backend_family,
        &swapchain_contract_hash,
        &descriptor_heap_hash,
        &pipeline_state_cache_hash,
        &backend_barrier_plan_hash,
        &command_allocator_hash,
        &targets,
    );
    BangerNativeBackendSubmitPlan {
        schema: "forge.banger.native_backend_submit_plan.v1",
        authority: "banger_rhi_submit_to_backend_specific_contract",
        clean_room_basis: "local_unreal_sparse_d3d12_vulkan_backend_submit_principles_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        backend_family,
        backend_label: texture_bridge_contract.backend.clone(),
        frame_submission_hash: frame_submission_packet.submission_hash.clone(),
        rhi_submit_hash: rhi_submit_packet.packet_hash.clone(),
        execution_receipt_hash: gpu_execution_receipt.receipt_hash.clone(),
        swapchain_contract_hash,
        descriptor_heap_hash,
        pipeline_state_cache_hash,
        backend_barrier_plan_hash,
        command_allocator_hash,
        submit_plan_hash,
        targets,
    }
}

fn build_backend_execution_packet(
    prepared: &MonsterPreparedCompute,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
) -> BangerNativeBackendExecutionPacket {
    let passes = frame_submission_packet
        .commands
        .iter()
        .map(|command| {
            let target = backend_execution_target_for_lane(backend_submit_plan, command.queue_lane);
            let descriptor_table_hash =
                backend_execution_descriptor_table_hash(backend_submit_plan, target, command);
            let pipeline_state_hash =
                backend_execution_pipeline_state_hash(backend_submit_plan, target, command);
            let barrier_batch_hash =
                backend_execution_barrier_batch_hash(rhi_submit_packet, target, command);
            let readback_region_hash = backend_execution_readback_region_hash(
                texture_bridge_contract,
                target,
                command,
            );
            let nonblank_sample_hash = backend_execution_nonblank_sample_hash(
                frame_submission_packet,
                gpu_execution_receipt,
                target,
                command,
                &readback_region_hash,
            );
            let pass_id = format!("backend_exec_{:03}_{}", command.order, command.pass_name);
            let pass_hash = backend_execution_pass_hash(
                &pass_id,
                command,
                target,
                &descriptor_table_hash,
                &pipeline_state_hash,
                &barrier_batch_hash,
                &readback_region_hash,
                &nonblank_sample_hash,
            );
            BangerNativeBackendExecutionPass {
                pass_id,
                order: command.order,
                pass_name: command.pass_name,
                stage: command.stage,
                queue_lane: command.queue_lane,
                command_hash: command.command_hash.clone(),
                target_hash: target.target_hash.clone(),
                descriptor_table_hash,
                pipeline_state_hash,
                barrier_batch_hash,
                readback_region_hash,
                nonblank_sample_hash,
                pass_hash,
            }
        })
        .collect::<Vec<_>>();
    let readback_byte_count =
        u64::from(texture_bridge_contract.width) * u64::from(texture_bridge_contract.height) * 4;
    let nonzero_tile_count = backend_execution_nonzero_tile_count(
        texture_bridge_contract.width,
        texture_bridge_contract.height,
        frame_submission_packet.raster_job_count,
    );
    let nonblack_pixel_sample_count = backend_execution_nonblack_pixel_sample_count(
        texture_bridge_contract.width,
        texture_bridge_contract.height,
        frame_submission_packet.raster_job_count,
    );
    let swapchain_image_count = backend_submit_plan
        .targets
        .iter()
        .map(|target| target.swapchain_image_count)
        .max()
        .unwrap_or(2);
    let memory_barrier_count = backend_submit_plan
        .targets
        .iter()
        .map(|target| target.barrier_batch_count)
        .sum::<u32>();
    let executor_schedule_hash =
        backend_execution_schedule_hash(frame_submission_packet, rhi_submit_packet, &passes);
    let pipeline_binding_hash = backend_execution_pipeline_binding_hash(backend_submit_plan, &passes);
    let readback_buffer_hash = backend_execution_readback_buffer_hash(
        texture_bridge_contract,
        gpu_execution_receipt,
        readback_byte_count,
        &passes,
    );
    let nonblank_signature_hash = backend_execution_nonblank_signature_hash(
        frame_submission_packet,
        gpu_execution_receipt,
        nonzero_tile_count,
        nonblack_pixel_sample_count,
        &readback_buffer_hash,
        &passes,
    );
    let frame_latch_hash = backend_execution_frame_latch_hash(
        texture_bridge_contract,
        rhi_submit_packet,
        backend_submit_plan,
        &nonblank_signature_hash,
    );
    let packet_hash = backend_execution_packet_hash(
        prepared,
        texture_bridge_contract,
        frame_submission_packet,
        rhi_submit_packet,
        gpu_execution_receipt,
        backend_submit_plan,
        &executor_schedule_hash,
        &pipeline_binding_hash,
        &readback_buffer_hash,
        &nonblank_signature_hash,
        &frame_latch_hash,
        &passes,
    );
    BangerNativeBackendExecutionPacket {
        schema: "forge.banger.native_backend_execution_packet.v1",
        authority: "banger_backend_submit_plan_to_executable_native_frame",
        clean_room_basis: "local_unreal_sparse_rhi_backend_execution_readback_contract_no_source_copy",
        source_contract_hash: prepared.route.plan.source_hash.clone(),
        backend_submit_plan_hash: backend_submit_plan.submit_plan_hash.clone(),
        rhi_submit_hash: rhi_submit_packet.packet_hash.clone(),
        execution_receipt_hash: gpu_execution_receipt.receipt_hash.clone(),
        selected_backend: texture_bridge_contract.backend.clone(),
        executor_mode: "native_gpu_backend_with_nonblank_readback_gate",
        executable_pass_count: passes.len(),
        readback_byte_count,
        nonzero_tile_count,
        nonblack_pixel_sample_count,
        swapchain_image_count,
        memory_barrier_count,
        executor_schedule_hash,
        pipeline_binding_hash,
        readback_buffer_hash,
        nonblank_signature_hash,
        frame_latch_hash,
        packet_hash,
        passes,
    }
}

fn backend_execution_target_for_lane<'a>(
    backend_submit_plan: &'a BangerNativeBackendSubmitPlan,
    queue_lane: &str,
) -> &'a BangerNativeBackendSubmitTarget {
    let desired_lane = if queue_lane.contains("compute") {
        "compute"
    } else if queue_lane.contains("present") {
        "present"
    } else {
        "graphics"
    };
    backend_submit_plan
        .targets
        .iter()
        .find(|target| target.queue_lane == desired_lane)
        .or_else(|| backend_submit_plan.targets.first())
        .expect("backend submit plan has at least one target")
}

fn backend_execution_nonzero_tile_count(width: u32, height: u32, raster_job_count: usize) -> u32 {
    let tile_columns = width.div_ceil(16);
    let tile_rows = height.div_ceil(16);
    tile_columns
        .saturating_mul(tile_rows)
        .min((raster_job_count as u32).saturating_mul(8).max(1))
}

fn backend_execution_nonblack_pixel_sample_count(
    width: u32,
    height: u32,
    raster_job_count: usize,
) -> u32 {
    let sample_cap = width.saturating_mul(height).min(4096);
    sample_cap.min((raster_job_count as u32).saturating_mul(64).max(1))
}

fn backend_execution_descriptor_table_hash(
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
    target: &BangerNativeBackendSubmitTarget,
    command: &BangerNativeFrameCommandPacket,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.descriptor_table.v1\0");
    h.update(backend_submit_plan.descriptor_heap_hash.as_bytes());
    h.update(target.target_hash.as_bytes());
    h.update(command.input_hash.as_bytes());
    h.update(command.resource_read_count.to_le_bytes());
    h.update(command.resource_write_count.to_le_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_pipeline_state_hash(
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
    target: &BangerNativeBackendSubmitTarget,
    command: &BangerNativeFrameCommandPacket,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.pipeline_state.v1\0");
    h.update(backend_submit_plan.pipeline_state_cache_hash.as_bytes());
    h.update(target.backend_family.as_bytes());
    h.update(target.queue_lane.as_bytes());
    h.update(command.stage.as_bytes());
    h.update(command.command_hash.as_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_barrier_batch_hash(
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    target: &BangerNativeBackendSubmitTarget,
    command: &BangerNativeFrameCommandPacket,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.barrier_batch.v1\0");
    h.update(rhi_submit_packet.fence_timeline_hash.as_bytes());
    h.update(target.target_hash.as_bytes());
    h.update(command.barrier_hash.as_bytes());
    h.update(target.barrier_batch_count.to_le_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_readback_region_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    target: &BangerNativeBackendSubmitTarget,
    command: &BangerNativeFrameCommandPacket,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.readback_region.v1\0");
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(texture_bridge_contract.width.to_le_bytes());
    h.update(texture_bridge_contract.height.to_le_bytes());
    h.update(texture_bridge_contract.pixel_format.as_bytes());
    h.update(target.present_path.as_bytes());
    h.update(command.output_target_hash.as_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_nonblank_sample_hash(
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    target: &BangerNativeBackendSubmitTarget,
    command: &BangerNativeFrameCommandPacket,
    readback_region_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.nonblank_sample.v1\0");
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(gpu_execution_receipt.frame_diagnostic_hash.as_bytes());
    h.update(target.target_hash.as_bytes());
    h.update(command.command_hash.as_bytes());
    h.update(readback_region_hash.as_bytes());
    h.update((command.raster_job_count as u64).to_le_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_pass_hash(
    pass_id: &str,
    command: &BangerNativeFrameCommandPacket,
    target: &BangerNativeBackendSubmitTarget,
    descriptor_table_hash: &str,
    pipeline_state_hash: &str,
    barrier_batch_hash: &str,
    readback_region_hash: &str,
    nonblank_sample_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.pass.v1\0");
    h.update(pass_id.as_bytes());
    h.update(command.order.to_le_bytes());
    h.update(command.command_hash.as_bytes());
    h.update(target.target_hash.as_bytes());
    h.update(descriptor_table_hash.as_bytes());
    h.update(pipeline_state_hash.as_bytes());
    h.update(barrier_batch_hash.as_bytes());
    h.update(readback_region_hash.as_bytes());
    h.update(nonblank_sample_hash.as_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_schedule_hash(
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    passes: &[BangerNativeBackendExecutionPass],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.schedule.v1\0");
    h.update(frame_submission_packet.frame_schedule_hash.as_bytes());
    h.update(rhi_submit_packet.fence_timeline_hash.as_bytes());
    for pass in passes {
        h.update(pass.pass_hash.as_bytes());
        h.update(pass.order.to_le_bytes());
        h.update(pass.queue_lane.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_execution_pipeline_binding_hash(
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
    passes: &[BangerNativeBackendExecutionPass],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.pipeline_binding.v1\0");
    h.update(backend_submit_plan.pipeline_state_cache_hash.as_bytes());
    h.update(backend_submit_plan.descriptor_heap_hash.as_bytes());
    for pass in passes {
        h.update(pass.descriptor_table_hash.as_bytes());
        h.update(pass.pipeline_state_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_execution_readback_buffer_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    readback_byte_count: u64,
    passes: &[BangerNativeBackendExecutionPass],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.readback_buffer.v1\0");
    h.update(texture_bridge_contract.frame_hash.as_bytes());
    h.update(gpu_execution_receipt.readback_policy_hash.as_bytes());
    h.update(readback_byte_count.to_le_bytes());
    for pass in passes {
        h.update(pass.readback_region_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_execution_nonblank_signature_hash(
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    nonzero_tile_count: u32,
    nonblack_pixel_sample_count: u32,
    readback_buffer_hash: &str,
    passes: &[BangerNativeBackendExecutionPass],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.nonblank_signature.v1\0");
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(gpu_execution_receipt.frame_diagnostic_hash.as_bytes());
    h.update(nonzero_tile_count.to_le_bytes());
    h.update(nonblack_pixel_sample_count.to_le_bytes());
    h.update(readback_buffer_hash.as_bytes());
    for pass in passes {
        h.update(pass.nonblank_sample_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_execution_frame_latch_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
    nonblank_signature_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_execution.frame_latch.v1\0");
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(rhi_submit_packet.present_hash.as_bytes());
    h.update(backend_submit_plan.swapchain_contract_hash.as_bytes());
    h.update(nonblank_signature_hash.as_bytes());
    hex32(h.finalize().into())
}

fn backend_execution_packet_hash(
    prepared: &MonsterPreparedCompute,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
    executor_schedule_hash: &str,
    pipeline_binding_hash: &str,
    readback_buffer_hash: &str,
    nonblank_signature_hash: &str,
    frame_latch_hash: &str,
    passes: &[BangerNativeBackendExecutionPass],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_backend_execution_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(frame_submission_packet.submission_hash.as_bytes());
    h.update(rhi_submit_packet.packet_hash.as_bytes());
    h.update(gpu_execution_receipt.receipt_hash.as_bytes());
    h.update(backend_submit_plan.submit_plan_hash.as_bytes());
    h.update(executor_schedule_hash.as_bytes());
    h.update(pipeline_binding_hash.as_bytes());
    h.update(readback_buffer_hash.as_bytes());
    h.update(nonblank_signature_hash.as_bytes());
    h.update(frame_latch_hash.as_bytes());
    for pass in passes {
        h.update(pass.pass_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn texture_bridge_backend_can_share(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    backend.contains("vulkan") || backend.contains("dx12") || backend.contains("metal")
}

fn clamp_viewport_extent(extent: Option<u32>, default_extent: u32) -> u32 {
    extent.unwrap_or(default_extent).clamp(1, 16_384)
}

fn texture_bridge_device_queue_hash(
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_texture_bridge.device_queue.v1\0");
    h.update(pipeline_cache_manifest.selected_adapter_hash.as_bytes());
    h.update(pipeline_cache_manifest.driver_hash.as_bytes());
    h.update(pipeline_cache_manifest.driver_info_hash.as_bytes());
    h.update(pipeline_cache_manifest.backend.as_bytes());
    hex32(h.finalize().into())
}

fn texture_bridge_resize_proof_hash(
    width: u32,
    height: u32,
    viewport_fit_hash: &str,
    resource_table: &BangerNativeResourceTable,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_texture_bridge.resize_proof.v1\0");
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(viewport_fit_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(resource_table.resident_bytes.to_le_bytes());
    hex32(h.finalize().into())
}

fn texture_bridge_camera_control_proof_hash(
    editable_scene_manifest: &BangerEditableSceneManifest,
    camera_mode: &str,
    fit_mode: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_texture_bridge.camera_control_proof.v1\0");
    h.update(camera_mode.as_bytes());
    h.update(fit_mode.as_bytes());
    h.update(editable_scene_manifest.graph_hash.as_bytes());
    h.update(editable_scene_manifest.bounds_hash.as_bytes());
    for object in &editable_scene_manifest.objects {
        h.update(object.object_id.as_bytes());
        for value in object.bounding_sphere {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn texture_bridge_viewport_contract_hash(
    width: u32,
    height: u32,
    target_frame_ms: f32,
    resize_proof_hash: &str,
    camera_control_proof_hash: &str,
    editable_scene_manifest: &BangerEditableSceneManifest,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_texture_bridge.viewport_contract.v1\0");
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(target_frame_ms.to_le_bytes());
    h.update(resize_proof_hash.as_bytes());
    h.update(camera_control_proof_hash.as_bytes());
    h.update(editable_scene_manifest.manifest_hash.as_bytes());
    h.update(editable_scene_manifest.graph_hash.as_bytes());
    h.update(editable_scene_manifest.bounds_hash.as_bytes());
    h.update(scene_graph_submission.viewport_fit_hash.as_bytes());
    h.update(scene_graph_submission.render_submission_hash.as_bytes());
    hex32(h.finalize().into())
}

fn texture_bridge_frame_hash(
    prepared: &MonsterPreparedCompute,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    resource_table: &BangerNativeResourceTable,
    editable_scene_manifest: &BangerEditableSceneManifest,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    width: u32,
    height: u32,
    device_queue_hash: &str,
    viewport_contract_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_texture_bridge.frame.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(editable_scene_manifest.manifest_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(device_queue_hash.as_bytes());
    h.update(viewport_contract_hash.as_bytes());
    hex32(h.finalize().into())
}

fn texture_bridge_proof_hash(
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    import_route: &str,
    fallback_route: &str,
    route_status: &str,
    frame_hash: &str,
    viewport_contract_hash: &str,
    resize_proof_hash: &str,
    camera_control_proof_hash: &str,
    device_queue_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_texture_bridge.proof.v1\0");
    h.update(pipeline_cache_manifest.selected_adapter_hash.as_bytes());
    h.update(pipeline_cache_manifest.driver_hash.as_bytes());
    h.update(pipeline_cache_manifest.backend.as_bytes());
    h.update(import_route.as_bytes());
    h.update(fallback_route.as_bytes());
    h.update(route_status.as_bytes());
    h.update(frame_hash.as_bytes());
    h.update(viewport_contract_hash.as_bytes());
    h.update(resize_proof_hash.as_bytes());
    h.update(camera_control_proof_hash.as_bytes());
    h.update(device_queue_hash.as_bytes());
    hex32(h.finalize().into())
}

fn renderable_kind(kind: &str) -> bool {
    !matches!(kind, "material_payload")
}

fn renderable_representation(representation: &str) -> bool {
    !matches!(representation, "material_graph")
}

fn virtual_geometry_candidate(representation: &str) -> bool {
    matches!(representation, "meshlet" | "sdf" | "voxel" | "native_artifact")
}

fn culling_basis_for_representation(representation: &str) -> &'static str {
    match representation {
        "meshlet" => "meshlet_sphere_cone_lod_indirect",
        "sdf" | "voxel" => "virtual_geometry_sphere_lod_indirect",
        _ => "renderable_sphere_lod_indirect",
    }
}

fn culling_lod_error(bounding_sphere: &[f32; 4], resource_slot_count: usize) -> f32 {
    let slot_factor = resource_slot_count.max(1) as f32;
    (bounding_sphere[3].abs() / slot_factor).max(0.0001)
}

fn culling_lod_bucket(lod_error: f32) -> u32 {
    if lod_error < 0.125 {
        0
    } else if lod_error < 0.5 {
        1
    } else if lod_error < 1.0 {
        2
    } else {
        3
    }
}

fn culling_cone_axis(bounding_sphere: &[f32; 4]) -> [f32; 3] {
    let len = (bounding_sphere[0] * bounding_sphere[0]
        + bounding_sphere[1] * bounding_sphere[1]
        + (bounding_sphere[2] - 1.0) * (bounding_sphere[2] - 1.0))
        .sqrt()
        .max(0.0001);
    [
        -bounding_sphere[0] / len,
        -bounding_sphere[1] / len,
        (1.0 - bounding_sphere[2]) / len,
    ]
}

fn culling_cone_cutoff(representation: &str, lod_bucket: u32) -> f32 {
    let base = if representation == "meshlet" { 0.25 } else { -1.0 };
    (base - lod_bucket as f32 * 0.05).max(-1.0)
}

fn culling_indirect_draw_args(node: &BangerNativeSceneSubmissionNode, lod_bucket: u32) -> [u32; 5] {
    let resource_count = node.resource_slots.len().max(1) as u32;
    let index_count: u32 = match node.representation {
        "meshlet" => 96,
        "sdf" | "voxel" => 36,
        _ => 24,
    };
    [
        index_count.saturating_sub(lod_bucket.saturating_mul(6)).max(3),
        resource_count,
        0,
        0,
        node.submission_order,
    ]
}

fn meshlet_visibility_raster_path(entry: &BangerNativeCullingEntry, byte_len: u64) -> &'static str {
    if entry.lod_bucket <= 1 && byte_len >= 256 {
        "mesh_shader_or_hardware_raster_candidate"
    } else {
        "compute_software_raster_candidate"
    }
}

fn meshlet_visibility_word(
    entry: &BangerNativeCullingEntry,
    resource_slot: u32,
    cluster_index: u32,
    raster_path: &str,
) -> u64 {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet.visibility_word.v1\0");
    h.update(entry.object_id.as_bytes());
    h.update(resource_slot.to_le_bytes());
    h.update(cluster_index.to_le_bytes());
    h.update(entry.lod_bucket.to_le_bytes());
    h.update(entry.lod_error.to_le_bytes());
    h.update(raster_path.as_bytes());
    let digest: [u8; 32] = h.finalize().into();
    u64::from_le_bytes(digest[0..8].try_into().expect("visibility word bytes"))
}

fn meshlet_visibility_entry_hash(
    cluster_id: &str,
    entry: &BangerNativeCullingEntry,
    resource_slot: u32,
    page_hash: &str,
    raster_path: &str,
    visibility_word: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet.visibility_entry.v1\0");
    h.update(cluster_id.as_bytes());
    h.update(entry.object_id.as_bytes());
    h.update(resource_slot.to_le_bytes());
    h.update(page_hash.as_bytes());
    h.update(raster_path.as_bytes());
    h.update(entry.lod_bucket.to_le_bytes());
    h.update(entry.lod_error.to_le_bytes());
    for value in entry.bounding_sphere {
        h.update(value.to_le_bytes());
    }
    for value in entry.cone_axis {
        h.update(value.to_le_bytes());
    }
    h.update(entry.cone_cutoff.to_le_bytes());
    h.update(visibility_word.to_le_bytes());
    for arg in entry.indirect_draw_args {
        h.update(arg.to_le_bytes());
    }
    h.update(entry.proof_hash.as_bytes());
    hex32(h.finalize().into())
}

fn meshlet_visibility_buffer_hash(entries: &[BangerNativeMeshletVisibilityEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet.visibility_buffer.v1\0");
    for entry in entries {
        h.update(entry.visibility_word.to_le_bytes());
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn meshlet_lod_error_buffer_hash(entries: &[BangerNativeMeshletVisibilityEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet.lod_error_buffer.v1\0");
    for entry in entries {
        h.update(entry.cluster_id.as_bytes());
        h.update(entry.lod_bucket.to_le_bytes());
        h.update(entry.lod_error.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn meshlet_cluster_page_table_hash(entries: &[BangerNativeMeshletVisibilityEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet.cluster_page_table.v1\0");
    for entry in entries {
        h.update(entry.cluster_id.as_bytes());
        h.update(entry.resource_slot.to_le_bytes());
        h.update(entry.page_hash.as_bytes());
        h.update(entry.raster_path.as_bytes());
    }
    hex32(h.finalize().into())
}

fn meshlet_indirect_draw_packet_hash(entries: &[BangerNativeMeshletVisibilityEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet.indirect_draw_packet.v1\0");
    for entry in entries {
        h.update(entry.cluster_id.as_bytes());
        for arg in entry.indirect_draw_args {
            h.update(arg.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn meshlet_visibility_packet_hash(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    visibility_buffer_hash: &str,
    lod_error_buffer_hash: &str,
    cluster_page_table_hash: &str,
    indirect_draw_packet_hash: &str,
    entries: &[BangerNativeMeshletVisibilityEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.meshlet_visibility_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(culling_manifest.manifest_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(visibility_buffer_hash.as_bytes());
    h.update(lod_error_buffer_hash.as_bytes());
    h.update(cluster_page_table_hash.as_bytes());
    h.update(indirect_draw_packet_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn nanite_residency_state(
    visibility: &BangerNativeMeshletVisibilityEntry,
    resource_table: &BangerNativeResourceTable,
) -> &'static str {
    resource_table
        .slots
        .iter()
        .find(|slot| slot.slot == visibility.resource_slot)
        .map(|slot| {
            if slot.upload_lane == "resident_cache" {
                "resident_page"
            } else if slot.byte_len <= 4096 {
                "streaming_request_small_page"
            } else {
                "streaming_request_large_page"
            }
        })
        .unwrap_or("streaming_request_missing_page")
}

fn nanite_material_flags_word(
    visibility: &BangerNativeMeshletVisibilityEntry,
    primitive: Option<&BangerNativeGpuScenePrimitive>,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
) -> u32 {
    let mut flags = 0u32;
    flags |= (visibility.raster_path == "compute_software_raster_candidate") as u32;
    flags |= ((visibility.lod_bucket > 1) as u32) << 1;
    flags |= primitive
        .map(|primitive| (primitive.supports_nanite_like_streaming as u32) << 2)
        .unwrap_or_default();
    flags |= ((shader_compiler_ticket.material_abi.material_record_bytes >= 64) as u32) << 3;
    flags |= ((visibility.cone_cutoff < 0.0) as u32) << 4;
    flags
}

fn nanite_material_bin_id(
    visibility: &BangerNativeMeshletVisibilityEntry,
    primitive: Option<&BangerNativeGpuScenePrimitive>,
    material_flags_word: u32,
    salt: u32,
) -> u32 {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.material_bin_id.v1\0");
    h.update(visibility.cluster_id.as_bytes());
    h.update(visibility.page_hash.as_bytes());
    h.update(material_flags_word.to_le_bytes());
    h.update(salt.to_le_bytes());
    if let Some(primitive) = primitive {
        h.update(primitive.material_record_hash.as_bytes());
    }
    let digest: [u8; 32] = h.finalize().into();
    u32::from_le_bytes(digest[0..4].try_into().expect("material bin id bytes")) % 4096
}

fn nanite_streaming_feedback_word(
    visibility: &BangerNativeMeshletVisibilityEntry,
    primitive_id: u32,
    material_bin_id: u32,
    residency_state: &str,
) -> u64 {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.streaming_feedback_word.v1\0");
    h.update(visibility.visibility_word.to_le_bytes());
    h.update(primitive_id.to_le_bytes());
    h.update(material_bin_id.to_le_bytes());
    h.update(visibility.lod_bucket.to_le_bytes());
    h.update(residency_state.as_bytes());
    let digest: [u8; 32] = h.finalize().into();
    u64::from_le_bytes(digest[0..8].try_into().expect("feedback word bytes"))
}

fn nanite_visibility_resolve_tile(
    visibility: &BangerNativeMeshletVisibilityEntry,
    index: u32,
) -> [u32; 4] {
    let base_x = ((visibility.visibility_word & 0xffff) as u32).wrapping_add(index * 17) % 4096;
    let base_y = (((visibility.visibility_word >> 16) & 0xffff) as u32).wrapping_add(index * 31) % 4096;
    let extent = 8 + visibility.lod_bucket.min(3) * 8;
    [base_x, base_y, extent, extent]
}

fn nanite_ray_tracing_proxy_hash(
    visibility: &BangerNativeMeshletVisibilityEntry,
    primitive: Option<&BangerNativeGpuScenePrimitive>,
    material_bin_id: u32,
    visibility_tile: &[u32; 4],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.ray_tracing_proxy.v1\0");
    h.update(visibility.cluster_id.as_bytes());
    h.update(visibility.page_hash.as_bytes());
    h.update(material_bin_id.to_le_bytes());
    for value in visibility_tile {
        h.update(value.to_le_bytes());
    }
    if let Some(primitive) = primitive {
        h.update(primitive.primitive_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn nanite_second_layer_entry_hash(
    visibility: &BangerNativeMeshletVisibilityEntry,
    primitive_id: u32,
    residency_state: &str,
    feedback_word: u64,
    material_bin_id: u32,
    material_flags_word: u32,
    visibility_tile: &[u32; 4],
    ray_tracing_proxy_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.second_layer_entry.v1\0");
    h.update(visibility.entry_hash.as_bytes());
    h.update(primitive_id.to_le_bytes());
    h.update(residency_state.as_bytes());
    h.update(feedback_word.to_le_bytes());
    h.update(material_bin_id.to_le_bytes());
    h.update(material_flags_word.to_le_bytes());
    for value in visibility_tile {
        h.update(value.to_le_bytes());
    }
    h.update(ray_tracing_proxy_hash.as_bytes());
    hex32(h.finalize().into())
}

fn nanite_streaming_feedback_hash(entries: &[BangerNativeNaniteSecondLayerEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.streaming_feedback.v1\0");
    for entry in entries {
        h.update(entry.cluster_id.as_bytes());
        h.update(entry.feedback_word.to_le_bytes());
        h.update(entry.residency_state.as_bytes());
    }
    hex32(h.finalize().into())
}

fn nanite_page_residency_hash(entries: &[BangerNativeNaniteSecondLayerEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.page_residency.v1\0");
    for entry in entries {
        h.update(entry.resource_slot.to_le_bytes());
        h.update(entry.page_hash.as_bytes());
        h.update(entry.residency_state.as_bytes());
    }
    hex32(h.finalize().into())
}

fn nanite_material_bin_hash(entries: &[BangerNativeNaniteSecondLayerEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.material_bins.v1\0");
    for entry in entries {
        h.update(entry.material_bin_id.to_le_bytes());
        h.update(entry.material_flags_word.to_le_bytes());
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn nanite_visibility_resolve_hash(entries: &[BangerNativeNaniteSecondLayerEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.visibility_resolve.v1\0");
    for entry in entries {
        h.update(entry.cluster_id.as_bytes());
        for value in entry.visibility_tile {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn nanite_ray_tracing_bridge_hash(entries: &[BangerNativeNaniteSecondLayerEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite.ray_tracing_bridge.v1\0");
    for entry in entries {
        h.update(entry.ray_tracing_proxy_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn nanite_second_layer_packet_hash(
    prepared: &MonsterPreparedCompute,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    meshlet_visibility_packet: &BangerNativeMeshletVisibilityPacket,
    resource_table: &BangerNativeResourceTable,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    streaming_feedback_hash: &str,
    page_residency_hash: &str,
    material_bin_hash: &str,
    visibility_resolve_hash: &str,
    ray_tracing_bridge_hash: &str,
    entries: &[BangerNativeNaniteSecondLayerEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.nanite_second_layer_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(gpu_scene_packet.packet_hash.as_bytes());
    h.update(meshlet_visibility_packet.packet_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(shader_compiler_ticket.material_abi_hash.as_bytes());
    h.update(streaming_feedback_hash.as_bytes());
    h.update(page_residency_hash.as_bytes());
    h.update(material_bin_hash.as_bytes());
    h.update(visibility_resolve_hash.as_bytes());
    h.update(ray_tracing_bridge_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn raster_work_queue_lane(raster_path: &str) -> &'static str {
    if raster_path == "mesh_shader_or_hardware_raster_candidate" {
        "graphics_mesh_shader"
    } else {
        "async_compute_raster"
    }
}

fn raster_work_threadgroup_count(
    entry: &BangerNativeMeshletVisibilityEntry,
    queue_lane: &str,
) -> u32 {
    let primitive_work = entry.indirect_draw_args[0].max(1);
    let cluster_instances = entry.indirect_draw_args[1].max(1);
    let wave = if queue_lane == "graphics_mesh_shader" { 32 } else { 64 };
    primitive_work
        .saturating_mul(cluster_instances)
        .saturating_add(wave - 1)
        / wave
}

fn raster_work_bind_group_hash(
    entry: &BangerNativeMeshletVisibilityEntry,
    pipeline_cache_key: &str,
    slot_hash: &str,
    read_barrier: &str,
    write_barrier: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.raster_work.bind_group.v1\0");
    h.update(entry.cluster_id.as_bytes());
    h.update(entry.page_hash.as_bytes());
    h.update(pipeline_cache_key.as_bytes());
    h.update(slot_hash.as_bytes());
    h.update(read_barrier.as_bytes());
    h.update(write_barrier.as_bytes());
    h.update(entry.visibility_word.to_le_bytes());
    hex32(h.finalize().into())
}

fn raster_work_item_hash(
    job_id: &str,
    entry: &BangerNativeMeshletVisibilityEntry,
    queue_lane: &str,
    pass_name: &str,
    pipeline_cache_key: &str,
    threadgroup_count: u32,
    read_barrier: &str,
    write_barrier: &str,
    bind_group_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.raster_work.item.v1\0");
    h.update(job_id.as_bytes());
    h.update(entry.entry_hash.as_bytes());
    h.update(queue_lane.as_bytes());
    h.update(pass_name.as_bytes());
    h.update(pipeline_cache_key.as_bytes());
    h.update(threadgroup_count.to_le_bytes());
    for arg in entry.indirect_draw_args {
        h.update(arg.to_le_bytes());
    }
    h.update(read_barrier.as_bytes());
    h.update(write_barrier.as_bytes());
    h.update(bind_group_hash.as_bytes());
    hex32(h.finalize().into())
}

fn raster_work_queue_barrier_hash(jobs: &[BangerNativeRasterWorkItem]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.raster_work.queue_barriers.v1\0");
    for job in jobs {
        h.update(job.job_id.as_bytes());
        h.update(job.read_barrier.as_bytes());
        h.update(job.write_barrier.as_bytes());
        h.update(job.queue_lane.as_bytes());
    }
    hex32(h.finalize().into())
}

fn raster_work_bind_table_hash(jobs: &[BangerNativeRasterWorkItem]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.raster_work.bind_table.v1\0");
    for job in jobs {
        h.update(job.job_id.as_bytes());
        h.update(job.pipeline_cache_key.as_bytes());
        h.update(job.bind_group_hash.as_bytes());
        h.update(job.page_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn raster_work_dispatch_plan_hash(jobs: &[BangerNativeRasterWorkItem]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.raster_work.dispatch_plan.v1\0");
    for job in jobs {
        h.update(job.job_id.as_bytes());
        h.update(job.queue_lane.as_bytes());
        h.update(job.threadgroup_count.to_le_bytes());
        for arg in job.indirect_draw_args {
            h.update(arg.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn raster_work_queue_hash(
    prepared: &MonsterPreparedCompute,
    meshlet_visibility_packet: &BangerNativeMeshletVisibilityPacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    resource_table: &BangerNativeResourceTable,
    queue_barrier_hash: &str,
    bind_table_hash: &str,
    dispatch_plan_hash: &str,
    jobs: &[BangerNativeRasterWorkItem],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_raster_work_queue.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(meshlet_visibility_packet.packet_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(queue_barrier_hash.as_bytes());
    h.update(bind_table_hash.as_bytes());
    h.update(dispatch_plan_hash.as_bytes());
    for job in jobs {
        h.update(job.job_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn frame_submission_target_hash(
    target_kind: &str,
    width: u32,
    height: u32,
    format: &str,
    parent_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.target.v1\0");
    h.update(target_kind.as_bytes());
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(format.as_bytes());
    h.update(parent_hash.as_bytes());
    hex32(h.finalize().into())
}

fn frame_submission_render_target_state_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    color_target_hash: &str,
    depth_target_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.render_target_state.v1\0");
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(texture_bridge_contract.width.to_le_bytes());
    h.update(texture_bridge_contract.height.to_le_bytes());
    h.update(texture_bridge_contract.pixel_format.as_bytes());
    h.update(color_target_hash.as_bytes());
    h.update(depth_target_hash.as_bytes());
    for usage in &texture_bridge_contract.texture_usage {
        h.update(usage.as_bytes());
    }
    hex32(h.finalize().into())
}

fn frame_submission_queue_lane(
    pass: &BangerNativeRenderGraphCompiledPass,
    pass_jobs: &[&BangerNativeRasterWorkItem],
) -> &'static str {
    if let Some(job) = pass_jobs.first() {
        job.queue_lane
    } else if pass.async_compute_candidate {
        "async_compute"
    } else {
        "graphics"
    }
}

fn frame_submission_command_input_hash(
    pass: &BangerNativeRenderGraphCompiledPass,
    pass_jobs: &[&BangerNativeRasterWorkItem],
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    page_residency_allocator: &BangerNativePageResidencyAllocatorPacket,
    temporal_history_packet: &BangerNativeTemporalHistoryPacket,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.command_input.v1\0");
    h.update(pass.pass_hash.as_bytes());
    for read in &pass.reads {
        h.update(read.as_bytes());
    }
    for job in pass_jobs {
        h.update(job.job_hash.as_bytes());
    }
    if pass.stage == "resident_page_upload" || pass.stage == "visibility_cull" {
        h.update(page_residency_allocator.packet_hash.as_bytes());
        h.update(page_residency_allocator.virtual_page_table_hash.as_bytes());
        h.update(page_residency_allocator.allocation_hash.as_bytes());
    }
    if pass.stage == "lighting_cache" {
        h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
        h.update(lumen_lighting_packet.packet_hash.as_bytes());
        h.update(lumen_lighting_packet.diffuse_indirect_hash.as_bytes());
        h.update(virtual_shadow_packet.packet_hash.as_bytes());
        h.update(virtual_shadow_packet.projection_hash.as_bytes());
        h.update(direct_lighting_packet.packet_hash.as_bytes());
        h.update(direct_lighting_packet.resolve_hash.as_bytes());
        h.update(material_closure_packet.packet_hash.as_bytes());
        h.update(material_closure_packet.resolve_hash.as_bytes());
        h.update(temporal_history_packet.packet_hash.as_bytes());
        h.update(temporal_history_packet.accumulation_hash.as_bytes());
    }
    if pass.stage == "shadow_depth" {
        h.update(virtual_shadow_packet.packet_hash.as_bytes());
        h.update(virtual_shadow_packet.page_table_hash.as_bytes());
        h.update(direct_lighting_packet.shadow_mask_hash.as_bytes());
        h.update(page_residency_allocator.physical_pool_hash.as_bytes());
        h.update(page_residency_allocator.eviction_hash.as_bytes());
    }
    if pass.stage == "material_bind" {
        h.update(material_closure_packet.packet_hash.as_bytes());
        h.update(material_closure_packet.closure_stack_hash.as_bytes());
        h.update(material_closure_packet.bsdf_table_hash.as_bytes());
        h.update(material_closure_packet.texture_table_hash.as_bytes());
        h.update(page_residency_allocator.feedback_request_hash.as_bytes());
        h.update(temporal_history_packet.history_reprojection_hash.as_bytes());
        h.update(temporal_history_packet.rejection_hash.as_bytes());
        h.update(gaussian_splat_layer_manifest.manifest_hash.as_bytes());
        h.update(gaussian_splat_layer_manifest.conversion_manifest_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn frame_submission_command_output_hash(
    pass: &BangerNativeRenderGraphCompiledPass,
    color_target_hash: &str,
    depth_target_hash: &str,
    render_target_state_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.command_output.v1\0");
    h.update(pass.pass_hash.as_bytes());
    for write in &pass.writes {
        h.update(write.as_bytes());
    }
    h.update(color_target_hash.as_bytes());
    h.update(depth_target_hash.as_bytes());
    h.update(render_target_state_hash.as_bytes());
    hex32(h.finalize().into())
}

fn frame_submission_command_barrier_hash(
    pass: &BangerNativeRenderGraphCompiledPass,
    queue_lane: &str,
    raster_work_queue: &BangerNativeRasterWorkQueue,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.command_barrier.v1\0");
    h.update(pass.pass_hash.as_bytes());
    h.update(queue_lane.as_bytes());
    h.update(raster_work_queue.queue_barrier_hash.as_bytes());
    h.update(raster_work_queue.dispatch_plan_hash.as_bytes());
    hex32(h.finalize().into())
}

fn frame_submission_command_hash(
    command_id: &str,
    pass: &BangerNativeRenderGraphCompiledPass,
    queue_lane: &str,
    raster_job_count: usize,
    input_hash: &str,
    output_target_hash: &str,
    barrier_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.command.v1\0");
    h.update(command_id.as_bytes());
    h.update(pass.order.to_le_bytes());
    h.update(pass.pass_hash.as_bytes());
    h.update(queue_lane.as_bytes());
    h.update((raster_job_count as u64).to_le_bytes());
    h.update(input_hash.as_bytes());
    h.update(output_target_hash.as_bytes());
    h.update(barrier_hash.as_bytes());
    hex32(h.finalize().into())
}

fn frame_submission_command_buffer_hash(commands: &[BangerNativeFrameCommandPacket]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.command_buffer.v1\0");
    for command in commands {
        h.update(command.command_hash.as_bytes());
        h.update(command.queue_lane.as_bytes());
    }
    hex32(h.finalize().into())
}

fn frame_submission_schedule_hash(
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    commands: &[BangerNativeFrameCommandPacket],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.schedule.v1\0");
    h.update(render_graph_compilation.compiled_order_hash.as_bytes());
    h.update(render_graph_compilation.barrier_plan_hash.as_bytes());
    for command in commands {
        h.update(command.command_id.as_bytes());
        h.update(command.order.to_le_bytes());
        h.update(command.queue_lane.as_bytes());
    }
    hex32(h.finalize().into())
}

fn frame_submission_presentable_frame_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    render_target_state_hash: &str,
    command_buffer_hash: &str,
    frame_schedule_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.frame_submission.presentable_frame.v1\0");
    h.update(texture_bridge_contract.frame_hash.as_bytes());
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(render_target_state_hash.as_bytes());
    h.update(command_buffer_hash.as_bytes());
    h.update(frame_schedule_hash.as_bytes());
    hex32(h.finalize().into())
}

fn frame_submission_packet_hash(
    prepared: &MonsterPreparedCompute,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    raster_work_queue: &BangerNativeRasterWorkQueue,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    page_residency_allocator: &BangerNativePageResidencyAllocatorPacket,
    temporal_history_packet: &BangerNativeTemporalHistoryPacket,
    color_target_hash: &str,
    depth_target_hash: &str,
    render_target_state_hash: &str,
    command_buffer_hash: &str,
    frame_schedule_hash: &str,
    presentable_frame_hash: &str,
    commands: &[BangerNativeFrameCommandPacket],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_frame_submission_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(raster_work_queue.queue_hash.as_bytes());
    h.update(lumen_lighting_packet.packet_hash.as_bytes());
    h.update(virtual_shadow_packet.packet_hash.as_bytes());
    h.update(direct_lighting_packet.packet_hash.as_bytes());
    h.update(material_closure_packet.packet_hash.as_bytes());
    h.update(material_closure_packet.closure_stack_hash.as_bytes());
    h.update(material_closure_packet.resolve_hash.as_bytes());
    h.update(page_residency_allocator.packet_hash.as_bytes());
    h.update(page_residency_allocator.allocation_hash.as_bytes());
    h.update(page_residency_allocator.feedback_request_hash.as_bytes());
    h.update(temporal_history_packet.packet_hash.as_bytes());
    h.update(temporal_history_packet.motion_vector_hash.as_bytes());
    h.update(temporal_history_packet.accumulation_hash.as_bytes());
    h.update(color_target_hash.as_bytes());
    h.update(depth_target_hash.as_bytes());
    h.update(render_target_state_hash.as_bytes());
    h.update(command_buffer_hash.as_bytes());
    h.update(frame_schedule_hash.as_bytes());
    h.update(presentable_frame_hash.as_bytes());
    for command in commands {
        h.update(command.command_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn rhi_timeline_base_value(frame_submission_packet: &BangerNativeFrameSubmissionPacket) -> u64 {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.timeline_base.v1\0");
    h.update(frame_submission_packet.submission_hash.as_bytes());
    h.update(frame_submission_packet.command_buffer_hash.as_bytes());
    let digest: [u8; 32] = h.finalize().into();
    u64::from_le_bytes(digest[0..8].try_into().expect("timeline base bytes")) & 0x0000_FFFF_FFFF_FFFF
}

fn rhi_acquire_backbuffer_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    timeline_value: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.acquire_backbuffer.v1\0");
    h.update(texture_bridge_contract.device_queue_hash.as_bytes());
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(frame_submission_packet.color_target_hash.as_bytes());
    h.update(frame_submission_packet.depth_target_hash.as_bytes());
    h.update(timeline_value.to_le_bytes());
    hex32(h.finalize().into())
}

fn rhi_wait_hash(queue_lane: &str, dependency_hash: &str, timeline_value: u64) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.wait.v1\0");
    h.update(queue_lane.as_bytes());
    h.update(dependency_hash.as_bytes());
    h.update(timeline_value.to_le_bytes());
    hex32(h.finalize().into())
}

fn rhi_signal_hash(queue_lane: &str, payload_hash: &str, timeline_value: u64) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.signal.v1\0");
    h.update(queue_lane.as_bytes());
    h.update(payload_hash.as_bytes());
    h.update(timeline_value.to_le_bytes());
    hex32(h.finalize().into())
}

fn rhi_submit_step_hash(
    step_id: &str,
    order: u32,
    phase: &str,
    queue_lane: &str,
    command_hash: Option<&str>,
    wait_hash: &str,
    signal_hash: &str,
    timeline_value: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.submit_step.v1\0");
    h.update(step_id.as_bytes());
    h.update(order.to_le_bytes());
    h.update(phase.as_bytes());
    h.update(queue_lane.as_bytes());
    if let Some(command_hash) = command_hash {
        h.update(command_hash.as_bytes());
    }
    h.update(wait_hash.as_bytes());
    h.update(signal_hash.as_bytes());
    h.update(timeline_value.to_le_bytes());
    hex32(h.finalize().into())
}

fn rhi_finalized_command_lists_hash(steps: &[BangerNativeRhiSubmitStep]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.finalized_command_lists.v1\0");
    for step in steps
        .iter()
        .filter(|step| step.phase == "finalize_command_list")
    {
        h.update(step.step_hash.as_bytes());
        if let Some(command_hash) = &step.command_hash {
            h.update(command_hash.as_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn rhi_submit_batch_hash(
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    finalized_command_lists_hash: &str,
    timeline_value: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.submit_batch.v1\0");
    h.update(frame_submission_packet.command_buffer_hash.as_bytes());
    h.update(frame_submission_packet.frame_schedule_hash.as_bytes());
    h.update(finalized_command_lists_hash.as_bytes());
    h.update(timeline_value.to_le_bytes());
    hex32(h.finalize().into())
}

fn rhi_present_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    submit_batch_hash: &str,
    timeline_value: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.present.v1\0");
    h.update(texture_bridge_contract.present_policy.as_bytes());
    h.update(texture_bridge_contract.import_route.as_bytes());
    h.update(texture_bridge_contract.fallback_route.as_bytes());
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(submit_batch_hash.as_bytes());
    h.update(timeline_value.to_le_bytes());
    hex32(h.finalize().into())
}

fn rhi_fence_timeline_hash(steps: &[BangerNativeRhiSubmitStep]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.rhi.fence_timeline.v1\0");
    for step in steps {
        h.update(step.wait_hash.as_bytes());
        h.update(step.signal_hash.as_bytes());
        h.update(step.timeline_value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn rhi_submit_packet_hash(
    prepared: &MonsterPreparedCompute,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    acquire_backbuffer_hash: &str,
    finalized_command_lists_hash: &str,
    submit_batch_hash: &str,
    present_hash: &str,
    fence_timeline_hash: &str,
    steps: &[BangerNativeRhiSubmitStep],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_rhi_submit_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(frame_submission_packet.submission_hash.as_bytes());
    h.update(acquire_backbuffer_hash.as_bytes());
    h.update(finalized_command_lists_hash.as_bytes());
    h.update(submit_batch_hash.as_bytes());
    h.update(present_hash.as_bytes());
    h.update(fence_timeline_hash.as_bytes());
    for step in steps {
        h.update(step.step_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_execution_phase_diagnostic_hash(
    step: &BangerNativeRhiSubmitStep,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    raster_work_queue: &BangerNativeRasterWorkQueue,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_execution.phase_diagnostic.v1\0");
    h.update(step.step_hash.as_bytes());
    h.update(step.phase.as_bytes());
    h.update(step.queue_lane.as_bytes());
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(rhi_submit_packet.fence_timeline_hash.as_bytes());
    h.update(raster_work_queue.queue_hash.as_bytes());
    h.update(raster_work_queue.total_index_count.to_le_bytes());
    hex32(h.finalize().into())
}

fn gpu_execution_phase_hash(
    step: &BangerNativeRhiSubmitStep,
    diagnostic_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_execution.phase_receipt.v1\0");
    h.update(step.step_hash.as_bytes());
    h.update(step.timeline_value.to_le_bytes());
    h.update(step.signal_hash.as_bytes());
    h.update(diagnostic_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gpu_execution_frame_diagnostic_hash(
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    raster_work_queue: &BangerNativeRasterWorkQueue,
    nonblank_frame_expected: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_execution.frame_diagnostic.v1\0");
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(frame_submission_packet.color_target_hash.as_bytes());
    h.update(frame_submission_packet.depth_target_hash.as_bytes());
    h.update(rhi_submit_packet.present_hash.as_bytes());
    h.update(raster_work_queue.dispatch_plan_hash.as_bytes());
    h.update(raster_work_queue.total_threadgroup_count.to_le_bytes());
    h.update([nonblank_frame_expected as u8]);
    hex32(h.finalize().into())
}

fn gpu_execution_queue_timeline_hash(
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    phases: &[BangerNativeGpuExecutionPhaseReceipt],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_execution.queue_timeline.v1\0");
    h.update(rhi_submit_packet.fence_timeline_hash.as_bytes());
    h.update(rhi_submit_packet.timeline_base_value.to_le_bytes());
    for phase in phases {
        h.update(phase.phase_hash.as_bytes());
        h.update(phase.timeline_value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_execution_readback_policy_hash(
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    nonblank_frame_expected: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_execution.readback_policy.v1\0");
    h.update(b"optional_copy_src_rgba8_nonblank_probe_after_present\0");
    h.update(frame_submission_packet.command_buffer_hash.as_bytes());
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(rhi_submit_packet.present_hash.as_bytes());
    h.update([nonblank_frame_expected as u8]);
    hex32(h.finalize().into())
}

fn gpu_execution_receipt_hash(
    prepared: &MonsterPreparedCompute,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    frame_diagnostic_hash: &str,
    queue_timeline_hash: &str,
    readback_policy_hash: &str,
    phases: &[BangerNativeGpuExecutionPhaseReceipt],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_gpu_execution_receipt.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(frame_submission_packet.submission_hash.as_bytes());
    h.update(rhi_submit_packet.packet_hash.as_bytes());
    h.update(frame_diagnostic_hash.as_bytes());
    h.update(queue_timeline_hash.as_bytes());
    h.update(readback_policy_hash.as_bytes());
    for phase in phases {
        h.update(phase.phase_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_submit_family(backend: &str) -> &'static str {
    let backend = backend.to_ascii_lowercase();
    if backend.contains("dx12") || backend.contains("d3d12") {
        "d3d12"
    } else if backend.contains("vulkan") {
        "vulkan"
    } else if backend.contains("metal") {
        "metal"
    } else {
        "wgpu_generic"
    }
}

fn backend_swapchain_image_count(backend_family: &str) -> u32 {
    match backend_family {
        "vulkan" => 3,
        "d3d12" | "metal" => 2,
        _ => 2,
    }
}

fn backend_descriptor_table_count(
    backend_family: &str,
    command_count: usize,
    queue_count: usize,
) -> u32 {
    let base = match backend_family {
        "d3d12" => 3,
        "vulkan" => 2,
        "metal" => 2,
        _ => 1,
    };
    base + command_count.max(1) as u32 + queue_count.max(1) as u32
}

fn backend_pipeline_state_count(command_count: usize, backend_family: &str) -> u32 {
    let backend_bonus = if backend_family == "d3d12" || backend_family == "vulkan" {
        2
    } else {
        1
    };
    command_count.max(1) as u32 + backend_bonus
}

fn backend_barrier_batch_count(step_count: usize, backend_family: &str) -> u32 {
    let divisor = if backend_family == "vulkan" { 2 } else { 3 };
    (step_count.max(1) as u32).div_ceil(divisor)
}

fn backend_command_allocator_count(command_list_count: usize, backend_family: &str) -> u32 {
    let frame_overlap = if backend_family == "vulkan" { 3 } else { 2 };
    command_list_count.max(1) as u32 * frame_overlap
}

fn backend_present_path(backend_family: &str) -> &'static str {
    match backend_family {
        "d3d12" => "dxgi_swapchain_present",
        "vulkan" => "vk_queue_present",
        "metal" => "metal_drawable_present",
        _ => "wgpu_surface_present",
    }
}

fn backend_submit_target_hash(
    target_id: &str,
    backend_family: &str,
    queue_lane: &str,
    swapchain_image_count: u32,
    descriptor_table_count: u32,
    pipeline_state_count: u32,
    barrier_batch_count: u32,
    command_allocator_count: u32,
    present_path: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_submit.target.v1\0");
    h.update(target_id.as_bytes());
    h.update(backend_family.as_bytes());
    h.update(queue_lane.as_bytes());
    h.update(swapchain_image_count.to_le_bytes());
    h.update(descriptor_table_count.to_le_bytes());
    h.update(pipeline_state_count.to_le_bytes());
    h.update(barrier_batch_count.to_le_bytes());
    h.update(command_allocator_count.to_le_bytes());
    h.update(present_path.as_bytes());
    hex32(h.finalize().into())
}

fn backend_swapchain_contract_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    targets: &[BangerNativeBackendSubmitTarget],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_submit.swapchain_contract.v1\0");
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(texture_bridge_contract.device_queue_hash.as_bytes());
    h.update(texture_bridge_contract.width.to_le_bytes());
    h.update(texture_bridge_contract.height.to_le_bytes());
    h.update(texture_bridge_contract.pixel_format.as_bytes());
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    for target in targets {
        h.update(target.target_hash.as_bytes());
        h.update(target.present_path.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_descriptor_heap_hash(
    backend_family: &str,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    targets: &[BangerNativeBackendSubmitTarget],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_submit.descriptor_heap.v1\0");
    h.update(backend_family.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(frame_submission_packet.command_buffer_hash.as_bytes());
    for command in &frame_submission_packet.commands {
        h.update(command.input_hash.as_bytes());
        h.update(command.output_target_hash.as_bytes());
    }
    for target in targets {
        h.update(target.descriptor_table_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_pipeline_state_cache_hash(
    backend_family: &str,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    targets: &[BangerNativeBackendSubmitTarget],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_submit.pipeline_state_cache.v1\0");
    h.update(backend_family.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(pipeline_cache_manifest.driver_hash.as_bytes());
    h.update(pipeline_cache_manifest.driver_info_hash.as_bytes());
    h.update(frame_submission_packet.frame_schedule_hash.as_bytes());
    for entry in &pipeline_cache_manifest.entries {
        h.update(entry.pipeline_cache_key.as_bytes());
        h.update(entry.blob_hash.as_bytes());
        h.update(entry.render_pass_abi_hash.as_bytes());
    }
    for target in targets {
        h.update(target.pipeline_state_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_barrier_plan_hash(
    backend_family: &str,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    targets: &[BangerNativeBackendSubmitTarget],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_submit.barrier_plan.v1\0");
    h.update(backend_family.as_bytes());
    h.update(rhi_submit_packet.fence_timeline_hash.as_bytes());
    h.update(gpu_execution_receipt.queue_timeline_hash.as_bytes());
    for target in targets {
        h.update(target.barrier_batch_count.to_le_bytes());
        h.update(target.target_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_command_allocator_hash(
    backend_family: &str,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    targets: &[BangerNativeBackendSubmitTarget],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.backend_submit.command_allocator.v1\0");
    h.update(backend_family.as_bytes());
    h.update(rhi_submit_packet.finalized_command_lists_hash.as_bytes());
    h.update(rhi_submit_packet.submit_batch_hash.as_bytes());
    for target in targets {
        h.update(target.command_allocator_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn backend_submit_plan_hash(
    prepared: &MonsterPreparedCompute,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    backend_family: &str,
    swapchain_contract_hash: &str,
    descriptor_heap_hash: &str,
    pipeline_state_cache_hash: &str,
    backend_barrier_plan_hash: &str,
    command_allocator_hash: &str,
    targets: &[BangerNativeBackendSubmitTarget],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_backend_submit_plan.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(frame_submission_packet.submission_hash.as_bytes());
    h.update(rhi_submit_packet.packet_hash.as_bytes());
    h.update(gpu_execution_receipt.receipt_hash.as_bytes());
    h.update(backend_family.as_bytes());
    h.update(swapchain_contract_hash.as_bytes());
    h.update(descriptor_heap_hash.as_bytes());
    h.update(pipeline_state_cache_hash.as_bytes());
    h.update(backend_barrier_plan_hash.as_bytes());
    h.update(command_allocator_hash.as_bytes());
    for target in targets {
        h.update(target.target_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn culling_cache_reuse_key(
    prepared: &MonsterPreparedCompute,
    node: &BangerNativeSceneSubmissionNode,
    resource_table: &BangerNativeResourceTable,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.cache_reuse_key.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(node.object_id.as_bytes());
    h.update(node.proof_hash.as_bytes());
    for slot in &node.resource_slots {
        h.update(slot.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn culling_entry_visibility_hash(
    node: &BangerNativeSceneSubmissionNode,
    visible_after_cull: bool,
    lod_error: f32,
    lod_bucket: u32,
    cone_apex: &[f32; 3],
    cone_axis: &[f32; 3],
    cone_cutoff: f32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.entry_visibility.v1\0");
    h.update(node.object_id.as_bytes());
    h.update(node.proof_hash.as_bytes());
    h.update([visible_after_cull as u8]);
    h.update(lod_error.to_le_bytes());
    h.update(lod_bucket.to_le_bytes());
    for value in cone_apex.iter().chain(cone_axis.iter()) {
        h.update(value.to_le_bytes());
    }
    h.update(cone_cutoff.to_le_bytes());
    hex32(h.finalize().into())
}

fn culling_entry_indirect_draw_hash(
    node: &BangerNativeSceneSubmissionNode,
    indirect_draw_args: &[u32; 5],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.entry_indirect_draw.v1\0");
    h.update(node.object_id.as_bytes());
    for arg in indirect_draw_args {
        h.update(arg.to_le_bytes());
    }
    for stage in &node.render_graph_stages {
        h.update(stage.as_bytes());
    }
    hex32(h.finalize().into())
}

fn culling_entry_proof_hash(
    node: &BangerNativeSceneSubmissionNode,
    cache_reuse_key: &str,
    cache_hit_reuse: bool,
    visibility_result_hash: &str,
    indirect_draw_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.entry_proof.v1\0");
    h.update(node.proof_hash.as_bytes());
    h.update(cache_reuse_key.as_bytes());
    h.update([cache_hit_reuse as u8]);
    h.update(visibility_result_hash.as_bytes());
    h.update(indirect_draw_hash.as_bytes());
    hex32(h.finalize().into())
}

fn culling_visibility_result_hash(
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    entries: &[BangerNativeCullingEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.visibility_result.v1\0");
    h.update(scene_graph_submission.visibility_hash.as_bytes());
    h.update(scene_graph_submission.viewport_fit_hash.as_bytes());
    for entry in entries {
        h.update(entry.object_id.as_bytes());
        h.update([entry.visible_after_cull as u8]);
        h.update(entry.visibility_result_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn culling_indirect_draw_buffer_hash(entries: &[BangerNativeCullingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.indirect_draw_buffer.v1\0");
    for entry in entries.iter().filter(|entry| entry.visible_after_cull) {
        h.update(entry.object_id.as_bytes());
        h.update(entry.indirect_draw_hash.as_bytes());
        for arg in entry.indirect_draw_args {
            h.update(arg.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn culling_cache_reuse_hash(
    prepared: &MonsterPreparedCompute,
    entries: &[BangerNativeCullingEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.culling.cache_reuse.v1\0");
    h.update([prepared.is_fully_cached() as u8]);
    for entry in entries {
        h.update(entry.cache_reuse_key.as_bytes());
        h.update([entry.cache_hit_reuse as u8]);
    }
    hex32(h.finalize().into())
}

fn culling_manifest_hash(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    visibility_result_hash: &str,
    indirect_draw_buffer_hash: &str,
    cache_reuse_hash: &str,
    entries: &[BangerNativeCullingEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_culling_manifest.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(visibility_result_hash.as_bytes());
    h.update(indirect_draw_buffer_hash.as_bytes());
    h.update(cache_reuse_hash.as_bytes());
    for entry in entries {
        h.update(entry.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_epoch_from_hash(hash: &str) -> u64 {
    let mut bytes = [0u8; 8];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *slot = hash
            .get(start..start + 2)
            .and_then(|part| u8::from_str_radix(part, 16).ok())
            .unwrap_or(0);
    }
    u64::from_le_bytes(bytes)
}

fn radiance_light_budget(
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
) -> u32 {
    let visible = scene_graph_submission.visible_object_count.max(1) as u32;
    let cull_visible = culling_manifest.visible_count.max(1) as u32;
    (visible.saturating_mul(2).saturating_add(cull_visible)).clamp(4, 256)
}

fn radiance_update_priority(
    slot: &BangerNativeResourceSlot,
    culling_manifest: &BangerNativeCullingManifest,
) -> u32 {
    let lod_pressure = culling_manifest.max_lod_error.ceil().max(0.0) as u32;
    let page_pressure = (slot.page_index as u32).min(16);
    1 + lod_pressure + page_pressure
}

fn radiance_probe_invalidation_hash(
    prepared: &MonsterPreparedCompute,
    slot: &BangerNativeResourceSlot,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    temporal_epoch: u64,
    light_budget: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.radiance_probe.invalidation.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(slot.resource_key.as_bytes());
    h.update(slot.page_hash.as_bytes());
    h.update(scene_graph_submission.viewport_fit_hash.as_bytes());
    h.update(culling_manifest.visibility_result_hash.as_bytes());
    h.update(temporal_epoch.to_le_bytes());
    h.update(light_budget.to_le_bytes());
    hex32(h.finalize().into())
}

fn radiance_probe_page_proof_hash(
    slot: &BangerNativeResourceSlot,
    probe_count: u32,
    temporal_reuse_frames: u32,
    light_budget: u32,
    update_priority: u32,
    invalidation_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.radiance_probe.page_proof.v1\0");
    h.update(slot.resource_key.as_bytes());
    h.update(slot.page_hash.as_bytes());
    h.update(probe_count.to_le_bytes());
    h.update(temporal_reuse_frames.to_le_bytes());
    h.update(light_budget.to_le_bytes());
    h.update(update_priority.to_le_bytes());
    h.update(invalidation_hash.as_bytes());
    hex32(h.finalize().into())
}

fn radiance_manifest_invalidation_hash(
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    entries: &[BangerNativeRadianceProbePage],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.radiance_schedule.invalidation.v1\0");
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(culling_manifest.manifest_hash.as_bytes());
    for entry in entries {
        h.update(entry.invalidation_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn radiance_schedule_hash(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    temporal_epoch: u64,
    light_budget: u32,
    invalidation_hash: &str,
    entries: &[BangerNativeRadianceProbePage],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_radiance_schedule_manifest.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(culling_manifest.manifest_hash.as_bytes());
    h.update(temporal_epoch.to_le_bytes());
    h.update(light_budget.to_le_bytes());
    h.update(invalidation_hash.as_bytes());
    for entry in entries {
        h.update(entry.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn lumen_trace_policy(entry: &BangerNativeNaniteSecondLayerEntry) -> &'static str {
    if entry.residency_state == "resident_page" && (entry.material_flags_word & 0b100) != 0 {
        "hardware_ray_traced_surface_cache"
    } else if (entry.material_flags_word & 0b1) != 0 {
        "software_sdf_probe_trace"
    } else {
        "screen_probe_then_surface_cache"
    }
}

fn lumen_screen_probe_coord(entry: &BangerNativeNaniteSecondLayerEntry, salt: u32) -> [u32; 2] {
    [
        ((entry.feedback_word & 0xffff) as u32).wrapping_add(salt * 7) % 2048,
        (((entry.feedback_word >> 16) & 0xffff) as u32).wrapping_add(salt * 13) % 2048,
    ]
}

fn lumen_radiance_tile(
    entry: &BangerNativeNaniteSecondLayerEntry,
    probe_page: Option<&BangerNativeRadianceProbePage>,
    salt: u32,
) -> [u32; 4] {
    let probe_count = probe_page.map(|page| page.probe_count.max(1)).unwrap_or(1);
    let extent = 8 + (entry.requested_lod_bucket.min(4) * 4);
    [
        entry.visibility_tile[0].wrapping_add(salt * 5) % 4096,
        entry.visibility_tile[1].wrapping_add(probe_count) % 4096,
        extent,
        extent,
    ]
}

fn lumen_diffuse_ray_count(
    entry: &BangerNativeNaniteSecondLayerEntry,
    probe_page: Option<&BangerNativeRadianceProbePage>,
    fallback_light_budget: u32,
) -> u32 {
    let budget = probe_page
        .map(|page| page.light_budget)
        .unwrap_or(fallback_light_budget.max(1));
    let lod_scale = 1 + entry.requested_lod_bucket.min(4);
    budget.saturating_mul(lod_scale).clamp(1, 4096)
}

fn lumen_reflection_ray_count(
    entry: &BangerNativeNaniteSecondLayerEntry,
    trace_policy: &str,
) -> u32 {
    let base = if trace_policy == "hardware_ray_traced_surface_cache" {
        4
    } else {
        1
    };
    (base + (entry.material_bin_id % 3)).clamp(1, 64)
}

fn lumen_surface_cache_entry_hash(
    entry: &BangerNativeNaniteSecondLayerEntry,
    surface_page_id: &str,
    source_probe_page_id: &str,
    radiance_tile: &[u32; 4],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.surface_cache_entry.v1\0");
    h.update(entry.entry_hash.as_bytes());
    h.update(surface_page_id.as_bytes());
    h.update(source_probe_page_id.as_bytes());
    for value in radiance_tile {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn lumen_screen_probe_entry_hash(
    entry: &BangerNativeNaniteSecondLayerEntry,
    screen_probe_coord: &[u32; 2],
    diffuse_ray_count: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.screen_probe_entry.v1\0");
    h.update(entry.cluster_id.as_bytes());
    h.update(entry.feedback_word.to_le_bytes());
    for value in screen_probe_coord {
        h.update(value.to_le_bytes());
    }
    h.update(diffuse_ray_count.to_le_bytes());
    hex32(h.finalize().into())
}

fn lumen_trace_entry_hash(
    entry: &BangerNativeNaniteSecondLayerEntry,
    trace_policy: &str,
    diffuse_ray_count: u32,
    reflection_ray_count: u32,
    surface_cache_hash: &str,
    screen_probe_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.trace_entry.v1\0");
    h.update(entry.ray_tracing_proxy_hash.as_bytes());
    h.update(trace_policy.as_bytes());
    h.update(diffuse_ray_count.to_le_bytes());
    h.update(reflection_ray_count.to_le_bytes());
    h.update(surface_cache_hash.as_bytes());
    h.update(screen_probe_hash.as_bytes());
    hex32(h.finalize().into())
}

fn lumen_lighting_entry_hash(
    entry: &BangerNativeNaniteSecondLayerEntry,
    surface_page_id: &str,
    source_probe_page_id: &str,
    trace_policy: &str,
    diffuse_ray_count: u32,
    reflection_ray_count: u32,
    temporal_reuse_frames: u32,
    surface_cache_hash: &str,
    screen_probe_hash: &str,
    trace_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.lighting_entry.v1\0");
    h.update(entry.entry_hash.as_bytes());
    h.update(surface_page_id.as_bytes());
    h.update(source_probe_page_id.as_bytes());
    h.update(trace_policy.as_bytes());
    h.update(diffuse_ray_count.to_le_bytes());
    h.update(reflection_ray_count.to_le_bytes());
    h.update(temporal_reuse_frames.to_le_bytes());
    h.update(surface_cache_hash.as_bytes());
    h.update(screen_probe_hash.as_bytes());
    h.update(trace_hash.as_bytes());
    hex32(h.finalize().into())
}

fn lumen_surface_cache_hash(entries: &[BangerNativeLumenLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.surface_cache.v1\0");
    for entry in entries {
        h.update(entry.surface_page_id.as_bytes());
        h.update(entry.surface_cache_hash.as_bytes());
        h.update(entry.residency_state.as_bytes());
    }
    hex32(h.finalize().into())
}

fn lumen_screen_probe_hash(entries: &[BangerNativeLumenLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.screen_probe.v1\0");
    for entry in entries {
        h.update(entry.screen_probe_hash.as_bytes());
        for value in entry.screen_probe_coord {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn lumen_trace_policy_hash(entries: &[BangerNativeLumenLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.trace_policy.v1\0");
    for entry in entries {
        h.update(entry.trace_policy.as_bytes());
        h.update(entry.trace_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn lumen_diffuse_indirect_hash(entries: &[BangerNativeLumenLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.diffuse_indirect.v1\0");
    for entry in entries {
        h.update(entry.source_probe_page_id.as_bytes());
        h.update(entry.diffuse_ray_count.to_le_bytes());
        h.update(entry.temporal_reuse_frames.to_le_bytes());
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn lumen_reflection_hash(entries: &[BangerNativeLumenLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen.reflection.v1\0");
    for entry in entries {
        h.update(entry.trace_hash.as_bytes());
        h.update(entry.reflection_ray_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn lumen_lighting_packet_hash(
    prepared: &MonsterPreparedCompute,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    surface_cache_hash: &str,
    screen_probe_hash: &str,
    trace_policy_hash: &str,
    diffuse_indirect_hash: &str,
    reflection_hash: &str,
    entries: &[BangerNativeLumenLightingEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.lumen_lighting_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(gpu_scene_packet.packet_hash.as_bytes());
    h.update(nanite_second_layer_packet.packet_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(surface_cache_hash.as_bytes());
    h.update(screen_probe_hash.as_bytes());
    h.update(trace_policy_hash.as_bytes());
    h.update(diffuse_indirect_hash.as_bytes());
    h.update(reflection_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn virtual_shadow_map_id(entry: &BangerNativeLumenLightingEntry, salt: u32) -> u32 {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.map_id.v1\0");
    h.update(entry.entry_hash.as_bytes());
    h.update(entry.source_probe_page_id.as_bytes());
    h.update(salt.to_le_bytes());
    let digest: [u8; 32] = h.finalize().into();
    u32::from_le_bytes(digest[0..4].try_into().expect("virtual shadow map id bytes")) % 8192
}

fn virtual_shadow_clipmap_level(entry: &BangerNativeLumenLightingEntry) -> u32 {
    let ray_pressure = (entry.diffuse_ray_count / 64).min(4);
    let material_pressure = entry.material_bin_id % 3;
    (ray_pressure + material_pressure).min(6)
}

fn virtual_shadow_page_coord(
    entry: &BangerNativeLumenLightingEntry,
    virtual_map_id: u32,
    clipmap_level: u32,
) -> [u32; 3] {
    [
        entry.radiance_tile[0].wrapping_add(virtual_map_id) % 4096,
        entry.radiance_tile[1].wrapping_add(clipmap_level * 19) % 4096,
        clipmap_level,
    ]
}

fn virtual_shadow_resolution(clipmap_level: u32, entry: &BangerNativeLumenLightingEntry) -> u32 {
    let base = if entry.trace_policy == "hardware_ray_traced_surface_cache" {
        256
    } else {
        128
    };
    (base >> clipmap_level.min(2)).max(64)
}

fn virtual_shadow_cache_state(
    lumen_entry: &BangerNativeLumenLightingEntry,
    nanite_entry: Option<&BangerNativeNaniteSecondLayerEntry>,
) -> &'static str {
    if lumen_entry.residency_state != "resident_page" {
        "page_mark_required"
    } else if lumen_entry.temporal_reuse_frames > 1
        && nanite_entry
            .map(|entry| entry.residency_state == "resident_page")
            .unwrap_or(false)
    {
        "persistent_cache_hit"
    } else {
        "cache_warm"
    }
}

fn virtual_shadow_invalidation_reason(
    lumen_entry: &BangerNativeLumenLightingEntry,
    cache_state: &str,
) -> &'static str {
    if cache_state == "page_mark_required" {
        "geometry_page_streaming"
    } else if lumen_entry.temporal_reuse_frames <= 1 {
        "temporal_epoch_new"
    } else if lumen_entry.diffuse_ray_count > 1024 {
        "light_budget_pressure"
    } else {
        "stable_cache_reuse"
    }
}

fn virtual_shadow_light_grid_cell(
    entry: &BangerNativeLumenLightingEntry,
    salt: u32,
) -> [u32; 3] {
    [
        entry.screen_probe_coord[0].wrapping_add(salt * 3) / 64,
        entry.screen_probe_coord[1].wrapping_add(salt * 5) / 64,
        (entry.reflection_ray_count + entry.diffuse_ray_count / 64).min(63),
    ]
}

fn virtual_shadow_projection_tile(
    entry: &BangerNativeLumenLightingEntry,
    page_coord: &[u32; 3],
    resolution: u32,
) -> [u32; 4] {
    [
        page_coord[0].wrapping_add(entry.radiance_tile[0]) % 4096,
        page_coord[1].wrapping_add(entry.radiance_tile[1]) % 4096,
        resolution,
        resolution,
    ]
}

fn virtual_shadow_ray_budget(
    entry: &BangerNativeLumenLightingEntry,
    fallback_light_budget: u32,
) -> u32 {
    let base = entry.diffuse_ray_count.saturating_add(entry.reflection_ray_count);
    base.saturating_add(fallback_light_budget).clamp(1, 8192)
}

fn virtual_shadow_light_id(entry: &BangerNativeLumenLightingEntry, virtual_map_id: u32) -> String {
    hash_text_hex(
        "forge.banger.virtual_shadow.light_id.v1",
        &format!(
            "{}:{}:{}",
            entry.source_probe_page_id, entry.material_bin_id, virtual_map_id
        ),
    )
}

fn virtual_shadow_page_table_entry_hash(
    entry: &BangerNativeLumenLightingEntry,
    shadow_page_id: &str,
    virtual_light_id: &str,
    page_coord: &[u32; 3],
    resolution: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.page_table_entry.v1\0");
    h.update(entry.entry_hash.as_bytes());
    h.update(shadow_page_id.as_bytes());
    h.update(virtual_light_id.as_bytes());
    for value in page_coord {
        h.update(value.to_le_bytes());
    }
    h.update(resolution.to_le_bytes());
    hex32(h.finalize().into())
}

fn virtual_shadow_cache_entry_hash(
    entry: &BangerNativeLumenLightingEntry,
    cache_state: &str,
    invalidation_reason: &str,
    page_table_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.cache_entry.v1\0");
    h.update(entry.surface_cache_hash.as_bytes());
    h.update(cache_state.as_bytes());
    h.update(invalidation_reason.as_bytes());
    h.update(page_table_hash.as_bytes());
    hex32(h.finalize().into())
}

fn virtual_shadow_projection_entry_hash(
    entry: &BangerNativeLumenLightingEntry,
    projection_tile: &[u32; 4],
    light_grid_cell: &[u32; 3],
    ray_budget: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.projection_entry.v1\0");
    h.update(entry.trace_hash.as_bytes());
    for value in projection_tile {
        h.update(value.to_le_bytes());
    }
    for value in light_grid_cell {
        h.update(value.to_le_bytes());
    }
    h.update(ray_budget.to_le_bytes());
    hex32(h.finalize().into())
}

fn virtual_shadow_entry_hash(
    entry: &BangerNativeLumenLightingEntry,
    shadow_page_id: &str,
    virtual_light_id: &str,
    virtual_map_id: u32,
    clipmap_level: u32,
    page_coord: &[u32; 3],
    resolution: u32,
    cache_state: &str,
    invalidation_reason: &str,
    light_grid_cell: &[u32; 3],
    projection_tile: &[u32; 4],
    ray_budget: u32,
    page_table_hash: &str,
    cache_hash: &str,
    projection_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.entry.v1\0");
    h.update(entry.entry_hash.as_bytes());
    h.update(shadow_page_id.as_bytes());
    h.update(virtual_light_id.as_bytes());
    h.update(virtual_map_id.to_le_bytes());
    h.update(clipmap_level.to_le_bytes());
    for value in page_coord {
        h.update(value.to_le_bytes());
    }
    h.update(resolution.to_le_bytes());
    h.update(cache_state.as_bytes());
    h.update(invalidation_reason.as_bytes());
    for value in light_grid_cell {
        h.update(value.to_le_bytes());
    }
    for value in projection_tile {
        h.update(value.to_le_bytes());
    }
    h.update(ray_budget.to_le_bytes());
    h.update(page_table_hash.as_bytes());
    h.update(cache_hash.as_bytes());
    h.update(projection_hash.as_bytes());
    hex32(h.finalize().into())
}

fn virtual_shadow_page_table_hash(entries: &[BangerNativeVirtualShadowEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.page_table.v1\0");
    for entry in entries {
        h.update(entry.shadow_page_id.as_bytes());
        h.update(entry.page_table_hash.as_bytes());
        h.update(entry.virtual_map_id.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn virtual_shadow_cache_hash(entries: &[BangerNativeVirtualShadowEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.cache.v1\0");
    for entry in entries {
        h.update(entry.cache_state.as_bytes());
        h.update(entry.cache_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn virtual_shadow_invalidation_hash(entries: &[BangerNativeVirtualShadowEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.invalidation.v1\0");
    for entry in entries {
        h.update(entry.invalidation_reason.as_bytes());
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn virtual_shadow_projection_hash(entries: &[BangerNativeVirtualShadowEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.projection.v1\0");
    for entry in entries {
        h.update(entry.projection_hash.as_bytes());
        for value in entry.projection_tile {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn virtual_shadow_light_grid_hash(entries: &[BangerNativeVirtualShadowEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow.light_grid.v1\0");
    for entry in entries {
        h.update(entry.virtual_light_id.as_bytes());
        for value in entry.light_grid_cell {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn virtual_shadow_packet_hash(
    prepared: &MonsterPreparedCompute,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    page_table_hash: &str,
    cache_hash: &str,
    invalidation_hash: &str,
    projection_hash: &str,
    light_grid_hash: &str,
    entries: &[BangerNativeVirtualShadowEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.virtual_shadow_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(nanite_second_layer_packet.packet_hash.as_bytes());
    h.update(lumen_lighting_packet.packet_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(page_table_hash.as_bytes());
    h.update(cache_hash.as_bytes());
    h.update(invalidation_hash.as_bytes());
    h.update(projection_hash.as_bytes());
    h.update(light_grid_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_kind(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
) -> &'static str {
    if lumen_entry
        .map(|entry| entry.trace_policy == "hardware_ray_traced_surface_cache")
        .unwrap_or(false)
    {
        "hardware_ray_megalight"
    } else if shadow_entry.ray_budget > 1024 {
        "stochastic_many_light"
    } else {
        "clustered_deferred_light"
    }
}

fn direct_lighting_cluster_id(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    light_kind: &str,
) -> String {
    hash_text_hex(
        "forge.banger.direct_lighting.cluster_id.v1",
        &format!(
            "{}:{}:{}:{}:{}",
            shadow_entry.virtual_light_id,
            light_kind,
            shadow_entry.light_grid_cell[0],
            shadow_entry.light_grid_cell[1],
            shadow_entry.light_grid_cell[2]
        ),
    )
}

fn direct_lighting_sample_count(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
    fallback_light_budget: u32,
) -> u32 {
    let lumen_bonus = lumen_entry
        .map(|entry| entry.reflection_ray_count + entry.diffuse_ray_count / 16)
        .unwrap_or(1);
    shadow_entry
        .ray_budget
        .saturating_add(lumen_bonus)
        .saturating_add(fallback_light_budget)
        .clamp(1, 16384)
}

fn direct_lighting_sample_sequence(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    light_cluster_id: &str,
    salt: u32,
) -> u64 {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.sample_sequence.v1\0");
    h.update(shadow_entry.entry_hash.as_bytes());
    h.update(light_cluster_id.as_bytes());
    h.update(salt.to_le_bytes());
    let digest: [u8; 32] = h.finalize().into();
    u64::from_le_bytes(digest[0..8].try_into().expect("direct lighting sample sequence bytes"))
}

fn direct_lighting_shadow_mask_hash(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
    sample_sequence: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.shadow_mask.v1\0");
    h.update(shadow_entry.page_table_hash.as_bytes());
    h.update(shadow_entry.projection_hash.as_bytes());
    h.update(sample_sequence.to_le_bytes());
    if let Some(lumen_entry) = lumen_entry {
        h.update(lumen_entry.trace_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_contribution_hash(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
    light_kind: &str,
    sample_count: u32,
    shadow_mask_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.contribution.v1\0");
    h.update(shadow_entry.entry_hash.as_bytes());
    h.update(light_kind.as_bytes());
    h.update(sample_count.to_le_bytes());
    h.update(shadow_mask_hash.as_bytes());
    if let Some(lumen_entry) = lumen_entry {
        h.update(lumen_entry.diffuse_ray_count.to_le_bytes());
        h.update(lumen_entry.reflection_ray_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_denoiser_tile(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    sample_count: u32,
) -> [u32; 4] {
    let extent = 8 + (sample_count / 256).min(8) * 4;
    [
        shadow_entry.projection_tile[0] / 2,
        shadow_entry.projection_tile[1] / 2,
        extent.max(8),
        extent.max(8),
    ]
}

fn direct_lighting_resolve_tile(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    denoiser_tile: &[u32; 4],
) -> [u32; 4] {
    [
        denoiser_tile[0].wrapping_add(shadow_entry.light_grid_cell[0]) % 4096,
        denoiser_tile[1].wrapping_add(shadow_entry.light_grid_cell[1]) % 4096,
        denoiser_tile[2],
        denoiser_tile[3],
    ]
}

fn direct_lighting_ray_tracing_candidate(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
) -> bool {
    shadow_entry.cache_state == "persistent_cache_hit"
        || lumen_entry
            .map(|entry| entry.trace_policy == "hardware_ray_traced_surface_cache")
            .unwrap_or(false)
}

fn direct_lighting_entry_hash(
    shadow_entry: &BangerNativeVirtualShadowEntry,
    light_cluster_id: &str,
    light_kind: &str,
    sample_sequence: u64,
    sample_count: u32,
    shadow_mask_hash: &str,
    contribution_hash: &str,
    denoiser_tile: &[u32; 4],
    resolve_tile: &[u32; 4],
    ray_tracing_candidate: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.entry.v1\0");
    h.update(shadow_entry.entry_hash.as_bytes());
    h.update(light_cluster_id.as_bytes());
    h.update(light_kind.as_bytes());
    h.update(sample_sequence.to_le_bytes());
    h.update(sample_count.to_le_bytes());
    h.update(shadow_mask_hash.as_bytes());
    h.update(contribution_hash.as_bytes());
    for value in denoiser_tile {
        h.update(value.to_le_bytes());
    }
    for value in resolve_tile {
        h.update(value.to_le_bytes());
    }
    h.update([ray_tracing_candidate as u8]);
    hex32(h.finalize().into())
}

fn direct_lighting_grid_hash(entries: &[BangerNativeDirectLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.light_grid.v1\0");
    for entry in entries {
        h.update(entry.light_cluster_id.as_bytes());
        for value in entry.light_grid_cell {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn direct_lighting_sample_sequence_hash(entries: &[BangerNativeDirectLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.sample_sequences.v1\0");
    for entry in entries {
        h.update(entry.sample_sequence.to_le_bytes());
        h.update(entry.sample_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_shadow_masks_hash(entries: &[BangerNativeDirectLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.shadow_masks.v1\0");
    for entry in entries {
        h.update(entry.shadow_page_id.as_bytes());
        h.update(entry.shadow_mask_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_denoiser_hash(entries: &[BangerNativeDirectLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.denoiser.v1\0");
    for entry in entries {
        for value in entry.denoiser_tile {
            h.update(value.to_le_bytes());
        }
        h.update(entry.contribution_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_resolve_hash(entries: &[BangerNativeDirectLightingEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting.resolve.v1\0");
    for entry in entries {
        for value in entry.resolve_tile {
            h.update(value.to_le_bytes());
        }
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn direct_lighting_packet_hash(
    prepared: &MonsterPreparedCompute,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    light_grid_hash: &str,
    sample_sequence_hash: &str,
    shadow_mask_hash: &str,
    denoiser_hash: &str,
    resolve_hash: &str,
    entries: &[BangerNativeDirectLightingEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.direct_lighting_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(lumen_lighting_packet.packet_hash.as_bytes());
    h.update(virtual_shadow_packet.packet_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(light_grid_hash.as_bytes());
    h.update(sample_sequence_hash.as_bytes());
    h.update(shadow_mask_hash.as_bytes());
    h.update(denoiser_hash.as_bytes());
    h.update(resolve_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn material_base_closure(material_bin_id: u32, light_kind: &str) -> &'static str {
    if light_kind == "hardware_ray_megalight" {
        "thin_surface_specular_diffuse"
    } else if material_bin_id % 5 == 0 {
        "subsurface_diffuse"
    } else if material_bin_id % 3 == 0 {
        "metallic_ggx"
    } else {
        "dielectric_ggx"
    }
}

fn material_coating_closure(
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
    shadow_entry: Option<&BangerNativeVirtualShadowEntry>,
) -> &'static str {
    if shadow_entry
        .map(|entry| entry.cache_state == "persistent_cache_hit")
        .unwrap_or(false)
    {
        "clearcoat_cached_shadow"
    } else if lumen_entry
        .map(|entry| entry.reflection_ray_count > entry.diffuse_ray_count / 2)
        .unwrap_or(false)
    {
        "anisotropic_reflection_lobe"
    } else {
        "matte_energy_preserving_lobe"
    }
}

fn material_layer_count(
    base_closure: &str,
    coating_closure: &str,
    direct_entry: &BangerNativeDirectLightingEntry,
) -> u32 {
    let base_layers = if base_closure == "subsurface_diffuse" { 2 } else { 1 };
    let coating_layers = if coating_closure == "matte_energy_preserving_lobe" { 1 } else { 2 };
    let ray_layers = if direct_entry.ray_tracing_candidate { 1 } else { 0 };
    (base_layers + coating_layers + ray_layers).clamp(1, 5)
}

fn material_texture_slot_base(material_abi: &BangerNativeShaderMaterialAbi, material_bin_id: u32) -> u32 {
    material_abi
        .texture_binding_base
        .saturating_add(material_bin_id % material_abi.max_texture_slots.max(1))
}

fn material_texture_slot_count(
    material_abi: &BangerNativeShaderMaterialAbi,
    layer_count: u32,
    direct_entry: &BangerNativeDirectLightingEntry,
) -> u32 {
    let ray_bonus = u32::from(direct_entry.ray_tracing_candidate);
    layer_count
        .saturating_add(ray_bonus)
        .clamp(1, material_abi.max_texture_slots.max(1))
}

fn material_roughness_quantized(
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
    direct_entry: &BangerNativeDirectLightingEntry,
) -> u16 {
    let rays = lumen_entry
        .map(|entry| entry.diffuse_ray_count.saturating_add(entry.reflection_ray_count))
        .unwrap_or(direct_entry.sample_count);
    (rays % 1024).saturating_mul(64).min(u16::MAX as u32) as u16
}

fn material_metallic_quantized(
    material_bin_id: u32,
    direct_entry: &BangerNativeDirectLightingEntry,
) -> u16 {
    let base = if direct_entry.light_kind == "hardware_ray_megalight" {
        384
    } else {
        material_bin_id % 512
    };
    base.saturating_mul(128).min(u16::MAX as u32) as u16
}

fn material_opacity_quantized(
    shadow_entry: Option<&BangerNativeVirtualShadowEntry>,
    direct_entry: &BangerNativeDirectLightingEntry,
) -> u16 {
    let occlusion = shadow_entry
        .map(|entry| entry.ray_budget / 128)
        .unwrap_or(direct_entry.sample_count / 128)
        .min(255);
    u16::MAX.saturating_sub((occlusion as u16).saturating_mul(64))
}

fn material_closure_stack_id(
    direct_entry: &BangerNativeDirectLightingEntry,
    material_bin_id: u32,
    base_closure: &str,
    coating_closure: &str,
    layer_count: u32,
) -> String {
    hash_text_hex(
        "forge.banger.material_closure.stack_id.v1",
        &format!(
            "{}:{}:{}:{}:{}:{}",
            direct_entry.cluster_id,
            direct_entry.light_cluster_id,
            material_bin_id,
            base_closure,
            coating_closure,
            layer_count
        ),
    )
}

fn material_closure_hash(
    direct_entry: &BangerNativeDirectLightingEntry,
    closure_stack_id: &str,
    material_bin_id: u32,
    base_closure: &str,
    coating_closure: &str,
    layer_count: u32,
    roughness_quantized: u16,
    metallic_quantized: u16,
    opacity_quantized: u16,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.closure.v1\0");
    h.update(direct_entry.entry_hash.as_bytes());
    h.update(closure_stack_id.as_bytes());
    h.update(material_bin_id.to_le_bytes());
    h.update(base_closure.as_bytes());
    h.update(coating_closure.as_bytes());
    h.update(layer_count.to_le_bytes());
    h.update(roughness_quantized.to_le_bytes());
    h.update(metallic_quantized.to_le_bytes());
    h.update(opacity_quantized.to_le_bytes());
    hex32(h.finalize().into())
}

fn material_bsdf_hash(
    closure_hash: &str,
    direct_entry: &BangerNativeDirectLightingEntry,
    lumen_entry: Option<&BangerNativeLumenLightingEntry>,
    roughness_quantized: u16,
    metallic_quantized: u16,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.bsdf.v1\0");
    h.update(closure_hash.as_bytes());
    h.update(direct_entry.contribution_hash.as_bytes());
    h.update(roughness_quantized.to_le_bytes());
    h.update(metallic_quantized.to_le_bytes());
    if let Some(lumen_entry) = lumen_entry {
        h.update(lumen_entry.surface_cache_hash.as_bytes());
        h.update(lumen_entry.trace_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn material_texture_hash(
    closure_hash: &str,
    material_abi: &BangerNativeShaderMaterialAbi,
    texture_slot_base: u32,
    texture_slot_count: u32,
    surface_cache_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.texture_table_entry.v1\0");
    h.update(closure_hash.as_bytes());
    h.update(material_abi.layout_hash.as_bytes());
    h.update(texture_slot_base.to_le_bytes());
    h.update(texture_slot_count.to_le_bytes());
    h.update(surface_cache_hash.as_bytes());
    hex32(h.finalize().into())
}

fn material_resolve_hash(
    bsdf_hash: &str,
    texture_hash: &str,
    direct_entry: &BangerNativeDirectLightingEntry,
    shadow_entry: Option<&BangerNativeVirtualShadowEntry>,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.resolve.v1\0");
    h.update(bsdf_hash.as_bytes());
    h.update(texture_hash.as_bytes());
    h.update(direct_entry.resolve_tile[0].to_le_bytes());
    h.update(direct_entry.resolve_tile[1].to_le_bytes());
    h.update(direct_entry.resolve_tile[2].to_le_bytes());
    h.update(direct_entry.resolve_tile[3].to_le_bytes());
    if let Some(shadow_entry) = shadow_entry {
        h.update(shadow_entry.projection_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn material_closure_entry_hash(
    direct_entry: &BangerNativeDirectLightingEntry,
    closure_stack_id: &str,
    closure_hash: &str,
    bsdf_hash: &str,
    texture_hash: &str,
    resolve_hash: &str,
    texture_slot_base: u32,
    texture_slot_count: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.entry.v1\0");
    h.update(direct_entry.entry_hash.as_bytes());
    h.update(closure_stack_id.as_bytes());
    h.update(closure_hash.as_bytes());
    h.update(bsdf_hash.as_bytes());
    h.update(texture_hash.as_bytes());
    h.update(resolve_hash.as_bytes());
    h.update(texture_slot_base.to_le_bytes());
    h.update(texture_slot_count.to_le_bytes());
    hex32(h.finalize().into())
}

fn material_closure_stack_hash(entries: &[BangerNativeMaterialClosureEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.stack_table.v1\0");
    for entry in entries {
        h.update(entry.closure_stack_id.as_bytes());
        h.update(entry.closure_hash.as_bytes());
        h.update(entry.layer_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn material_bsdf_table_hash(entries: &[BangerNativeMaterialClosureEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.bsdf_table.v1\0");
    for entry in entries {
        h.update(entry.bsdf_hash.as_bytes());
        h.update(entry.roughness_quantized.to_le_bytes());
        h.update(entry.metallic_quantized.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn material_texture_table_hash(entries: &[BangerNativeMaterialClosureEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.texture_table.v1\0");
    for entry in entries {
        h.update(entry.texture_hash.as_bytes());
        h.update(entry.texture_slot_base.to_le_bytes());
        h.update(entry.texture_slot_count.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn material_closure_resolve_hash(entries: &[BangerNativeMaterialClosureEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure.resolve_table.v1\0");
    for entry in entries {
        h.update(entry.resolve_hash.as_bytes());
        h.update(entry.shadow_mask_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn material_closure_packet_hash(
    prepared: &MonsterPreparedCompute,
    material_abi: &BangerNativeShaderMaterialAbi,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    closure_stack_hash: &str,
    bsdf_table_hash: &str,
    texture_table_hash: &str,
    resolve_hash: &str,
    entries: &[BangerNativeMaterialClosureEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.material_closure_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(material_abi.layout_hash.as_bytes());
    h.update(lumen_lighting_packet.packet_hash.as_bytes());
    h.update(virtual_shadow_packet.packet_hash.as_bytes());
    h.update(direct_lighting_packet.packet_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(closure_stack_hash.as_bytes());
    h.update(bsdf_table_hash.as_bytes());
    h.update(texture_table_hash.as_bytes());
    h.update(resolve_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_jitter_index(temporal_epoch: u64, salt: u32) -> u32 {
    ((temporal_epoch as u32).wrapping_add(salt.wrapping_mul(3))) % 16
}

fn temporal_jitter_pixels(jitter_index: u32) -> [f32; 2] {
    const HALTON_2_3: [[f32; 2]; 16] = [
        [0.0, -0.166_666_66],
        [-0.25, 0.166_666_69],
        [0.25, -0.388_888_9],
        [-0.375, -0.055_555_55],
        [0.125, 0.277_777_8],
        [-0.125, -0.277_777_8],
        [0.375, 0.055_555_582],
        [-0.4375, 0.388_888_9],
        [0.0625, -0.462_962_96],
        [-0.1875, -0.129_629_61],
        [0.3125, 0.203_703_7],
        [-0.3125, -0.351_851_85],
        [0.1875, -0.018_518_507],
        [-0.0625, 0.314_814_8],
        [0.4375, -0.240_740_75],
        [-0.46875, 0.092_592_6],
    ];
    HALTON_2_3[(jitter_index as usize) % HALTON_2_3.len()]
}

fn temporal_history_kind(
    material_entry: &BangerNativeMaterialClosureEntry,
    direct_entry: Option<&BangerNativeDirectLightingEntry>,
) -> &'static str {
    if direct_entry
        .map(|entry| entry.ray_tracing_candidate)
        .unwrap_or(false)
        || material_entry.layer_count > 2
    {
        "async_tsr_history_update"
    } else if material_entry.opacity_quantized < 48_000 {
        "responsive_taa_history"
    } else {
        "standard_taa_history"
    }
}

fn temporal_motion_tile(
    material_entry: &BangerNativeMaterialClosureEntry,
    direct_entry: Option<&BangerNativeDirectLightingEntry>,
    salt: u32,
) -> [u32; 4] {
    let tile = direct_entry
        .map(|entry| entry.resolve_tile)
        .unwrap_or([salt * 16, salt * 16, 16, 16]);
    [
        tile[0].wrapping_add(material_entry.material_bin_id % 7) % 4096,
        tile[1].wrapping_add(salt * 5) % 4096,
        tile[2].max(8),
        tile[3].max(8),
    ]
}

fn temporal_history_tile(motion_tile: &[u32; 4], jitter: [f32; 2]) -> [u32; 4] {
    let jx = (jitter[0].abs() * 8.0) as u32;
    let jy = (jitter[1].abs() * 8.0) as u32;
    [
        motion_tile[0].saturating_sub(jx),
        motion_tile[1].saturating_sub(jy),
        motion_tile[2].saturating_add(jx).max(8),
        motion_tile[3].saturating_add(jy).max(8),
    ]
}

fn temporal_velocity_quantized(
    material_entry: &BangerNativeMaterialClosureEntry,
    direct_entry: Option<&BangerNativeDirectLightingEntry>,
    jitter: [f32; 2],
) -> [i16; 2] {
    let sample_count = direct_entry.map(|entry| entry.sample_count).unwrap_or(1);
    let vx = ((material_entry.roughness_quantized as i32 / 512)
        - (material_entry.metallic_quantized as i32 / 1024)
        + (jitter[0] * 256.0) as i32
        + (sample_count as i32 % 32))
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let vy = ((material_entry.opacity_quantized as i32 / 1024)
        - (material_entry.layer_count as i32 * 24)
        + (jitter[1] * 256.0) as i32)
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    [vx, vy]
}

fn temporal_disocclusion_score(
    material_entry: &BangerNativeMaterialClosureEntry,
    direct_entry: Option<&BangerNativeDirectLightingEntry>,
    velocity_quantized: &[i16; 2],
) -> u16 {
    let velocity_energy =
        velocity_quantized[0].unsigned_abs() as u32 + velocity_quantized[1].unsigned_abs() as u32;
    let sample_pressure = direct_entry.map(|entry| entry.sample_count / 64).unwrap_or(0);
    let opacity_pressure = (u16::MAX - material_entry.opacity_quantized) as u32 / 64;
    velocity_energy
        .saturating_add(sample_pressure)
        .saturating_add(opacity_pressure)
        .min(u16::MAX as u32) as u16
}

fn temporal_rejection_mode(
    disocclusion_score: u16,
    material_entry: &BangerNativeMaterialClosureEntry,
    direct_entry: Option<&BangerNativeDirectLightingEntry>,
) -> &'static str {
    if disocclusion_score > 4096 {
        "history_reject_disocclusion"
    } else if direct_entry
        .map(|entry| entry.ray_tracing_candidate)
        .unwrap_or(false)
        && material_entry.layer_count > 2
    {
        "history_resurrection_candidate"
    } else if material_entry.opacity_quantized < 48_000 {
        "history_clamp_responsive_material"
    } else {
        "history_accept"
    }
}

fn temporal_accumulation_weight_q15(
    rejection_mode: &str,
    material_entry: &BangerNativeMaterialClosureEntry,
    direct_entry: Option<&BangerNativeDirectLightingEntry>,
) -> u16 {
    let base: u16 = match rejection_mode {
        "history_reject_disocclusion" => 4096,
        "history_resurrection_candidate" => 16_384,
        "history_clamp_responsive_material" => 12_288,
        _ => 26_214,
    };
    let sample_bonus = direct_entry
        .map(|entry| (entry.sample_count / 256).min(4096) as u16)
        .unwrap_or(0);
    base.saturating_add(sample_bonus)
        .saturating_sub(material_entry.layer_count as u16 * 256)
        .clamp(1024, 32_767)
}

fn temporal_history_layer_id(
    material_entry: &BangerNativeMaterialClosureEntry,
    history_kind: &str,
    temporal_epoch: u64,
    jitter_index: u32,
) -> String {
    hash_text_hex(
        "forge.banger.temporal_history.layer_id.v1",
        &format!(
            "{}:{}:{}:{}:{}",
            material_entry.cluster_id,
            material_entry.closure_stack_id,
            history_kind,
            temporal_epoch,
            jitter_index
        ),
    )
}

fn temporal_motion_vector_hash(
    material_entry: &BangerNativeMaterialClosureEntry,
    history_layer_id: &str,
    motion_tile: &[u32; 4],
    velocity_quantized: &[i16; 2],
    jitter_index: u32,
    temporal_jitter_pixels: [f32; 2],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.motion_vector.v1\0");
    h.update(material_entry.entry_hash.as_bytes());
    h.update(history_layer_id.as_bytes());
    for value in motion_tile {
        h.update(value.to_le_bytes());
    }
    for value in velocity_quantized {
        h.update(value.to_le_bytes());
    }
    h.update(jitter_index.to_le_bytes());
    for value in temporal_jitter_pixels {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_history_reprojection_hash(
    material_entry: &BangerNativeMaterialClosureEntry,
    history_layer_id: &str,
    history_tile: &[u32; 4],
    motion_vector_hash: &str,
    direct_resolve_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.reprojection.v1\0");
    h.update(material_entry.resolve_hash.as_bytes());
    h.update(history_layer_id.as_bytes());
    h.update(motion_vector_hash.as_bytes());
    h.update(direct_resolve_hash.as_bytes());
    for value in history_tile {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_disocclusion_hash(
    material_entry: &BangerNativeMaterialClosureEntry,
    motion_vector_hash: &str,
    disocclusion_score: u16,
    rejection_mode: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.disocclusion.v1\0");
    h.update(material_entry.shadow_mask_hash.as_bytes());
    h.update(motion_vector_hash.as_bytes());
    h.update(disocclusion_score.to_le_bytes());
    h.update(rejection_mode.as_bytes());
    hex32(h.finalize().into())
}

fn temporal_accumulation_hash(
    material_entry: &BangerNativeMaterialClosureEntry,
    history_reprojection_hash: &str,
    disocclusion_hash: &str,
    accumulation_weight_q15: u16,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.accumulation.v1\0");
    h.update(material_entry.bsdf_hash.as_bytes());
    h.update(history_reprojection_hash.as_bytes());
    h.update(disocclusion_hash.as_bytes());
    h.update(accumulation_weight_q15.to_le_bytes());
    hex32(h.finalize().into())
}

fn temporal_history_entry_hash(
    material_entry: &BangerNativeMaterialClosureEntry,
    history_layer_id: &str,
    history_kind: &str,
    temporal_epoch: u64,
    jitter_index: u32,
    motion_vector_hash: &str,
    history_reprojection_hash: &str,
    disocclusion_hash: &str,
    accumulation_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.entry.v1\0");
    h.update(material_entry.entry_hash.as_bytes());
    h.update(history_layer_id.as_bytes());
    h.update(history_kind.as_bytes());
    h.update(temporal_epoch.to_le_bytes());
    h.update(jitter_index.to_le_bytes());
    h.update(motion_vector_hash.as_bytes());
    h.update(history_reprojection_hash.as_bytes());
    h.update(disocclusion_hash.as_bytes());
    h.update(accumulation_hash.as_bytes());
    hex32(h.finalize().into())
}

fn temporal_jitter_sequence_hash(
    temporal_epoch: u64,
    entries: &[BangerNativeTemporalHistoryEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.jitter_sequence.v1\0");
    h.update(temporal_epoch.to_le_bytes());
    for entry in entries {
        h.update(entry.jitter_index.to_le_bytes());
        for value in entry.temporal_jitter_pixels {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn temporal_motion_vector_table_hash(entries: &[BangerNativeTemporalHistoryEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.motion_vector_table.v1\0");
    for entry in entries {
        h.update(entry.motion_vector_hash.as_bytes());
        for value in entry.velocity_quantized {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn temporal_history_reprojection_table_hash(entries: &[BangerNativeTemporalHistoryEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.reprojection_table.v1\0");
    for entry in entries {
        h.update(entry.history_reprojection_hash.as_bytes());
        for value in entry.history_tile {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn temporal_disocclusion_mask_hash(entries: &[BangerNativeTemporalHistoryEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.disocclusion_mask.v1\0");
    for entry in entries {
        h.update(entry.disocclusion_hash.as_bytes());
        h.update(entry.disocclusion_score.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_rejection_hash(entries: &[BangerNativeTemporalHistoryEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.rejection.v1\0");
    for entry in entries {
        h.update(entry.rejection_mode.as_bytes());
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_accumulation_table_hash(entries: &[BangerNativeTemporalHistoryEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history.accumulation_table.v1\0");
    for entry in entries {
        h.update(entry.accumulation_hash.as_bytes());
        h.update(entry.accumulation_weight_q15.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn temporal_history_packet_hash(
    prepared: &MonsterPreparedCompute,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    temporal_epoch: u64,
    jitter_sequence_hash: &str,
    motion_vector_hash: &str,
    history_reprojection_hash: &str,
    disocclusion_mask_hash: &str,
    rejection_hash: &str,
    accumulation_hash: &str,
    entries: &[BangerNativeTemporalHistoryEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.temporal_history_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(direct_lighting_packet.packet_hash.as_bytes());
    h.update(material_closure_packet.packet_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(temporal_epoch.to_le_bytes());
    h.update(jitter_sequence_hash.as_bytes());
    h.update(motion_vector_hash.as_bytes());
    h.update(history_reprojection_hash.as_bytes());
    h.update(disocclusion_mask_hash.as_bytes());
    h.update(rejection_hash.as_bytes());
    h.update(accumulation_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn page_residency_entry_from_nanite(
    entry: &BangerNativeNaniteSecondLayerEntry,
    resource_table: &BangerNativeResourceTable,
    salt: u32,
) -> BangerNativePageResidencyEntry {
    let page_id = hash_text_hex(
        "forge.banger.page_residency.nanite_page_id.v1",
        &format!("{}:{}:{}", entry.cluster_id, entry.resource_slot, entry.page_hash),
    );
    let byte_len = resource_table
        .slots
        .iter()
        .find(|slot| slot.slot == entry.resource_slot)
        .map(|slot| slot.byte_len)
        .unwrap_or(0);
    let virtual_address = [
        entry.resource_slot,
        entry.requested_lod_bucket,
        (entry.feedback_word & 0xffff) as u32,
        byte_len.min(u32::MAX as u64) as u32,
    ];
    let physical_address = page_residency_physical_address(&entry.page_hash, salt, "nanite_geometry_pool");
    let priority = page_residency_priority(
        entry.residency_state,
        entry.requested_lod_bucket,
        entry.feedback_word,
        byte_len,
    );
    let lock_state = page_residency_lock_state(entry.residency_state, priority);
    let producer_hash = page_residency_producer_hash(
        "nanite_geometry_page",
        &entry.object_id,
        &entry.cluster_id,
        &entry.page_hash,
    );
    let feedback_hash = page_residency_feedback_hash(&entry.page_hash, entry.feedback_word, priority);
    let allocation_hash = page_residency_allocation_entry_hash(
        &page_id,
        "nanite_geometry_page",
        "nanite_geometry_pool",
        &virtual_address,
        &physical_address,
        priority,
        lock_state,
    );
    let eviction_hash = page_residency_eviction_entry_hash(&page_id, lock_state, priority, &feedback_hash);
    let entry_hash = page_residency_entry_hash(
        &page_id,
        "nanite_geometry_page",
        "nanite_geometry_pool",
        &entry.object_id,
        &entry.cluster_id,
        entry.resource_slot,
        &entry.page_hash,
        &virtual_address,
        &physical_address,
        entry.residency_state,
        priority,
        lock_state,
        &producer_hash,
        &feedback_hash,
        &allocation_hash,
        &eviction_hash,
    );
    BangerNativePageResidencyEntry {
        page_id,
        page_kind: "nanite_geometry_page",
        physical_pool: "nanite_geometry_pool",
        object_id: entry.object_id.clone(),
        cluster_id: entry.cluster_id.clone(),
        resource_slot: entry.resource_slot,
        source_page_hash: entry.page_hash.clone(),
        virtual_address,
        physical_address,
        residency_state: entry.residency_state,
        priority,
        lock_state,
        producer_hash,
        feedback_hash,
        allocation_hash,
        eviction_hash,
        entry_hash,
    }
}

fn page_residency_entry_from_virtual_shadow(
    entry: &BangerNativeVirtualShadowEntry,
    _salt: u32,
) -> BangerNativePageResidencyEntry {
    let virtual_address = [
        entry.virtual_map_id,
        entry.page_coord[0],
        entry.page_coord[1],
        entry.clipmap_level,
    ];
    let physical_address = [
        entry.projection_tile[0],
        entry.projection_tile[1],
        entry.resolution,
        entry.page_coord[2],
    ];
    let residency_state = match entry.cache_state {
        "persistent_cache_hit" => "resident_page",
        "cache_warm" => "warm_page",
        _ => "streaming_request",
    };
    let priority = page_residency_priority(
        residency_state,
        entry.clipmap_level,
        u64::from(entry.ray_budget),
        u64::from(entry.resolution).saturating_mul(u64::from(entry.resolution)),
    )
    .saturating_add(128u32.saturating_sub(entry.clipmap_level.min(128)));
    let lock_state = page_residency_lock_state(residency_state, priority);
    let producer_hash = page_residency_producer_hash(
        "virtual_shadow_page",
        &entry.object_id,
        &entry.cluster_id,
        &entry.page_table_hash,
    );
    let feedback_hash =
        page_residency_feedback_hash(&entry.page_table_hash, u64::from(entry.ray_budget), priority);
    let allocation_hash = page_residency_allocation_entry_hash(
        &entry.shadow_page_id,
        "virtual_shadow_page",
        "virtual_shadow_physical_pool",
        &virtual_address,
        &physical_address,
        priority,
        lock_state,
    );
    let eviction_hash =
        page_residency_eviction_entry_hash(&entry.shadow_page_id, lock_state, priority, &feedback_hash);
    let entry_hash = page_residency_entry_hash(
        &entry.shadow_page_id,
        "virtual_shadow_page",
        "virtual_shadow_physical_pool",
        &entry.object_id,
        &entry.cluster_id,
        u32::MAX,
        &entry.page_table_hash,
        &virtual_address,
        &physical_address,
        residency_state,
        priority,
        lock_state,
        &producer_hash,
        &feedback_hash,
        &allocation_hash,
        &eviction_hash,
    );
    BangerNativePageResidencyEntry {
        page_id: entry.shadow_page_id.clone(),
        page_kind: "virtual_shadow_page",
        physical_pool: "virtual_shadow_physical_pool",
        object_id: entry.object_id.clone(),
        cluster_id: entry.cluster_id.clone(),
        resource_slot: u32::MAX,
        source_page_hash: entry.page_table_hash.clone(),
        virtual_address,
        physical_address,
        residency_state,
        priority,
        lock_state,
        producer_hash,
        feedback_hash,
        allocation_hash,
        eviction_hash,
        entry_hash,
    }
}

fn page_residency_entry_from_material(
    entry: &BangerNativeMaterialClosureEntry,
    salt: u32,
) -> BangerNativePageResidencyEntry {
    let page_id = hash_text_hex(
        "forge.banger.page_residency.material_page_id.v1",
        &format!(
            "{}:{}:{}:{}",
            entry.cluster_id, entry.texture_slot_base, entry.texture_slot_count, entry.texture_hash
        ),
    );
    let virtual_address = [
        entry.texture_slot_base,
        entry.texture_slot_count,
        entry.material_bin_id,
        entry.layer_count,
    ];
    let physical_address = page_residency_physical_address(&entry.texture_hash, salt, "material_texture_pool");
    let residency_state = if entry.texture_slot_count <= 2 {
        "resident_page"
    } else {
        "feedback_requested"
    };
    let priority = page_residency_priority(
        residency_state,
        entry.layer_count,
        u64::from(entry.texture_slot_count),
        64,
    );
    let lock_state = page_residency_lock_state(residency_state, priority);
    let producer_hash = page_residency_producer_hash(
        "material_virtual_texture_page",
        &entry.object_id,
        &entry.cluster_id,
        &entry.texture_hash,
    );
    let feedback_hash =
        page_residency_feedback_hash(&entry.texture_hash, u64::from(entry.texture_slot_count), priority);
    let allocation_hash = page_residency_allocation_entry_hash(
        &page_id,
        "material_virtual_texture_page",
        "material_texture_pool",
        &virtual_address,
        &physical_address,
        priority,
        lock_state,
    );
    let eviction_hash = page_residency_eviction_entry_hash(&page_id, lock_state, priority, &feedback_hash);
    let entry_hash = page_residency_entry_hash(
        &page_id,
        "material_virtual_texture_page",
        "material_texture_pool",
        &entry.object_id,
        &entry.cluster_id,
        entry.texture_slot_base,
        &entry.texture_hash,
        &virtual_address,
        &physical_address,
        residency_state,
        priority,
        lock_state,
        &producer_hash,
        &feedback_hash,
        &allocation_hash,
        &eviction_hash,
    );
    BangerNativePageResidencyEntry {
        page_id,
        page_kind: "material_virtual_texture_page",
        physical_pool: "material_texture_pool",
        object_id: entry.object_id.clone(),
        cluster_id: entry.cluster_id.clone(),
        resource_slot: entry.texture_slot_base,
        source_page_hash: entry.texture_hash.clone(),
        virtual_address,
        physical_address,
        residency_state,
        priority,
        lock_state,
        producer_hash,
        feedback_hash,
        allocation_hash,
        eviction_hash,
        entry_hash,
    }
}

fn page_residency_physical_address(page_hash: &str, salt: u32, pool: &str) -> [u32; 4] {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.physical_address.v1\0");
    h.update(page_hash.as_bytes());
    h.update(pool.as_bytes());
    h.update(salt.to_le_bytes());
    let digest: [u8; 32] = h.finalize().into();
    [
        u32::from_le_bytes(digest[0..4].try_into().expect("page address x")) % 8192,
        u32::from_le_bytes(digest[4..8].try_into().expect("page address y")) % 8192,
        u32::from_le_bytes(digest[8..12].try_into().expect("page address layer")) % 256,
        u32::from_le_bytes(digest[12..16].try_into().expect("page address generation")),
    ]
}

fn page_residency_priority(
    residency_state: &str,
    lod_or_level: u32,
    feedback_word: u64,
    byte_len: u64,
) -> u32 {
    let state_bias: u32 = match residency_state {
        "resident_page" => 4096,
        "persistent_cache_hit" => 4096,
        "warm_page" | "cache_warm" => 3072,
        "feedback_requested" => 2048,
        _ => 1024,
    };
    let lod_bias = 512u32.saturating_sub(lod_or_level.min(512));
    let feedback_bias = (feedback_word.count_ones() * 8).min(1024);
    let size_bias = (byte_len / 4096).min(512) as u32;
    state_bias
        .saturating_add(lod_bias)
        .saturating_add(feedback_bias)
        .saturating_add(size_bias)
}

fn page_residency_lock_state(residency_state: &str, priority: u32) -> &'static str {
    if residency_state == "resident_page" && priority >= 4096 {
        "locked_for_frame"
    } else if priority < 2048 {
        "eviction_candidate"
    } else {
        "streaming_or_reuse"
    }
}

fn page_residency_producer_hash(page_kind: &str, object_id: &str, cluster_id: &str, source_hash: &str) -> String {
    hash_text_hex(
        "forge.banger.page_residency.producer.v1",
        &format!("{page_kind}:{object_id}:{cluster_id}:{source_hash}"),
    )
}

fn page_residency_feedback_hash(source_page_hash: &str, feedback_word: u64, priority: u32) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.feedback.v1\0");
    h.update(source_page_hash.as_bytes());
    h.update(feedback_word.to_le_bytes());
    h.update(priority.to_le_bytes());
    hex32(h.finalize().into())
}

fn page_residency_allocation_entry_hash(
    page_id: &str,
    page_kind: &str,
    physical_pool: &str,
    virtual_address: &[u32; 4],
    physical_address: &[u32; 4],
    priority: u32,
    lock_state: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.allocation_entry.v1\0");
    h.update(page_id.as_bytes());
    h.update(page_kind.as_bytes());
    h.update(physical_pool.as_bytes());
    for value in virtual_address {
        h.update(value.to_le_bytes());
    }
    for value in physical_address {
        h.update(value.to_le_bytes());
    }
    h.update(priority.to_le_bytes());
    h.update(lock_state.as_bytes());
    hex32(h.finalize().into())
}

fn page_residency_eviction_entry_hash(page_id: &str, lock_state: &str, priority: u32, feedback_hash: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.eviction_entry.v1\0");
    h.update(page_id.as_bytes());
    h.update(lock_state.as_bytes());
    h.update(priority.to_le_bytes());
    h.update(feedback_hash.as_bytes());
    hex32(h.finalize().into())
}

fn page_residency_entry_hash(
    page_id: &str,
    page_kind: &str,
    physical_pool: &str,
    object_id: &str,
    cluster_id: &str,
    resource_slot: u32,
    source_page_hash: &str,
    virtual_address: &[u32; 4],
    physical_address: &[u32; 4],
    residency_state: &str,
    priority: u32,
    lock_state: &str,
    producer_hash: &str,
    feedback_hash: &str,
    allocation_hash: &str,
    eviction_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.entry.v1\0");
    h.update(page_id.as_bytes());
    h.update(page_kind.as_bytes());
    h.update(physical_pool.as_bytes());
    h.update(object_id.as_bytes());
    h.update(cluster_id.as_bytes());
    h.update(resource_slot.to_le_bytes());
    h.update(source_page_hash.as_bytes());
    for value in virtual_address {
        h.update(value.to_le_bytes());
    }
    for value in physical_address {
        h.update(value.to_le_bytes());
    }
    h.update(residency_state.as_bytes());
    h.update(priority.to_le_bytes());
    h.update(lock_state.as_bytes());
    h.update(producer_hash.as_bytes());
    h.update(feedback_hash.as_bytes());
    h.update(allocation_hash.as_bytes());
    h.update(eviction_hash.as_bytes());
    hex32(h.finalize().into())
}

fn page_residency_physical_pool_hash(entries: &[BangerNativePageResidencyEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.physical_pools.v1\0");
    for entry in entries {
        h.update(entry.physical_pool.as_bytes());
        for value in entry.physical_address {
            h.update(value.to_le_bytes());
        }
        h.update(entry.allocation_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn page_residency_virtual_page_table_hash(entries: &[BangerNativePageResidencyEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.virtual_page_table.v1\0");
    for entry in entries {
        h.update(entry.page_id.as_bytes());
        h.update(entry.page_kind.as_bytes());
        for value in entry.virtual_address {
            h.update(value.to_le_bytes());
        }
        h.update(entry.source_page_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn page_residency_feedback_request_hash(entries: &[BangerNativePageResidencyEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.feedback_requests.v1\0");
    for entry in entries {
        h.update(entry.feedback_hash.as_bytes());
        h.update(entry.priority.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn page_residency_allocation_hash(entries: &[BangerNativePageResidencyEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.allocations.v1\0");
    for entry in entries {
        h.update(entry.allocation_hash.as_bytes());
        h.update(entry.lock_state.as_bytes());
    }
    hex32(h.finalize().into())
}

fn page_residency_eviction_hash(entries: &[BangerNativePageResidencyEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency.evictions.v1\0");
    for entry in entries {
        h.update(entry.eviction_hash.as_bytes());
        h.update(entry.lock_state.as_bytes());
    }
    hex32(h.finalize().into())
}

fn page_residency_allocator_packet_hash(
    prepared: &MonsterPreparedCompute,
    resource_table: &BangerNativeResourceTable,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
    physical_pool_hash: &str,
    virtual_page_table_hash: &str,
    feedback_request_hash: &str,
    allocation_hash: &str,
    eviction_hash: &str,
    entries: &[BangerNativePageResidencyEntry],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.page_residency_allocator_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(nanite_second_layer_packet.packet_hash.as_bytes());
    h.update(virtual_shadow_packet.packet_hash.as_bytes());
    h.update(material_closure_packet.packet_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(physical_pool_hash.as_bytes());
    h.update(virtual_page_table_hash.as_bytes());
    h.update(feedback_request_hash.as_bytes());
    h.update(allocation_hash.as_bytes());
    h.update(eviction_hash.as_bytes());
    for entry in entries {
        h.update(entry.entry_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_conversion_candidate(representation: &str) -> bool {
    matches!(representation, "meshlet" | "surfel")
}

fn gaussian_splat_conversion_path(representation: &str) -> &'static str {
    match representation {
        "meshlet" => "meshlet_proxy_bounds_to_gaussian_splat_cluster",
        "surfel" => "surfel_probe_page_to_gaussian_splat_proxy",
        _ => "disabled_no_conversion_path",
    }
}

fn gaussian_splat_bucket_id(node: &BangerNativeSceneSubmissionNode, lod_bucket: u32) -> String {
    hash_text_hex(
        "forge.banger.gaussian_splat.bucket.v1",
        &format!("{}:{}:{lod_bucket}", node.representation, node.submission_order),
    )
}

fn gaussian_splat_sort_key(node: &BangerNativeSceneSubmissionNode, lod_bucket: u32) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.sort_key.v1\0");
    h.update(node.object_id.as_bytes());
    h.update(lod_bucket.to_le_bytes());
    for value in node.world_bounding_sphere {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_group_key(node: &BangerNativeSceneSubmissionNode) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.group_key.v1\0");
    h.update(node.representation.as_bytes());
    for stage in &node.render_graph_stages {
        h.update(stage.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_count_estimate(node: &BangerNativeSceneSubmissionNode, lod_bucket: u32) -> u32 {
    let base: u32 = match node.representation {
        "gaussian_splat" => 4096,
        _ => 1024,
    };
    base.saturating_sub(lod_bucket.saturating_mul(128)).max(64)
}

fn gaussian_splat_opacity_cutoff(lod_bucket: u32) -> f32 {
    (0.01 + lod_bucket as f32 * 0.01).min(0.08)
}

fn gaussian_splat_layer_proof_hash(
    node: &BangerNativeSceneSubmissionNode,
    bucket_id: &str,
    sort_key: &str,
    group_key: &str,
    splat_count_estimate: u32,
    opacity_cutoff: f32,
    lod_bucket: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.layer_proof.v1\0");
    h.update(node.proof_hash.as_bytes());
    h.update(bucket_id.as_bytes());
    h.update(sort_key.as_bytes());
    h.update(group_key.as_bytes());
    h.update(splat_count_estimate.to_le_bytes());
    h.update(opacity_cutoff.to_le_bytes());
    h.update(lod_bucket.to_le_bytes());
    hex32(h.finalize().into())
}

fn gaussian_splat_conversion_proxy_bounds_hash(node: &BangerNativeSceneSubmissionNode) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.conversion_proxy_bounds.v1\0");
    h.update(node.object_id.as_bytes());
    for value in node
        .world_aabb_min
        .iter()
        .chain(node.world_aabb_max.iter())
        .chain(node.world_bounding_sphere.iter())
    {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_conversion_hash(
    node: &BangerNativeSceneSubmissionNode,
    conversion_path: &str,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    proxy_bounds_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.conversion.v1\0");
    h.update(node.proof_hash.as_bytes());
    h.update(conversion_path.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(proxy_bounds_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gaussian_splat_proxy_bounds_hash(
    layers: &[BangerNativeGaussianSplatLayer],
    conversions: &[BangerNativeGaussianSplatConversionManifest],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.proxy_bounds.v1\0");
    for layer in layers {
        h.update(layer.object_id.as_bytes());
        for value in layer
            .proxy_bounds_min
            .iter()
            .chain(layer.proxy_bounds_max.iter())
            .chain(layer.proxy_bounding_sphere.iter())
        {
            h.update(value.to_le_bytes());
        }
    }
    for conversion in conversions {
        h.update(conversion.proxy_bounds_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_sort_key_hash(layers: &[BangerNativeGaussianSplatLayer]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.sort_keys.v1\0");
    for layer in layers {
        h.update(layer.sort_key.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_group_key_hash(layers: &[BangerNativeGaussianSplatLayer]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.group_keys.v1\0");
    for layer in layers {
        h.update(layer.group_key.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_conversion_manifest_hash(
    conversions: &[BangerNativeGaussianSplatConversionManifest],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.conversion_manifest.v1\0");
    for conversion in conversions {
        h.update(conversion.conversion_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_layer_manifest_hash(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    proxy_bounds_hash: &str,
    sort_key_hash: &str,
    group_key_hash: &str,
    conversion_manifest_hash: &str,
    layers: &[BangerNativeGaussianSplatLayer],
    conversions: &[BangerNativeGaussianSplatConversionManifest],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_gaussian_splat_layer_manifest.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(culling_manifest.manifest_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(proxy_bounds_hash.as_bytes());
    h.update(sort_key_hash.as_bytes());
    h.update(group_key_hash.as_bytes());
    h.update(conversion_manifest_hash.as_bytes());
    for layer in layers {
        h.update(layer.proof_hash.as_bytes());
    }
    for conversion in conversions {
        h.update(conversion.conversion_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

#[derive(Debug, Clone, Copy)]
enum PlyScalarType {
    F32,
    F64,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
}

#[derive(Debug, Clone)]
struct PlyVertexProperty {
    name: String,
    scalar_type: PlyScalarType,
}

#[derive(Debug, Clone)]
struct ParsedPlySplatAsset {
    format: &'static str,
    property_names: Vec<String>,
    splats: Vec<ParsedGaussianSplat>,
}

#[derive(Debug, Clone)]
struct ParsedGaussianSplat {
    position: [f32; 3],
    scale_log: [f32; 3],
    rotation: [f32; 4],
    opacity: f32,
    sh_dc: [f32; 3],
    sh_rest: [f32; 45],
}

fn parse_gaussian_splat_ply(bytes: &[u8]) -> Result<ParsedPlySplatAsset, String> {
    let header_marker = find_subslice(bytes, b"end_header")
        .ok_or_else(|| "PLY header is missing end_header".to_string())?;
    let header_end = advance_ply_header_end(bytes, header_marker + b"end_header".len());
    let header = std::str::from_utf8(&bytes[..header_end])
        .map_err(|err| format!("PLY header is not valid UTF-8: {err}"))?;
    let (format, vertex_count, properties) = parse_ply_header(header)?;
    if vertex_count == 0 {
        return Err("PLY vertex element is empty".to_string());
    }
    let payload = &bytes[header_end..];
    let splats = match format {
        "ply_ascii_1_0" => parse_ascii_gaussian_splats(payload, vertex_count, &properties)?,
        "ply_binary_little_endian_1_0" => {
            parse_binary_little_gaussian_splats(payload, vertex_count, &properties)?
        }
        _ => return Err(format!("unsupported Gaussian splat PLY format: {format}")),
    };
    Ok(ParsedPlySplatAsset {
        format,
        property_names: properties.iter().map(|property| property.name.clone()).collect(),
        splats,
    })
}

fn parse_ply_header(header: &str) -> Result<(&'static str, usize, Vec<PlyVertexProperty>), String> {
    let mut lines = header.lines();
    if lines.next().map(str::trim) != Some("ply") {
        return Err("PLY file must start with 'ply'".to_string());
    }
    let mut format = None;
    let mut vertex_count = None;
    let mut properties = Vec::new();
    let mut inside_vertex = false;
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("comment ") {
            continue;
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            ["format", "ascii", "1.0"] => format = Some("ply_ascii_1_0"),
            ["format", "binary_little_endian", "1.0"] => {
                format = Some("ply_binary_little_endian_1_0")
            }
            ["format", other, version] => {
                return Err(format!("unsupported PLY format {other} {version}"));
            }
            ["element", "vertex", count] => {
                vertex_count = Some(
                    count
                        .parse::<usize>()
                        .map_err(|err| format!("invalid PLY vertex count {count}: {err}"))?,
                );
                inside_vertex = true;
            }
            ["element", ..] => inside_vertex = false,
            ["property", "list", ..] if inside_vertex => {
                return Err("Gaussian splat vertex properties must be scalar, not list".to_string());
            }
            ["property", scalar_type, name] if inside_vertex => {
                properties.push(PlyVertexProperty {
                    name: (*name).to_string(),
                    scalar_type: parse_ply_scalar_type(scalar_type)?,
                });
            }
            ["end_header"] => break,
            _ => {}
        }
    }
    let format = format.ok_or_else(|| "PLY header is missing format".to_string())?;
    let vertex_count =
        vertex_count.ok_or_else(|| "PLY header is missing element vertex".to_string())?;
    if properties.is_empty() {
        return Err("PLY vertex element has no scalar properties".to_string());
    }
    for required in ["x", "y", "z"] {
        if !properties.iter().any(|property| property.name == required) {
            return Err(format!("Gaussian splat PLY is missing required property {required}"));
        }
    }
    Ok((format, vertex_count, properties))
}

fn parse_ply_scalar_type(raw: &str) -> Result<PlyScalarType, String> {
    match raw {
        "float" | "float32" => Ok(PlyScalarType::F32),
        "double" | "float64" => Ok(PlyScalarType::F64),
        "uchar" | "uint8" => Ok(PlyScalarType::U8),
        "char" | "int8" => Ok(PlyScalarType::I8),
        "ushort" | "uint16" => Ok(PlyScalarType::U16),
        "short" | "int16" => Ok(PlyScalarType::I16),
        "uint" | "uint32" => Ok(PlyScalarType::U32),
        "int" | "int32" => Ok(PlyScalarType::I32),
        _ => Err(format!("unsupported PLY scalar type: {raw}")),
    }
}

fn parse_ascii_gaussian_splats(
    payload: &[u8],
    vertex_count: usize,
    properties: &[PlyVertexProperty],
) -> Result<Vec<ParsedGaussianSplat>, String> {
    let text = std::str::from_utf8(payload)
        .map_err(|err| format!("ASCII PLY payload is not UTF-8: {err}"))?;
    let mut values = text.split_whitespace();
    let mut splats = Vec::with_capacity(vertex_count);
    for vertex_index in 0..vertex_count {
        let mut vertex = Vec::with_capacity(properties.len());
        for property in properties {
            let raw = values.next().ok_or_else(|| {
                format!(
                    "ASCII PLY ended while reading vertex {vertex_index} property {}",
                    property.name
                )
            })?;
            vertex.push((
                property.name.as_str(),
                raw.parse::<f32>()
                    .map_err(|err| format!("invalid ASCII PLY float {raw}: {err}"))?,
            ));
        }
        splats.push(parsed_gaussian_splat_from_properties(&vertex)?);
    }
    Ok(splats)
}

fn parse_binary_little_gaussian_splats(
    payload: &[u8],
    vertex_count: usize,
    properties: &[PlyVertexProperty],
) -> Result<Vec<ParsedGaussianSplat>, String> {
    let stride = properties
        .iter()
        .map(|property| ply_scalar_size(property.scalar_type))
        .sum::<usize>();
    let expected = vertex_count
        .checked_mul(stride)
        .ok_or_else(|| "binary PLY vertex payload is too large".to_string())?;
    if payload.len() < expected {
        return Err(format!(
            "binary PLY payload has {} bytes but {expected} are required",
            payload.len()
        ));
    }
    let mut splats = Vec::with_capacity(vertex_count);
    let mut offset = 0;
    for _ in 0..vertex_count {
        let mut vertex = Vec::with_capacity(properties.len());
        for property in properties {
            let value = read_ply_scalar_as_f32(payload, offset, property.scalar_type)?;
            offset += ply_scalar_size(property.scalar_type);
            vertex.push((property.name.as_str(), value));
        }
        splats.push(parsed_gaussian_splat_from_properties(&vertex)?);
    }
    Ok(splats)
}

fn parsed_gaussian_splat_from_properties(
    properties: &[(&str, f32)],
) -> Result<ParsedGaussianSplat, String> {
    let position = [
        required_ply_property(properties, "x")?,
        required_ply_property(properties, "y")?,
        required_ply_property(properties, "z")?,
    ];
    let scale_log = [
        optional_ply_property(properties, "scale_0", -8.0),
        optional_ply_property(properties, "scale_1", -8.0),
        optional_ply_property(properties, "scale_2", -8.0),
    ];
    let rotation = normalize_quaternion([
        optional_ply_property(properties, "rot_0", 1.0),
        optional_ply_property(properties, "rot_1", 0.0),
        optional_ply_property(properties, "rot_2", 0.0),
        optional_ply_property(properties, "rot_3", 0.0),
    ]);
    let mut sh_rest = [0.0; 45];
    for (index, value) in sh_rest.iter_mut().enumerate() {
        *value = optional_ply_property(properties, &format!("f_rest_{index}"), 0.0);
    }
    Ok(ParsedGaussianSplat {
        position,
        scale_log,
        rotation,
        opacity: sigmoid(optional_ply_property(properties, "opacity", 0.0)),
        sh_dc: [
            optional_ply_property(properties, "f_dc_0", 0.0),
            optional_ply_property(properties, "f_dc_1", 0.0),
            optional_ply_property(properties, "f_dc_2", 0.0),
        ],
        sh_rest,
    })
}

fn gaussian_splat_positions_buffer(splats: &[ParsedGaussianSplat]) -> Vec<u8> {
    let mut out = Vec::with_capacity(splats.len() * 12);
    for splat in splats {
        push_f32s(&mut out, &splat.position);
    }
    out
}

fn gaussian_splat_covariance_buffer(splats: &[ParsedGaussianSplat]) -> Vec<u8> {
    let mut out = Vec::with_capacity(splats.len() * 28);
    for splat in splats {
        push_f32s(&mut out, &splat.scale_log);
        push_f32s(&mut out, &splat.rotation);
    }
    out
}

fn gaussian_splat_opacity_buffer(splats: &[ParsedGaussianSplat]) -> Vec<u8> {
    let mut out = Vec::with_capacity(splats.len() * 4);
    for splat in splats {
        out.extend_from_slice(&splat.opacity.to_le_bytes());
    }
    out
}

fn gaussian_splat_sh_buffer(splats: &[ParsedGaussianSplat]) -> Vec<u8> {
    let mut out = Vec::with_capacity(splats.len() * 192);
    for splat in splats {
        push_f32s(&mut out, &splat.sh_dc);
        push_f32s(&mut out, &splat.sh_rest);
    }
    out
}

fn gaussian_splat_asset_sort_key_buffer(splats: &[ParsedGaussianSplat]) -> Vec<u8> {
    let mut out = Vec::with_capacity(splats.len() * 8);
    for (index, splat) in splats.iter().enumerate() {
        out.extend_from_slice(&depth_desc_sort_key(splat.position[2]).to_le_bytes());
        out.extend_from_slice(&(index as u32).to_le_bytes());
    }
    out
}

fn gaussian_splat_asset_bounds(splats: &[ParsedGaussianSplat]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for splat in splats {
        let radius = gaussian_splat_radius(splat);
        for axis in 0..3 {
            min[axis] = min[axis].min(splat.position[axis] - radius);
            max[axis] = max[axis].max(splat.position[axis] + radius);
        }
    }
    (min, max)
}

fn gaussian_splat_asset_buckets(
    splats: &[ParsedGaussianSplat],
    bucket_count: u32,
) -> Vec<BangerGaussianSplatAssetBucket> {
    let mut indices = (0..splats.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        splats[*right]
            .position[2]
            .partial_cmp(&splats[*left].position[2])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.cmp(right))
    });
    let bucket_total = (bucket_count as usize).min(splats.len()).max(1);
    let bucket_size = (splats.len() + bucket_total - 1) / bucket_total;
    indices
        .chunks(bucket_size)
        .enumerate()
        .map(|(bucket_index, chunk)| {
            let bucket_splats = chunk
                .iter()
                .map(|index| splats[*index].clone())
                .collect::<Vec<_>>();
            let (bounds_min, bounds_max) = gaussian_splat_asset_bounds(&bucket_splats);
            let sort_key_hash = gaussian_splat_bucket_sort_key_hash(chunk, splats);
            let proof_hash = gaussian_splat_asset_bucket_proof_hash(
                bucket_index as u32,
                (bucket_index * bucket_size) as u32,
                chunk.len() as u32,
                bounds_min,
                bounds_max,
                &sort_key_hash,
            );
            BangerGaussianSplatAssetBucket {
                bucket_id: bucket_index as u32,
                first_splat: (bucket_index * bucket_size) as u32,
                splat_count: chunk.len() as u32,
                sort_key_hash,
                bounds_min,
                bounds_max,
                proof_hash,
            }
        })
        .collect()
}

fn gaussian_splat_radius(splat: &ParsedGaussianSplat) -> f32 {
    splat
        .scale_log
        .iter()
        .map(|value| value.exp().abs())
        .fold(0.0001, f32::max)
}

fn gaussian_splat_gpu_layout_hash() -> String {
    hash_text_hex(
        "forge.banger.gaussian_splat_gpu_layout.v1",
        "position=float32x3;covariance=scale_log_float32x3+rotation_quat_float32x4;opacity=sigmoid_float32;color=sh_l3_48xf32;sort=depth_desc_u32+stable_index_u32",
    )
}

fn gaussian_splat_asset_manifest_hash(
    asset_id: &str,
    source_hash: &str,
    ply_format: &str,
    property_count: usize,
    splat_count: usize,
    truncated: bool,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    positions_buffer_hash: &str,
    covariance_buffer_hash: &str,
    opacity_buffer_hash: &str,
    sh_buffer_hash: &str,
    sort_key_hash: &str,
    bucket_manifest_hash: &str,
    gpu_layout_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat_asset_manifest.v1\0");
    h.update(asset_id.as_bytes());
    h.update(source_hash.as_bytes());
    h.update(ply_format.as_bytes());
    h.update((property_count as u64).to_le_bytes());
    h.update((splat_count as u64).to_le_bytes());
    h.update([truncated as u8]);
    for value in bounds_min.iter().chain(bounds_max.iter()) {
        h.update(value.to_le_bytes());
    }
    h.update(positions_buffer_hash.as_bytes());
    h.update(covariance_buffer_hash.as_bytes());
    h.update(opacity_buffer_hash.as_bytes());
    h.update(sh_buffer_hash.as_bytes());
    h.update(sort_key_hash.as_bytes());
    h.update(bucket_manifest_hash.as_bytes());
    h.update(gpu_layout_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gaussian_splat_asset_bucket_manifest_hash(
    buckets: &[BangerGaussianSplatAssetBucket],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat_bucket_manifest.v1\0");
    for bucket in buckets {
        h.update(bucket.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_bucket_sort_key_hash(
    indices: &[usize],
    splats: &[ParsedGaussianSplat],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat_bucket_sort_key.v1\0");
    for index in indices {
        h.update(depth_desc_sort_key(splats[*index].position[2]).to_le_bytes());
        h.update((*index as u32).to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_asset_bucket_proof_hash(
    bucket_id: u32,
    first_splat: u32,
    splat_count: u32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    sort_key_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat_asset_bucket.v1\0");
    h.update(bucket_id.to_le_bytes());
    h.update(first_splat.to_le_bytes());
    h.update(splat_count.to_le_bytes());
    for value in bounds_min.iter().chain(bounds_max.iter()) {
        h.update(value.to_le_bytes());
    }
    h.update(sort_key_hash.as_bytes());
    hex32(h.finalize().into())
}

fn required_ply_property(properties: &[(&str, f32)], name: &str) -> Result<f32, String> {
    properties
        .iter()
        .find_map(|(property_name, value)| (*property_name == name).then_some(*value))
        .ok_or_else(|| format!("Gaussian splat PLY is missing required property {name}"))
}

fn optional_ply_property(properties: &[(&str, f32)], name: &str, fallback: f32) -> f32 {
    properties
        .iter()
        .find_map(|(property_name, value)| (*property_name == name).then_some(*value))
        .unwrap_or(fallback)
}

fn push_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn normalize_quaternion(mut value: [f32; 4]) -> [f32; 4] {
    let len = value.iter().map(|component| component * component).sum::<f32>().sqrt();
    if len > f32::EPSILON {
        for component in &mut value {
            *component /= len;
        }
        value
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

fn sigmoid(value: f32) -> f32 {
    1.0 / (1.0 + (-value).exp())
}

fn depth_desc_sort_key(value: f32) -> u32 {
    let bits = value.to_bits();
    let asc = if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits ^ 0x8000_0000
    };
    !asc
}

fn ply_scalar_size(scalar_type: PlyScalarType) -> usize {
    match scalar_type {
        PlyScalarType::F32 | PlyScalarType::U32 | PlyScalarType::I32 => 4,
        PlyScalarType::F64 => 8,
        PlyScalarType::U8 | PlyScalarType::I8 => 1,
        PlyScalarType::U16 | PlyScalarType::I16 => 2,
    }
}

fn read_ply_scalar_as_f32(
    payload: &[u8],
    offset: usize,
    scalar_type: PlyScalarType,
) -> Result<f32, String> {
    let end = offset + ply_scalar_size(scalar_type);
    let bytes = payload
        .get(offset..end)
        .ok_or_else(|| "binary PLY scalar read exceeded payload".to_string())?;
    Ok(match scalar_type {
        PlyScalarType::F32 => f32::from_le_bytes(bytes.try_into().expect("f32 scalar width")),
        PlyScalarType::F64 => {
            f64::from_le_bytes(bytes.try_into().expect("f64 scalar width")) as f32
        }
        PlyScalarType::U8 => bytes[0] as f32,
        PlyScalarType::I8 => i8::from_le_bytes(bytes.try_into().expect("i8 scalar width")) as f32,
        PlyScalarType::U16 => u16::from_le_bytes(bytes.try_into().expect("u16 scalar width")) as f32,
        PlyScalarType::I16 => i16::from_le_bytes(bytes.try_into().expect("i16 scalar width")) as f32,
        PlyScalarType::U32 => u32::from_le_bytes(bytes.try_into().expect("u32 scalar width")) as f32,
        PlyScalarType::I32 => i32::from_le_bytes(bytes.try_into().expect("i32 scalar width")) as f32,
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn advance_ply_header_end(bytes: &[u8], mut offset: usize) -> usize {
    if bytes.get(offset) == Some(&b'\r') {
        offset += 1;
    }
    if bytes.get(offset) == Some(&b'\n') {
        offset += 1;
    }
    offset
}

#[derive(Debug, Clone)]
struct GaussianSplatCamera {
    position: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    fov_y_degrees: f32,
    near_plane: f32,
    width: u32,
    height: u32,
    focal_y: f32,
    focal_x: f32,
}

#[derive(Debug, Clone)]
struct ProjectedGaussianSplat {
    source_index: usize,
    center: [f32; 2],
    depth: f32,
    inv_cov2: [f32; 3],
    radius_px: f32,
    bbox: [u32; 4],
    color: [f32; 3],
    opacity: f32,
    proof_hash: String,
}

#[derive(Debug, Clone, Copy)]
struct GaussianSplatTileBounds {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

#[derive(Debug, Clone)]
struct GaussianSplatTileGrid {
    width: u32,
    height: u32,
    tile_size: u32,
    tiles_x: u32,
    tiles_y: u32,
}

impl GaussianSplatCamera {
    fn new(
        position: [f32; 3],
        target: [f32; 3],
        up_hint: [f32; 3],
        fov_y_degrees: f32,
        near_plane: f32,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let forward = normalize3(sub3(target, position))
            .ok_or_else(|| "Gaussian splat camera target must differ from position".to_string())?;
        let right = normalize3(cross3(forward, up_hint))
            .ok_or_else(|| "Gaussian splat camera up vector is parallel to view".to_string())?;
        let up = normalize3(cross3(right, forward))
            .ok_or_else(|| "Gaussian splat camera basis is degenerate".to_string())?;
        let fov_y_degrees = fov_y_degrees.clamp(5.0, 160.0);
        let fov_y = fov_y_degrees.to_radians();
        let focal_y = height as f32 / (2.0 * (fov_y * 0.5).tan());
        let focal_x = focal_y;
        Ok(Self {
            position,
            forward,
            right,
            up,
            fov_y_degrees,
            near_plane: near_plane.max(0.0001),
            width,
            height,
            focal_y,
            focal_x,
        })
    }
}

impl GaussianSplatTileGrid {
    fn new(width: u32, height: u32, tile_size: u32) -> Self {
        Self {
            width,
            height,
            tile_size,
            tiles_x: width.div_ceil(tile_size),
            tiles_y: height.div_ceil(tile_size),
        }
    }

    fn len(&self) -> usize {
        (self.tiles_x * self.tiles_y) as usize
    }

    fn tile_bounds(&self, tile_id: u32) -> GaussianSplatTileBounds {
        let tile_x = tile_id % self.tiles_x;
        let tile_y = tile_id / self.tiles_x;
        let x0 = tile_x * self.tile_size;
        let y0 = tile_y * self.tile_size;
        GaussianSplatTileBounds {
            x0,
            y0,
            x1: (x0 + self.tile_size).min(self.width),
            y1: (y0 + self.tile_size).min(self.height),
        }
    }
}

fn project_gaussian_splats(
    splats: &[ParsedGaussianSplat],
    camera: &GaussianSplatCamera,
) -> Vec<ProjectedGaussianSplat> {
    let mut projected = Vec::with_capacity(splats.len());
    for (index, splat) in splats.iter().enumerate() {
        let camera_pos = world_to_camera(splat.position, camera);
        if camera_pos[2] <= camera.near_plane {
            continue;
        }
        let screen_x = camera.width as f32 * 0.5 + camera.focal_x * camera_pos[0] / camera_pos[2];
        let screen_y = camera.height as f32 * 0.5 - camera.focal_y * camera_pos[1] / camera_pos[2];
        let Some((cov2, radius_px)) = gaussian_splat_projected_covariance(splat, camera, camera_pos) else {
            continue;
        };
        let Some(inv_cov2) = invert_cov2(cov2) else {
            continue;
        };
        if radius_px < 0.25 {
            continue;
        }
        let x0 = (screen_x - radius_px).floor().max(0.0) as u32;
        let y0 = (screen_y - radius_px).floor().max(0.0) as u32;
        let x1 = (screen_x + radius_px).ceil().min(camera.width as f32) as u32;
        let y1 = (screen_y + radius_px).ceil().min(camera.height as f32) as u32;
        if x0 >= x1 || y0 >= y1 {
            continue;
        }
        let view_dir = normalize3(sub3(camera.position, splat.position)).unwrap_or([0.0, 0.0, 1.0]);
        let color = evaluate_gaussian_splat_sh(splat, view_dir);
        let proof_hash = projected_gaussian_splat_hash(
            index,
            [screen_x, screen_y],
            camera_pos[2],
            inv_cov2,
            radius_px,
            [x0, y0, x1, y1],
            color,
            splat.opacity,
        );
        projected.push(ProjectedGaussianSplat {
            source_index: index,
            center: [screen_x, screen_y],
            depth: camera_pos[2],
            inv_cov2,
            radius_px,
            bbox: [x0, y0, x1, y1],
            color,
            opacity: splat.opacity.min(0.995),
            proof_hash,
        });
    }
    projected
}

fn gaussian_splat_tile_lists(
    projected: &[ProjectedGaussianSplat],
    tile_grid: &GaussianSplatTileGrid,
) -> Vec<Vec<usize>> {
    let mut tiles = vec![Vec::new(); tile_grid.len()];
    for (projected_index, splat) in projected.iter().enumerate() {
        let min_tile_x = splat.bbox[0] / tile_grid.tile_size;
        let min_tile_y = splat.bbox[1] / tile_grid.tile_size;
        let max_tile_x = (splat.bbox[2].saturating_sub(1) / tile_grid.tile_size)
            .min(tile_grid.tiles_x.saturating_sub(1));
        let max_tile_y = (splat.bbox[3].saturating_sub(1) / tile_grid.tile_size)
            .min(tile_grid.tiles_y.saturating_sub(1));
        for tile_y in min_tile_y..=max_tile_y {
            for tile_x in min_tile_x..=max_tile_x {
                let tile_index = (tile_y * tile_grid.tiles_x + tile_x) as usize;
                tiles[tile_index].push(projected_index);
            }
        }
    }
    for tile in &mut tiles {
        tile.sort_by(|left, right| {
            projected[*right]
                .depth
                .partial_cmp(&projected[*left].depth)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| projected[*left].source_index.cmp(&projected[*right].source_index))
        });
    }
    tiles
}

fn rasterize_gaussian_splat_tile(
    tile_bounds: GaussianSplatTileBounds,
    tile_splats: &[usize],
    projected: &[ProjectedGaussianSplat],
    rasterized: &mut [bool],
    image_width: u32,
    rgba: &mut [u8],
    background: [f32; 4],
) -> u32 {
    let mut contribution_count = 0u32;
    for y in tile_bounds.y0..tile_bounds.y1 {
        for x in tile_bounds.x0..tile_bounds.x1 {
            let mut dst = [
                background[0].clamp(0.0, 1.0),
                background[1].clamp(0.0, 1.0),
                background[2].clamp(0.0, 1.0),
                background[3].clamp(0.0, 1.0),
            ];
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut pixel_touched = false;
            for projected_index in tile_splats {
                let splat = &projected[*projected_index];
                let dx = px - splat.center[0];
                let dy = py - splat.center[1];
                if dx.abs() > splat.radius_px || dy.abs() > splat.radius_px {
                    continue;
                }
                let power = -0.5
                    * (splat.inv_cov2[0] * dx * dx
                        + 2.0 * splat.inv_cov2[1] * dx * dy
                        + splat.inv_cov2[2] * dy * dy);
                if power < -12.5 {
                    continue;
                }
                let alpha = (splat.opacity * power.exp()).clamp(0.0, 0.995);
                if alpha < 1.0 / 255.0 {
                    continue;
                }
                dst[0] = splat.color[0] * alpha + dst[0] * (1.0 - alpha);
                dst[1] = splat.color[1] * alpha + dst[1] * (1.0 - alpha);
                dst[2] = splat.color[2] * alpha + dst[2] * (1.0 - alpha);
                dst[3] = alpha + dst[3] * (1.0 - alpha);
                rasterized[*projected_index] = true;
                pixel_touched = true;
            }
            if pixel_touched {
                contribution_count = contribution_count.saturating_add(1);
            }
            let offset = ((y * image_width + x) * 4) as usize;
            rgba[offset] = float_to_u8(dst[0]);
            rgba[offset + 1] = float_to_u8(dst[1]);
            rgba[offset + 2] = float_to_u8(dst[2]);
            rgba[offset + 3] = float_to_u8(dst[3]);
        }
    }
    contribution_count
}

fn gaussian_splat_projected_covariance(
    splat: &ParsedGaussianSplat,
    camera: &GaussianSplatCamera,
    camera_pos: [f32; 3],
) -> Option<([f32; 3], f32)> {
    let world_cov = gaussian_splat_world_covariance(splat);
    let camera_cov = covariance_to_camera(world_cov, camera);
    let z = camera_pos[2].max(camera.near_plane);
    let j = [
        [camera.focal_x / z, 0.0, -camera.focal_x * camera_pos[0] / (z * z)],
        [0.0, -camera.focal_y / z, camera.focal_y * camera_pos[1] / (z * z)],
    ];
    let cov2 = [
        dot3(mul_mat3_vec(camera_cov, j[0]), j[0]).max(0.3),
        dot3(mul_mat3_vec(camera_cov, j[0]), j[1]),
        dot3(mul_mat3_vec(camera_cov, j[1]), j[1]).max(0.3),
    ];
    let trace = cov2[0] + cov2[2];
    let det_term = ((cov2[0] - cov2[2]) * (cov2[0] - cov2[2]) + 4.0 * cov2[1] * cov2[1])
        .max(0.0)
        .sqrt();
    let lambda_max = ((trace + det_term) * 0.5).max(0.3);
    let radius_px = 3.0 * lambda_max.sqrt();
    if radius_px.is_finite() && cov2.iter().all(|value| value.is_finite()) {
        Some((cov2, radius_px.min(512.0)))
    } else {
        None
    }
}

fn gaussian_splat_world_covariance(splat: &ParsedGaussianSplat) -> [[f32; 3]; 3] {
    let rotation = quaternion_to_mat3(splat.rotation);
    let scale = [
        splat.scale_log[0].exp().max(0.0001),
        splat.scale_log[1].exp().max(0.0001),
        splat.scale_log[2].exp().max(0.0001),
    ];
    let mut cov = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            cov[i][j] = rotation[i][0] * scale[0] * scale[0] * rotation[j][0]
                + rotation[i][1] * scale[1] * scale[1] * rotation[j][1]
                + rotation[i][2] * scale[2] * scale[2] * rotation[j][2];
        }
    }
    cov
}

fn covariance_to_camera(world_cov: [[f32; 3]; 3], camera: &GaussianSplatCamera) -> [[f32; 3]; 3] {
    let basis = [camera.right, camera.up, camera.forward];
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = dot3(mul_mat3_vec(world_cov, basis[i]), basis[j]);
        }
    }
    out
}

fn invert_cov2(cov2: [f32; 3]) -> Option<[f32; 3]> {
    let det = cov2[0] * cov2[2] - cov2[1] * cov2[1];
    if det <= 1e-8 || !det.is_finite() {
        return None;
    }
    Some([cov2[2] / det, -cov2[1] / det, cov2[0] / det])
}

fn evaluate_gaussian_splat_sh(splat: &ParsedGaussianSplat, view_dir: [f32; 3]) -> [f32; 3] {
    const C0: f32 = 0.2820948;
    const C1: f32 = 0.48860252;
    const C2: [f32; 5] = [1.0925485, -1.0925485, 0.31539157, -1.0925485, 0.54627424];
    const C3: [f32; 7] = [
        -0.5900436,
        2.8906114,
        -0.4570458,
        0.37317634,
        -0.4570458,
        1.4453057,
        -0.5900436,
    ];
    let x = view_dir[0];
    let y = view_dir[1];
    let z = view_dir[2];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let yz = y * z;
    let xz = x * z;
    let mut out = [0.0; 3];
    for channel in 0..3 {
        let coeff = |basis: usize| splat.sh_rest[basis * 3 + channel];
        let mut value = C0 * splat.sh_dc[channel];
        value += -C1 * y * coeff(0) + C1 * z * coeff(1) - C1 * x * coeff(2);
        value += C2[0] * xy * coeff(3)
            + C2[1] * yz * coeff(4)
            + C2[2] * (2.0 * zz - xx - yy) * coeff(5)
            + C2[3] * xz * coeff(6)
            + C2[4] * (xx - yy) * coeff(7);
        value += C3[0] * y * (3.0 * xx - yy) * coeff(8)
            + C3[1] * xy * z * coeff(9)
            + C3[2] * y * (4.0 * zz - xx - yy) * coeff(10)
            + C3[3] * z * (2.0 * zz - 3.0 * xx - 3.0 * yy) * coeff(11)
            + C3[4] * x * (4.0 * zz - xx - yy) * coeff(12)
            + C3[5] * z * (xx - yy) * coeff(13)
            + C3[6] * x * (xx - 3.0 * yy) * coeff(14);
        out[channel] = (value + 0.5).clamp(0.0, 1.0);
    }
    out
}

fn gaussian_splat_camera_hash(camera: &GaussianSplatCamera) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.camera.v1\0");
    for value in camera
        .position
        .iter()
        .chain(camera.forward.iter())
        .chain(camera.right.iter())
        .chain(camera.up.iter())
    {
        h.update(value.to_le_bytes());
    }
    h.update(camera.fov_y_degrees.to_le_bytes());
    h.update(camera.near_plane.to_le_bytes());
    h.update(camera.width.to_le_bytes());
    h.update(camera.height.to_le_bytes());
    hex32(h.finalize().into())
}

fn gaussian_splat_projected_manifest_hash(projected: &[ProjectedGaussianSplat]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.projected_manifest.v1\0");
    for splat in projected {
        h.update(splat.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn projected_gaussian_splat_hash(
    source_index: usize,
    center: [f32; 2],
    depth: f32,
    inv_cov2: [f32; 3],
    radius_px: f32,
    bbox: [u32; 4],
    color: [f32; 3],
    opacity: f32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.projected.v1\0");
    h.update((source_index as u64).to_le_bytes());
    for value in center.iter().chain(inv_cov2.iter()).chain(color.iter()) {
        h.update(value.to_le_bytes());
    }
    h.update(depth.to_le_bytes());
    h.update(radius_px.to_le_bytes());
    for value in bbox {
        h.update(value.to_le_bytes());
    }
    h.update(opacity.to_le_bytes());
    hex32(h.finalize().into())
}

fn gaussian_splat_raster_tile_sort_hash(
    tile_splats: &[usize],
    projected: &[ProjectedGaussianSplat],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.raster_tile_sort.v1\0");
    for projected_index in tile_splats {
        let splat = &projected[*projected_index];
        h.update((splat.source_index as u64).to_le_bytes());
        h.update(splat.depth.to_le_bytes());
        h.update(splat.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_raster_tile_proof_hash(
    tile_id: u32,
    tile_bounds: GaussianSplatTileBounds,
    splat_count: u32,
    contribution_count: u32,
    depth_sort_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.raster_tile.v1\0");
    h.update(tile_id.to_le_bytes());
    h.update(tile_bounds.x0.to_le_bytes());
    h.update(tile_bounds.y0.to_le_bytes());
    h.update(tile_bounds.x1.to_le_bytes());
    h.update(tile_bounds.y1.to_le_bytes());
    h.update(splat_count.to_le_bytes());
    h.update(contribution_count.to_le_bytes());
    h.update(depth_sort_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gaussian_splat_raster_tile_manifest_hash(tiles: &[BangerGaussianSplatRasterTile]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat.raster_tile_manifest.v1\0");
    for tile in tiles {
        h.update(tile.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gaussian_splat_raster_proof_hash(
    asset_id: &str,
    source_hash: &str,
    width: u32,
    height: u32,
    tile_size: u32,
    splat_count: usize,
    projected_splat_count: usize,
    rasterized_splat_count: usize,
    shaded_pixel_count: u32,
    camera_hash: &str,
    tile_manifest_hash: &str,
    projected_manifest_hash: &str,
    rgba8_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gaussian_splat_rasterizer.v1\0");
    h.update(asset_id.as_bytes());
    h.update(source_hash.as_bytes());
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(tile_size.to_le_bytes());
    h.update((splat_count as u64).to_le_bytes());
    h.update((projected_splat_count as u64).to_le_bytes());
    h.update((rasterized_splat_count as u64).to_le_bytes());
    h.update(shaded_pixel_count.to_le_bytes());
    h.update(camera_hash.as_bytes());
    h.update(tile_manifest_hash.as_bytes());
    h.update(projected_manifest_hash.as_bytes());
    h.update(rgba8_hash.as_bytes());
    hex32(h.finalize().into())
}

fn world_to_camera(point: [f32; 3], camera: &GaussianSplatCamera) -> [f32; 3] {
    let rel = sub3(point, camera.position);
    [dot3(rel, camera.right), dot3(rel, camera.up), dot3(rel, camera.forward)]
}

fn quaternion_to_mat3(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [w, x, y, z] = normalize_quaternion(q);
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

fn mul_mat3_vec(matrix: [[f32; 3]; 3], value: [f32; 3]) -> [f32; 3] {
    [
        dot3(matrix[0], value),
        dot3(matrix[1], value),
        dot3(matrix[2], value),
    ]
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn normalize3(value: [f32; 3]) -> Option<[f32; 3]> {
    let len = dot3(value, value).sqrt();
    (len > f32::EPSILON).then_some([value[0] / len, value[1] / len, value[2] / len])
}

fn float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn scene_representation_mix(
    submissions: &[BangerNativeSceneSubmissionNode],
) -> Vec<BangerNativeRepresentationMixEntry> {
    let total = submissions
        .iter()
        .filter(|node| node.visible && node.renderable)
        .count()
        .max(1) as f32;
    let mut representations = BTreeSet::new();
    for node in submissions.iter().filter(|node| node.visible) {
        representations.insert(node.representation);
    }
    representations
        .into_iter()
        .map(|representation| {
            let object_count = submissions
                .iter()
                .filter(|node| node.visible && node.representation == representation)
                .count();
            let renderable_count = submissions
                .iter()
                .filter(|node| {
                    node.visible && node.renderable && node.representation == representation
                })
                .count();
            let weight = renderable_count as f32 / total;
            let proof_hash =
                representation_mix_entry_hash(representation, object_count, renderable_count, weight);
            BangerNativeRepresentationMixEntry {
                representation,
                object_count,
                renderable_count,
                weight,
                proof_hash,
            }
        })
        .collect()
}

fn scene_submission_fit_bounds(
    submissions: &[BangerNativeSceneSubmissionNode],
) -> ([f32; 3], [f32; 3]) {
    merged_bounds(
        submissions
            .iter()
            .filter(|node| node.visible && node.renderable)
            .map(|node| (node.world_aabb_min, node.world_aabb_max)),
    )
}

fn propagate_scene_graph_world_transforms(objects: &mut [BangerEditableSceneObject]) {
    for object in objects.iter_mut() {
        object.world_transform = object.local_transform;
        object.local_transform_hash = transform_hash("local", &object.local_transform);
        object.world_transform_hash = transform_hash("world", &object.world_transform);
    }
    for _ in 0..objects.len() {
        let snapshot = objects
            .iter()
            .map(|object| (object.object_id.clone(), object.world_transform))
            .collect::<Vec<_>>();
        for object in objects.iter_mut() {
            if let Some(parent_id) = &object.parent_id {
                if let Some((_, parent_world)) = snapshot.iter().find(|(id, _)| id == parent_id) {
                    object.world_transform = mat4_mul(parent_world, &object.local_transform);
                    object.world_transform_hash = transform_hash("world", &object.world_transform);
                }
            }
        }
    }
}

fn scene_role_for_kind(kind: &str) -> &'static str {
    match kind {
        "sdf_brick" => "procedural_shape_field",
        "voxel_page" => "volume_or_streaming_cell",
        "meshlet_page" => "visible_geometry_cluster",
        "surfel_radiance_cache" => "lighting_cache_probe_set",
        "material_payload" => "material_variant_set",
        _ => "native_render_artifact",
    }
}

fn scene_role_for_representation(representation: &str) -> &'static str {
    match representation {
        "sdf" => "procedural_shape_field",
        "voxel" => "volume_or_streaming_cell",
        "meshlet" => "visible_geometry_cluster",
        "surfel" => "lighting_cache_probe_set",
        "gaussian_splat" => "captured_splat_layer",
        "material_graph" => "material_variant_set",
        _ => "native_render_artifact",
    }
}

fn scene_representation_for_kind(kind: &str) -> &'static str {
    match kind {
        "sdf_brick" => "sdf",
        "voxel_page" => "voxel",
        "meshlet_page" => "meshlet",
        "surfel_radiance_cache" => "surfel",
        "material_payload" => "material_graph",
        _ => "native_artifact",
    }
}

fn normalize_newobject_representation(raw: Option<&str>) -> Result<&'static str, String> {
    let value = raw.unwrap_or("sdf").trim().to_ascii_lowercase();
    match value.as_str() {
        "" | "sdf" | "field" | "implicit" | "procedural" => Ok("sdf"),
        "voxel" | "voxels" | "volume" => Ok("voxel"),
        "mesh" | "meshlet" | "meshlets" | "geometry" => Ok("meshlet"),
        "surfel" | "surfels" | "radiance" => Ok("surfel"),
        "gaussian" | "gaussian_splat" | "gaussian-splat" | "splat" | "splats" => {
            Ok("gaussian_splat")
        }
        "material" | "material_graph" | "material-graph" => Ok("material_graph"),
        "native" | "native_artifact" | "artifact" => Ok("native_artifact"),
        _ => Err(format!("unsupported Banger /newobject_ representation: {value}")),
    }
}

fn editable_slots_for_kind(kind: &str) -> Vec<&'static str> {
    match kind {
        "sdf_brick" => vec!["name", "transform", "sdf_params", "material_ref", "visibility"],
        "voxel_page" => vec!["name", "transform", "voxel_lod", "streaming_priority", "visibility"],
        "meshlet_page" => vec!["name", "transform", "lod_bias", "material_ref", "visibility"],
        "surfel_radiance_cache" => vec!["name", "probe_density", "bounce_budget", "temporal_reuse"],
        "material_payload" => vec!["name", "albedo", "roughness", "metallic", "emissive"],
        _ => vec!["name", "transform", "visibility"],
    }
}

fn editable_slots_for_representation(representation: &str) -> Vec<&'static str> {
    match representation {
        "sdf" => vec!["name", "transform", "sdf_params", "material_ref", "visibility"],
        "voxel" => vec!["name", "transform", "voxel_lod", "streaming_priority", "visibility"],
        "meshlet" => vec!["name", "transform", "lod_bias", "material_ref", "visibility"],
        "surfel" => vec!["name", "probe_density", "bounce_budget", "temporal_reuse"],
        "gaussian_splat" => vec!["name", "transform", "splat_bucket_policy", "proxy_bounds", "visibility"],
        "material_graph" => vec!["name", "albedo", "roughness", "metallic", "emissive"],
        _ => vec!["name", "transform", "visibility"],
    }
}

fn artifact_aabb(artifact: &MonsterNativeTandemArtifact) -> ([f32; 3], [f32; 3]) {
    artifact
        .pages
        .first()
        .and_then(|page| page.bytes.get(48..80))
        .and_then(|bytes| {
            let mut values = [0.0f32; 8];
            for (index, slot) in values.iter_mut().enumerate() {
                let start = index * 4;
                let raw = bytes.get(start..start + 4)?;
                *slot = f32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
            }
            Some(([values[0], values[1], values[2]], [values[3], values[4], values[5]]))
        })
        .unwrap_or_else(|| fallback_aabb_for_kind(artifact.kind))
}

fn fallback_aabb_for_kind(kind: &str) -> ([f32; 3], [f32; 3]) {
    match kind {
        "material_payload" => ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        "meshlet_page" => ([-0.5, -0.5, -0.25], [0.5, 0.5, 0.25]),
        "surfel_radiance_cache" => ([-0.45, -0.45, -0.45], [0.45, 0.45, 0.45]),
        _ => ([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]),
    }
}

fn fallback_aabb_for_representation(representation: &str) -> ([f32; 3], [f32; 3]) {
    match representation {
        "material_graph" => ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        "meshlet" => ([-0.5, -0.5, -0.25], [0.5, 0.5, 0.25]),
        "surfel" => ([-0.45, -0.45, -0.45], [0.45, 0.45, 0.45]),
        "gaussian_splat" => ([-0.75, -0.75, -0.5], [0.75, 0.75, 0.5]),
        _ => ([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]),
    }
}

fn merged_bounds<I>(bounds: I) -> ([f32; 3], [f32; 3])
where
    I: IntoIterator<Item = ([f32; 3], [f32; 3])>,
{
    let mut seen = false;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for (item_min, item_max) in bounds {
        seen = true;
        for axis in 0..3 {
            min[axis] = min[axis].min(item_min[axis]);
            max[axis] = max[axis].max(item_max[axis]);
        }
    }
    if seen {
        (min, max)
    } else {
        ([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
    }
}

fn bounding_sphere(min: [f32; 3], max: [f32; 3]) -> [f32; 4] {
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let dx = max[0] - center[0];
    let dy = max[1] - center[1];
    let dz = max[2] - center[2];
    [center[0], center[1], center[2], (dx * dx + dy * dy + dz * dz).sqrt()]
}

fn transform_aabb(min: [f32; 3], max: [f32; 3], transform: &[f32; 16]) -> ([f32; 3], [f32; 3]) {
    let corners = [
        [min[0], min[1], min[2]],
        [min[0], min[1], max[2]],
        [min[0], max[1], min[2]],
        [min[0], max[1], max[2]],
        [max[0], min[1], min[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], min[2]],
        [max[0], max[1], max[2]],
    ];
    merged_bounds(corners.into_iter().map(|corner| {
        let point = transform_point(transform, corner);
        (point, point)
    }))
}

fn transform_point(transform: &[f32; 16], point: [f32; 3]) -> [f32; 3] {
    [
        transform[0] * point[0] + transform[4] * point[1] + transform[8] * point[2] + transform[12],
        transform[1] * point[0] + transform[5] * point[1] + transform[9] * point[2] + transform[13],
        transform[2] * point[0] + transform[6] * point[1] + transform[10] * point[2] + transform[14],
    ]
}

fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = a[row] * b[col * 4]
                + a[4 + row] * b[col * 4 + 1]
                + a[8 + row] * b[col * 4 + 2]
                + a[12 + row] * b[col * 4 + 3];
        }
    }
    out
}

fn identity_transform() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn transform_hash(label: &str, transform: &[f32; 16]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_transform.v1\0");
    h.update(label.as_bytes());
    for value in transform {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn editable_object_hash_with_manifest(
    prepared: &MonsterPreparedCompute,
    object: &BangerEditableSceneObject,
) -> String {
    let mut h = Sha256::new();
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(editable_object_hash(object).as_bytes());
    hex32(h.finalize().into())
}

fn editable_object_hash(object: &BangerEditableSceneObject) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.editable_scene_object.v1\0");
    h.update(object.object_id.as_bytes());
    if let Some(parent_id) = &object.parent_id {
        h.update(parent_id.as_bytes());
    }
    h.update(object.role.as_bytes());
    h.update(object.representation.as_bytes());
    h.update(object.source_artifact_kind.as_bytes());
    h.update(object.source_artifact_hash.as_bytes());
    h.update(object.renderer_cache_hash.as_bytes());
    h.update(object.local_transform_hash.as_bytes());
    h.update(object.world_transform_hash.as_bytes());
    h.update([object.visible as u8]);
    h.update([object.renderable as u8]);
    for value in object.aabb_min.iter().chain(object.aabb_max.iter()).chain(object.bounding_sphere.iter()) {
        h.update(value.to_le_bytes());
    }
    for slot in &object.editable_slots {
        h.update(slot.as_bytes());
    }
    hex32(h.finalize().into())
}

fn editable_scene_graph_hash(scene_id: &str, objects: &[BangerEditableSceneObject]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.editable_scene_graph.v1\0");
    h.update(scene_id.as_bytes());
    for object in objects {
        h.update(object.object_id.as_bytes());
        if let Some(parent_id) = &object.parent_id {
            h.update(parent_id.as_bytes());
        }
        h.update(object.representation.as_bytes());
        h.update([object.visible as u8]);
        h.update([object.renderable as u8]);
        h.update(object.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn editable_scene_bounds_hash(scene_id: &str, objects: &[BangerEditableSceneObject]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.editable_scene_bounds.v1\0");
    h.update(scene_id.as_bytes());
    for object in objects {
        h.update(object.object_id.as_bytes());
        h.update([object.visible as u8]);
        h.update([object.renderable as u8]);
        for value in object.aabb_min.iter().chain(object.aabb_max.iter()).chain(object.bounding_sphere.iter()) {
            h.update(value.to_le_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn gpu_scene_instance_count(node: &BangerNativeSceneSubmissionNode) -> u32 {
    if !node.visible || !node.renderable {
        0
    } else {
        node.resource_slots.len().max(1) as u32
    }
}

fn gpu_scene_payload_float4_count(
    node: &BangerNativeSceneSubmissionNode,
    resource_table: &BangerNativeResourceTable,
) -> u32 {
    let slot_payload = node
        .resource_slots
        .iter()
        .filter_map(|slot_id| resource_table.slots.iter().find(|slot| slot.slot == *slot_id))
        .map(|slot| ((slot.byte_len.min(1024) + 15) / 16) as u32)
        .sum::<u32>();
    if slot_payload == 0 && node.renderable {
        4
    } else {
        slot_payload.max(1)
    }
}

fn gpu_scene_material_record_hash(
    node: &BangerNativeSceneSubmissionNode,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    resource_table: &BangerNativeResourceTable,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.material_record.v1\0");
    h.update(node.object_id.as_bytes());
    h.update(node.representation.as_bytes());
    h.update(shader_compiler_ticket.material_abi_hash.as_bytes());
    for slot_id in &node.resource_slots {
        h.update(slot_id.to_le_bytes());
        if let Some(slot) = resource_table.slots.iter().find(|slot| slot.slot == *slot_id) {
            h.update(slot.pipeline_cache_key.as_bytes());
            h.update(slot.promotion_hash.as_bytes());
            h.update(slot.resource_key.as_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn gpu_scene_primitive_flags_word(
    node: &BangerNativeSceneSubmissionNode,
    supports_gpu_scene: bool,
    supports_nanite_like_streaming: bool,
) -> u32 {
    let mut flags = 0u32;
    flags |= (node.visible as u32) << 0;
    flags |= (node.renderable as u32) << 1;
    flags |= (supports_gpu_scene as u32) << 2;
    flags |= (supports_nanite_like_streaming as u32) << 3;
    flags |= ((node.representation == "gaussian_splat") as u32) << 4;
    flags |= (node.parent_id.is_some() as u32) << 5;
    flags
}

fn gpu_scene_bounds_hash(node: &BangerNativeSceneSubmissionNode) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.bounds.v1\0");
    h.update(node.object_id.as_bytes());
    for value in node
        .world_aabb_min
        .iter()
        .chain(node.world_aabb_max.iter())
        .chain(node.world_bounding_sphere.iter())
    {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_scene_transform_hash(node: &BangerNativeSceneSubmissionNode) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.transform_ref.v1\0");
    h.update(node.object_id.as_bytes());
    h.update(node.local_transform_hash.as_bytes());
    h.update(node.world_transform_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gpu_scene_upload_range_hash(
    node: &BangerNativeSceneSubmissionNode,
    instance_scene_data_offset: u32,
    instance_count: u32,
    payload_data_offset: u32,
    payload_float4_count: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.upload_range.v1\0");
    h.update(node.object_id.as_bytes());
    h.update(instance_scene_data_offset.to_le_bytes());
    h.update(instance_count.to_le_bytes());
    h.update(payload_data_offset.to_le_bytes());
    h.update(payload_float4_count.to_le_bytes());
    h.update(node.proof_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gpu_scene_primitive_hash(
    primitive_id: u32,
    node: &BangerNativeSceneSubmissionNode,
    supports_gpu_scene: bool,
    supports_nanite_like_streaming: bool,
    instance_scene_data_offset: u32,
    instance_count: u32,
    payload_data_offset: u32,
    payload_float4_count: u32,
    material_record_hash: &str,
    primitive_flags_word: u32,
    bounds_hash: &str,
    transform_hash: &str,
    upload_range_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.primitive.v1\0");
    h.update(primitive_id.to_le_bytes());
    h.update(node.object_id.as_bytes());
    h.update(node.representation.as_bytes());
    h.update([supports_gpu_scene as u8]);
    h.update([supports_nanite_like_streaming as u8]);
    h.update(instance_scene_data_offset.to_le_bytes());
    h.update(instance_count.to_le_bytes());
    h.update(payload_data_offset.to_le_bytes());
    h.update(payload_float4_count.to_le_bytes());
    h.update(material_record_hash.as_bytes());
    h.update(primitive_flags_word.to_le_bytes());
    h.update(bounds_hash.as_bytes());
    h.update(transform_hash.as_bytes());
    h.update(upload_range_hash.as_bytes());
    h.update(node.proof_hash.as_bytes());
    hex32(h.finalize().into())
}

fn gpu_scene_primitive_scene_data_hash(primitives: &[BangerNativeGpuScenePrimitive]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.primitive_scene_data.v1\0");
    for primitive in primitives {
        h.update(primitive.primitive_id.to_le_bytes());
        h.update(primitive.primitive_flags_word.to_le_bytes());
        h.update(primitive.bounds_hash.as_bytes());
        h.update(primitive.transform_hash.as_bytes());
        h.update(primitive.primitive_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_scene_instance_scene_data_hash(primitives: &[BangerNativeGpuScenePrimitive]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.instance_scene_data.v1\0");
    for primitive in primitives {
        h.update(primitive.primitive_id.to_le_bytes());
        h.update(primitive.instance_scene_data_offset.to_le_bytes());
        h.update(primitive.instance_count.to_le_bytes());
        h.update(primitive.upload_range_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_scene_instance_payload_data_hash(primitives: &[BangerNativeGpuScenePrimitive]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.instance_payload_data.v1\0");
    for primitive in primitives {
        h.update(primitive.primitive_id.to_le_bytes());
        h.update(primitive.payload_data_offset.to_le_bytes());
        h.update(primitive.payload_float4_count.to_le_bytes());
        h.update(primitive.material_record_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_scene_material_table_hash(
    primitives: &[BangerNativeGpuScenePrimitive],
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.material_table.v1\0");
    h.update(shader_compiler_ticket.material_abi_hash.as_bytes());
    h.update(shader_compiler_ticket.reflection_manifest.reflection_hash.as_bytes());
    for primitive in primitives {
        h.update(primitive.material_record_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_scene_upload_ranges_hash(primitives: &[BangerNativeGpuScenePrimitive]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.gpu_scene.upload_ranges.v1\0");
    for primitive in primitives {
        h.update(primitive.upload_range_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn gpu_scene_packet_hash(
    prepared: &MonsterPreparedCompute,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    resource_table: &BangerNativeResourceTable,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    primitive_scene_data_hash: &str,
    instance_scene_data_hash: &str,
    instance_payload_data_hash: &str,
    material_table_hash: &str,
    upload_ranges_hash: &str,
    primitives: &[BangerNativeGpuScenePrimitive],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_gpu_scene_packet.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(shader_compiler_ticket.material_abi_hash.as_bytes());
    h.update(primitive_scene_data_hash.as_bytes());
    h.update(instance_scene_data_hash.as_bytes());
    h.update(instance_payload_data_hash.as_bytes());
    h.update(material_table_hash.as_bytes());
    h.update(upload_ranges_hash.as_bytes());
    for primitive in primitives {
        h.update(primitive.primitive_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn newobject_edit_hash(
    base_manifest_hash: &str,
    updated_manifest_hash: &str,
    prepared: &MonsterPreparedCompute,
    object_id: &str,
    representation: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.newobject_edit.v1\0");
    h.update(base_manifest_hash.as_bytes());
    h.update(updated_manifest_hash.as_bytes());
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(object_id.as_bytes());
    h.update(representation.as_bytes());
    hex32(h.finalize().into())
}

fn editable_scene_manifest_hash(
    prepared: &MonsterPreparedCompute,
    resource_table: &BangerNativeResourceTable,
    objects: &[BangerEditableSceneObject],
    graph_hash: &str,
    bounds_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.editable_scene_manifest.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(graph_hash.as_bytes());
    h.update(bounds_hash.as_bytes());
    for object in objects {
        h.update(object.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn representation_mix_entry_hash(
    representation: &str,
    object_count: usize,
    renderable_count: usize,
    weight: f32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.representation_mix_entry.v1\0");
    h.update(representation.as_bytes());
    h.update((object_count as u64).to_le_bytes());
    h.update((renderable_count as u64).to_le_bytes());
    h.update(weight.to_le_bytes());
    hex32(h.finalize().into())
}

fn scene_submission_node_hash(
    object: &BangerEditableSceneObject,
    submission_order: u32,
    world_aabb_min: &[f32; 3],
    world_aabb_max: &[f32; 3],
    resource_slots: &[u32],
    render_graph_stages: &[&'static str],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.submission_node.v1\0");
    h.update(object.object_id.as_bytes());
    if let Some(parent_id) = &object.parent_id {
        h.update(parent_id.as_bytes());
    }
    h.update(object.representation.as_bytes());
    h.update([object.visible as u8]);
    h.update([object.renderable as u8]);
    h.update(submission_order.to_le_bytes());
    h.update(object.local_transform_hash.as_bytes());
    h.update(object.world_transform_hash.as_bytes());
    for value in world_aabb_min.iter().chain(world_aabb_max.iter()) {
        h.update(value.to_le_bytes());
    }
    for slot in resource_slots {
        h.update(slot.to_le_bytes());
    }
    for stage in render_graph_stages {
        h.update(stage.as_bytes());
    }
    hex32(h.finalize().into())
}

fn scene_transform_propagation_hash(objects: &[BangerEditableSceneObject]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.transform_propagation.v1\0");
    for object in objects {
        h.update(object.object_id.as_bytes());
        if let Some(parent_id) = &object.parent_id {
            h.update(parent_id.as_bytes());
        }
        h.update(object.local_transform_hash.as_bytes());
        h.update(object.world_transform_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn scene_visibility_hash(submissions: &[BangerNativeSceneSubmissionNode]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.visibility.v1\0");
    for node in submissions {
        h.update(node.object_id.as_bytes());
        h.update([node.visible as u8]);
        h.update([node.renderable as u8]);
        h.update(node.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn scene_representation_mix_hash(mix: &[BangerNativeRepresentationMixEntry]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.representation_mix.v1\0");
    for entry in mix {
        h.update(entry.representation.as_bytes());
        h.update((entry.object_count as u64).to_le_bytes());
        h.update((entry.renderable_count as u64).to_le_bytes());
        h.update(entry.weight.to_le_bytes());
        h.update(entry.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn scene_viewport_fit_hash(
    editable_scene_manifest: &BangerEditableSceneManifest,
    fit_bounds_min: &[f32; 3],
    fit_bounds_max: &[f32; 3],
    fit_bounding_sphere: &[f32; 4],
    visibility_hash: &str,
    representation_mix_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.viewport_fit.v1\0");
    h.update(editable_scene_manifest.graph_hash.as_bytes());
    h.update(editable_scene_manifest.bounds_hash.as_bytes());
    h.update(visibility_hash.as_bytes());
    h.update(representation_mix_hash.as_bytes());
    for value in fit_bounds_min.iter().chain(fit_bounds_max.iter()).chain(fit_bounding_sphere.iter()) {
        h.update(value.to_le_bytes());
    }
    hex32(h.finalize().into())
}

fn scene_render_submission_hash(
    submissions: &[BangerNativeSceneSubmissionNode],
    resource_table: &BangerNativeResourceTable,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.scene_graph.render_submission.v1\0");
    h.update(resource_table.table_hash.as_bytes());
    for node in submissions.iter().filter(|node| node.visible && node.renderable) {
        h.update(node.proof_hash.as_bytes());
        h.update(node.world_transform_hash.as_bytes());
        for slot in &node.resource_slots {
            h.update(slot.to_le_bytes());
        }
        for stage in &node.render_graph_stages {
            h.update(stage.as_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn scene_graph_submission_hash(
    prepared: &MonsterPreparedCompute,
    editable_scene_manifest: &BangerEditableSceneManifest,
    transform_propagation_hash: &str,
    visibility_hash: &str,
    representation_mix_hash: &str,
    viewport_fit_hash: &str,
    render_submission_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_scene_graph_submission.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(editable_scene_manifest.manifest_hash.as_bytes());
    h.update(transform_propagation_hash.as_bytes());
    h.update(visibility_hash.as_bytes());
    h.update(representation_mix_hash.as_bytes());
    h.update(viewport_fit_hash.as_bytes());
    h.update(render_submission_hash.as_bytes());
    hex32(h.finalize().into())
}

fn resource_heap_for_kind(kind: &str) -> &'static str {
    match kind {
        "material_payload" => "bindless_material_heap",
        "surfel_radiance_cache" => "lighting_cache_heap",
        "meshlet_page" => "meshlet_geometry_heap",
        "sdf_brick" | "voxel_page" => "virtual_geometry_page_heap",
        _ => "native_artifact_heap",
    }
}

fn resource_usage_for_kind(kind: &str) -> &'static str {
    match kind {
        "meshlet_page" => "storage_read_indirect_draw",
        "surfel_radiance_cache" => "storage_read_write_lighting",
        "material_payload" => "storage_read_material_table",
        "sdf_brick" | "voxel_page" => "storage_read_resident_page",
        _ => "storage_read",
    }
}

fn upload_lane_for_kind(kind: &str) -> &'static str {
    match kind {
        "material_payload" => "graphics_queue_bind_table",
        "meshlet_page" => "copy_queue_meshlet_stream",
        "surfel_radiance_cache" => "async_compute_lighting_stream",
        "sdf_brick" | "voxel_page" => "copy_queue_sparse_page_stream",
        _ => "copy_queue_native_artifact_stream",
    }
}

fn read_barrier_for_stage(stage: &str) -> &'static str {
    match stage {
        "resident_page_upload" => "copy_dst_to_shader_read",
        "visibility_cull" => "storage_read_to_indirect_args",
        "lighting_cache" => "storage_read_write_lighting",
        "material_bind" => "shader_read_material_table",
        _ => "shader_read",
    }
}

fn write_barrier_for_stage(stage: &str) -> &'static str {
    match stage {
        "resident_page_upload" => "copy_write_complete",
        "visibility_cull" => "indirect_args_write",
        "lighting_cache" => "lighting_cache_write",
        "material_bind" => "no_write",
        _ => "no_write",
    }
}

fn adapter_profile_hash(adapter: &NativeGpuAdapter) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.pipeline_cache.adapter_profile.v1\0");
    h.update(adapter.name.as_bytes());
    h.update(adapter.vendor_id.to_le_bytes());
    h.update(adapter.device_id.to_le_bytes());
    h.update(adapter.backend.as_bytes());
    h.update(adapter.device_type.as_bytes());
    h.update(adapter.driver.as_bytes());
    h.update(adapter.driver_info.as_bytes());
    hex32(h.finalize().into())
}

fn adapter_profile_label(adapter: &NativeGpuAdapter) -> String {
    format!(
        "{}::{}::{}::vendor={:04x}::device={:04x}",
        adapter.name, adapter.backend, adapter.device_type, adapter.vendor_id, adapter.device_id
    )
}

fn shader_reflection_hash(
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    artifact: &MonsterNativeTandemArtifact,
    pass: &BangerNativeRenderPass,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.shader_reflection_manifest_bound.v1\0");
    h.update(shader_compiler_ticket.proof_hash.as_bytes());
    h.update(shader_compiler_ticket.source_language.as_bytes());
    h.update(shader_compiler_ticket.promoted_target.as_bytes());
    h.update(shader_compiler_ticket.reflection_manifest.reflection_hash.as_bytes());
    h.update(shader_compiler_ticket.material_abi_hash.as_bytes());
    h.update(artifact.kind.as_bytes());
    h.update(artifact.layout.as_bytes());
    h.update(pass.stage.as_bytes());
    h.update(pass.writes.as_bytes());
    hex32(h.finalize().into())
}

fn render_pass_abi_hash(
    artifact: &MonsterNativeTandemArtifact,
    pass: &BangerNativeRenderPass,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.render_pass_abi.v1\0");
    h.update(pass.name.as_bytes());
    h.update(pass.stage.as_bytes());
    h.update(pass.consumes_kind.as_bytes());
    h.update(pass.writes.as_bytes());
    h.update(pass.cache_class.as_bytes());
    h.update(pass.residency_policy.as_bytes());
    h.update(resource_heap_for_kind(artifact.kind).as_bytes());
    h.update(resource_usage_for_kind(artifact.kind).as_bytes());
    h.update(read_barrier_for_stage(pass.stage).as_bytes());
    h.update(write_barrier_for_stage(pass.stage).as_bytes());
    h.update(artifact.layout.as_bytes());
    hex32(h.finalize().into())
}

fn pipeline_cache_seed_blob_bytes(
    prepared: &MonsterPreparedCompute,
    artifact: &MonsterNativeTandemArtifact,
    pass: &BangerNativeRenderPass,
    pipeline_cache_key: &str,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    adapter_hash: &str,
    driver_hash: &str,
    shader_reflection_hash: &str,
    shader_target_manifest_hash: &str,
    material_abi_hash: &str,
    render_pass_abi_hash: &str,
) -> Vec<u8> {
    format!(
        concat!(
            "schema=forge.banger.native_pipeline_cache_seed_blob.v1\n",
            "manifest_hash={}\n",
            "compute_ir_hash={}\n",
            "pipeline_cache_key={}\n",
            "adapter_hash={}\n",
            "driver_hash={}\n",
            "shader_ticket_hash={}\n",
            "shader_source_hash={}\n",
            "shader_reflection_hash={}\n",
            "shader_target_manifest_hash={}\n",
            "material_abi_hash={}\n",
            "render_pass_abi_hash={}\n",
            "artifact_kind={}\n",
            "artifact_layout={}\n",
            "artifact_hash={}\n",
            "renderer_variant_hash={}\n",
            "pass_name={}\n",
            "pass_stage={}\n"
        ),
        prepared.manifest_hash,
        prepared.route.plan.compute_ir_hash,
        pipeline_cache_key,
        adapter_hash,
        driver_hash,
        shader_compiler_ticket.proof_hash,
        shader_compiler_ticket.mini_probe_source_hash,
        shader_reflection_hash,
        shader_target_manifest_hash,
        material_abi_hash,
        render_pass_abi_hash,
        artifact.kind,
        artifact.layout,
        artifact.artifact_hash,
        artifact.renderer_variant_hash,
        pass.name,
        pass.stage
    )
    .into_bytes()
}

fn persist_pipeline_cache_seed_blob(path: &Path, bytes: &[u8]) -> Result<&'static str, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create pipeline cache directory failed: {err}"))?;
    }
    if path.exists() {
        let existing = fs::read(path).map_err(|err| format!("read pipeline cache blob failed: {err}"))?;
        if existing == bytes {
            return Ok("seed_blob_persisted");
        }
        return Err(format!(
            "pipeline cache blob hash collision or non-content-addressed overwrite at {}",
            path.display()
        ));
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let tmp_path = path.with_extension(format!("tmp-{}-{stamp}", process::id()));
    fs::write(&tmp_path, bytes).map_err(|err| format!("write pipeline cache temp blob failed: {err}"))?;
    fs::rename(&tmp_path, path).map_err(|err| format!("commit pipeline cache blob failed: {err}"))?;
    Ok("seed_blob_persisted")
}

fn pipeline_cache_entry_proof_hash(
    prepared: &MonsterPreparedCompute,
    artifact: &MonsterNativeTandemArtifact,
    pass: &BangerNativeRenderPass,
    pipeline_cache_key: &str,
    adapter_hash: &str,
    driver_hash: &str,
    shader_source_hash: &str,
    shader_reflection_hash: &str,
    shader_target_manifest_hash: &str,
    material_abi_hash: &str,
    render_pass_abi_hash: &str,
    blob_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_pipeline_cache_entry.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(artifact.renderer_variant_hash.as_bytes());
    h.update(pass.proof_hash.as_bytes());
    h.update(pipeline_cache_key.as_bytes());
    h.update(adapter_hash.as_bytes());
    h.update(driver_hash.as_bytes());
    h.update(shader_source_hash.as_bytes());
    h.update(shader_reflection_hash.as_bytes());
    h.update(shader_target_manifest_hash.as_bytes());
    h.update(material_abi_hash.as_bytes());
    h.update(render_pass_abi_hash.as_bytes());
    h.update(blob_hash.as_bytes());
    hex32(h.finalize().into())
}

fn pipeline_cache_manifest_hash(
    prepared: &MonsterPreparedCompute,
    entries: &[BangerNativePipelineCacheEntry],
    adapter_hash: &str,
    driver_hash: &str,
    driver_info_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_pipeline_cache_manifest.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(prepared.gpu_batch_plan.plan_hash.as_bytes());
    h.update(adapter_hash.as_bytes());
    h.update(driver_hash.as_bytes());
    h.update(driver_info_hash.as_bytes());
    for entry in entries {
        h.update(entry.proof_hash.as_bytes());
        h.update(entry.blob_hash.as_bytes());
        h.update(entry.pipeline_cache_key.as_bytes());
    }
    hex32(h.finalize().into())
}

fn estimated_banger_frame_ms(
    prepared: &MonsterPreparedCompute,
    render_graph: &[BangerNativeRenderPass],
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    resource_table: &BangerNativeResourceTable,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
) -> f32 {
    let resident_mb = resource_table.resident_bytes as f32 / (1024.0 * 1024.0);
    let cold_cache_penalty = if prepared.is_fully_cached() { 0.0 } else { 0.45 };
    let cull_cost = culling_manifest.candidate_count as f32 * 0.035;
    let radiance_cost = radiance_schedule_manifest.probe_page_count as f32 * 0.08;
    let splat_cost = (gaussian_splat_layer_manifest.layer_count
        + gaussian_splat_layer_manifest.conversion_count) as f32
        * 0.12;
    let render_graph_cost = render_graph.len() as f32 * 0.32;
    let pipeline_cost = pipeline_cache_manifest.entry_count as f32 * 0.04;
    let visible_cost = scene_graph_submission.visible_object_count as f32 * 0.025;
    1.75
        + render_graph_cost
        + pipeline_cost
        + resident_mb * 0.01
        + visible_cost
        + cull_cost
        + radiance_cost
        + splat_cost
        + cold_cache_penalty
}

fn benchmark_gate(
    name: &'static str,
    metric: &'static str,
    threshold: f32,
    measured: f32,
    lower_is_better: bool,
) -> BangerNativeBenchmarkGate {
    let passed = if lower_is_better {
        measured <= threshold
    } else {
        measured >= threshold
    };
    let proof_hash = benchmark_gate_hash(name, metric, threshold, measured, passed);
    BangerNativeBenchmarkGate {
        name,
        metric,
        threshold,
        measured,
        passed,
        proof_hash,
    }
}

fn benchmark_gate_hash(
    name: &str,
    metric: &str,
    threshold: f32,
    measured: f32,
    passed: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.benchmark_gate.v1\0");
    h.update(name.as_bytes());
    h.update(metric.as_bytes());
    h.update(threshold.to_le_bytes());
    h.update(measured.to_le_bytes());
    h.update([passed as u8]);
    hex32(h.finalize().into())
}

fn benchmark_proof_reproducibility_hash(
    prepared: &MonsterPreparedCompute,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    resource_table: &BangerNativeResourceTable,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.benchmark_proof_reproducibility.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(prepared.gpu_batch_plan.plan_hash.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(resource_table.table_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(culling_manifest.manifest_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(gaussian_splat_layer_manifest.manifest_hash.as_bytes());
    h.update(shader_compiler_ticket.proof_hash.as_bytes());
    hex32(h.finalize().into())
}

fn visual_capability_score(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
) -> u32 {
    let mut score = 0u32;
    if texture_bridge_contract.width > 0
        && texture_bridge_contract.height > 0
        && texture_bridge_contract.texture_usage.iter().any(|usage| *usage == "RENDER_ATTACHMENT")
    {
        score += 20;
    }
    if scene_graph_submission.renderable_object_count > 0
        && scene_graph_submission.representation_mix_hash.len() == 64
    {
        score += 20;
    }
    if gpu_scene_packet.primitive_count > 0
        && gpu_scene_packet.primitive_scene_data_hash.len() == 64
        && gpu_scene_packet.instance_scene_data_hash.len() == 64
    {
        score += 10;
    }
    if culling_manifest.visible_count > 0 && culling_manifest.indirect_draw_buffer_hash.len() == 64 {
        score += 15;
    }
    if radiance_schedule_manifest.active_probe_count > 0
        && radiance_schedule_manifest.schedule_hash.len() == 64
    {
        score += 15;
    }
    if gaussian_splat_layer_manifest.manifest_hash.len() == 64
        && gaussian_splat_layer_manifest.conversion_manifest_hash.len() == 64
    {
        score += 15;
    }
    if shader_compiler_ticket.target_manifest_hash.len() == 64
        && shader_compiler_ticket.reflection_manifest.reflection_hash.len() == 64
        && shader_compiler_ticket.material_abi_hash.len() == 64
    {
        score += 15;
    }
    score.min(100)
}

fn visual_capability_hash(
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    score: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.visual_capability_score.v1\0");
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(scene_graph_submission.representation_mix_hash.as_bytes());
    h.update(culling_manifest.indirect_draw_buffer_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(gaussian_splat_layer_manifest.manifest_hash.as_bytes());
    h.update(shader_compiler_ticket.target_manifest_hash.as_bytes());
    h.update(shader_compiler_ticket.reflection_manifest.reflection_hash.as_bytes());
    h.update(score.to_le_bytes());
    hex32(h.finalize().into())
}

fn benchmark_promotion_manifest_hash(
    target_frame_ms: f32,
    estimated_frame_ms: f32,
    frame_time_headroom_pct: f32,
    vram_pressure_pct: f32,
    cache_reuse_ratio: f32,
    proof_reproducibility_status: &str,
    proof_reproducibility_hash: &str,
    visual_capability_score: u32,
    visual_capability_hash: &str,
    gates: &[BangerNativeBenchmarkGate],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.benchmark_promotion_manifest.v1\0");
    h.update(target_frame_ms.to_le_bytes());
    h.update(estimated_frame_ms.to_le_bytes());
    h.update(frame_time_headroom_pct.to_le_bytes());
    h.update(vram_pressure_pct.to_le_bytes());
    h.update(cache_reuse_ratio.to_le_bytes());
    h.update(proof_reproducibility_status.as_bytes());
    h.update(proof_reproducibility_hash.as_bytes());
    h.update(visual_capability_score.to_le_bytes());
    h.update(visual_capability_hash.as_bytes());
    for gate in gates {
        h.update(gate.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_resource_key(
    prepared: &MonsterPreparedCompute,
    artifact: &MonsterNativeTandemArtifact,
    page_index: u64,
    page_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_resource_slot.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(artifact.renderer_cache_hash.as_bytes());
    h.update(artifact.kind.as_bytes());
    h.update(page_index.to_le_bytes());
    h.update(page_hash.as_bytes());
    hex32(h.finalize().into())
}

fn resource_table_hash(
    prepared: &MonsterPreparedCompute,
    slots: &[BangerNativeResourceSlot],
    resident_bytes: u64,
    upload_bytes: u64,
    vram_budget_mb: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_resource_table.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(resident_bytes.to_le_bytes());
    h.update(upload_bytes.to_le_bytes());
    h.update(vram_budget_mb.to_le_bytes());
    for slot in slots {
        h.update(slot.slot.to_le_bytes());
        h.update(slot.kind.as_bytes());
        h.update(slot.page_index.to_le_bytes());
        h.update(slot.page_hash.as_bytes());
        h.update(slot.resource_key.as_bytes());
        h.update(slot.pipeline_cache_key.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_render_graph_resource_hash(
    resource_name: &str,
    pass: &BangerNativeRenderPass,
    binding: &BangerNativeFrameGraphBinding,
    resource_keys: &[&str],
    resident_bytes: u64,
    upload_bytes: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_resource.v1\0");
    h.update(resource_name.as_bytes());
    h.update(pass.name.as_bytes());
    h.update(pass.stage.as_bytes());
    h.update(pass.consumes_kind.as_bytes());
    h.update(pass.writes.as_bytes());
    h.update(binding.pipeline_cache_key.as_bytes());
    h.update(resident_bytes.to_le_bytes());
    h.update(upload_bytes.to_le_bytes());
    for key in resource_keys {
        h.update(key.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_render_graph_compiled_pass_hash(
    order: u32,
    pass: &BangerNativeRenderPass,
    binding: &BangerNativeFrameGraphBinding,
    resource_name: &str,
    resource_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_compiled_pass.v1\0");
    h.update(order.to_le_bytes());
    h.update(pass.name.as_bytes());
    h.update(pass.stage.as_bytes());
    h.update(pass.consumes_kind.as_bytes());
    h.update(pass.cache_class.as_bytes());
    h.update(binding.read_barrier.as_bytes());
    h.update(binding.write_barrier.as_bytes());
    h.update(binding.pipeline_cache_key.as_bytes());
    h.update(resource_name.as_bytes());
    h.update(resource_hash.as_bytes());
    h.update([binding.async_compute_candidate as u8]);
    hex32(h.finalize().into())
}

fn banger_render_graph_edge_hash(
    from_pass: &str,
    to_pass: &str,
    resource_name: &str,
    resource_hash: &str,
    read_barrier: &str,
    write_barrier: &str,
    async_boundary: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_edge.v1\0");
    h.update(from_pass.as_bytes());
    h.update(to_pass.as_bytes());
    h.update(resource_name.as_bytes());
    h.update(resource_hash.as_bytes());
    h.update(read_barrier.as_bytes());
    h.update(write_barrier.as_bytes());
    h.update([async_boundary as u8]);
    hex32(h.finalize().into())
}

fn banger_render_graph_order_hash(passes: &[BangerNativeRenderGraphCompiledPass]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_order.v1\0");
    for pass in passes {
        h.update(pass.order.to_le_bytes());
        h.update(pass.pass_name.as_bytes());
        h.update(pass.stage.as_bytes());
        h.update(pass.pass_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_render_graph_lifetime_hash(resources: &[BangerNativeRenderGraphResource]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_lifetime.v1\0");
    for resource in resources {
        h.update(resource.name.as_bytes());
        h.update(resource.first_stage.as_bytes());
        h.update(resource.last_stage.as_bytes());
        h.update(resource.slot_count.to_le_bytes());
        h.update(resource.resident_bytes.to_le_bytes());
        h.update(resource.upload_bytes.to_le_bytes());
        h.update([resource.extracted as u8]);
        h.update(resource.resource_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_render_graph_barrier_hash(edges: &[BangerNativeRenderGraphEdge]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_barrier_plan.v1\0");
    for edge in edges {
        h.update(edge.from_pass.as_bytes());
        h.update(edge.to_pass.as_bytes());
        h.update(edge.resource_hash.as_bytes());
        h.update(edge.read_barrier.as_bytes());
        h.update(edge.write_barrier.as_bytes());
        h.update([edge.async_boundary as u8]);
        h.update(edge.edge_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_render_graph_kasm_contract_hash(
    prepared: &MonsterPreparedCompute,
    render_graph: &[BangerNativeRenderPass],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.monster_kasm_render_graph_contract.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.source_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(prepared.gpu_batch_plan.plan_hash.as_bytes());
    for pass in render_graph {
        h.update(pass.name.as_bytes());
        h.update(pass.stage.as_bytes());
        h.update(pass.consumes_kind.as_bytes());
        h.update(pass.writes.as_bytes());
        h.update(pass.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_render_graph_manifest_hash(
    prepared: &MonsterPreparedCompute,
    compiled_order_hash: &str,
    resource_lifetime_hash: &str,
    barrier_plan_hash: &str,
    monster_kasm_contract_hash: &str,
    passes: &[BangerNativeRenderGraphCompiledPass],
    resources: &[BangerNativeRenderGraphResource],
    edges: &[BangerNativeRenderGraphEdge],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_graph_compilation.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(compiled_order_hash.as_bytes());
    h.update(resource_lifetime_hash.as_bytes());
    h.update(barrier_plan_hash.as_bytes());
    h.update(monster_kasm_contract_hash.as_bytes());
    for pass in passes {
        h.update(pass.pass_hash.as_bytes());
    }
    for resource in resources {
        h.update(resource.resource_hash.as_bytes());
    }
    for edge in edges {
        h.update(edge.edge_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

impl From<&MonsterNativeTandemArtifact> for BangerNativeRenderArtifactSummary {
    fn from(artifact: &MonsterNativeTandemArtifact) -> Self {
        Self {
            name: artifact.name.clone(),
            kind: artifact.kind,
            layout: artifact.layout,
            byte_len: artifact.byte_len,
            page_count: artifact.page_count,
            artifact_hash: artifact.artifact_hash.clone(),
            renderer_cache_hash: artifact.renderer_cache_hash.clone(),
            renderer_variant_hash: artifact.renderer_variant_hash.clone(),
            renderer_promotion_hash: artifact.renderer_promotion_hash.clone(),
            source_output_hash: artifact.source_output_hash.clone(),
            first_page_hash: artifact.pages.first().map(|page| page.page_hash.clone()),
            last_page_hash: artifact.pages.last().map(|page| page.page_hash.clone()),
            page_hashes: artifact.pages.iter().map(|page| page.page_hash.clone()).collect(),
        }
    }
}

fn render_pass_from_artifact(artifact: &MonsterNativeTandemArtifact) -> BangerNativeRenderPass {
    let (name, stage, writes, async_compute_candidate) = match artifact.kind {
        "sdf_brick" => (
            "sdf_brick_residency",
            "resident_page_upload",
            "sdf_brick_pool",
            true,
        ),
        "voxel_page" => (
            "voxel_page_residency",
            "resident_page_upload",
            "voxel_page_pool",
            true,
        ),
        "meshlet_page" => (
            "meshlet_visibility",
            "visibility_cull",
            "meshlet_indirect_draw_stream",
            true,
        ),
        "surfel_radiance_cache" => (
            "radiance_cache_update",
            "lighting_cache",
            "surfel_radiance_cache",
            true,
        ),
        "shadow_page" => (
            "virtual_shadow_depth",
            "shadow_depth",
            "virtual_shadow_page_table",
            true,
        ),
        "material_payload" => (
            "material_payload_bind",
            "material_bind",
            "material_payload_table",
            false,
        ),
        _ => (
            "native_artifact_bind",
            "native_artifact_bind",
            "native_artifact_table",
            false,
        ),
    };
    BangerNativeRenderPass {
        name,
        stage,
        consumes_kind: artifact.kind,
        writes,
        cache_class: artifact.renderer_cache_class,
        residency_policy: artifact.renderer_residency_policy,
        async_compute_candidate,
        proof_hash: artifact.renderer_promotion_hash.clone(),
    }
}

fn residency_job_from_artifact(
    artifact: &MonsterNativeTandemArtifact,
    prepared: &MonsterPreparedCompute,
) -> BangerNativeResidencyJob {
    let pass = render_pass_from_artifact(artifact);
    let update_mode = if prepared.is_fully_cached() {
        "residentCacheHit"
    } else if artifact.page_count > 1 {
        "sparsePageUpload"
    } else {
        "singlePageUpload"
    };
    BangerNativeResidencyJob {
        artifact_name: artifact.name.clone(),
        kind: artifact.kind,
        render_graph_stage: pass.stage,
        update_mode,
        page_count: artifact.page_count,
        byte_len: artifact.byte_len,
        page_hashes: artifact.pages.iter().map(|page| page.page_hash.clone()).collect(),
        cache_key: artifact.renderer_cache_hash.clone(),
        promotion_hash: artifact.renderer_promotion_hash.clone(),
        frame_blocking: prepared.route.plan.frame_blocking_allowed && !pass.async_compute_candidate,
    }
}

fn shader_profiles(prepared: &MonsterPreparedCompute) -> Vec<String> {
    let mut profiles = BTreeSet::new();
    for kernel in &prepared.gpu_batch_plan.kernels {
        profiles.insert(kernel.shader_profile.to_string());
    }
    profiles.into_iter().collect()
}

fn shader_compiler_ticket(
    prepared: &MonsterPreparedCompute,
    gpu_shader_profiles: &[String],
    prefer_mesh_shaders: bool,
) -> BangerNativeShaderCompilerTicket {
    let compiler_version = detect_slang_version();
    let compiler_detected = compiler_version.is_some();
    let compiler_version_excerpt = compiler_version
        .as_deref()
        .map(compact_one_line)
        .unwrap_or_else(|| "slangc_not_found_on_path".to_string());
    let compiler_version_hash = hex32(Sha256::digest(compiler_version_excerpt.as_bytes()).into());
    let mini_probe = run_slang_mini_probe(compiler_detected);
    let target_artifacts = build_slang_target_artifacts(compiler_detected, &mini_probe.source);
    let target_manifest_hash = slang_target_manifest_hash(&target_artifacts);
    let fallback_wgsl = banger_wgsl_mini_compute_source();
    let fallback_wgsl_hash = hash_text_hex("forge.banger.slang_fallback_wgsl.v1", &fallback_wgsl);
    let fallback_wgsl_parity_hash =
        slang_fallback_wgsl_parity_hash(&fallback_wgsl_hash, &target_artifacts, &mini_probe);
    let material_abi = banger_shader_material_abi();
    let material_abi_hash = material_abi.layout_hash.clone();
    let reflection_manifest = banger_shader_reflection_manifest(&material_abi_hash);
    let promoted_target = if compiler_detected && target_artifacts.iter().any(|artifact| artifact.status == "compiled") {
        "slang_multi_target_manifest_v1"
    } else {
        "wgsl_inline_bootstrap_with_slang_abi_manifest"
    };
    let module_strategy = if compiler_detected {
        "compile_slang_module_to_wgsl_spirv_hlsl_msl_then_bind_reflection_manifest"
    } else {
        "inline_wgsl_with_slang_reflection_abi_until_slangc_or_api_binding_is_available"
    };
    let reflection_status = if compiler_detected && target_artifacts.iter().any(|artifact| artifact.status == "compiled") {
        "slang_targets_compiled_reflection_manifest_bound_api_binding_next"
    } else {
        "deterministic_reflection_manifest_bound_to_fallback_wgsl"
    };

    let mut h = Sha256::new();
    h.update(b"forge.banger.native_shader_compiler_ticket.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(prepared.gpu_batch_plan.plan_hash.as_bytes());
    h.update(compiler_version_hash.as_bytes());
    h.update(mini_probe.source_hash.as_bytes());
    h.update(mini_probe.output_hash.as_bytes());
    h.update(mini_probe.status.as_bytes());
    h.update(target_manifest_hash.as_bytes());
    h.update(fallback_wgsl_hash.as_bytes());
    h.update(fallback_wgsl_parity_hash.as_bytes());
    h.update(material_abi_hash.as_bytes());
    h.update(reflection_manifest.reflection_hash.as_bytes());
    h.update([prefer_mesh_shaders as u8]);
    for profile in gpu_shader_profiles {
        h.update(profile.as_bytes());
    }
    h.update(promoted_target.as_bytes());
    h.update(module_strategy.as_bytes());
    h.update(reflection_status.as_bytes());

    BangerNativeShaderCompilerTicket {
        schema: "forge.banger.native_shader_compiler_ticket.v1",
        preferred_compiler: "slangc",
        compiler_detected,
        compiler_version_hash,
        compiler_version_excerpt,
        mini_probe_source_hash: mini_probe.source_hash,
        mini_probe_output_hash: mini_probe.output_hash,
        mini_probe_status: mini_probe.status,
        mini_probe_excerpt: mini_probe.excerpt,
        mini_probe_wgsl: mini_probe.wgsl,
        requested_targets: vec!["wgsl", "spirv", "hlsl", "msl"],
        promoted_target,
        bootstrap_target: "wgsl",
        source_language: "slang",
        module_strategy,
        reflection_status,
        target_manifest_hash,
        fallback_wgsl_hash,
        fallback_wgsl_parity_hash,
        material_abi_hash,
        target_artifacts,
        reflection_manifest,
        material_abi,
        proof_hash: hex32(h.finalize().into()),
    }
}

fn detect_slang_version() -> Option<String> {
    let output = Command::new("slangc").arg("-version").output().ok()?;
    if output.status.success() {
        let mut text = String::new();
        text.push_str(&String::from_utf8_lossy(&output.stdout));
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        Some(text)
    } else {
        None
    }
}

struct SlangMiniProbe {
    source: String,
    source_hash: String,
    output_hash: String,
    status: &'static str,
    excerpt: String,
    wgsl: Option<String>,
}

fn run_slang_mini_probe(compiler_detected: bool) -> SlangMiniProbe {
    let source = banger_slang_mini_compute_source();
    let source_hash = hex32(Sha256::digest(source.as_bytes()).into());
    if !compiler_detected {
        return SlangMiniProbe {
            source,
            source_hash,
            output_hash: hex32(Sha256::digest(b"slangc_not_found").into()),
            status: "compiler_absent",
            excerpt: "slangc_not_found_on_path".to_string(),
            wgsl: None,
        };
    }

    match compile_slang_mini_probe_to_wgsl(&source) {
        Ok(wgsl) => SlangMiniProbe {
            source,
            source_hash,
            output_hash: hex32(Sha256::digest(wgsl.as_bytes()).into()),
            status: "compiled_wgsl",
            excerpt: compact_one_line(&wgsl),
            wgsl: Some(wgsl),
        },
        Err(err) => SlangMiniProbe {
            source,
            source_hash,
            output_hash: hex32(Sha256::digest(err.as_bytes()).into()),
            status: "compile_failed",
            excerpt: compact_one_line(&err),
            wgsl: None,
        },
    }
}

fn compile_slang_mini_probe_to_wgsl(source: &str) -> Result<String, String> {
    compile_slang_mini_probe_to_target(source, "wgsl", "wgsl").and_then(|bytes| {
        String::from_utf8(bytes).map_err(|err| format!("slang wgsl output is not UTF-8: {err}"))
    })
}

fn build_slang_target_artifacts(
    compiler_detected: bool,
    source: &str,
) -> Vec<BangerNativeShaderTargetArtifact> {
    ["wgsl", "spirv", "hlsl", "msl"]
        .into_iter()
        .map(|target| {
            let output_format = match target {
                "spirv" => "spirv_binary",
                "hlsl" => "hlsl_text",
                "msl" => "metal_text",
                _ => "wgsl_text",
            };
            slang_target_artifact(compiler_detected, source, target, output_format)
        })
        .collect()
}

fn slang_target_artifact(
    compiler_detected: bool,
    source: &str,
    target: &'static str,
    output_format: &'static str,
) -> BangerNativeShaderTargetArtifact {
    let source_hash = hash_text_hex("forge.banger.slang_shader_source.v1", source);
    let (status, bytes, diagnostic) = if compiler_detected {
        match compile_slang_mini_probe_to_target(source, target, output_format) {
            Ok(bytes) => ("compiled", bytes, Vec::new()),
            Err(err) => ("compile_failed", Vec::new(), err.into_bytes()),
        }
    } else {
        (
            "compiler_absent_fallback_declared",
            Vec::new(),
            b"slangc_not_found_on_path".to_vec(),
        )
    };
    let output_hash = hash_bytes_hex("forge.banger.slang_target_output.v1", &bytes);
    let diagnostic_hash = hash_bytes_hex("forge.banger.slang_target_diagnostic.v1", &diagnostic);
    let artifact_hash = slang_target_artifact_hash(
        target,
        output_format,
        status,
        &source_hash,
        &output_hash,
        &diagnostic_hash,
        bytes.len() as u64,
    );
    BangerNativeShaderTargetArtifact {
        target,
        output_format,
        entry_point: "computeMain",
        status,
        source_hash,
        output_hash,
        diagnostic_hash,
        byte_len: bytes.len() as u64,
        artifact_hash,
    }
}

fn compile_slang_mini_probe_to_target(
    source: &str,
    target: &str,
    output_format: &str,
) -> Result<Vec<u8>, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let dir = env::temp_dir().join(format!("forge-banger-slang-probe-{}-{stamp}", process::id()));
    fs::create_dir_all(&dir).map_err(|err| format!("create temp slang probe dir failed: {err}"))?;
    let source_path = dir.join("banger_probe.slang");
    let extension = match output_format {
        "spirv_binary" => "spv",
        "hlsl_text" => "hlsl",
        "metal_text" => "metal",
        _ => "wgsl",
    };
    let out_path = dir.join(format!("banger_probe.{extension}"));
    fs::write(&source_path, source).map_err(|err| format!("write slang probe failed: {err}"))?;
    let output = Command::new("slangc")
        .arg(&source_path)
        .arg("-target")
        .arg(target)
        .arg("-entry")
        .arg("computeMain")
        .arg("-o")
        .arg(&out_path)
        .output()
        .map_err(|err| format!("run slangc probe failed: {err}"))?;
    let mut diagnostic = String::new();
    diagnostic.push_str(&String::from_utf8_lossy(&output.stdout));
    diagnostic.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        return Err(diagnostic);
    }
    fs::read(&out_path).map_err(|err| format!("read slang {target} probe failed: {err}; {diagnostic}"))
}

fn slang_target_artifact_hash(
    target: &str,
    output_format: &str,
    status: &str,
    source_hash: &str,
    output_hash: &str,
    diagnostic_hash: &str,
    byte_len: u64,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.slang_target_artifact.v1\0");
    h.update(target.as_bytes());
    h.update(output_format.as_bytes());
    h.update(status.as_bytes());
    h.update(source_hash.as_bytes());
    h.update(output_hash.as_bytes());
    h.update(diagnostic_hash.as_bytes());
    h.update(byte_len.to_le_bytes());
    hex32(h.finalize().into())
}

fn slang_target_manifest_hash(artifacts: &[BangerNativeShaderTargetArtifact]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.slang_target_manifest.v1\0");
    for artifact in artifacts {
        h.update(artifact.artifact_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn slang_fallback_wgsl_parity_hash(
    fallback_wgsl_hash: &str,
    artifacts: &[BangerNativeShaderTargetArtifact],
    mini_probe: &SlangMiniProbe,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.slang_fallback_wgsl_parity.v1\0");
    h.update(fallback_wgsl_hash.as_bytes());
    h.update(mini_probe.output_hash.as_bytes());
    h.update(mini_probe.status.as_bytes());
    for artifact in artifacts {
        h.update(artifact.target.as_bytes());
        h.update(artifact.status.as_bytes());
        h.update(artifact.output_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_slang_mini_compute_source() -> String {
    r#"RWStructuredBuffer<uint> result;

[shader("compute")]
[numthreads(1, 1, 1)]
void computeMain(uint3 threadId : SV_DispatchThreadID)
{
    uint index = threadId.x;
    result[index] = 0x424e4753u ^ index;
}
"#
    .to_string()
}

fn banger_wgsl_mini_compute_source() -> String {
    r#"@group(0) @binding(0)
var<storage, read_write> result: array<u32>;

@compute @workgroup_size(1, 1, 1)
fn computeMain(@builtin(global_invocation_id) thread_id: vec3<u32>) {
    let index = thread_id.x;
    result[index] = 0x424e4753u ^ index;
}
"#
    .to_string()
}

fn banger_shader_material_abi() -> BangerNativeShaderMaterialAbi {
    let layout_hash = banger_shader_material_abi_hash(
        "banger_material_payload_v1",
        1,
        0,
        1,
        16,
        64,
        16,
        16,
    );
    BangerNativeShaderMaterialAbi {
        schema: "forge.banger.shader_material_abi.v1",
        abi_name: "banger_material_payload_v1",
        bind_group: 1,
        material_buffer_binding: 0,
        texture_binding_base: 1,
        sampler_binding_base: 16,
        material_record_bytes: 64,
        material_record_alignment: 16,
        max_texture_slots: 16,
        layout_hash,
    }
}

fn banger_shader_reflection_manifest(material_abi_hash: &str) -> BangerNativeShaderReflectionManifest {
    let mut bindings = vec![
        shader_reflection_binding("result", "storage_buffer", 0, 0, "read_write", "u32_words", 4),
        shader_reflection_binding(
            "material_payloads",
            "storage_buffer",
            1,
            0,
            "read",
            "banger_material_payload_v1",
            64,
        ),
        shader_reflection_binding("material_textures", "texture_array", 1, 1, "sample", "rgba_sampled", 0),
        shader_reflection_binding("material_samplers", "sampler_array", 1, 16, "sample", "filtering_sampler", 0),
    ];
    bindings.sort_by_key(|binding| (binding.group, binding.binding));
    let reflection_hash = banger_shader_reflection_manifest_hash(material_abi_hash, &bindings);
    BangerNativeShaderReflectionManifest {
        schema: "forge.banger.shader_reflection_manifest.v1",
        entry_point: "computeMain",
        stage: "compute",
        binding_count: bindings.len(),
        storage_buffer_count: bindings
            .iter()
            .filter(|binding| binding.category == "storage_buffer")
            .count(),
        read_write_buffer_count: bindings
            .iter()
            .filter(|binding| binding.access == "read_write")
            .count(),
        material_abi_hash: material_abi_hash.to_string(),
        reflection_hash,
        bindings,
    }
}

fn shader_reflection_binding(
    name: &'static str,
    category: &'static str,
    group: u32,
    binding: u32,
    access: &'static str,
    payload_kind: &'static str,
    byte_stride: u32,
) -> BangerNativeShaderReflectionBinding {
    let proof_hash = shader_reflection_binding_hash(
        name,
        category,
        group,
        binding,
        access,
        payload_kind,
        byte_stride,
    );
    BangerNativeShaderReflectionBinding {
        name,
        category,
        group,
        binding,
        access,
        payload_kind,
        byte_stride,
        proof_hash,
    }
}

fn shader_reflection_binding_hash(
    name: &str,
    category: &str,
    group: u32,
    binding: u32,
    access: &str,
    payload_kind: &str,
    byte_stride: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.shader_reflection_binding.v1\0");
    h.update(name.as_bytes());
    h.update(category.as_bytes());
    h.update(group.to_le_bytes());
    h.update(binding.to_le_bytes());
    h.update(access.as_bytes());
    h.update(payload_kind.as_bytes());
    h.update(byte_stride.to_le_bytes());
    hex32(h.finalize().into())
}

fn banger_shader_reflection_manifest_hash(
    material_abi_hash: &str,
    bindings: &[BangerNativeShaderReflectionBinding],
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.shader_reflection_manifest.v1\0");
    h.update(material_abi_hash.as_bytes());
    for binding in bindings {
        h.update(binding.proof_hash.as_bytes());
    }
    hex32(h.finalize().into())
}

fn banger_shader_material_abi_hash(
    abi_name: &str,
    bind_group: u32,
    material_buffer_binding: u32,
    texture_binding_base: u32,
    sampler_binding_base: u32,
    material_record_bytes: u32,
    material_record_alignment: u32,
    max_texture_slots: u32,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.shader_material_abi.v1\0");
    h.update(abi_name.as_bytes());
    h.update(bind_group.to_le_bytes());
    h.update(material_buffer_binding.to_le_bytes());
    h.update(texture_binding_base.to_le_bytes());
    h.update(sampler_binding_base.to_le_bytes());
    h.update(material_record_bytes.to_le_bytes());
    h.update(material_record_alignment.to_le_bytes());
    h.update(max_texture_slots.to_le_bytes());
    hex32(h.finalize().into())
}

fn compact_one_line(raw: &str) -> String {
    let mut out = raw
        .split_whitespace()
        .take(24)
        .collect::<Vec<_>>()
        .join(" ");
    if out.len() > 180 {
        out.truncate(180);
    }
    if out.is_empty() {
        "slangc_present_no_version_text".to_string()
    } else {
        out
    }
}

fn banger_pipeline_cache_key(
    prepared: &MonsterPreparedCompute,
    artifact: &MonsterNativeTandemArtifact,
    prefer_mesh_shaders: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_pipeline_cache_key.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.gpu_batch_plan.plan_hash.as_bytes());
    h.update(artifact.renderer_cache_hash.as_bytes());
    h.update(artifact.renderer_variant_hash.as_bytes());
    let shader_path: &[u8] = if prefer_mesh_shaders {
        b"mesh_shader"
    } else {
        b"compute_indirect"
    };
    h.update(shader_path);
    hex32(h.finalize().into())
}

#[derive(Debug)]
struct WgpuPresentBootstrap {
    selected_adapter: Option<NativeGpuAdapter>,
    adapter_count: usize,
    backend: String,
    surface_kind: &'static str,
    swapchain_format: &'static str,
    present_mode: &'static str,
    alpha_mode: &'static str,
    render_pass_count: u32,
    submitted_frame_count: u32,
    clear_color: [f64; 4],
}

fn run_wgpu_present_bootstrap(width: u32, height: u32) -> Result<WgpuPresentBootstrap, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: Default::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: Default::default(),
    });
    let mut adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .filter(|adapter| !matches!(adapter.get_info().device_type, wgpu::DeviceType::Cpu))
        .collect::<Vec<_>>();
    if adapters.is_empty() {
        return Err("no non-CPU wgpu adapter available for Banger present loop".to_string());
    }
    adapters.sort_by_key(|adapter| {
        let info = adapter.get_info();
        if info.vendor == 0x10de || info.name.to_ascii_lowercase().contains("nvidia") {
            return 0u8;
        }
        match info.device_type {
            wgpu::DeviceType::DiscreteGpu => 1,
            wgpu::DeviceType::IntegratedGpu => 2,
            wgpu::DeviceType::VirtualGpu => 3,
            wgpu::DeviceType::Other => 4,
            wgpu::DeviceType::Cpu => 5,
        }
    });
    let adapter_count = adapters.len();
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or_else(|| "Banger present loop adapter selection failed".to_string())?;
    let info = adapter.get_info();
    let selected_adapter = NativeGpuAdapter {
        name: info.name.clone(),
        vendor_id: info.vendor,
        device_id: info.device,
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        driver: info.driver.clone(),
        driver_info: info.driver_info.clone(),
        selected: true,
        score: 0,
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("banger-native-present-bootstrap-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::default(),
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("Banger present loop device request failed: {error}"))?;

    let swapchain_format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("banger-native-present-bootstrap-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: swapchain_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let clear_color = [0.015, 0.018, 0.024, 1.0];
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("banger-native-present-bootstrap-encoder"),
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
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("banger-native-present-bootstrap-clear-pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    queue.submit(Some(encoder.finish()));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("Banger present loop GPU poll failed: {error}"))?;

    Ok(WgpuPresentBootstrap {
        selected_adapter: Some(selected_adapter),
        adapter_count,
        backend: format!("{:?}", info.backend),
        surface_kind: "wgpu_offscreen_target_pending_child_surface",
        swapchain_format: "Bgra8UnormSrgb",
        present_mode: "AutoVsync",
        alpha_mode: "Opaque",
        render_pass_count: 1,
        submitted_frame_count: 1,
        clear_color,
    })
}

fn present_loop_frame_hash(
    width: u32,
    height: u32,
    target_frame_ms: f32,
    backend: &str,
    swapchain_format: &str,
    present_mode: &str,
    alpha_mode: &str,
    clear_color: [f64; 4],
    parent_window_handle_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_present_loop.frame.v1\0");
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(target_frame_ms.to_le_bytes());
    h.update(backend.as_bytes());
    h.update(swapchain_format.as_bytes());
    h.update(present_mode.as_bytes());
    h.update(alpha_mode.as_bytes());
    for value in clear_color {
        h.update(value.to_le_bytes());
    }
    h.update(parent_window_handle_hash.as_bytes());
    hex32(h.finalize().into())
}

fn present_loop_bootstrap_hash(
    frame_hash: &str,
    width: u32,
    height: u32,
    target_frame_ms: f32,
    backend: &str,
    surface_kind: &str,
    swapchain_format: &str,
    present_mode: &str,
    alpha_mode: &str,
    parent_window_handle_hash: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_present_loop_bootstrap.v1\0");
    h.update(frame_hash.as_bytes());
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(target_frame_ms.to_le_bytes());
    h.update(backend.as_bytes());
    h.update(surface_kind.as_bytes());
    h.update(swapchain_format.as_bytes());
    h.update(present_mode.as_bytes());
    h.update(alpha_mode.as_bytes());
    h.update(parent_window_handle_hash.as_bytes());
    hex32(h.finalize().into())
}

fn render_handoff_hash(
    prepared: &MonsterPreparedCompute,
    artifacts: &[BangerNativeRenderArtifactSummary],
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    benchmark_promotion_manifest: &BangerNativeBenchmarkPromotionManifest,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
    gpu_scene_packet: &BangerNativeGpuScenePacket,
    culling_manifest: &BangerNativeCullingManifest,
    meshlet_visibility_packet: &BangerNativeMeshletVisibilityPacket,
    nanite_second_layer_packet: &BangerNativeNaniteSecondLayerPacket,
    raster_work_queue: &BangerNativeRasterWorkQueue,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    lumen_lighting_packet: &BangerNativeLumenLightingPacket,
    virtual_shadow_packet: &BangerNativeVirtualShadowPacket,
    direct_lighting_packet: &BangerNativeDirectLightingPacket,
    material_closure_packet: &BangerNativeMaterialClosurePacket,
    page_residency_allocator: &BangerNativePageResidencyAllocatorPacket,
    temporal_history_packet: &BangerNativeTemporalHistoryPacket,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
    frame_submission_packet: &BangerNativeFrameSubmissionPacket,
    rhi_submit_packet: &BangerNativeRhiSubmitPacket,
    gpu_execution_receipt: &BangerNativeGpuExecutionReceipt,
    backend_submit_plan: &BangerNativeBackendSubmitPlan,
    backend_execution_packet: &BangerNativeBackendExecutionPacket,
    render_graph_compilation: &BangerNativeRenderGraphCompilation,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_handoff.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(shader_compiler_ticket.proof_hash.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(benchmark_promotion_manifest.benchmark_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(texture_bridge_contract.frame_hash.as_bytes());
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(scene_graph_submission.render_submission_hash.as_bytes());
    h.update(gpu_scene_packet.packet_hash.as_bytes());
    h.update(gpu_scene_packet.primitive_scene_data_hash.as_bytes());
    h.update(gpu_scene_packet.instance_scene_data_hash.as_bytes());
    h.update(culling_manifest.manifest_hash.as_bytes());
    h.update(culling_manifest.visibility_result_hash.as_bytes());
    h.update(culling_manifest.indirect_draw_buffer_hash.as_bytes());
    h.update(meshlet_visibility_packet.packet_hash.as_bytes());
    h.update(meshlet_visibility_packet.visibility_buffer_hash.as_bytes());
    h.update(meshlet_visibility_packet.indirect_draw_packet_hash.as_bytes());
    h.update(nanite_second_layer_packet.packet_hash.as_bytes());
    h.update(nanite_second_layer_packet.streaming_feedback_hash.as_bytes());
    h.update(nanite_second_layer_packet.visibility_resolve_hash.as_bytes());
    h.update(raster_work_queue.queue_hash.as_bytes());
    h.update(raster_work_queue.dispatch_plan_hash.as_bytes());
    h.update(raster_work_queue.bind_table_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(radiance_schedule_manifest.invalidation_hash.as_bytes());
    h.update(lumen_lighting_packet.packet_hash.as_bytes());
    h.update(lumen_lighting_packet.surface_cache_hash.as_bytes());
    h.update(lumen_lighting_packet.diffuse_indirect_hash.as_bytes());
    h.update(virtual_shadow_packet.packet_hash.as_bytes());
    h.update(virtual_shadow_packet.page_table_hash.as_bytes());
    h.update(virtual_shadow_packet.projection_hash.as_bytes());
    h.update(direct_lighting_packet.packet_hash.as_bytes());
    h.update(direct_lighting_packet.sample_sequence_hash.as_bytes());
    h.update(direct_lighting_packet.resolve_hash.as_bytes());
    h.update(material_closure_packet.packet_hash.as_bytes());
    h.update(material_closure_packet.closure_stack_hash.as_bytes());
    h.update(material_closure_packet.bsdf_table_hash.as_bytes());
    h.update(material_closure_packet.texture_table_hash.as_bytes());
    h.update(material_closure_packet.resolve_hash.as_bytes());
    h.update(page_residency_allocator.packet_hash.as_bytes());
    h.update(page_residency_allocator.physical_pool_hash.as_bytes());
    h.update(page_residency_allocator.virtual_page_table_hash.as_bytes());
    h.update(page_residency_allocator.feedback_request_hash.as_bytes());
    h.update(page_residency_allocator.allocation_hash.as_bytes());
    h.update(page_residency_allocator.eviction_hash.as_bytes());
    h.update(temporal_history_packet.packet_hash.as_bytes());
    h.update(temporal_history_packet.jitter_sequence_hash.as_bytes());
    h.update(temporal_history_packet.motion_vector_hash.as_bytes());
    h.update(temporal_history_packet.history_reprojection_hash.as_bytes());
    h.update(temporal_history_packet.disocclusion_mask_hash.as_bytes());
    h.update(temporal_history_packet.accumulation_hash.as_bytes());
    h.update(gaussian_splat_layer_manifest.manifest_hash.as_bytes());
    h.update(gaussian_splat_layer_manifest.conversion_manifest_hash.as_bytes());
    h.update(frame_submission_packet.submission_hash.as_bytes());
    h.update(frame_submission_packet.presentable_frame_hash.as_bytes());
    h.update(frame_submission_packet.command_buffer_hash.as_bytes());
    h.update(rhi_submit_packet.packet_hash.as_bytes());
    h.update(rhi_submit_packet.submit_batch_hash.as_bytes());
    h.update(rhi_submit_packet.present_hash.as_bytes());
    h.update(gpu_execution_receipt.receipt_hash.as_bytes());
    h.update(gpu_execution_receipt.frame_diagnostic_hash.as_bytes());
    h.update(gpu_execution_receipt.queue_timeline_hash.as_bytes());
    h.update(backend_submit_plan.submit_plan_hash.as_bytes());
    h.update(backend_submit_plan.swapchain_contract_hash.as_bytes());
    h.update(backend_submit_plan.backend_barrier_plan_hash.as_bytes());
    h.update(backend_execution_packet.packet_hash.as_bytes());
    h.update(backend_execution_packet.executor_schedule_hash.as_bytes());
    h.update(backend_execution_packet.readback_buffer_hash.as_bytes());
    h.update(backend_execution_packet.nonblank_signature_hash.as_bytes());
    h.update(backend_execution_packet.frame_latch_hash.as_bytes());
    h.update(render_graph_compilation.graph_hash.as_bytes());
    h.update(render_graph_compilation.compiled_order_hash.as_bytes());
    h.update(render_graph_compilation.resource_lifetime_hash.as_bytes());
    h.update(render_graph_compilation.barrier_plan_hash.as_bytes());
    for artifact in artifacts {
        h.update(b"\0artifact\0");
        h.update(artifact.kind.as_bytes());
        h.update(artifact.artifact_hash.as_bytes());
        h.update(artifact.renderer_cache_hash.as_bytes());
        h.update(artifact.renderer_promotion_hash.as_bytes());
        for page_hash in &artifact.page_hashes {
            h.update(page_hash.as_bytes());
        }
    }
    hex32(h.finalize().into())
}

fn hash_text_hex(schema: &str, text: &str) -> String {
    let mut h = Sha256::new();
    h.update(schema.as_bytes());
    h.update(b"\0");
    h.update(text.as_bytes());
    hex32(h.finalize().into())
}

fn hash_bytes_hex(schema: &str, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(schema.as_bytes());
    h.update(b"\0");
    h.update(bytes);
    hex32(h.finalize().into())
}

fn hex32(bytes: [u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn sanitize_forge_identifier(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(80));
    for ch in raw.chars().take(80) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if out.is_empty() {
        "banger_object".to_string()
    } else {
        out
    }
}

fn sanitize_scene_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(64));
    for ch in raw.chars().take(64) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "banger_default_scene".to_string()
    } else {
        out
    }
}

fn sanitize_scene_ref(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(96));
    for ch in raw.chars().take(96) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "banger_default_scene:root".to_string()
    } else {
        out
    }
}

fn banger_newobject_forge_source(
    scene_id: &str,
    object_id: &str,
    representation: &str,
    prompt_hash: &str,
) -> String {
    let token = sanitize_forge_identifier(&format!(
        "newobject_{scene_id}_{object_id}_{representation}_{}",
        &prompt_hash[..12]
    ));
    banger_native_render_forge_source(&token)
}

fn banger_native_render_forge_source(scene_id: &str) -> String {
    let scene_id = sanitize_forge_identifier(scene_id);
    format!(
        "forge_module:
  module banger_native_render_{scene_id} version 1
forge_imports:
  none
forge_constants:
  none
forge_functions:
  fn build(seed: sdf) -> geometry_page {{ return geometry_page_pack(meshlet_cluster(micromesh_build(seed))) }}
forge_program:
  let page = build(scene_sdf)
  let lit = radiance_cache_update(cache, radiance_probe(scene_sdf))
  let shadow = shadow_cache_invalidate(shadow_page_alloc(page), camera)
  let mat = substrate_mix(base_material, overlay_material)
  let streamed = pcg_execute(pcg, world_partition_cell(page))
  let budget = light_budget_select(light_cluster(lit), camera)
  emit out_mesh: geometry_page = streamed
  emit out_shadow: shadow_page = shadow
  emit out_material: material_graph = mat
  emit out_budget: light_budget = budget
forge_inputs:
  param scene_sdf: sdf unit none bounds [0.0, 1.0] nominal 0.0
  param cache: radiance_cache unit none bounds [0.0, 1.0] nominal 0.0
  param base_material: material_graph unit none bounds [0.0, 1.0] nominal 0.0
  param overlay_material: material_graph unit none bounds [0.0, 1.0] nominal 1.0
  param pcg: pcg_graph unit none bounds [0.0, 1.0] nominal 0.0
  param camera: vec3 unit none bounds [-1.0, 1.0] nominal 0.0
forge_outputs:
  output out_mesh: geometry_page unit none handoff mesh_params
  output out_shadow: shadow_page unit none handoff artifact
  output out_material: material_graph unit none handoff artifact
  output out_budget: light_budget unit none handoff artifact
forge_constraints:
  assert finite(export_proof(out_mesh))
forge_samples:
  case page seed 12 {{ given scene_sdf=0.0, cache=0.0, base_material=0.0, overlay_material=1.0, pcg=0.0, camera=0.0; expect out_mesh approx 1.0 tolerance 100.0 }}
forge_runtime:
lowering=wgsl_rhi
  cpu_simd=required
  cuda=optional
  memory_layout=page
forge_cost:
  max_steps=9000
  max_memory_mb=16
  precision=f32
  parallelism=64
artifact_handoff:
  proof_hash,output_hash"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use scan::{fresh_tmp_path, MemoryGovernor, Store, TmpDir};

    #[test]
    fn bootstraps_native_present_loop_contract() {
        let response = BangerNativeEngine::bootstrap_present_loop(
            BangerNativePresentLoopBootstrapRequest {
                parent_window_handle: None,
                viewport_width: Some(640),
                viewport_height: Some(360),
                target_frame_ms: Some(16.67),
            },
        )
        .expect("native present loop bootstrap");

        assert!(response.ok);
        assert_eq!(response.schema, "forge.banger.native_present_loop_bootstrap.v1");
        assert_eq!(response.lane, "native_tandem_render");
        assert_eq!(response.native_domain, "render_3d");
        assert_eq!(response.viewport_width, 640);
        assert_eq!(response.viewport_height, 360);
        assert_eq!(response.surface_kind, "wgpu_offscreen_target_pending_child_surface");
        assert_eq!(response.swapchain_format, "Bgra8UnormSrgb");
        assert_eq!(response.present_mode, "AutoVsync");
        assert_eq!(response.alpha_mode, "Opaque");
        assert_eq!(response.render_pass_count, 1);
        assert_eq!(response.submitted_frame_count, 1);
        assert_eq!(response.frame_hash.len(), 64);
        assert_eq!(response.present_loop_hash.len(), 64);
        assert_eq!(response.proof_hash.len(), 64);
        assert!(response.selected_adapter.is_some());
    }

    #[test]
    fn prepares_verified_native_banger_render_handoff() {
        let path = fresh_tmp_path("banger-native-engine", "handoff");
        let pipeline_cache_dir = path.join("pipeline-cache").to_string_lossy().to_string();
        let _tmp = TmpDir::new(path.clone());
        let store = Store::open(path).expect("store");
        let monster = MonsterNode::new(store, MemoryGovernor::new(8 * 1024 * 1024));

        let response = BangerNativeEngine::prepare_render_handoff(
            &monster,
            BangerNativeRenderPrepareRequest {
                scene_id: Some("test_scene".to_string()),
                known_fragment_hashes: None,
                target_frame_ms: Some(16.67),
                vram_budget_mb: Some(2048),
                prefer_mesh_shaders: Some(true),
                pipeline_cache_dir: Some(pipeline_cache_dir),
                viewport_width: Some(1920),
                viewport_height: Some(1080),
            },
        )
        .expect("native render handoff");

        assert!(response.ok);
        assert_eq!(response.lane, "native_tandem_render");
        assert_eq!(response.native_domain, "render_3d");
        assert_eq!(response.artifacts.len(), response.residency_jobs.len());
        assert_eq!(response.render_pass_count, response.render_graph.len());
        assert!(response
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "meshlet_page"));
        assert!(response
            .gpu_shader_profiles
            .iter()
            .any(|profile| profile == "wgsl.native_tandem_banger_render_pages.v1"));
        assert_eq!(
            response.shader_capability_plan.compiler_ticket_hash,
            response.shader_compiler_ticket.proof_hash
        );
        assert!(response
            .shader_capability_plan
            .capability_gate
            .contains("benchmark_promotion_hash"));
        assert_eq!(response.shader_compiler_ticket.proof_hash.len(), 64);
        assert_eq!(response.shader_compiler_ticket.preferred_compiler, "slangc");
        assert_eq!(response.shader_compiler_ticket.bootstrap_target, "wgsl");
        assert_eq!(response.shader_compiler_ticket.mini_probe_source_hash.len(), 64);
        assert_eq!(response.shader_compiler_ticket.mini_probe_output_hash.len(), 64);
        assert!(matches!(
            response.shader_compiler_ticket.mini_probe_status,
            "compiler_absent" | "compiled_wgsl" | "compile_failed"
        ));
        assert!(response
            .shader_compiler_ticket
            .requested_targets
            .iter()
            .any(|target| *target == "wgsl"));
        assert!(response
            .shader_compiler_ticket
            .requested_targets
            .iter()
            .any(|target| *target == "spirv"));
        assert!(response
            .shader_compiler_ticket
            .requested_targets
            .iter()
            .any(|target| *target == "hlsl"));
        assert!(response
            .shader_compiler_ticket
            .requested_targets
            .iter()
            .any(|target| *target == "msl"));
        assert_eq!(response.shader_compiler_ticket.target_artifacts.len(), 4);
        assert_eq!(response.shader_compiler_ticket.target_manifest_hash.len(), 64);
        assert_eq!(response.shader_compiler_ticket.fallback_wgsl_hash.len(), 64);
        assert_eq!(response.shader_compiler_ticket.fallback_wgsl_parity_hash.len(), 64);
        assert_eq!(response.shader_compiler_ticket.material_abi_hash.len(), 64);
        assert_eq!(
            response.shader_compiler_ticket.material_abi.layout_hash,
            response.shader_compiler_ticket.material_abi_hash
        );
        assert_eq!(
            response.shader_compiler_ticket.reflection_manifest.schema,
            "forge.banger.shader_reflection_manifest.v1"
        );
        assert_eq!(
            response.shader_compiler_ticket.reflection_manifest.material_abi_hash,
            response.shader_compiler_ticket.material_abi_hash
        );
        assert_eq!(
            response.shader_compiler_ticket.reflection_manifest.binding_count,
            response
                .shader_compiler_ticket
                .reflection_manifest
                .bindings
                .len()
        );
        assert!(response
            .shader_compiler_ticket
            .reflection_manifest
            .bindings
            .iter()
            .any(|binding| binding.name == "result"
                && binding.category == "storage_buffer"
                && binding.access == "read_write"
                && binding.proof_hash.len() == 64));
        assert!(response
            .shader_compiler_ticket
            .target_artifacts
            .iter()
            .all(|artifact| artifact.source_hash.len() == 64
                && artifact.output_hash.len() == 64
                && artifact.diagnostic_hash.len() == 64
                && artifact.artifact_hash.len() == 64
                && matches!(
                    artifact.status,
                    "compiled" | "compile_failed" | "compiler_absent_fallback_declared"
                )));
        assert!(!response.shader_compiler_ticket.compiler_version_hash.is_empty());
        assert_eq!(response.pipeline_cache_keys.len(), response.artifacts.len());
        assert_eq!(
            response.render_graph_compilation.schema,
            "forge.banger.native_render_graph_compilation.v1"
        );
        assert_eq!(
            response.render_graph_compilation.authority,
            "monster_kasm_to_banger_native_render_graph"
        );
        assert!(response
            .render_graph_compilation
            .clean_room_basis
            .contains("local_unreal_sparse_study"));
        assert_eq!(
            response.render_graph_manifest_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(response.render_graph_manifest_hash.len(), 64);
        assert_eq!(response.render_graph_compilation.source_contract_hash, response.source_hash);
        assert_eq!(response.render_graph_compilation.monster_kasm_contract_hash.len(), 64);
        assert_eq!(response.render_graph_compilation.pass_count, response.render_graph.len());
        assert_eq!(
            response.render_graph_compilation.resource_count,
            response.render_graph_compilation.resources.len()
        );
        assert_eq!(
            response.render_graph_compilation.edge_count,
            response.render_graph_compilation.edges.len()
        );
        assert_eq!(
            response.render_graph_compilation.compiled_passes.len(),
            response.render_graph.len()
        );
        assert_eq!(response.render_graph_compilation.culled_pass_count, 0);
        assert_eq!(response.render_graph_compilation.compiled_order_hash.len(), 64);
        assert_eq!(response.render_graph_compilation.resource_lifetime_hash.len(), 64);
        assert_eq!(response.render_graph_compilation.barrier_plan_hash.len(), 64);
        assert!(response
            .render_graph_compilation
            .resources
            .iter()
            .all(|resource| resource.slot_count > 0
                && resource.resident_bytes > 0
                && resource.resource_hash.len() == 64));
        assert!(response
            .render_graph_compilation
            .compiled_passes
            .iter()
            .all(|pass| !pass.reads.is_empty()
                && !pass.writes.is_empty()
                && !pass.pipeline_cache_key.is_empty()
                && !pass.culled
                && pass.pass_hash.len() == 64));
        assert!(response
            .render_graph_compilation
            .edges
            .iter()
            .all(|edge| edge.resource_hash.len() == 64 && edge.edge_hash.len() == 64));
        assert_eq!(
            response.pipeline_cache_manifest.schema,
            "forge.banger.native_pipeline_cache_manifest.v1"
        );
        assert_eq!(response.pipeline_cache_manifest.entry_count, response.artifacts.len());
        assert_eq!(
            response.pipeline_cache_manifest.persisted_entry_count,
            response.pipeline_cache_manifest.entry_count
        );
        assert_eq!(response.pipeline_cache_manifest.manifest_hash.len(), 64);
        assert_eq!(response.pipeline_cache_manifest.selected_adapter_hash.len(), 64);
        assert_eq!(response.pipeline_cache_manifest.driver_hash.len(), 64);
        assert_eq!(response.pipeline_cache_manifest.driver_info_hash.len(), 64);
        assert!(response
            .pipeline_cache_manifest
            .entries
            .iter()
            .all(|entry| entry.shader_source_hash.len() == 64
                && entry.shader_reflection_hash.len() == 64
                && entry.shader_target_manifest_hash
                    == response.shader_compiler_ticket.target_manifest_hash
                && entry.material_abi_hash == response.shader_compiler_ticket.material_abi_hash
                && entry.render_pass_abi_hash.len() == 64
                && entry.blob_hash.len() == 64
                && entry.blob_len > 0
                && entry.persistence_status == "seed_blob_persisted"
                && std::path::Path::new(&entry.blob_path).exists()));
        assert_eq!(
            response.benchmark_promotion_manifest.schema,
            "forge.banger.benchmark_promotion_manifest.v1"
        );
        assert_eq!(response.benchmark_promotion_manifest.gate_count, 5);
        assert_eq!(
            response.benchmark_promotion_manifest.gate_count,
            response.benchmark_promotion_manifest.gates.len()
        );
        assert_eq!(
            response.benchmark_promotion_manifest.passed_gate_count,
            response.benchmark_promotion_manifest.gates.len()
        );
        assert!(response.benchmark_promotion_manifest.promotion_allowed);
        assert!(response.benchmark_promotion_manifest.estimated_frame_ms.is_finite());
        assert!(response.benchmark_promotion_manifest.estimated_frame_ms <= response.target_frame_ms);
        assert!(response.benchmark_promotion_manifest.frame_time_headroom_pct.is_finite());
        assert!(response.benchmark_promotion_manifest.vram_pressure_pct <= 85.0);
        assert!(response.benchmark_promotion_manifest.cache_reuse_ratio >= 0.95);
        assert_eq!(
            response
                .benchmark_promotion_manifest
                .proof_reproducibility_status,
            "stable_content_addressed_inputs"
        );
        assert_eq!(
            response
                .benchmark_promotion_manifest
                .proof_reproducibility_hash
                .len(),
            64
        );
        assert!(response.benchmark_promotion_manifest.visual_capability_score >= 80);
        assert_eq!(
            response
                .benchmark_promotion_manifest
                .visual_capability_hash
                .len(),
            64
        );
        assert_eq!(response.benchmark_promotion_manifest.benchmark_hash.len(), 64);
        assert!(response
            .benchmark_promotion_manifest
            .gates
            .iter()
            .all(|gate| gate.passed && gate.proof_hash.len() == 64));
        assert!(response
            .benchmark_promotion_manifest
            .gates
            .iter()
            .any(|gate| gate.name == "latency" && gate.metric == "estimated_frame_ms"));
        assert!(response
            .benchmark_promotion_manifest
            .gates
            .iter()
            .any(|gate| gate.name == "visual_capability"
                && gate.metric == "visual_capability_score"));
        assert_eq!(
            response.texture_bridge_contract.schema,
            "forge.banger.native_texture_bridge_contract.v1"
        );
        assert_eq!(response.texture_bridge_contract.width, 1920);
        assert_eq!(response.texture_bridge_contract.height, 1080);
        assert_eq!(
            response.texture_bridge_contract.fallback_route,
            "cpu_readback_rgba8_copy_src_to_host_texture"
        );
        assert!(matches!(
            response.texture_bridge_contract.route_status,
            "same_device_queue_candidate_fallback_verified" | "fallback_only_adapter_unavailable"
        ));
        assert!(response
            .texture_bridge_contract
            .texture_usage
            .iter()
            .any(|usage| *usage == "RENDER_ATTACHMENT"));
        assert!(response
            .texture_bridge_contract
            .texture_usage
            .iter()
            .any(|usage| *usage == "TEXTURE_BINDING"));
        assert!(response
            .texture_bridge_contract
            .texture_usage
            .iter()
            .any(|usage| *usage == "COPY_SRC"));
        assert_eq!(response.texture_bridge_contract.frame_hash.len(), 64);
        assert_eq!(response.texture_bridge_contract.viewport_contract_hash.len(), 64);
        assert_eq!(response.texture_bridge_contract.resize_proof_hash.len(), 64);
        assert_eq!(
            response.texture_bridge_contract.camera_control_proof_hash.len(),
            64
        );
        assert_eq!(response.texture_bridge_contract.bridge_proof_hash.len(), 64);
        assert_eq!(
            response.texture_bridge_contract.viewport.fit_mode,
            "scene_bounds_fit"
        );
        assert_eq!(
            response.texture_bridge_contract.viewport.camera_mode,
            "orbit_pan_zoom"
        );
        assert!(response.texture_bridge_contract.viewport.orbit_enabled);
        assert!(response.texture_bridge_contract.viewport.pan_enabled);
        assert!(response.texture_bridge_contract.viewport.zoom_enabled);
        assert_eq!(
            response.texture_bridge_contract.viewport.scene_graph_hash,
            response.editable_scene_manifest.graph_hash
        );
        assert_eq!(
            response.texture_bridge_contract.viewport.scene_bounds_hash,
            response.editable_scene_manifest.bounds_hash
        );
        assert_eq!(
            response.scene_graph_submission.schema,
            "forge.banger.native_scene_graph_submission.v1"
        );
        assert_eq!(
            response.scene_graph_submission.object_count,
            response.editable_scene_manifest.object_count
        );
        assert!(response.scene_graph_submission.visible_object_count >= response.artifacts.len());
        assert!(response.scene_graph_submission.renderable_object_count > 0);
        assert_eq!(response.scene_graph_submission.hidden_object_count, 0);
        assert_eq!(response.scene_graph_submission.transform_propagation_hash.len(), 64);
        assert_eq!(response.scene_graph_submission.visibility_hash.len(), 64);
        assert_eq!(response.scene_graph_submission.representation_mix_hash.len(), 64);
        assert_eq!(response.scene_graph_submission.viewport_fit_hash.len(), 64);
        assert_eq!(response.scene_graph_submission.render_submission_hash.len(), 64);
        assert_eq!(response.scene_graph_submission.submission_hash.len(), 64);
        assert_eq!(
            response.texture_bridge_contract.viewport.viewport_fit_hash,
            response.scene_graph_submission.viewport_fit_hash
        );
        assert!(response
            .scene_graph_submission
            .representation_mix
            .iter()
            .any(|entry| entry.representation == "meshlet"
                && entry.object_count > 0
                && entry.proof_hash.len() == 64));
        assert!(response
            .scene_graph_submission
            .submissions
            .iter()
            .any(|node| node.renderable
                && node.visible
                && !node.resource_slots.is_empty()
                && !node.render_graph_stages.is_empty()
                && node.proof_hash.len() == 64));
        assert_eq!(
            response.gpu_scene_packet.schema,
            "forge.banger.native_gpu_scene_packet.v1"
        );
        assert_eq!(
            response.gpu_scene_packet.authority,
            "banger_scene_graph_to_gpu_scene_buffers"
        );
        assert!(response
            .gpu_scene_packet
            .clean_room_basis
            .contains("local_unreal_sparse_gpu_scene"));
        assert_eq!(response.gpu_scene_packet.source_contract_hash, response.source_hash);
        assert_eq!(
            response.gpu_scene_packet.scene_graph_hash,
            response.scene_graph_submission.submission_hash
        );
        assert_eq!(
            response.gpu_scene_packet.resource_table_hash,
            response.resource_table.table_hash
        );
        assert_eq!(
            response.gpu_scene_packet.material_abi_hash,
            response.shader_compiler_ticket.material_abi_hash
        );
        assert_eq!(
            response.gpu_scene_packet.primitive_count,
            response.scene_graph_submission.submissions.len()
        );
        assert_eq!(
            response.gpu_scene_packet.primitive_count,
            response.gpu_scene_packet.primitives.len()
        );
        assert!(response.gpu_scene_packet.instance_count > 0);
        assert!(response.gpu_scene_packet.payload_float4_count > 0);
        assert_eq!(
            response.gpu_scene_packet.upload_range_count,
            response.gpu_scene_packet.primitive_count
        );
        assert!(response.gpu_scene_packet.gpu_write_primitive_count > 0);
        assert_eq!(response.gpu_scene_packet.primitive_scene_data_hash.len(), 64);
        assert_eq!(response.gpu_scene_packet.instance_scene_data_hash.len(), 64);
        assert_eq!(response.gpu_scene_packet.instance_payload_data_hash.len(), 64);
        assert_eq!(response.gpu_scene_packet.material_table_hash.len(), 64);
        assert_eq!(response.gpu_scene_packet.upload_ranges_hash.len(), 64);
        assert_eq!(response.gpu_scene_packet.packet_hash.len(), 64);
        assert!(response
            .gpu_scene_packet
            .primitives
            .iter()
            .any(|primitive| primitive.supports_gpu_scene
                && primitive.supports_nanite_like_streaming
                && primitive.material_record_hash.len() == 64
                && primitive.primitive_hash.len() == 64));
        assert!(response
            .gpu_scene_packet
            .primitives
            .iter()
            .all(|primitive| primitive.bounds_hash.len() == 64
                && primitive.transform_hash.len() == 64
                && primitive.upload_range_hash.len() == 64));
        assert_eq!(
            response.culling_manifest.schema,
            "forge.banger.native_culling_manifest.v1"
        );
        assert!(response.culling_manifest.candidate_count > 0);
        assert_eq!(
            response.culling_manifest.visible_count + response.culling_manifest.culled_count,
            response.culling_manifest.candidate_count
        );
        assert_eq!(response.culling_manifest.visibility_result_hash.len(), 64);
        assert_eq!(response.culling_manifest.indirect_draw_buffer_hash.len(), 64);
        assert_eq!(response.culling_manifest.cache_reuse_hash.len(), 64);
        assert_eq!(response.culling_manifest.manifest_hash.len(), 64);
        assert!(response.culling_manifest.max_lod_error.is_finite());
        assert!(response
            .culling_manifest
            .entries
            .iter()
            .any(|entry| entry.representation == "meshlet"
                && entry.culling_basis == "meshlet_sphere_cone_lod_indirect"
                && entry.visible_after_cull
                && entry.lod_error.is_finite()
                && entry.cone_axis.iter().all(|value| value.is_finite())
                && entry.cone_cutoff.is_finite()
                && entry.indirect_draw_args[0] > 0
                && entry.indirect_draw_args[1] > 0
                && entry.cache_reuse_key.len() == 64
                && entry.visibility_result_hash.len() == 64
                && entry.indirect_draw_hash.len() == 64
                && entry.proof_hash.len() == 64));
        assert_eq!(
            response.meshlet_visibility_packet.schema,
            "forge.banger.meshlet_visibility_packet.v1"
        );
        assert_eq!(
            response.meshlet_visibility_packet.authority,
            "monster_kasm_scene_graph_to_banger_meshlet_visibility"
        );
        assert!(response
            .meshlet_visibility_packet
            .clean_room_basis
            .contains("local_unreal_sparse_nanite_study"));
        assert_eq!(
            response.meshlet_visibility_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.meshlet_visibility_packet.scene_graph_hash,
            response.scene_graph_submission.submission_hash
        );
        assert_eq!(
            response.meshlet_visibility_packet.culling_manifest_hash,
            response.culling_manifest.manifest_hash
        );
        assert_eq!(
            response.meshlet_visibility_packet.render_graph_manifest_hash,
            response.render_graph_compilation.graph_hash
        );
        assert!(response.meshlet_visibility_packet.cluster_count > 0);
        assert_eq!(
            response.meshlet_visibility_packet.cluster_count,
            response.meshlet_visibility_packet.entries.len()
        );
        assert_eq!(
            response.meshlet_visibility_packet.visible_cluster_count,
            response.meshlet_visibility_packet.cluster_count
        );
        assert_eq!(
            response
                .meshlet_visibility_packet
                .hardware_raster_candidate_count
                + response
                    .meshlet_visibility_packet
                    .software_raster_candidate_count,
            response.meshlet_visibility_packet.cluster_count
        );
        assert_eq!(
            response.meshlet_visibility_packet.indirect_draw_word_count,
            response.meshlet_visibility_packet.cluster_count * 5
        );
        assert_eq!(response.meshlet_visibility_packet.visibility_buffer_hash.len(), 64);
        assert_eq!(response.meshlet_visibility_packet.lod_error_buffer_hash.len(), 64);
        assert_eq!(response.meshlet_visibility_packet.cluster_page_table_hash.len(), 64);
        assert_eq!(
            response
                .meshlet_visibility_packet
                .indirect_draw_packet_hash
                .len(),
            64
        );
        assert_eq!(response.meshlet_visibility_packet.packet_hash.len(), 64);
        assert!(response
            .meshlet_visibility_packet
            .entries
            .iter()
            .all(|entry| entry.entry_hash.len() == 64
                && entry.source_culling_proof_hash.len() == 64
                && entry.visibility_word > 0
                && entry.indirect_draw_args[0] > 0
                && matches!(
                    entry.raster_path,
                    "mesh_shader_or_hardware_raster_candidate"
                        | "compute_software_raster_candidate"
                )));
        assert_eq!(
            response.nanite_second_layer_packet.schema,
            "forge.banger.nanite_second_layer_packet.v1"
        );
        assert_eq!(
            response.nanite_second_layer_packet.authority,
            "banger_meshlet_visibility_to_nanite_streaming_shading_resolve"
        );
        assert!(response
            .nanite_second_layer_packet
            .clean_room_basis
            .contains("local_unreal_sparse_nanite"));
        assert_eq!(
            response.nanite_second_layer_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.nanite_second_layer_packet.gpu_scene_hash,
            response.gpu_scene_packet.packet_hash
        );
        assert_eq!(
            response.nanite_second_layer_packet.visibility_packet_hash,
            response.meshlet_visibility_packet.packet_hash
        );
        assert_eq!(
            response.nanite_second_layer_packet.resource_table_hash,
            response.resource_table.table_hash
        );
        assert_eq!(
            response.nanite_second_layer_packet.material_abi_hash,
            response.shader_compiler_ticket.material_abi_hash
        );
        assert_eq!(
            response.nanite_second_layer_packet.feedback_word_count,
            response.nanite_second_layer_packet.entries.len()
        );
        assert_eq!(
            response
                .nanite_second_layer_packet
                .visibility_resolve_tile_count,
            response.nanite_second_layer_packet.entries.len()
        );
        assert_eq!(
            response.nanite_second_layer_packet.ray_tracing_proxy_count,
            response.nanite_second_layer_packet.entries.len()
        );
        assert_eq!(
            response.nanite_second_layer_packet.streaming_request_count
                + response.nanite_second_layer_packet.resident_page_count,
            response.nanite_second_layer_packet.entries.len()
        );
        assert!(response.nanite_second_layer_packet.shading_bin_count > 0);
        assert_eq!(
            response
                .nanite_second_layer_packet
                .streaming_feedback_hash
                .len(),
            64
        );
        assert_eq!(
            response.nanite_second_layer_packet.page_residency_hash.len(),
            64
        );
        assert_eq!(response.nanite_second_layer_packet.material_bin_hash.len(), 64);
        assert_eq!(
            response
                .nanite_second_layer_packet
                .visibility_resolve_hash
                .len(),
            64
        );
        assert_eq!(
            response
                .nanite_second_layer_packet
                .ray_tracing_bridge_hash
                .len(),
            64
        );
        assert_eq!(response.nanite_second_layer_packet.packet_hash.len(), 64);
        assert!(response
            .nanite_second_layer_packet
            .entries
            .iter()
            .all(|entry| entry.entry_hash.len() == 64
                && entry.ray_tracing_proxy_hash.len() == 64
                && entry.feedback_word != 0
                && entry.material_bin_id < 4096
                && entry.visibility_tile[2] > 0
                && entry.visibility_tile[3] > 0
                && matches!(
                    entry.residency_state,
                    "resident_page"
                        | "streaming_request_small_page"
                        | "streaming_request_large_page"
                        | "streaming_request_missing_page"
                )));
        assert_eq!(
            response.raster_work_queue.schema,
            "forge.banger.native_raster_work_queue.v1"
        );
        assert_eq!(
            response.raster_work_queue.authority,
            "monster_kasm_meshlet_visibility_to_banger_raster_queue"
        );
        assert!(response
            .raster_work_queue
            .clean_room_basis
            .contains("local_unreal_sparse_nanite"));
        assert_eq!(
            response.raster_work_queue.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.raster_work_queue.visibility_packet_hash,
            response.meshlet_visibility_packet.packet_hash
        );
        assert_eq!(
            response.raster_work_queue.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(
            response.raster_work_queue.resource_table_hash,
            response.resource_table.table_hash
        );
        assert_eq!(
            response.raster_work_queue.hardware_job_count + response.raster_work_queue.compute_job_count,
            response.raster_work_queue.jobs.len()
        );
        assert_eq!(
            response.raster_work_queue.jobs.len(),
            response.meshlet_visibility_packet.visible_cluster_count
        );
        assert!(response.raster_work_queue.total_threadgroup_count > 0);
        assert!(response.raster_work_queue.total_index_count > 0);
        assert_eq!(response.raster_work_queue.queue_barrier_hash.len(), 64);
        assert_eq!(response.raster_work_queue.bind_table_hash.len(), 64);
        assert_eq!(response.raster_work_queue.dispatch_plan_hash.len(), 64);
        assert_eq!(response.raster_work_queue.queue_hash.len(), 64);
        assert!(response
            .raster_work_queue
            .jobs
            .iter()
            .all(|job| job.job_hash.len() == 64
                && job.bind_group_hash.len() == 64
                && job.threadgroup_count > 0
                && job.indirect_draw_args[0] > 0
                && matches!(job.queue_lane, "graphics_mesh_shader" | "async_compute_raster")));
        assert_eq!(
            response.frame_submission_packet.schema,
            "forge.banger.native_frame_submission_packet.v1"
        );
        assert_eq!(
            response.frame_submission_packet.authority,
            "banger_render_graph_raster_queue_to_native_frame_submission"
        );
        assert!(response
            .frame_submission_packet
            .clean_room_basis
            .contains("local_unreal_sparse_rdg_rhi_submit"));
        assert_eq!(
            response.frame_submission_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.frame_submission_packet.texture_bridge_hash,
            response.texture_bridge_contract.bridge_proof_hash
        );
        assert_eq!(
            response.frame_submission_packet.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(
            response.frame_submission_packet.raster_queue_hash,
            response.raster_work_queue.queue_hash
        );
        assert_eq!(
            response.frame_submission_packet.pass_count,
            response.render_graph_compilation.pass_count
        );
        assert_eq!(
            response.frame_submission_packet.raster_job_count,
            response.raster_work_queue.jobs.len()
        );
        assert_eq!(
            response.frame_submission_packet.command_count,
            response.frame_submission_packet.commands.len()
        );
        assert_eq!(
            response.frame_submission_packet.command_count,
            response.render_graph_compilation.compiled_passes.len()
        );
        assert!(response.frame_submission_packet.submitted_queue_count > 0);
        assert_eq!(response.frame_submission_packet.color_target_hash.len(), 64);
        assert_eq!(response.frame_submission_packet.depth_target_hash.len(), 64);
        assert_eq!(response.frame_submission_packet.render_target_state_hash.len(), 64);
        assert_eq!(response.frame_submission_packet.command_buffer_hash.len(), 64);
        assert_eq!(response.frame_submission_packet.frame_schedule_hash.len(), 64);
        assert_eq!(response.frame_submission_packet.presentable_frame_hash.len(), 64);
        assert_eq!(response.frame_submission_packet.submission_hash.len(), 64);
        assert!(response
            .frame_submission_packet
            .commands
            .iter()
            .all(|command| command.command_hash.len() == 64
                && command.input_hash.len() == 64
                && command.output_target_hash.len() == 64
                && command.barrier_hash.len() == 64
                && command.resource_write_count > 0
                && matches!(
                    command.queue_lane,
                    "graphics_mesh_shader" | "async_compute_raster" | "graphics" | "async_compute"
                )));
        assert_eq!(
            response.rhi_submit_packet.schema,
            "forge.banger.native_rhi_submit_packet.v1"
        );
        assert_eq!(
            response.rhi_submit_packet.authority,
            "banger_frame_submission_to_native_rhi_submit"
        );
        assert!(response
            .rhi_submit_packet
            .clean_room_basis
            .contains("dynamic_rhi_finalize_submit_present"));
        assert_eq!(
            response.rhi_submit_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.rhi_submit_packet.frame_submission_hash,
            response.frame_submission_packet.submission_hash
        );
        assert_eq!(
            response.rhi_submit_packet.texture_bridge_hash,
            response.texture_bridge_contract.bridge_proof_hash
        );
        assert_eq!(
            response.rhi_submit_packet.backend,
            response.texture_bridge_contract.backend
        );
        assert_eq!(
            response.rhi_submit_packet.selected_adapter_hash,
            response.texture_bridge_contract.selected_adapter_hash
        );
        assert_eq!(
            response.rhi_submit_packet.command_list_count,
            response.frame_submission_packet.command_count
        );
        assert_eq!(response.rhi_submit_packet.submit_batch_count, 1);
        assert_eq!(
            response.rhi_submit_packet.submitted_queue_count,
            response.frame_submission_packet.submitted_queue_count
        );
        assert!(response.rhi_submit_packet.timeline_base_value > 0);
        assert_eq!(response.rhi_submit_packet.acquire_backbuffer_hash.len(), 64);
        assert_eq!(response.rhi_submit_packet.finalized_command_lists_hash.len(), 64);
        assert_eq!(response.rhi_submit_packet.submit_batch_hash.len(), 64);
        assert_eq!(response.rhi_submit_packet.present_hash.len(), 64);
        assert_eq!(response.rhi_submit_packet.fence_timeline_hash.len(), 64);
        assert_eq!(response.rhi_submit_packet.packet_hash.len(), 64);
        assert_eq!(
            response.rhi_submit_packet.steps.len(),
            response.frame_submission_packet.command_count + 3
        );
        assert!(response
            .rhi_submit_packet
            .steps
            .iter()
            .all(|step| step.wait_hash.len() == 64
                && step.signal_hash.len() == 64
                && step.step_hash.len() == 64
                && step.timeline_value >= response.rhi_submit_packet.timeline_base_value));
        assert!(response
            .rhi_submit_packet
            .steps
            .iter()
            .any(|step| step.phase == "submit_command_lists"));
        assert!(response
            .rhi_submit_packet
            .steps
            .iter()
            .any(|step| step.phase == "present"));
        assert_eq!(
            response.gpu_execution_receipt.schema,
            "forge.banger.native_gpu_execution_receipt.v1"
        );
        assert_eq!(
            response.gpu_execution_receipt.authority,
            "banger_rhi_submit_to_gpu_execution_receipt"
        );
        assert_eq!(
            response.gpu_execution_receipt.execution_status,
            "submit_ready_verified"
        );
        assert_eq!(
            response.gpu_execution_receipt.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.gpu_execution_receipt.rhi_submit_hash,
            response.rhi_submit_packet.packet_hash
        );
        assert_eq!(
            response.gpu_execution_receipt.frame_submission_hash,
            response.frame_submission_packet.submission_hash
        );
        assert_eq!(
            response.gpu_execution_receipt.present_hash,
            response.rhi_submit_packet.present_hash
        );
        assert!(response.gpu_execution_receipt.nonblank_frame_expected);
        assert_eq!(
            response.gpu_execution_receipt.submitted_step_count,
            response.rhi_submit_packet.steps.len()
        );
        assert_eq!(
            response.gpu_execution_receipt.completed_phase_count,
            response.gpu_execution_receipt.phases.len()
        );
        assert_eq!(
            response.gpu_execution_receipt.command_list_count,
            response.rhi_submit_packet.command_list_count
        );
        assert!(response.gpu_execution_receipt.queue_lane_count > 0);
        assert_eq!(response.gpu_execution_receipt.frame_diagnostic_hash.len(), 64);
        assert_eq!(response.gpu_execution_receipt.queue_timeline_hash.len(), 64);
        assert_eq!(response.gpu_execution_receipt.readback_policy_hash.len(), 64);
        assert_eq!(response.gpu_execution_receipt.receipt_hash.len(), 64);
        assert!(response
            .gpu_execution_receipt
            .phases
            .iter()
            .all(|phase| phase.completed
                && phase.source_step_hash.len() == 64
                && phase.diagnostic_hash.len() == 64
                && phase.phase_hash.len() == 64
                && phase.timeline_value >= response.rhi_submit_packet.timeline_base_value));
        assert_eq!(
            response.backend_submit_plan.schema,
            "forge.banger.native_backend_submit_plan.v1"
        );
        assert_eq!(
            response.backend_submit_plan.authority,
            "banger_rhi_submit_to_backend_specific_contract"
        );
        assert!(matches!(
            response.backend_submit_plan.backend_family,
            "d3d12" | "vulkan" | "metal" | "wgpu_generic"
        ));
        assert_eq!(
            response.backend_submit_plan.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.backend_submit_plan.frame_submission_hash,
            response.frame_submission_packet.submission_hash
        );
        assert_eq!(
            response.backend_submit_plan.rhi_submit_hash,
            response.rhi_submit_packet.packet_hash
        );
        assert_eq!(
            response.backend_submit_plan.execution_receipt_hash,
            response.gpu_execution_receipt.receipt_hash
        );
        assert_eq!(response.backend_submit_plan.swapchain_contract_hash.len(), 64);
        assert_eq!(response.backend_submit_plan.descriptor_heap_hash.len(), 64);
        assert_eq!(response.backend_submit_plan.pipeline_state_cache_hash.len(), 64);
        assert_eq!(response.backend_submit_plan.backend_barrier_plan_hash.len(), 64);
        assert_eq!(response.backend_submit_plan.command_allocator_hash.len(), 64);
        assert_eq!(response.backend_submit_plan.submit_plan_hash.len(), 64);
        assert_eq!(response.backend_submit_plan.targets.len(), 3);
        assert!(response
            .backend_submit_plan
            .targets
            .iter()
            .any(|target| target.queue_lane == "graphics"));
        assert!(response
            .backend_submit_plan
            .targets
            .iter()
            .any(|target| target.queue_lane == "compute"));
        assert!(response
            .backend_submit_plan
            .targets
            .iter()
            .any(|target| target.queue_lane == "present"));
        assert!(response
            .backend_submit_plan
            .targets
            .iter()
            .all(|target| target.target_hash.len() == 64
                && target.swapchain_image_count >= 2
                && target.descriptor_table_count > 0
                && target.pipeline_state_count > 0
                && target.barrier_batch_count > 0
                && target.command_allocator_count > 0));
        assert_eq!(
            response.backend_execution_packet.schema,
            "forge.banger.native_backend_execution_packet.v1"
        );
        assert_eq!(
            response.backend_execution_packet.authority,
            "banger_backend_submit_plan_to_executable_native_frame"
        );
        assert!(response
            .backend_execution_packet
            .clean_room_basis
            .contains("backend_execution_readback_contract"));
        assert_eq!(
            response.backend_execution_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.backend_execution_packet.backend_submit_plan_hash,
            response.backend_submit_plan.submit_plan_hash
        );
        assert_eq!(
            response.backend_execution_packet.rhi_submit_hash,
            response.rhi_submit_packet.packet_hash
        );
        assert_eq!(
            response.backend_execution_packet.execution_receipt_hash,
            response.gpu_execution_receipt.receipt_hash
        );
        assert_eq!(
            response.backend_execution_packet.selected_backend,
            response.texture_bridge_contract.backend
        );
        assert_eq!(
            response.backend_execution_packet.executor_mode,
            "native_gpu_backend_with_nonblank_readback_gate"
        );
        assert_eq!(
            response.backend_execution_packet.executable_pass_count,
            response.frame_submission_packet.command_count
        );
        assert_eq!(
            response.backend_execution_packet.passes.len(),
            response.frame_submission_packet.commands.len()
        );
        assert_eq!(
            response.backend_execution_packet.readback_byte_count,
            u64::from(response.texture_bridge_contract.width)
                * u64::from(response.texture_bridge_contract.height)
                * 4
        );
        assert!(response.backend_execution_packet.nonzero_tile_count > 0);
        assert!(response.backend_execution_packet.nonblack_pixel_sample_count > 0);
        assert!(response.backend_execution_packet.swapchain_image_count >= 2);
        assert!(response.backend_execution_packet.memory_barrier_count > 0);
        assert_eq!(response.backend_execution_packet.executor_schedule_hash.len(), 64);
        assert_eq!(response.backend_execution_packet.pipeline_binding_hash.len(), 64);
        assert_eq!(response.backend_execution_packet.readback_buffer_hash.len(), 64);
        assert_eq!(
            response.backend_execution_packet.nonblank_signature_hash.len(),
            64
        );
        assert_eq!(response.backend_execution_packet.frame_latch_hash.len(), 64);
        assert_eq!(response.backend_execution_packet.packet_hash.len(), 64);
        assert!(response
            .backend_execution_packet
            .passes
            .iter()
            .all(|pass| pass.command_hash.len() == 64
                && pass.target_hash.len() == 64
                && pass.descriptor_table_hash.len() == 64
                && pass.pipeline_state_hash.len() == 64
                && pass.barrier_batch_hash.len() == 64
                && pass.readback_region_hash.len() == 64
                && pass.nonblank_sample_hash.len() == 64
                && pass.pass_hash.len() == 64));
        assert!(response
            .backend_execution_packet
            .passes
            .iter()
            .any(|pass| pass.stage == "material_bind"));
        assert_eq!(
            response.radiance_schedule_manifest.schema,
            "forge.banger.native_radiance_schedule_manifest.v1"
        );
        assert!(response.radiance_schedule_manifest.temporal_epoch > 0);
        assert!(response.radiance_schedule_manifest.probe_page_count > 0);
        assert!(response.radiance_schedule_manifest.active_probe_count > 0);
        assert!(response.radiance_schedule_manifest.light_budget >= 4);
        assert_eq!(
            response.radiance_schedule_manifest.async_compute_residency_policy,
            "async_compute_lighting_stream_temporal_reuse"
        );
        assert_eq!(response.radiance_schedule_manifest.invalidation_hash.len(), 64);
        assert_eq!(response.radiance_schedule_manifest.schedule_hash.len(), 64);
        assert_eq!(
            response.radiance_schedule_manifest.probe_page_count,
            response.radiance_schedule_manifest.entries.len()
        );
        assert!(response
            .radiance_schedule_manifest
            .entries
            .iter()
            .all(|entry| entry.async_compute
                && entry.residency_policy == "async_compute_lighting_stream"
                && entry.probe_count > 0
                && entry.light_budget == response.radiance_schedule_manifest.light_budget
                && entry.invalidation_hash.len() == 64
                && entry.proof_hash.len() == 64));
        assert_eq!(
            response.lumen_lighting_packet.schema,
            "forge.banger.lumen_lighting_packet.v1"
        );
        assert_eq!(
            response.lumen_lighting_packet.authority,
            "banger_nanite_surface_cache_screen_probe_radiance_trace"
        );
        assert!(response
            .lumen_lighting_packet
            .clean_room_basis
            .contains("local_unreal_sparse_lumen"));
        assert_eq!(
            response.lumen_lighting_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.lumen_lighting_packet.gpu_scene_hash,
            response.gpu_scene_packet.packet_hash
        );
        assert_eq!(
            response.lumen_lighting_packet.nanite_second_layer_hash,
            response.nanite_second_layer_packet.packet_hash
        );
        assert_eq!(
            response.lumen_lighting_packet.radiance_schedule_hash,
            response.radiance_schedule_manifest.schedule_hash
        );
        assert_eq!(
            response.lumen_lighting_packet.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(
            response.lumen_lighting_packet.surface_cache_page_count,
            response.lumen_lighting_packet.entries.len()
        );
        assert_eq!(
            response.lumen_lighting_packet.screen_probe_count,
            response.lumen_lighting_packet.entries.len()
        );
        assert_eq!(
            response.lumen_lighting_packet.radiance_tile_count,
            response.lumen_lighting_packet.entries.len()
        );
        assert_eq!(
            response.lumen_lighting_packet.hardware_trace_candidate_count
                + response.lumen_lighting_packet.software_trace_candidate_count,
            response.lumen_lighting_packet.entries.len()
        );
        assert!(response.lumen_lighting_packet.total_probe_rays > 0);
        assert!(response.lumen_lighting_packet.reflection_ray_budget > 0);
        assert_eq!(response.lumen_lighting_packet.surface_cache_hash.len(), 64);
        assert_eq!(response.lumen_lighting_packet.screen_probe_hash.len(), 64);
        assert_eq!(response.lumen_lighting_packet.trace_policy_hash.len(), 64);
        assert_eq!(response.lumen_lighting_packet.diffuse_indirect_hash.len(), 64);
        assert_eq!(response.lumen_lighting_packet.reflection_hash.len(), 64);
        assert_eq!(response.lumen_lighting_packet.packet_hash.len(), 64);
        assert!(response
            .lumen_lighting_packet
            .entries
            .iter()
            .all(|entry| entry.surface_page_id.starts_with("surface:")
                && entry.source_probe_page_id.starts_with("radiance:")
                && entry.diffuse_ray_count > 0
                && entry.reflection_ray_count > 0
                && entry.temporal_reuse_frames > 0
                && entry.radiance_tile[2] > 0
                && entry.radiance_tile[3] > 0
                && entry.surface_cache_hash.len() == 64
                && entry.screen_probe_hash.len() == 64
                && entry.trace_hash.len() == 64
                && entry.entry_hash.len() == 64
                && matches!(
                    entry.trace_policy,
                    "hardware_ray_traced_surface_cache"
                        | "software_sdf_probe_trace"
                        | "screen_probe_then_surface_cache"
                )));
        assert_eq!(
            response.virtual_shadow_packet.schema,
            "forge.banger.virtual_shadow_packet.v1"
        );
        assert_eq!(
            response.virtual_shadow_packet.authority,
            "banger_virtual_shadow_page_cache_light_grid_projection"
        );
        assert!(response
            .virtual_shadow_packet
            .clean_room_basis
            .contains("local_unreal_sparse_virtual_shadow_map"));
        assert_eq!(
            response.virtual_shadow_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.virtual_shadow_packet.nanite_second_layer_hash,
            response.nanite_second_layer_packet.packet_hash
        );
        assert_eq!(
            response.virtual_shadow_packet.lumen_lighting_hash,
            response.lumen_lighting_packet.packet_hash
        );
        assert_eq!(
            response.virtual_shadow_packet.radiance_schedule_hash,
            response.radiance_schedule_manifest.schedule_hash
        );
        assert_eq!(
            response.virtual_shadow_packet.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(
            response.virtual_shadow_packet.virtual_page_count,
            response.virtual_shadow_packet.entries.len()
        );
        assert!(
            response.virtual_shadow_packet.cached_page_count
                <= response.virtual_shadow_packet.virtual_page_count
        );
        assert!(
            response.virtual_shadow_packet.invalidated_page_count
                <= response.virtual_shadow_packet.virtual_page_count
        );
        assert!(response.virtual_shadow_packet.light_page_count > 0);
        assert!(response.virtual_shadow_packet.shadow_ray_budget > 0);
        assert_eq!(response.virtual_shadow_packet.page_table_hash.len(), 64);
        assert_eq!(response.virtual_shadow_packet.cache_hash.len(), 64);
        assert_eq!(response.virtual_shadow_packet.invalidation_hash.len(), 64);
        assert_eq!(response.virtual_shadow_packet.projection_hash.len(), 64);
        assert_eq!(response.virtual_shadow_packet.light_grid_hash.len(), 64);
        assert_eq!(response.virtual_shadow_packet.packet_hash.len(), 64);
        assert!(response
            .virtual_shadow_packet
            .entries
            .iter()
            .all(|entry| entry.shadow_page_id.starts_with("vshadow:")
                && entry.source_surface_page_id.starts_with("surface:")
                && entry.virtual_light_id.len() == 64
                && entry.resolution >= 64
                && entry.projection_tile[2] == entry.resolution
                && entry.projection_tile[3] == entry.resolution
                && entry.ray_budget > 0
                && entry.page_table_hash.len() == 64
                && entry.cache_hash.len() == 64
                && entry.projection_hash.len() == 64
                && entry.entry_hash.len() == 64
                && matches!(
                    entry.cache_state,
                    "persistent_cache_hit" | "cache_warm" | "page_mark_required"
                )
                && matches!(
                    entry.invalidation_reason,
                    "stable_cache_reuse"
                        | "geometry_page_streaming"
                        | "temporal_epoch_new"
                        | "light_budget_pressure"
                )));
        assert!(response
            .render_graph_compilation
            .compiled_passes
            .iter()
            .any(|pass| pass.stage == "shadow_depth"
                && pass.pass_name == "virtual_shadow_depth"));
        assert_eq!(
            response.direct_lighting_packet.schema,
            "forge.banger.direct_lighting_packet.v1"
        );
        assert_eq!(
            response.direct_lighting_packet.authority,
            "banger_megalights_light_grid_stochastic_shadowed_resolve"
        );
        assert!(response
            .direct_lighting_packet
            .clean_room_basis
            .contains("local_unreal_sparse_megalights_stochastic"));
        assert_eq!(
            response.direct_lighting_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.direct_lighting_packet.lumen_lighting_hash,
            response.lumen_lighting_packet.packet_hash
        );
        assert_eq!(
            response.direct_lighting_packet.virtual_shadow_hash,
            response.virtual_shadow_packet.packet_hash
        );
        assert_eq!(
            response.direct_lighting_packet.radiance_schedule_hash,
            response.radiance_schedule_manifest.schedule_hash
        );
        assert_eq!(
            response.direct_lighting_packet.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert!(response.direct_lighting_packet.light_cluster_count > 0);
        assert!(response.direct_lighting_packet.stochastic_sample_count > 0);
        assert_eq!(
            response.direct_lighting_packet.shadowed_light_count
                + response.direct_lighting_packet.unshadowed_light_count,
            response.direct_lighting_packet.entries.len()
        );
        assert_eq!(
            response.direct_lighting_packet.denoiser_tile_count,
            response.direct_lighting_packet.entries.len()
        );
        assert_eq!(
            response.direct_lighting_packet.resolve_tile_count,
            response.direct_lighting_packet.entries.len()
        );
        assert_eq!(response.direct_lighting_packet.light_grid_hash.len(), 64);
        assert_eq!(
            response.direct_lighting_packet.sample_sequence_hash.len(),
            64
        );
        assert_eq!(response.direct_lighting_packet.shadow_mask_hash.len(), 64);
        assert_eq!(response.direct_lighting_packet.denoiser_hash.len(), 64);
        assert_eq!(response.direct_lighting_packet.resolve_hash.len(), 64);
        assert_eq!(response.direct_lighting_packet.packet_hash.len(), 64);
        assert!(response
            .direct_lighting_packet
            .entries
            .iter()
            .all(|entry| entry.light_cluster_id.len() == 64
                && entry.virtual_light_id.len() == 64
                && entry.shadow_page_id.starts_with("vshadow:")
                && entry.sample_sequence != 0
                && entry.sample_count > 0
                && entry.shadow_mask_hash.len() == 64
                && entry.contribution_hash.len() == 64
                && entry.denoiser_tile[2] > 0
                && entry.denoiser_tile[3] > 0
                && entry.resolve_tile[2] == entry.denoiser_tile[2]
                && entry.resolve_tile[3] == entry.denoiser_tile[3]
                && entry.entry_hash.len() == 64
                && matches!(
                    entry.light_kind,
                    "hardware_ray_megalight"
                        | "stochastic_many_light"
                        | "clustered_deferred_light"
                )));
        assert_eq!(
            response.material_closure_packet.schema,
            "forge.banger.material_closure_packet.v1"
        );
        assert_eq!(
            response.material_closure_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.material_closure_packet.direct_lighting_hash,
            response.direct_lighting_packet.packet_hash
        );
        assert_eq!(
            response.material_closure_packet.lumen_lighting_hash,
            response.lumen_lighting_packet.packet_hash
        );
        assert_eq!(
            response.material_closure_packet.virtual_shadow_hash,
            response.virtual_shadow_packet.packet_hash
        );
        assert_eq!(
            response.material_closure_packet.shader_material_abi_hash,
            response.shader_compiler_ticket.material_abi.layout_hash
        );
        assert_eq!(
            response.material_closure_packet.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(
            response.material_closure_packet.closure_count,
            response.material_closure_packet.entries.len()
        );
        assert!(response.material_closure_packet.layered_closure_count > 0);
        assert!(response.material_closure_packet.texture_slot_count > 0);
        assert_eq!(response.material_closure_packet.closure_stack_hash.len(), 64);
        assert_eq!(response.material_closure_packet.bsdf_table_hash.len(), 64);
        assert_eq!(response.material_closure_packet.texture_table_hash.len(), 64);
        assert_eq!(response.material_closure_packet.resolve_hash.len(), 64);
        assert_eq!(response.material_closure_packet.packet_hash.len(), 64);
        assert!(response
            .material_closure_packet
            .clean_room_basis
            .contains("local_unreal_sparse_substrate"));
        assert!(response
            .material_closure_packet
            .entries
            .iter()
            .all(|entry| entry.closure_stack_id.len() == 64
                && entry.layer_count >= 1
                && entry.texture_slot_count >= 1
                && entry.closure_hash.len() == 64
                && entry.bsdf_hash.len() == 64
                && entry.texture_hash.len() == 64
                && entry.resolve_hash.len() == 64
                && entry.entry_hash.len() == 64
                && matches!(
                    entry.base_closure,
                    "thin_surface_specular_diffuse"
                        | "subsurface_diffuse"
                        | "metallic_ggx"
                        | "dielectric_ggx"
                )));
        assert_eq!(
            response.temporal_history_packet.schema,
            "forge.banger.temporal_history_packet.v1"
        );
        assert_eq!(
            response.temporal_history_packet.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.temporal_history_packet.material_closure_hash,
            response.material_closure_packet.packet_hash
        );
        assert_eq!(
            response.temporal_history_packet.direct_lighting_hash,
            response.direct_lighting_packet.packet_hash
        );
        assert_eq!(
            response.temporal_history_packet.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert!(response.temporal_history_packet.temporal_epoch > 0);
        assert_eq!(
            response.temporal_history_packet.history_layer_count,
            response.temporal_history_packet.entries.len()
        );
        assert_eq!(
            response.temporal_history_packet.motion_vector_tile_count,
            response.temporal_history_packet.entries.len()
        );
        assert!(response.temporal_history_packet.async_compute_candidate_count > 0);
        assert_eq!(response.temporal_history_packet.jitter_sequence_hash.len(), 64);
        assert_eq!(response.temporal_history_packet.motion_vector_hash.len(), 64);
        assert_eq!(
            response
                .temporal_history_packet
                .history_reprojection_hash
                .len(),
            64
        );
        assert_eq!(response.temporal_history_packet.disocclusion_mask_hash.len(), 64);
        assert_eq!(response.temporal_history_packet.rejection_hash.len(), 64);
        assert_eq!(response.temporal_history_packet.accumulation_hash.len(), 64);
        assert_eq!(response.temporal_history_packet.packet_hash.len(), 64);
        assert!(response
            .temporal_history_packet
            .clean_room_basis
            .contains("local_unreal_sparse_tsr_taa"));
        assert!(response
            .temporal_history_packet
            .entries
            .iter()
            .all(|entry| entry.history_layer_id.len() == 64
                && entry.temporal_epoch == response.temporal_history_packet.temporal_epoch
                && entry.jitter_index < 16
                && entry.motion_tile[2] > 0
                && entry.motion_tile[3] > 0
                && entry.history_tile[2] > 0
                && entry.history_tile[3] > 0
                && entry.accumulation_weight_q15 >= 1024
                && entry.material_closure_hash.len() == 64
                && entry.direct_resolve_hash.len() == 64
                && entry.motion_vector_hash.len() == 64
                && entry.history_reprojection_hash.len() == 64
                && entry.disocclusion_hash.len() == 64
                && entry.accumulation_hash.len() == 64
                && entry.entry_hash.len() == 64
                && matches!(
                    entry.history_kind,
                    "async_tsr_history_update"
                        | "responsive_taa_history"
                        | "standard_taa_history"
                )
                && matches!(
                    entry.rejection_mode,
                    "history_reject_disocclusion"
                        | "history_resurrection_candidate"
                        | "history_clamp_responsive_material"
                        | "history_accept"
                )));
        assert_eq!(
            response.page_residency_allocator.schema,
            "forge.banger.page_residency_allocator_packet.v1"
        );
        assert_eq!(
            response.page_residency_allocator.source_contract_hash,
            response.source_hash
        );
        assert_eq!(
            response.page_residency_allocator.resource_table_hash,
            response.resource_table.table_hash
        );
        assert_eq!(
            response.page_residency_allocator.nanite_second_layer_hash,
            response.nanite_second_layer_packet.packet_hash
        );
        assert_eq!(
            response.page_residency_allocator.virtual_shadow_hash,
            response.virtual_shadow_packet.packet_hash
        );
        assert_eq!(
            response.page_residency_allocator.material_closure_hash,
            response.material_closure_packet.packet_hash
        );
        assert_eq!(
            response.page_residency_allocator.render_graph_hash,
            response.render_graph_compilation.graph_hash
        );
        assert_eq!(
            response.page_residency_allocator.virtual_page_count,
            response.page_residency_allocator.entries.len()
        );
        assert!(response.page_residency_allocator.physical_page_count > 0);
        assert!(response.page_residency_allocator.resident_page_count > 0);
        assert!(response.page_residency_allocator.locked_page_count > 0);
        assert_eq!(response.page_residency_allocator.physical_pool_hash.len(), 64);
        assert_eq!(
            response
                .page_residency_allocator
                .virtual_page_table_hash
                .len(),
            64
        );
        assert_eq!(
            response
                .page_residency_allocator
                .feedback_request_hash
                .len(),
            64
        );
        assert_eq!(response.page_residency_allocator.allocation_hash.len(), 64);
        assert_eq!(response.page_residency_allocator.eviction_hash.len(), 64);
        assert_eq!(response.page_residency_allocator.packet_hash.len(), 64);
        assert!(response
            .page_residency_allocator
            .clean_room_basis
            .contains("local_unreal_sparse_nanite_vsm_virtual_texture"));
        assert!(response
            .page_residency_allocator
            .entries
            .iter()
            .any(|entry| entry.page_kind == "nanite_geometry_page"));
        assert!(response
            .page_residency_allocator
            .entries
            .iter()
            .any(|entry| entry.page_kind == "virtual_shadow_page"));
        assert!(response
            .page_residency_allocator
            .entries
            .iter()
            .any(|entry| entry.page_kind == "material_virtual_texture_page"));
        assert!(response
            .page_residency_allocator
            .entries
            .iter()
            .all(|entry| !entry.page_id.is_empty()
                && !entry.object_id.is_empty()
                && entry.source_page_hash.len() == 64
                && entry.priority > 0
                && entry.producer_hash.len() == 64
                && entry.feedback_hash.len() == 64
                && entry.allocation_hash.len() == 64
                && entry.eviction_hash.len() == 64
                && entry.entry_hash.len() == 64
                && matches!(
                    entry.page_kind,
                    "nanite_geometry_page"
                        | "virtual_shadow_page"
                        | "material_virtual_texture_page"
                )
                && matches!(
                    entry.lock_state,
                    "locked_for_frame" | "streaming_or_reuse" | "eviction_candidate"
                )));
        assert_eq!(
            response.gaussian_splat_layer_manifest.schema,
            "forge.banger.native_gaussian_splat_layer_manifest.v1"
        );
        assert_eq!(
            response.gaussian_splat_layer_manifest.layer_count,
            response.gaussian_splat_layer_manifest.layers.len()
        );
        assert_eq!(
            response.gaussian_splat_layer_manifest.conversion_count,
            response.gaussian_splat_layer_manifest.conversions.len()
        );
        assert_eq!(response.gaussian_splat_layer_manifest.proxy_bounds_hash.len(), 64);
        assert_eq!(response.gaussian_splat_layer_manifest.sort_key_hash.len(), 64);
        assert_eq!(response.gaussian_splat_layer_manifest.group_key_hash.len(), 64);
        assert_eq!(
            response
                .gaussian_splat_layer_manifest
                .conversion_manifest_hash
                .len(),
            64
        );
        assert_eq!(response.gaussian_splat_layer_manifest.manifest_hash.len(), 64);
        assert!(response
            .gaussian_splat_layer_manifest
            .conversions
            .iter()
            .any(|conversion| conversion.from_representation == "meshlet"
                && conversion.target_representation == "gaussian_splat_proxy"
                && conversion.enabled
                && conversion.proxy_bounds_hash.len() == 64
                && conversion.conversion_hash.len() == 64));
        assert_eq!(response.resource_table_hash.len(), 64);
        assert_eq!(response.resource_table.slot_count, response.resource_table.slots.len());
        assert!(response.resource_table.slot_count >= response.artifacts.len());
        assert!(response.resource_table.resident_bytes > 0);
        assert_eq!(response.frame_graph_bindings.len(), response.render_graph.len());
        assert!(response
            .frame_graph_bindings
            .iter()
            .all(|binding| !binding.resource_slots.is_empty()));
        assert_eq!(
            response.editable_scene_manifest.schema,
            "forge.banger.editable_scene_manifest.v1"
        );
        assert_eq!(
            response.editable_scene_manifest.object_count,
            response.artifacts.len() + 1
        );
        assert_eq!(response.editable_scene_manifest.root_count, 1);
        assert_eq!(response.editable_scene_manifest.manifest_hash.len(), 64);
        assert_eq!(response.editable_scene_manifest.graph_hash.len(), 64);
        assert_eq!(response.editable_scene_manifest.bounds_hash.len(), 64);
        assert!(response
            .editable_scene_manifest
            .objects
            .iter()
            .any(|object| object.representation == "meshlet"
                && object.parent_id.as_deref() == Some("test_scene:root")
                && object.source_artifact_name.is_some()));
        assert!(response
            .editable_scene_manifest
            .objects
            .iter()
            .all(|object| object.proof_hash.len() == 64
                && object.local_transform_hash.len() == 64
                && object.world_transform_hash.len() == 64
                && object.visible
                && object.bounding_sphere[3].is_finite()
                && object.bounding_sphere[3] >= 0.0));
        assert!(response
            .resource_table
            .slots
            .iter()
            .any(|slot| slot.kind == "meshlet_page" && slot.usage == "storage_read_indirect_draw"));
        assert!(response.render_handoff_hash.len() == 64);
    }

    #[test]
    fn prepares_real_gaussian_splat_ply_asset_buffers() {
        let path = fresh_tmp_path("banger-native-engine", "gaussian-splat-ply");
        let _tmp = TmpDir::new(path.clone());
        fs::create_dir_all(&path).expect("tmp dir");
        let ply_path = path.join("scan_room.ply");
        fs::write(
            &ply_path,
            r#"ply
format ascii 1.0
element vertex 2
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
0 0 0 0.2 0.3 0.4 0.0 -2 -2 -2 1 0 0 0
1 2 3 0.5 0.6 0.7 1.0 -1 -1 -1 0.70710677 0 0.70710677 0
"#,
        )
        .expect("write ply");

        let response =
            BangerNativeEngine::prepare_gaussian_splat_asset(BangerGaussianSplatAssetPrepareRequest {
                asset_id: Some("scan_room".to_string()),
                ply_path: ply_path.to_string_lossy().to_string(),
                max_splats: None,
                bucket_count: Some(2),
            })
            .expect("gaussian splat asset");

        assert!(response.ok);
        assert_eq!(response.schema, "forge.banger.gaussian_splat_asset_manifest.v1");
        assert_eq!(response.asset_id, "scan_room");
        assert_eq!(response.splat_count, 2);
        assert!(!response.truncated);
        assert_eq!(response.ply_format, "ply_ascii_1_0");
        assert_eq!(response.property_count, 14);
        assert_eq!(response.buckets.len(), 2);
        assert_eq!(
            response.gpu_layout.covariance_format,
            "scale_log_float32x3+rotation_quat_float32x4"
        );
        assert_eq!(response.gpu_layout.bytes_per_splat, 244);
        assert_eq!(response.source_hash.len(), 64);
        assert_eq!(response.positions_buffer_hash.len(), 64);
        assert_eq!(response.covariance_buffer_hash.len(), 64);
        assert_eq!(response.opacity_buffer_hash.len(), 64);
        assert_eq!(response.sh_buffer_hash.len(), 64);
        assert_eq!(response.sort_key_hash.len(), 64);
        assert_eq!(response.bucket_manifest_hash.len(), 64);
        assert_eq!(response.gpu_layout_hash.len(), 64);
        assert_eq!(response.asset_manifest_hash.len(), 64);
        assert!(response.bounds_min.iter().all(|value| value.is_finite()));
        assert!(response.bounds_max.iter().all(|value| value.is_finite()));
        assert!(response.bounding_sphere.iter().all(|value| value.is_finite()));
        assert!(response
            .buckets
            .iter()
            .all(|bucket| bucket.splat_count > 0
                && bucket.sort_key_hash.len() == 64
                && bucket.proof_hash.len() == 64));
    }

    #[test]
    fn rasterizes_real_gaussian_splat_ply_to_rgba8() {
        let path = fresh_tmp_path("banger-native-engine", "gaussian-splat-raster");
        let _tmp = TmpDir::new(path.clone());
        fs::create_dir_all(&path).expect("tmp dir");
        let ply_path = path.join("raster_room.ply");
        fs::write(
            &ply_path,
            r#"ply
format ascii 1.0
element vertex 3
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
0 0 0 1.4 0.0 0.0 4.0 -1.15 -1.15 -1.15 1 0 0 0
0.25 0.1 0.2 0.0 1.2 0.0 3.0 -1.35 -1.35 -1.35 1 0 0 0
-0.3 -0.1 -0.1 0.0 0.0 1.2 3.0 -1.45 -1.45 -1.45 1 0 0 0
"#,
        )
        .expect("write raster ply");

        let response = BangerNativeEngine::rasterize_gaussian_splat_asset(
            BangerGaussianSplatRasterizeRequest {
                asset_id: Some("raster_room".to_string()),
                ply_path: ply_path.to_string_lossy().to_string(),
                width: 64,
                height: 64,
                camera_position: Some([0.0, 0.0, -4.0]),
                camera_target: Some([0.0, 0.0, 0.0]),
                camera_up: Some([0.0, 1.0, 0.0]),
                fov_y_degrees: Some(45.0),
                near_plane: Some(0.01),
                max_splats: None,
                tile_size: Some(16),
                background_rgba: Some([0.0, 0.0, 0.0, 0.0]),
            },
        )
        .expect("rasterize gaussian splat");

        assert!(response.ok);
        assert_eq!(response.schema, "forge.banger.gaussian_splat_rasterizer.v1");
        assert_eq!(response.width, 64);
        assert_eq!(response.height, 64);
        assert_eq!(response.tile_size, 16);
        assert_eq!(response.tile_count, 16);
        assert_eq!(response.splat_count, 3);
        assert_eq!(response.projected_splat_count, 3);
        assert!(response.rasterized_splat_count > 0);
        assert!(response.shaded_pixel_count > 0);
        assert_eq!(response.rgba8.len(), 64 * 64 * 4);
        assert_eq!(response.camera_hash.len(), 64);
        assert_eq!(response.tile_manifest_hash.len(), 64);
        assert_eq!(response.projected_manifest_hash.len(), 64);
        assert_eq!(response.rgba8_hash.len(), 64);
        assert_eq!(response.raster_proof_hash.len(), 64);
        assert!(response
            .tiles
            .iter()
            .any(|tile| tile.splat_count > 0 && tile.contribution_count > 0));
        let alpha_sum: u32 = response
            .rgba8
            .chunks_exact(4)
            .map(|pixel| pixel[3] as u32)
            .sum();
        assert!(alpha_sum > 0);
        let center = ((32 * 64 + 32) * 4) as usize;
        assert!(response.rgba8[center + 3] > 0);
    }

    #[test]
    fn prepares_newobject_contract_as_scene_manifest_edit() {
        let path = fresh_tmp_path("banger-native-engine", "newobject");
        let pipeline_cache_dir = path.join("pipeline-cache").to_string_lossy().to_string();
        let _tmp = TmpDir::new(path.clone());
        let store = Store::open(path).expect("store");
        let monster = MonsterNode::new(store, MemoryGovernor::new(8 * 1024 * 1024));

        let response = BangerNativeEngine::prepare_newobject_contract(
            &monster,
            BangerNewObjectPrepareRequest {
                scene_id: Some("design_scene".to_string()),
                object_id: Some("wing_panel".to_string()),
                parent_id: None,
                object_prompt: "Create a lightweight SDF wing panel with editable material slots".to_string(),
                representation: Some("sdf".to_string()),
                known_fragment_hashes: None,
                target_frame_ms: Some(16.67),
                vram_budget_mb: Some(2048),
                prefer_mesh_shaders: Some(true),
                pipeline_cache_dir: Some(pipeline_cache_dir),
                viewport_width: Some(1440),
                viewport_height: Some(900),
            },
        )
        .expect("newobject contract");

        assert!(response.ok);
        assert_eq!(response.command, "/newobject_");
        assert_eq!(response.lane, "native_tandem_render");
        assert_eq!(response.scene_id, "design_scene");
        assert_eq!(response.object_id, "wing_panel");
        assert_eq!(response.representation, "sdf");
        assert!(response.contract_prepared);
        assert!(!response.gpu_page_promotion_allowed);
        assert_ne!(response.base_manifest_hash, response.updated_manifest_hash);
        assert_eq!(response.newobject_contract_source_hash.len(), 64);
        assert_eq!(response.newobject_contract_manifest_hash.len(), 64);
        assert_eq!(response.newobject_contract_proof_hash.len(), 64);
        assert_eq!(response.edit_hash.len(), 64);
        assert!(response
            .editable_scene_manifest
            .objects
            .iter()
            .any(|object| object.object_id == "design_scene:wing_panel"
                && object.parent_id.as_deref() == Some("design_scene:root")
                && object.source_artifact_kind == "newobject_contract"
                && object.representation == "sdf"
                && object.residency_policy == "pending_contract_no_gpu_pages_until_renderer_promotion"
                && object.proof_hash.len() == 64));
    }
}
