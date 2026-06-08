pub mod banger_viewport;
pub mod brain_gpu;
pub mod code_assets;
pub mod cutover_audit;
pub mod front_compute_cache;
pub mod front_runtime_cache;
pub mod motion_lane;
pub mod obsolete_front;
pub mod packaging_security;
pub mod proof;
pub mod product_sections;
pub mod services;
pub mod state;
pub mod visual_parity;
pub mod webexplorer;
pub mod webatlas;
pub mod webview_probe;
pub mod wgpu_probe;

pub use banger_viewport::{
    banger_frame_image, banger_viewport_frame_from_rgba, render_banger_viewport_frame,
    render_banger_viewport_frame_with, BangerSlintTextureBridgeProof, BangerViewportFrame,
    BangerViewportRequest,
};
pub use code_assets::{google_logo_wine_image, GOOGLE_LOGO_WINE_SVG};
pub use cutover_audit::{
    build_cutover_audit_report, CutoverAuditReport, NativeShellManifestCheck,
    ProtectedBackendService,
};
pub use front_compute_cache::{
    front_compute_bytes, front_compute_json, front_perf_animation_sample, front_perf_scope,
};
pub use front_runtime_cache::{
    build_cached_banger_viewport_frame, build_cached_stage0_report, cached_banger_viewport_rgba,
    cached_local_service_snapshot, cached_work_motion_lane,
};
pub use brain_gpu::BrainGpuRenderer;
pub use motion_lane::{
    css_work_loader_rgba_frames, render_brain_core_lava_rgba, MotionLane, MotionLaneManifest,
    BRAIN_CORE_DIM, BRAIN_CORE_FRAME_COUNT, WORK_MOTION_FRAME_COUNT, WORK_MOTION_HEIGHT,
    WORK_MOTION_WIDTH,
};
pub use obsolete_front::{
    build_obsolete_front_manifest, ObsoleteFrontManifest, ObsoleteFrontPath,
};
pub use packaging_security::{
    build_packaging_security_manifest, crash_recovery_record, decide_local_path_access,
    native_app_paths, native_capability_policy, CrashRecoveryRecord, LocalPathDecision,
    NativeAppPaths, NativeCapabilityPolicy, PackagingSecurityManifest,
};
pub use proof::{build_stage0_report, stage0_report_json, Stage0Report};
pub use product_sections::{
    build_product_sections_manifest, product_section_projection, ProductSectionProjection,
    ProductSectionState, ProductSectionsManifest,
};
pub use services::{
    fake_service_snapshot, local_service_command, local_service_snapshot, spawn_fake_long_job,
    native_uploaded_code_from_text, native_uploaded_file_from_path, DirectNativeServices,
    FakeNativeServices, NativeCommandServices, NativeServiceCommand, NativeServiceCommandResult,
    NativeUiServices, ServiceSnapshot, ServiceStreamEvent,
};
pub use state::{
    checkpoint_json, replay_checkpoint, replay_projection, NativeAgentCard, NativeAgentCardKind,
    NativeChatMessage, NativeMessageRole, NativeSessionSummary, NativeStateCheckpoint,
    NativeStateKernel, NativeUiEvent, NativeUiProjection, NativeUiState, NativeUploadKind,
    NativeUploadedFile,
};
pub use visual_parity::{forge_first_viewport_parity, ForgeFirstViewportParity};
pub use webexplorer::{
    prepare_webexplorer_runtime_action, webexplorer_fixture_markup,
    webexplorer_initialization_script, webexplorer_isolation_proof, webexplorer_runtime_bridge,
    WebExplorerBounds, WebExplorerIsolationProof, WebExplorerNavigationDecision,
    WebExplorerPolicy, WebExplorerRuntimeActionPlan, WebExplorerRuntimeBridge,
};
pub use webatlas::{
    atlas_monster_perception_plan, atlas_query_projection, atlas_safety_policy_report,
    atlas_ui_projection, atlas_navigateweb_gate, benchmark_webatlas_fixture, capture_fixture_webatlas,
    capture_webatlas_from_markup, diff_webatlas_manifests, heal_webatlas_ref,
    AtlasBenchmarkReport, AtlasBenchmarkRow, AtlasBounds, AtlasCoverageReport,
    AtlasIncrementalDiff, AtlasManifest, AtlasMonsterPerceptionJob,
    AtlasMonsterPerceptionPlan, AtlasNode, AtlasNodeDelta, AtlasPolicyFinding,
    AtlasNavigateWebAction, AtlasNavigateWebGate, AtlasQueryItem, AtlasQueryProjection,
    AtlasQueryView, AtlasRefHealingReport, AtlasRefHypothesis, AtlasResource,
    AtlasSafetyPolicyReport, AtlasUiProjection,
};
