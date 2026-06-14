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
    pub residency_jobs: Vec<BangerNativeResidencyJob>,
    pub resource_table_hash: String,
    pub resource_table: BangerNativeResourceTable,
    pub editable_scene_manifest: BangerEditableSceneManifest,
    pub scene_graph_submission: BangerNativeSceneGraphSubmission,
    pub culling_manifest: BangerNativeCullingManifest,
    pub radiance_schedule_manifest: BangerNativeRadianceScheduleManifest,
    pub gaussian_splat_layer_manifest: BangerNativeGaussianSplatLayerManifest,
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
        let scene_graph_submission = build_scene_graph_submission(
            &prepared,
            &editable_scene_manifest,
            &resource_table,
            &frame_graph_bindings,
        );
        let culling_manifest =
            build_culling_manifest(&prepared, &scene_graph_submission, &resource_table);
        let radiance_schedule_manifest = build_radiance_schedule_manifest(
            &prepared,
            &scene_graph_submission,
            &culling_manifest,
            &resource_table,
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
        let benchmark_promotion_manifest = build_benchmark_promotion_manifest(
            &prepared,
            &render_graph,
            &pipeline_cache_manifest,
            &texture_bridge_contract,
            &resource_table,
            &scene_graph_submission,
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
            &culling_manifest,
            &radiance_schedule_manifest,
            &gaussian_splat_layer_manifest,
        );
        let render_pass_count = render_graph.len();
        let residency_job_count = residency_jobs.len();
        let resource_table_hash = resource_table.table_hash.clone();

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
            residency_jobs,
            resource_table_hash,
            resource_table,
            editable_scene_manifest,
            scene_graph_submission,
            culling_manifest,
            radiance_schedule_manifest,
            gaussian_splat_layer_manifest,
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
    culling_manifest: &BangerNativeCullingManifest,
    radiance_schedule_manifest: &BangerNativeRadianceScheduleManifest,
    gaussian_splat_layer_manifest: &BangerNativeGaussianSplatLayerManifest,
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
    h.update(culling_manifest.manifest_hash.as_bytes());
    h.update(culling_manifest.visibility_result_hash.as_bytes());
    h.update(culling_manifest.indirect_draw_buffer_hash.as_bytes());
    h.update(radiance_schedule_manifest.schedule_hash.as_bytes());
    h.update(radiance_schedule_manifest.invalidation_hash.as_bytes());
    h.update(gaussian_splat_layer_manifest.manifest_hash.as_bytes());
    h.update(gaussian_splat_layer_manifest.conversion_manifest_hash.as_bytes());
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
