//! SCAN/KASM: one hot node, one tiny bytecode, one content store.

mod codec;
mod key;
pub mod kasm;
mod memory;
pub mod agent;
pub mod apply;
pub mod atlas;
pub mod brain;
pub mod compute_core;
pub mod cpu_bits;
pub mod cpu_simd;
pub mod fast_hash;
pub use kasm::fbc;
pub mod godel;
pub mod introspection;
pub mod landauer;
pub mod act_codes;
pub mod meta;
mod monster;
pub mod native_gpu_probe;
mod store;
pub mod svdag;

pub struct TmpDir(std::path::PathBuf);

impl TmpDir {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<std::path::Path> for TmpDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn fresh_tmp_path(prefix: &str, tag: &str) -> std::path::PathBuf {
    let mut p = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(".codex-tmp");
    p.push(format!("{prefix}-{tag}-{nanos}"));
    p
}

pub use agent::extract;
pub use kasm::numeric;

pub use codec::{
    nanocube_pack_recipe_i64, nanocube_unpack_recipe_i64, pack_lossless, unpack_lossless,
    CodecError,
};
pub use brain::{
    brain_semantic_ref, brain_substitution_ref, publish_program_substitution,
    publish_brain_skill_promotion, publish_semantic_attractor, resolve_program_hash,
    tighten_program_for_execution, BrainAction, BrainExperienceDigest, BrainMemory,
    BrainProofProjection, BrainSkillCandidate, BrainSkillPromotionProof, ForgeBrain,
    BRAIN_HEAD_REF, BRAIN_LATEST_ACTIVE_REF, BRAIN_SEMANTIC_REF_PREFIX,
    BRAIN_SKILL_PROMOTION_REF_PREFIX, BRAIN_SKILL_REF_PREFIX, BRAIN_STATE_REF,
    BRAIN_SUBSTITUTION_REF_PREFIX,
};
pub use kasm::{PartialEvalReport, RewriteReport};
pub use key::CallKey;
pub use memory::MemoryGovernor;
pub use monster::{
    canonical_outputs, fnv64_outputs, migrate_legacy_hot_atlas, output_fingerprint,
    Atlas, AtlasIngest, BatchCall, BatchInput, BatchPackedOutput, BulkEvaluator,
    CudaStatus, DispatchResult, GpuNode, GpuNodeBootstrap, GpuNodeRuntime,
    InlineCache, KasmStructure, LiveAtlas, LiveAtlasCounters, MonsterBackendHint,
    MonsterCall, MonsterComputeFragment, MonsterComputeGraphError, MonsterComputeGraphPlan,
    MonsterComputeScale, MonsterComputeWorkUnit, MonsterDifferentialExecution,
    MonsterDifferentialTestPlan, MonsterEngineLane, MonsterEngineRoute,
    MonsterFeatureDisposition, MonsterGpuBatchPlan, MonsterMassComputeError,
    MonsterMassComputeExecution, MonsterMathCapabilityManifest, MonsterMathClass,
    MonsterMathClassTemplate, MonsterMathConstant, MonsterMathContract,
    MonsterMathContractError, MonsterMathOutputContract, MonsterMathSample,
    MonsterMathVariable, MonsterCompiledMathContract, MonsterCodeTemplate,
    MonsterCodeTemplateError, MonsterCodeTemplateKind, MonsterCodeTemplateManifest,
    MonsterGeneratedCodeFile, MonsterGeneratedRustModule, MonsterMaterializedCodeFile,
    MonsterMaterializedRustModule, MonsterRustConnector, MonsterRustField, MonsterRustMetric,
    MonsterRustPortAdapterContract,
    MonsterMultiAdapterSchedule,
    MonsterNativeTandemArtifact, MonsterNativeTandemArtifactPage, MonsterNativeTandemDomain,
    MonsterNewComputeLlmResult, MonsterNewComputeLlmScalarOutput,
    MonsterNewComputeLlmScientificMetric, MonsterNewComputeLlmTypedBuffer,
    MonsterNode, MonsterNumericPolicy, MonsterOutputArtifact, MonsterPreparedCompute,
    MonsterSource, MonsterStats, MonsterSymbolicMathOutput, MonsterSymbolicMathPlan,
    MonsterTypedResultArtifact, MonsterTypedResultBuffer, MonsterTypedResultPage,
    MonsterUniversalComputeTemplate, MonsterValue, PhysicalEnvelope,
    PredictedSlot, SizeClass,
    StridePredictor, GPUNODE_BOOTSTRAP_PATH, INLINE_MASK, INLINE_SLOTS,
    LEGACY_HOT_ATLAS_PATH, LIVE_ATLAS_PATH, bootstrap_gpunodes,
    bootstrap_gpunodes_best_effort, dx12_available, find_cuda_bin_dir,
    generate_rust_port_adapter, gpu_capability_report, materialize_generated_rust_module,
    monster_code_template_manifest, register_cuda_dll_path, take_last_cuda_status,
    vulkan_available,
};
#[cfg(feature = "wgpu")]
pub use monster::run_wgpu_universal_for_test;

#[cfg(feature = "cuda")]
pub mod cuda_min;
#[cfg(feature = "cuda")]
pub use cuda_min::{
    cuda_min_available as forge_cuda_min_available, kasm_runtime as forge_kasm_runtime,
    KASM_INTERPRET_FATBIN as FORGE_KASM_INTERPRET_FATBIN,
};
pub use monster::{SwarmKnowledgeFrame, SwarmMemo, SwarmPresence};
pub use native_gpu_probe::{native_gpu_probe, NativeGpuAdapter, NativeGpuProbe};
pub use store::{Hash, Store};
