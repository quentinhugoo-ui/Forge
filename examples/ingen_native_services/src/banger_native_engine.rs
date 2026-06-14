use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    process::{self, Command},
    time::{SystemTime, UNIX_EPOCH},
};

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
    pub pipeline_cache_keys: Vec<String>,
    pub render_graph: Vec<BangerNativeRenderPass>,
    pub residency_jobs: Vec<BangerNativeResidencyJob>,
    pub resource_table_hash: String,
    pub resource_table: BangerNativeResourceTable,
    pub editable_scene_manifest: BangerEditableSceneManifest,
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
        let render_handoff_hash = render_handoff_hash(&prepared, &artifacts, &shader_compiler_ticket);
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
            pipeline_cache_keys,
            render_graph,
            residency_jobs,
            resource_table_hash,
            resource_table,
            editable_scene_manifest,
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
) -> String {
    let mut h = Sha256::new();
    h.update(b"forge.banger.native_render_handoff.v1\0");
    h.update(prepared.manifest_hash.as_bytes());
    h.update(prepared.route.plan.proof_hash.as_bytes());
    h.update(shader_compiler_ticket.proof_hash.as_bytes());
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
