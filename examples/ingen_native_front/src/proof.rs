use crate::banger_viewport::{
    render_banger_viewport_frame_with, BangerSlintTextureBridgeProof, BangerViewportRequest,
    BANGER_VIEWPORT_HEIGHT, BANGER_VIEWPORT_WIDTH,
};
use crate::cutover_audit::{build_cutover_audit_report, CutoverAuditReport};
use crate::obsolete_front::{build_obsolete_front_manifest, ObsoleteFrontManifest};
use crate::packaging_security::{
    build_packaging_security_manifest, PackagingSecurityManifest,
};
use crate::product_sections::{
    build_product_sections_manifest, product_section_projection, ProductSectionProjection,
    ProductSectionsManifest,
};
use crate::state::{replay_checkpoint, NativeUiEvent};
use crate::services::{
    fake_service_snapshot, spawn_fake_long_job, DirectNativeServices, NativeCommandServices,
    NativeServiceCommand, NativeServiceCommandResult, NativeUiServices, ServiceSnapshot,
};
use crate::visual_parity::{forge_first_viewport_parity, ForgeFirstViewportParity};
use crate::webexplorer::{webexplorer_isolation_proof, WebExplorerIsolationProof};
use crate::webatlas::{atlas_ui_projection, capture_fixture_webatlas, AtlasManifest, AtlasUiProjection};
use crate::webview_probe::{run_webview_probe, WebViewProbe};
use crate::wgpu_probe::{run_wgpu_probe, WgpuProbe};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlintProbe {
    pub backend: String,
    pub renderer_policy: String,
    pub ui_source: String,
    pub app_shell_uses_tauri: bool,
    pub app_shell_uses_html_css_js: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateProbe {
    pub kernel_source: String,
    pub projection_schema: String,
    pub checkpoint_schema: String,
    pub canonical_replay_hash: String,
    pub canonical_event_log_hash: String,
    pub canonical_state_hash: String,
    pub keyboard_shortcuts: Vec<String>,
    pub direct_mutation_blocked: bool,
    pub browser_ipc_required: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServicesProbe {
    pub trait_source: String,
    pub fake_snapshot: ServiceSnapshot,
    pub direct_snapshot: ServiceSnapshot,
    pub direct_refresh_command: NativeServiceCommandResult,
    pub fake_long_job_statuses: Vec<String>,
    pub browser_ipc_required: bool,
    pub direct_probe_services_connected: bool,
    pub real_services_connected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BangerViewportProbe {
    pub schema: String,
    pub source: String,
    pub scene_id: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub transport: String,
    pub backend: String,
    pub adapter_name: String,
    pub texture_usage: Vec<String>,
    pub render_target_hash: String,
    pub frame_hash: String,
    pub telemetry_hash: String,
    pub viewport_contract_hash: String,
    pub slint_texture_bridge: BangerSlintTextureBridgeProof,
    pub visible_in_slint: bool,
    pub browser_canvas_required: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesignProbe {
    pub reference_front: String,
    pub inventory: String,
    pub native_tokens: String,
    pub locked_dimensions: Vec<String>,
    pub native_component_targets: Vec<String>,
    pub forge_first_viewport: ForgeFirstViewportParity,
    pub parity_ready: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Stage0Report {
    pub schema: String,
    pub stage: String,
    pub created_for: String,
    pub crate_root: String,
    pub os: String,
    pub design: DesignProbe,
    pub state: StateProbe,
    pub services: ServicesProbe,
    pub banger_viewport: BangerViewportProbe,
    pub webexplorer: WebExplorerIsolationProof,
    pub web_atlas: AtlasManifest,
    pub web_atlas_ui: AtlasUiProjection,
    pub product_sections: ProductSectionsManifest,
    pub product_section_ui: ProductSectionProjection,
    pub packaging_security: PackagingSecurityManifest,
    pub obsolete_front: ObsoleteFrontManifest,
    pub cutover_audit: CutoverAuditReport,
    pub slint: SlintProbe,
    pub wgpu: WgpuProbe,
    pub webview: WebViewProbe,
    pub limitations: Vec<String>,
    pub manual_proofs_required: Vec<String>,
    pub promotion_ready: bool,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stage0HashEnvelope<'a> {
    schema: &'a str,
    stage: &'a str,
    created_for: &'a str,
    crate_root: &'a str,
    os: &'a str,
    design: &'a DesignProbe,
    state: &'a StateProbe,
    services: &'a ServicesProbe,
    banger_viewport: &'a BangerViewportProbe,
    webexplorer: &'a WebExplorerIsolationProof,
    web_atlas: &'a AtlasManifest,
    web_atlas_ui: &'a AtlasUiProjection,
    product_sections: &'a ProductSectionsManifest,
    product_section_ui: &'a ProductSectionProjection,
    packaging_security: &'a PackagingSecurityManifest,
    obsolete_front: &'a ObsoleteFrontManifest,
    cutover_audit: &'a CutoverAuditReport,
    slint: &'a SlintProbe,
    wgpu: &'a WgpuProbe,
    webview: &'a WebViewProbe,
    limitations: &'a [String],
    manual_proofs_required: &'a [String],
    promotion_ready: bool,
}

pub fn build_stage0_report() -> Stage0Report {
    build_stage0_report_from(run_wgpu_probe(), run_webview_probe())
}

pub fn stage0_report_json(report: &Stage0Report) -> String {
    serde_json::to_string_pretty(report).expect("serialize stage0 report")
}

pub fn build_stage0_report_from(wgpu: WgpuProbe, webview: WebViewProbe) -> Stage0Report {
    let direct_services = DirectNativeServices::from_probes(wgpu.clone(), webview.clone());
    let slint = SlintProbe {
        backend: "winit".to_string(),
        renderer_policy: [
            "femtovg/software first for Stage 1 visual MVP;",
            "skia remains gated because skia-bindings is too heavy for the current disk/toolchain state",
        ]
        .join(" "),
        ui_source: ".slint".to_string(),
        app_shell_uses_tauri: false,
        app_shell_uses_html_css_js: false,
    };
    let design = DesignProbe {
        reference_front: "examples/forge_tauri_ui/ui".to_string(),
        inventory: "examples/ingen_native_front/design/current_tauri_front_inventory.md".to_string(),
        native_tokens: "examples/ingen_native_front/ui/tokens.slint".to_string(),
        locked_dimensions: vec![
            "titlebar=38px".to_string(),
            "left_panel=279px".to_string(),
            "right_panel=287px".to_string(),
            "chat_command_square=94px".to_string(),
            "banger_shell_columns=240px_1fr_280px".to_string(),
        ],
        native_component_targets: vec![
            "AppWindow".to_string(),
            "TitleBar".to_string(),
            "LeftPanel".to_string(),
            "WorkspaceHeader".to_string(),
            "DropCanvas".to_string(),
            "SectionStatusDock".to_string(),
            "AgentSurface".to_string(),
            "ProductSectionSurface".to_string(),
            "ChatBar".to_string(),
            "NativeModal".to_string(),
        ],
        forge_first_viewport: forge_first_viewport_parity(),
        parity_ready: false,
    };
    let canonical_checkpoint = replay_checkpoint(&[
        NativeUiEvent::Navigate {
            section: "banger".to_string(),
        },
        NativeUiEvent::ChatDraftChanged {
            draft: "/newcompute_ sdf".to_string(),
        },
        NativeUiEvent::SendChat,
    ]);
    let state = StateProbe {
        kernel_source: "examples/ingen_native_front/src/state.rs".to_string(),
        projection_schema: "NativeStateKernel -> NativeUiEvent log -> NativeUiProjection".to_string(),
        checkpoint_schema: canonical_checkpoint.schema.clone(),
        canonical_replay_hash: canonical_checkpoint.projection.projection_hash.clone(),
        canonical_event_log_hash: canonical_checkpoint.event_log_hash.clone(),
        canonical_state_hash: canonical_checkpoint.state_hash.clone(),
        keyboard_shortcuts: vec![
            "Control+Tab -> NavigateNext".to_string(),
            "Escape -> CloseModal".to_string(),
        ],
        direct_mutation_blocked: true,
        browser_ipc_required: false,
    };
    let services = ServicesProbe {
        trait_source: "examples/ingen_native_front/src/services.rs".to_string(),
        fake_snapshot: fake_service_snapshot(),
        direct_snapshot: direct_services.snapshot(),
        direct_refresh_command: direct_services.handle_command(NativeServiceCommand::RefreshSnapshot),
        fake_long_job_statuses: fake_long_job_statuses(),
        browser_ipc_required: false,
        direct_probe_services_connected: true,
        real_services_connected: false,
    };
    let banger_frame = render_banger_viewport_frame_with(
        BangerViewportRequest {
            scene_id: "stage4-native-fixture".to_string(),
            width: BANGER_VIEWPORT_WIDTH,
            height: BANGER_VIEWPORT_HEIGHT,
            frame_index: 0,
        },
        wgpu.clone(),
    );
    let banger_viewport = BangerViewportProbe {
        schema: "ingen.native_front.stage4_banger_viewport.v1".to_string(),
        source: "examples/ingen_native_front/src/banger_viewport.rs".to_string(),
        scene_id: banger_frame.scene_id.clone(),
        width: banger_frame.width,
        height: banger_frame.height,
        format: banger_frame.format.clone(),
        transport: "wgpu texture manifest + Slint SharedPixelBuffer fallback".to_string(),
        backend: banger_frame.backend.clone(),
        adapter_name: banger_frame.adapter_name.clone(),
        texture_usage: banger_frame.texture_usage.clone(),
        render_target_hash: banger_frame.render_target_hash.clone(),
        frame_hash: banger_frame.frame_hash.clone(),
        telemetry_hash: banger_frame.telemetry_hash.clone(),
        viewport_contract_hash: banger_frame.viewport_contract_hash.clone(),
        slint_texture_bridge: banger_frame.slint_texture_bridge.clone(),
        visible_in_slint: true,
        browser_canvas_required: false,
    };
    let webexplorer = webexplorer_isolation_proof();
    let web_atlas = capture_fixture_webatlas();
    let web_atlas_ui = atlas_ui_projection(&web_atlas, 12);
    let product_sections = build_product_sections_manifest(
        &services.direct_snapshot,
        &banger_viewport.frame_hash,
        &web_atlas_ui.projection_hash,
    );
    let product_section_ui = product_section_projection(&product_sections, "trading");
    let packaging_security =
        build_packaging_security_manifest(&canonical_checkpoint.state_hash);
    let obsolete_front = build_obsolete_front_manifest();
    let cutover_audit = build_cutover_audit_report();

    let limitations = vec![
        "Stage 1 visual MVP is structurally native but not screenshot-parity approved; the old Tauri UI remains the design authority.".to_string(),
        "Direct Slint/wgpu texture sharing is gated by Slint's versioned unstable wgpu integration; Stage 4 displays the Banger frame through a Slint image fallback while preserving a texture-binding proof for direct import promotion.".to_string(),
        "WRY/WebView2 child creation, navigation policy and bounds proof are now encoded; manual focus/z-order proof is still required before promotion.".to_string(),
        "WebExplorer is isolated by default with dangerous schemes blocked, devtools/IPC/host objects off and external opens denied until atlas refs exist.".to_string(),
        "Banger now has a native viewport frame in Slint, but full direct external texture import is still blocked until Slint and the Banger renderer share compatible wgpu types.".to_string(),
        "Native visual parity is not done; the current Tauri front remains the product design reference until the user approves native parity.".to_string(),
        "Native state kernel is deterministic and replayable; product services now enter through the direct Rust service trait boundary.".to_string(),
        "Direct Rust services are connected to local wgpu/WebView2 capability probes; real Brain/Monster/Banger/WebExplorer/trading/real-estate product adapters are not fully wired yet.".to_string(),
        "Long-job streaming is proven with a background thread and non-blocking channel polling; production async runtimes still need Stage 4+ adapters.".to_string(),
        "Native chat and agent surfaces are now Slint/Rust projections; real provider streaming is still represented by direct service jobs until the full product adapters are connected.".to_string(),
        "Native product sections now have content-addressed Rust manifests and Slint projections; real domain adapters still need to replace fixture summaries before deletion of the old front.".to_string(),
        "Native packaging and security policy now records app-data/log/crash/secrets paths and blocks protected local roots; real installer signing/update automation remains a later release-engineering gate.".to_string(),
        "Stage 11 front cutover is guarded by obsolete-front and cutover-audit manifests; protected backend services under the old Tauri tree are tracked as a separate retirement wall.".to_string(),
        "Safe Web CodeAct remains explicitly deferred until the Slint/Rust frontend and obsolete-front deletion stages are complete.".to_string(),
    ];

    let manual_proofs_required = vec![
        "first visible frame is dark and stable".to_string(),
        "chat LineEdit receives focus without geometry jump".to_string(),
        "WRY/WebView2 fixture receives focus as a child peripheral".to_string(),
        "focus returns from WebView2 to Slint".to_string(),
        "resize keeps Slint, wgpu viewport region and WebView bounds coherent".to_string(),
        "closing the app leaves no stuck process".to_string(),
        "native shell visually matches the existing Tauri front first viewport".to_string(),
        "native transcript remains responsive on long sessions".to_string(),
        "provider/model picker and proof cards match the original visual density".to_string(),
        "trading, real-estate, forge, alpha, banger and webexplorer product panels match the original interaction density".to_string(),
        "fresh native package installs and restarts from the crash-recovery record".to_string(),
        "obsolete Tauri/WebView app-shell paths have either been deleted or are explicitly blocked by the Stage 11 manifest".to_string(),
        "stage11 front cutover audit returns ready after old app-shell paths are absent and the native shell manifest has no forbidden shell dependencies".to_string(),
        "full Tauri backend retirement remains blocked until protected backend services have been extracted or retired".to_string(),
    ];

    let promotion_ready =
        wgpu.available && webview.compile_time_capability && manual_proofs_required.is_empty();
    let mut report = Stage0Report {
        schema: "ingen.native_front.stage11_delete_global_webview_front.v1".to_string(),
        stage: "Migration Front Stage 11 - Delete Global WebView Front".to_string(),
        created_for: "Slint native shell + obsolete Tauri/WebView front deletion audit".to_string(),
        crate_root: "examples/ingen_native_front".to_string(),
        os: std::env::consts::OS.to_string(),
        design,
        state,
        services,
        banger_viewport,
        webexplorer,
        web_atlas,
        web_atlas_ui,
        product_sections,
        product_section_ui,
        packaging_security,
        obsolete_front,
        cutover_audit,
        slint,
        wgpu,
        webview,
        limitations,
        manual_proofs_required,
        promotion_ready,
        proof_hash: String::new(),
    };
    report.proof_hash = report_hash(&report);
    report
}

fn report_hash(report: &Stage0Report) -> String {
    let envelope = Stage0HashEnvelope {
        schema: &report.schema,
        stage: &report.stage,
        created_for: &report.created_for,
        crate_root: &report.crate_root,
        os: &report.os,
        design: &report.design,
        state: &report.state,
        services: &report.services,
        banger_viewport: &report.banger_viewport,
        webexplorer: &report.webexplorer,
        web_atlas: &report.web_atlas,
        web_atlas_ui: &report.web_atlas_ui,
        product_sections: &report.product_sections,
        product_section_ui: &report.product_section_ui,
        packaging_security: &report.packaging_security,
        obsolete_front: &report.obsolete_front,
        cutover_audit: &report.cutover_audit,
        slint: &report.slint,
        wgpu: &report.wgpu,
        webview: &report.webview,
        limitations: &report.limitations,
        manual_proofs_required: &report.manual_proofs_required,
        promotion_ready: report.promotion_ready,
    };
    let bytes = serde_json::to_vec(&envelope).expect("serialize stage0 hash envelope");
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webview_probe::WebViewProbe;
    use crate::wgpu_probe::WgpuProbe;

    #[test]
    fn proof_hash_is_stable_for_same_probe_inputs() {
        let wgpu = WgpuProbe::synthetic_available();
        let webview = WebViewProbe::synthetic_windows_capability();

        let first = build_stage0_report_from(wgpu.clone(), webview.clone());
        let second = build_stage0_report_from(wgpu, webview);

        assert_eq!(first.proof_hash, second.proof_hash);
        assert_eq!(first.crate_root, "examples/ingen_native_front");
        assert!(!first.promotion_ready);
    }

    #[test]
    fn stage0_report_records_native_front_boundaries() {
        let report = build_stage0_report_from(
            WgpuProbe::synthetic_available(),
            WebViewProbe::synthetic_windows_capability(),
        );

        assert_eq!(
            report.schema,
            "ingen.native_front.stage11_delete_global_webview_front.v1"
        );
        assert!(report.stage.contains("Stage 11"));
        assert!(!report.slint.app_shell_uses_tauri);
        assert!(!report.slint.app_shell_uses_html_css_js);
        assert!(!report.state.browser_ipc_required);
        assert!(report.state.direct_mutation_blocked);
        assert!(!report.services.browser_ipc_required);
        assert!(report.services.direct_probe_services_connected);
        assert!(!report.services.real_services_connected);
        assert_eq!(
            report.services.direct_snapshot.provider.provider,
            "direct-rust"
        );
        assert_eq!(
            report.services.direct_refresh_command.message,
            "direct Rust service snapshot refreshed without browser IPC"
        );
        assert!(!report.services.direct_refresh_command.proof_hash.is_empty());
        assert_eq!(
            report.banger_viewport.schema,
            "ingen.native_front.stage4_banger_viewport.v1"
        );
        assert!(report.banger_viewport.visible_in_slint);
        assert!(!report.banger_viewport.browser_canvas_required);
        assert_eq!(report.banger_viewport.width, BANGER_VIEWPORT_WIDTH);
        assert_eq!(report.banger_viewport.height, BANGER_VIEWPORT_HEIGHT);
        assert!(report
            .banger_viewport
            .texture_usage
            .contains(&"TEXTURE_BINDING".to_string()));
        assert!(report.banger_viewport.frame_hash.len() == 64);
        assert!(report
            .banger_viewport
            .slint_texture_bridge
            .texture_binding_ready);
        assert!(!report
            .banger_viewport
            .slint_texture_bridge
            .direct_texture_import_ready);
        assert_eq!(
            report.webexplorer.schema,
            "ingen.webexplorer.isolation_proof.v1"
        );
        assert!(report.webexplorer.allowed_navigation.allowed);
        assert!(!report.webexplorer.blocked_navigation.allowed);
        assert!(report
            .webexplorer
            .blocked_navigation
            .reason
            .contains("blocked dangerous scheme"));
        assert_eq!(report.webexplorer.proof_hash.len(), 64);
        assert_eq!(report.web_atlas.schema, "ingen.webatlas.manifest.v1");
        assert!(report.web_atlas.node_count >= 8);
        assert!(report.web_atlas.coverage.layout_ratio == 100);
        assert!(report
            .web_atlas
            .nodes
            .iter()
            .any(|node| node.role == "textbox"));
        assert!(report
            .web_atlas
            .resources
            .iter()
            .any(|resource| resource.kind == "policy"));
        assert_eq!(report.web_atlas_ui.selected_index, 12);
        assert!(report.web_atlas_ui.tree_lines.contains("<input>"));
        assert!(report.web_atlas_ui.action_candidates.contains("button"));
        assert!(report.web_atlas_ui.blind_spot_lines.contains("runtime JavaScript"));
        assert_eq!(report.web_atlas_ui.projection_hash.len(), 64);
        assert_eq!(
            report.services.fake_long_job_statuses,
            vec![
                "queued".to_string(),
                "running".to_string(),
                "done".to_string()
            ]
        );
        assert_eq!(
            report.services.trait_source,
            "examples/ingen_native_front/src/services.rs"
        );
        assert!(report
            .state
            .keyboard_shortcuts
            .contains(&"Control+Tab -> NavigateNext".to_string()));
        assert_eq!(
            report.state.kernel_source,
            "examples/ingen_native_front/src/state.rs"
        );
        assert_eq!(
            report.state.checkpoint_schema,
            "ingen.native_front.state_checkpoint.v1"
        );
        assert!(!report.state.canonical_event_log_hash.is_empty());
        assert!(!report.state.canonical_state_hash.is_empty());
        assert_eq!(
            report.design.inventory,
            "examples/ingen_native_front/design/current_tauri_front_inventory.md"
        );
        assert_eq!(report.design.locked_dimensions[1], "left_panel=279px");
        assert!(report
            .design
            .native_component_targets
            .contains(&"ChatBar".to_string()));
        assert!(report
            .design
            .native_component_targets
            .contains(&"AgentSurface".to_string()));
        assert!(report
            .design
            .native_component_targets
            .contains(&"ProductSectionSurface".to_string()));
        assert!(report.state.canonical_replay_hash.len() == 64);
        assert_eq!(
            report.product_sections.schema,
            "ingen.native_front.stage9_product_sections.v1"
        );
        assert_eq!(report.product_sections.sections.len(), 7);
        assert!(report
            .product_sections
            .sections
            .iter()
            .any(|section| section.section_id == "real-estate"));
        assert!(report
            .product_sections
            .sections
            .iter()
            .filter(|section| section.section_id != "webexplorer")
            .all(|section| !section.webview_required));
        assert_eq!(report.product_section_ui.active_section, "trading");
        assert!(report.product_section_ui.metric_lines.contains("timeframes"));
        assert_eq!(report.product_section_ui.projection_hash.len(), 64);
        assert_eq!(
            report.packaging_security.schema,
            "ingen.native_front.stage10_packaging_security.v1"
        );
        assert!(!report.packaging_security.tauri_shell_required);
        assert!(report.packaging_security.slint_desktop_supported);
        assert!(report
            .packaging_security
            .app_paths
            .crash_recovery_dir
            .contains("crash-recovery"));
        assert!(!report.packaging_security.protected_path_decision.allowed);
        assert!(report.packaging_security.app_data_path_decision.allowed);
        assert!(report
            .packaging_security
            .capability_policy
            .webview_profile_isolated);
        assert!(!report
            .packaging_security
            .capability_policy
            .secrets_in_logs_allowed);
        assert_eq!(report.packaging_security.crash_recovery.proof_hash.len(), 64);
        assert_eq!(
            report.obsolete_front.schema,
            "ingen.native_front.stage11_obsolete_front_manifest.v1"
        );
        assert!(report
            .obsolete_front
            .obsolete_paths
            .iter()
            .any(|path| path.path == "examples/forge_tauri_ui/ui/src"));
        assert!(report
            .obsolete_front
            .anti_regression_rules
            .iter()
            .any(|rule| rule.contains("normal startup")));
        assert_eq!(report.obsolete_front.proof_hash.len(), 64);
        assert_eq!(
            report.cutover_audit.schema,
            "ingen.native_front.stage11_cutover_audit.v1"
        );
        assert!(!report.cutover_audit.rollback_required);
        assert!(report.cutover_audit.cutover_ready);
        assert!(report.cutover_audit.backend_extraction_required);
        assert!(report.cutover_audit.tauri_backend_retirement_required);
        assert!(!report.cutover_audit.full_tauri_retirement_ready);
        assert!(report.cutover_audit.proof_hash.len() == 64);
        assert!(report.design.forge_first_viewport.passed);
        assert_eq!(
            report.design.forge_first_viewport.canvas_title,
            "Drop any file"
        );
        assert!(!report.design.parity_ready);
        assert!(report
            .manual_proofs_required
            .iter()
            .any(|item| item.contains("focus returns")));
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Direct Slint/wgpu texture sharing")));
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Safe Web CodeAct remains explicitly deferred")));
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Native packaging and security policy")));
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Stage 11 front cutover is guarded")));
    }
}

fn fake_long_job_statuses() -> Vec<String> {
    let receiver = spawn_fake_long_job("stage3-report-fixture".to_string());
    let mut statuses = Vec::new();
    for _ in 0..3 {
        if let Ok(event) = receiver.recv_timeout(std::time::Duration::from_secs(1)) {
            statuses.push(format!("{:?}", event.job.status).to_lowercase());
        }
    }
    statuses
}
