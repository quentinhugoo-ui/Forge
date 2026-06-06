pub mod banger_viewport;
pub mod cutover_audit;
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
    banger_frame_image, render_banger_viewport_frame, render_banger_viewport_frame_with,
    BangerSlintTextureBridgeProof, BangerViewportFrame, BangerViewportRequest,
};
pub use cutover_audit::{
    build_cutover_audit_report, CutoverAuditReport, NativeShellManifestCheck,
    ProtectedBackendService,
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
    DirectNativeServices, FakeNativeServices, NativeCommandServices, NativeServiceCommand,
    NativeServiceCommandResult, NativeUiServices, ServiceSnapshot, ServiceStreamEvent,
};
pub use state::{
    checkpoint_json, replay_checkpoint, replay_projection, NativeAgentCard, NativeAgentCardKind,
    NativeChatMessage, NativeMessageRole, NativeSessionSummary, NativeStateCheckpoint,
    NativeStateKernel, NativeUiEvent, NativeUiProjection, NativeUiState,
};
pub use visual_parity::{forge_first_viewport_parity, ForgeFirstViewportParity};
pub use webexplorer::{
    webexplorer_fixture_markup, webexplorer_initialization_script, webexplorer_isolation_proof,
    WebExplorerBounds, WebExplorerIsolationProof, WebExplorerNavigationDecision,
    WebExplorerPolicy,
};
pub use webatlas::{
    atlas_ui_projection, capture_fixture_webatlas, capture_webatlas_from_markup, AtlasBounds,
    AtlasCoverageReport, AtlasManifest, AtlasNode, AtlasResource, AtlasUiProjection,
};
