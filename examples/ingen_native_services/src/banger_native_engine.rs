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
    pub texture_bridge_contract: BangerNativeTextureBridgeContract,
    pub pipeline_cache_keys: Vec<String>,
    pub render_graph: Vec<BangerNativeRenderPass>,
    pub residency_jobs: Vec<BangerNativeResidencyJob>,
    pub resource_table_hash: String,
    pub resource_table: BangerNativeResourceTable,
    pub editable_scene_manifest: BangerEditableSceneManifest,
    pub scene_graph_submission: BangerNativeSceneGraphSubmission,
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
    pub proof_hash: String,
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
        let render_handoff_hash = render_handoff_hash(
            &prepared,
            &artifacts,
            &shader_compiler_ticket,
            &pipeline_cache_manifest,
            &texture_bridge_contract,
            &scene_graph_submission,
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
                fallback_path: "compute_cull_indirect_draw_when_mesh_shader_unavailable",
                capability_gate: "render_manifest_hash+shader_profile+renderer_cache_hash+shader_compiler_ticket_hash",
                compiler_ticket_hash: shader_compiler_ticket.proof_hash.clone(),
            },
            shader_compiler_ticket,
            pipeline_cache_manifest,
            texture_bridge_contract,
            pipeline_cache_keys,
            render_graph,
            residency_jobs,
            resource_table_hash,
            resource_table,
            editable_scene_manifest,
            scene_graph_submission,
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
        render_pass_abi_hash,
        renderer_variant_hash: artifact.renderer_variant_hash.clone(),
        blob_hash,
        blob_len: blob_bytes.len() as u64,
        blob_path: blob_path.to_string_lossy().to_string(),
        persistence_status,
        proof_hash,
    })
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
    h.update(b"forge.banger.shader_reflection_stub.v1\0");
    h.update(shader_compiler_ticket.proof_hash.as_bytes());
    h.update(shader_compiler_ticket.source_language.as_bytes());
    h.update(shader_compiler_ticket.promoted_target.as_bytes());
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
    let promoted_target = if compiler_detected {
        "slang_wgsl_manifest_v0"
    } else {
        "wgsl_inline_bootstrap"
    };
    let module_strategy = if compiler_detected {
        "compile_slang_module_then_validate_wgsl_with_wgpu"
    } else {
        "inline_wgsl_until_slangc_or_api_binding_is_available"
    };
    let reflection_status = if compiler_detected {
        "command_line_compile_only_reflection_api_not_bound_yet"
    } else {
        "reflection_deferred_until_slang_api_binding"
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
            source_hash,
            output_hash: hex32(Sha256::digest(b"slangc_not_found").into()),
            status: "compiler_absent",
            excerpt: "slangc_not_found_on_path".to_string(),
            wgsl: None,
        };
    }

    match compile_slang_mini_probe_to_wgsl(&source) {
        Ok(wgsl) => SlangMiniProbe {
            source_hash,
            output_hash: hex32(Sha256::digest(wgsl.as_bytes()).into()),
            status: "compiled_wgsl",
            excerpt: compact_one_line(&wgsl),
            wgsl: Some(wgsl),
        },
        Err(err) => SlangMiniProbe {
            source_hash,
            output_hash: hex32(Sha256::digest(err.as_bytes()).into()),
            status: "compile_failed",
            excerpt: compact_one_line(&err),
            wgsl: None,
        },
    }
}

fn compile_slang_mini_probe_to_wgsl(source: &str) -> Result<String, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let dir = env::temp_dir().join(format!("forge-banger-slang-probe-{}-{stamp}", process::id()));
    fs::create_dir_all(&dir).map_err(|err| format!("create temp slang probe dir failed: {err}"))?;
    let source_path = dir.join("banger_probe.slang");
    let out_path = dir.join("banger_probe.wgsl");
    fs::write(&source_path, source).map_err(|err| format!("write slang probe failed: {err}"))?;
    let output = Command::new("slangc")
        .arg(&source_path)
        .arg("-target")
        .arg("wgsl")
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
    fs::read_to_string(&out_path).map_err(|err| format!("read slang wgsl probe failed: {err}; {diagnostic}"))
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

fn render_handoff_hash(
    prepared: &MonsterPreparedCompute,
    artifacts: &[BangerNativeRenderArtifactSummary],
    shader_compiler_ticket: &BangerNativeShaderCompilerTicket,
    pipeline_cache_manifest: &BangerNativePipelineCacheManifest,
    texture_bridge_contract: &BangerNativeTextureBridgeContract,
    scene_graph_submission: &BangerNativeSceneGraphSubmission,
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_handoff.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(shader_compiler_ticket.proof_hash.as_bytes());
    h.update(pipeline_cache_manifest.manifest_hash.as_bytes());
    h.update(texture_bridge_contract.bridge_proof_hash.as_bytes());
    h.update(texture_bridge_contract.frame_hash.as_bytes());
    h.update(texture_bridge_contract.viewport_contract_hash.as_bytes());
    h.update(scene_graph_submission.submission_hash.as_bytes());
    h.update(scene_graph_submission.render_submission_hash.as_bytes());
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
                && entry.render_pass_abi_hash.len() == 64
                && entry.blob_hash.len() == 64
                && entry.blob_len > 0
                && entry.persistence_status == "seed_blob_persisted"
                && std::path::Path::new(&entry.blob_path).exists()));
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
