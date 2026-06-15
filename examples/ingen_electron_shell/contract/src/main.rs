use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const IPC_VERSION: u32 = 1;

const BRAIN_STR_CONSTS: &[&str] = &[
    "BRAIN_SEARCHARCHIVE_COMMAND",
    "BRAIN_SEARCHARCHIVE_RESULT_SCHEMA",
    "BRAIN_RENAME_SESSION_COMMAND",
    "BRAIN_RENAME_SESSION_RESULT_SCHEMA",
    "BRAIN_WEBSEARCH_COMMAND",
    "BRAIN_WEBSEARCH_RESULT_SCHEMA",
    "BRAIN_CODEDOCS_COMMAND",
    "BRAIN_CODEDOCS_RESULT_SCHEMA",
    "BRAIN_GITHUB_MCP_COMMAND",
    "BRAIN_GITHUB_MCP_RESULT_SCHEMA",
    "BRAIN_WEBACT_COMMAND",
    "BRAIN_WEBACT_RESULT_SCHEMA",
    "BRAIN_SECURITYSCAN_COMMAND",
    "BRAIN_SECURITYSCAN_RESULT_SCHEMA",
    "BRAIN_GOOGLEWEB_COMMAND",
    "BRAIN_GOOGLEWEB_RESULT_SCHEMA",
    "BRAIN_SCRAPERS_COMMAND",
    "BRAIN_SCRAPERS_RESULT_SCHEMA",
    "BRAIN_MAPS_COMMAND",
    "BRAIN_MAPS_RESULT_SCHEMA",
    "BRAIN_GMAIL_COMMAND",
    "BRAIN_GMAIL_COM_COMMAND",
    "BRAIN_GMAIL_RESULT_SCHEMA",
    "BRAIN_AIRBNB_COMMAND",
    "BRAIN_AIRBNB_RESULT_SCHEMA",
    "BRAIN_NEWIMAGE_COMMAND",
    "BRAIN_EDITIMAGE_COMMAND",
    "BRAIN_IMAGE_RESULT_SCHEMA",
    "BRAIN_QUESTIONNAIRE_COMMAND",
    "BRAIN_QUESTIONNAIRE_RESULT_SCHEMA",
    "BRAIN_SCIENCE_COMMAND",
    "BRAIN_CODING_COMMAND",
    "BRAIN_SEGMENT_RESULT_SCHEMA",
    "BRAIN_NEWBRAIN_COMMAND",
    "BRAIN_MODIFY_NAMED_BRAIN_COMMAND",
    "BRAIN_DOMAIN_BRAIN_RESULT_SCHEMA",
    "BRAIN_CODEACT_ROUTING_RULES",
    "BRAIN_WORKSPACE_COMMAND",
    "BRAIN_CAPABILITIES_COMMAND",
    "BRAIN_CODING_LIVE_PREVIEW_COMMAND",
    "BRAIN_NEWCOMPUTE_COMMAND",
    "BRAIN_SELECTCOMPUTE_COMMAND",
    "BRAIN_NAMED_COMPUTE_COMMAND",
    "BRAIN_NEWOBJECT_COMMAND",
    "BRAIN_WEB_COMMAND",
    "BRAIN_FRONTDESIGN_COMMAND",
    "BRAIN_GOOGLE_AGENDA_COMMAND",
    "BRAIN_BRAIN_COMMAND",
    "BRAIN_NEWMODULE_COMMAND",
    "BRAIN_RUST_PORT_ADAPTER_COMMAND",
    "BRAIN_RUST_STATE_STORE_COMMAND",
    "BRAIN_SEARCHARCHIVE_COMMAND_DESCRIPTION",
    "BRAIN_RENAME_SESSION_COMMAND_DESCRIPTION",
    "BRAIN_WEBSEARCH_COMMAND_DESCRIPTION",
    "BRAIN_CODEDOCS_COMMAND_DESCRIPTION",
    "BRAIN_GITHUB_MCP_COMMAND_DESCRIPTION",
    "BRAIN_WEBACT_COMMAND_DESCRIPTION",
    "BRAIN_SECURITYSCAN_COMMAND_DESCRIPTION",
    "BRAIN_GOOGLEWEB_COMMAND_DESCRIPTION",
    "BRAIN_SCRAPERS_COMMAND_DESCRIPTION",
    "BRAIN_MAPS_COMMAND_DESCRIPTION",
    "BRAIN_GMAIL_COMMAND_DESCRIPTION",
    "BRAIN_GMAIL_COM_COMMAND_DESCRIPTION",
    "BRAIN_AIRBNB_COMMAND_DESCRIPTION",
    "BRAIN_NEWIMAGE_COMMAND_DESCRIPTION",
    "BRAIN_EDITIMAGE_COMMAND_DESCRIPTION",
    "BRAIN_QUESTIONNAIRE_COMMAND_DESCRIPTION",
    "BRAIN_SCIENCE_COMMAND_DESCRIPTION",
    "BRAIN_CODING_COMMAND_DESCRIPTION",
    "BRAIN_NEWBRAIN_COMMAND_DESCRIPTION",
    "BRAIN_MODIFY_NAMED_BRAIN_COMMAND_DESCRIPTION",
    "BRAIN_WORKSPACE_COMMAND_DESCRIPTION",
    "BRAIN_CAPABILITIES_COMMAND_DESCRIPTION",
    "BRAIN_CODING_LIVE_PREVIEW_COMMAND_DESCRIPTION",
    "BRAIN_NEWCOMPUTE_COMMAND_DESCRIPTION",
    "BRAIN_SELECTCOMPUTE_COMMAND_DESCRIPTION",
    "BRAIN_NAMED_COMPUTE_COMMAND_DESCRIPTION",
    "BRAIN_NEWOBJECT_COMMAND_DESCRIPTION",
    "BRAIN_WEB_COMMAND_DESCRIPTION",
    "BRAIN_FRONTDESIGN_COMMAND_DESCRIPTION",
    "BRAIN_GOOGLE_AGENDA_COMMAND_DESCRIPTION",
    "BRAIN_BRAIN_COMMAND_DESCRIPTION",
    "BRAIN_NEWMODULE_COMMAND_DESCRIPTION",
    "BRAIN_RUST_PORT_ADAPTER_COMMAND_DESCRIPTION",
    "BRAIN_RUST_STATE_STORE_COMMAND_DESCRIPTION",
    "BRAIN_SCIENCE_VISIBLE_CATALOG",
    "BRAIN_CODING_VISIBLE_CATALOG",
];

const BRAIN_COMMAND_DESCRIPTION_PAIRS: &[(&str, &str)] = &[
    ("BRAIN_SEARCHARCHIVE_COMMAND", "BRAIN_SEARCHARCHIVE_COMMAND_DESCRIPTION"),
    ("BRAIN_RENAME_SESSION_COMMAND", "BRAIN_RENAME_SESSION_COMMAND_DESCRIPTION"),
    ("BRAIN_WEBSEARCH_COMMAND", "BRAIN_WEBSEARCH_COMMAND_DESCRIPTION"),
    ("BRAIN_CODEDOCS_COMMAND", "BRAIN_CODEDOCS_COMMAND_DESCRIPTION"),
    ("BRAIN_GITHUB_MCP_COMMAND", "BRAIN_GITHUB_MCP_COMMAND_DESCRIPTION"),
    ("BRAIN_WEBACT_COMMAND", "BRAIN_WEBACT_COMMAND_DESCRIPTION"),
    ("BRAIN_SECURITYSCAN_COMMAND", "BRAIN_SECURITYSCAN_COMMAND_DESCRIPTION"),
    ("BRAIN_GOOGLEWEB_COMMAND", "BRAIN_GOOGLEWEB_COMMAND_DESCRIPTION"),
    ("BRAIN_SCRAPERS_COMMAND", "BRAIN_SCRAPERS_COMMAND_DESCRIPTION"),
    ("BRAIN_MAPS_COMMAND", "BRAIN_MAPS_COMMAND_DESCRIPTION"),
    ("BRAIN_GMAIL_COMMAND", "BRAIN_GMAIL_COMMAND_DESCRIPTION"),
    ("BRAIN_GMAIL_COM_COMMAND", "BRAIN_GMAIL_COM_COMMAND_DESCRIPTION"),
    ("BRAIN_AIRBNB_COMMAND", "BRAIN_AIRBNB_COMMAND_DESCRIPTION"),
    ("BRAIN_NEWIMAGE_COMMAND", "BRAIN_NEWIMAGE_COMMAND_DESCRIPTION"),
    ("BRAIN_EDITIMAGE_COMMAND", "BRAIN_EDITIMAGE_COMMAND_DESCRIPTION"),
    ("BRAIN_QUESTIONNAIRE_COMMAND", "BRAIN_QUESTIONNAIRE_COMMAND_DESCRIPTION"),
    ("BRAIN_SCIENCE_COMMAND", "BRAIN_SCIENCE_COMMAND_DESCRIPTION"),
    ("BRAIN_CODING_COMMAND", "BRAIN_CODING_COMMAND_DESCRIPTION"),
    ("BRAIN_NEWBRAIN_COMMAND", "BRAIN_NEWBRAIN_COMMAND_DESCRIPTION"),
    (
        "BRAIN_MODIFY_NAMED_BRAIN_COMMAND",
        "BRAIN_MODIFY_NAMED_BRAIN_COMMAND_DESCRIPTION",
    ),
    ("BRAIN_WORKSPACE_COMMAND", "BRAIN_WORKSPACE_COMMAND_DESCRIPTION"),
    ("BRAIN_CAPABILITIES_COMMAND", "BRAIN_CAPABILITIES_COMMAND_DESCRIPTION"),
    ("BRAIN_CODING_LIVE_PREVIEW_COMMAND", "BRAIN_CODING_LIVE_PREVIEW_COMMAND_DESCRIPTION"),
    ("BRAIN_NEWCOMPUTE_COMMAND", "BRAIN_NEWCOMPUTE_COMMAND_DESCRIPTION"),
    ("BRAIN_SELECTCOMPUTE_COMMAND", "BRAIN_SELECTCOMPUTE_COMMAND_DESCRIPTION"),
    ("BRAIN_NAMED_COMPUTE_COMMAND", "BRAIN_NAMED_COMPUTE_COMMAND_DESCRIPTION"),
    ("BRAIN_NEWOBJECT_COMMAND", "BRAIN_NEWOBJECT_COMMAND_DESCRIPTION"),
    ("BRAIN_WEB_COMMAND", "BRAIN_WEB_COMMAND_DESCRIPTION"),
    ("BRAIN_FRONTDESIGN_COMMAND", "BRAIN_FRONTDESIGN_COMMAND_DESCRIPTION"),
    ("BRAIN_GOOGLE_AGENDA_COMMAND", "BRAIN_GOOGLE_AGENDA_COMMAND_DESCRIPTION"),
    ("BRAIN_BRAIN_COMMAND", "BRAIN_BRAIN_COMMAND_DESCRIPTION"),
    ("BRAIN_NEWMODULE_COMMAND", "BRAIN_NEWMODULE_COMMAND_DESCRIPTION"),
    ("BRAIN_RUST_PORT_ADAPTER_COMMAND", "BRAIN_RUST_PORT_ADAPTER_COMMAND_DESCRIPTION"),
    ("BRAIN_RUST_STATE_STORE_COMMAND", "BRAIN_RUST_STATE_STORE_COMMAND_DESCRIPTION"),
];

#[derive(Clone, Copy)]
struct EnumSpec {
    name: &'static str,
    variants: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct FieldSpec {
    name: &'static str,
    ts_type: &'static str,
    optional: bool,
}

#[derive(Clone, Copy)]
struct InterfaceSpec {
    name: &'static str,
    fields: &'static [FieldSpec],
}

const FRONT_SLICE_MODE: EnumSpec = EnumSpec {
    name: "FrontSliceMode",
    variants: &["electron", "shadow"],
};

const NATIVE_SECTION: EnumSpec = EnumSpec {
    name: "NativeSection",
    variants: &[
        "forge",
        "webexplorer",
        "banger",
        "trading",
        "real-estate",
        "alpha",
        "shell",
    ],
};

const HEADER_COMMAND_KIND: EnumSpec = EnumSpec {
    name: "HeaderCommandKind",
    variants: &[
        "toggle_left_panel",
        "open_sessions_canvas",
        "open_webexplorer",
        "open_banger",
        "open_trading",
        "navigate_workspace",
        "toggle_right_panel",
        "window_minimize",
        "window_toggle_maximize",
        "window_close",
    ],
};

const SIDEBAR_COMMAND_KIND: EnumSpec = EnumSpec {
    name: "SidebarCommandKind",
    variants: &[
        "navigate",
        "open_session",
        "rename_session",
        "open_profile_canvas",
        "archive_session",
        "activate_control",
        "switch_sessions_mode",
        "toggle_profile_menu",
        "set_active_drawer",
        "hide_tool",
        "restore_tool",
        "pin_session",
        "confirm_archive",
        "cancel_archive",
    ],
};

const IPC_ERROR_CODE: EnumSpec = EnumSpec {
    name: "IpcErrorCode",
    variants: &[
        "bad_version",
        "bad_sender",
        "bad_payload",
        "rust_unavailable",
        "shadow_only",
        "cancelled",
    ],
};

const PROFILE_CANVAS: EnumSpec = EnumSpec {
    name: "ProfileCanvas",
    variants: &["", "sessions", "brain", "profile", "llm"],
};

const SESSIONS_MENU_MODE: EnumSpec = EnumSpec {
    name: "SessionsMenuMode",
    variants: &["recents", "archived"],
};

const HEADER_RESULT_EVENT: EnumSpec = EnumSpec {
    name: "HeaderCommandResultEvent",
    variants: &[
        "shadow_manifest_recorded",
        "electron_command_applied",
        "rejected",
    ],
};

const SIDEBAR_RESULT_EVENT: EnumSpec = EnumSpec {
    name: "SidebarCommandResultEvent",
    variants: &[
        "shadow_manifest_recorded",
        "electron_command_applied",
        "rejected",
    ],
};

const PANELS_CHAT_BOTTOM_COMMAND_KIND: EnumSpec = EnumSpec {
    name: "PanelsChatBottomCommandKind",
    variants: &[
        "refresh_probes",
        "activate_control",
        "attach_files",
        "attach_dropped_files",
        "chat_text_edited",
        "send_chat",
        "send_parallel_chat_batch",
        "stop_assistant",
        "assistant_write_complete",
        "update_brain_identity",
        "new_session",
        "select_llm",
        "open_llm_providers",
        "cycle_llm_model",
        "cycle_llm_reasoning",
        "permission_mode_selected",
        "stage_attachment_for_edit",
        "upload_preview_scroll",
    ],
};

const CANVAS_SURFACES_COMMAND_KIND: EnumSpec = EnumSpec {
    name: "CanvasSurfacesCommandKind",
    variants: &[
        "activate_control",
        "open_profile_canvas",
        "request_native_surface",
        "refresh_surface_proofs",
    ],
};

const RIGHT_PANEL_COMMAND_KIND: EnumSpec = EnumSpec {
    name: "RightPanelCommandKind",
    variants: &[
        "refresh",
        "activate_control",
        "toggle_panel",
        "select_tab",
    ],
};

const PANELS_CHAT_BOTTOM_RESULT_EVENT: EnumSpec = EnumSpec {
    name: "PanelsChatBottomCommandResultEvent",
    variants: &[
        "shadow_manifest_recorded",
        "electron_command_applied",
        "rejected",
    ],
};

const CANVAS_SURFACES_RESULT_EVENT: EnumSpec = EnumSpec {
    name: "CanvasSurfacesCommandResultEvent",
    variants: &[
        "shadow_manifest_recorded",
        "electron_command_applied",
        "rejected",
    ],
};

const RIGHT_PANEL_RESULT_EVENT: EnumSpec = EnumSpec {
    name: "RightPanelCommandResultEvent",
    variants: &[
        "shadow_manifest_recorded",
        "electron_command_applied",
        "rejected",
    ],
};

const NATIVE_AUTHORITY: EnumSpec = EnumSpec {
    name: "NativeAuthority",
    variants: &["rust", "window", "electron-shadow"],
};

const HEADER_SURFACE_KIND: EnumSpec = EnumSpec {
    name: "HeaderSurfaceKind",
    variants: &[
        "drop_canvas",
        "webexplorer_webview",
        "webexplorer_atlas",
        "banger_native_child",
        "product_section",
        "delegated",
    ],
};

const HEADER_SURFACE_STATUS: EnumSpec = EnumSpec {
    name: "HeaderSurfaceStatus",
    variants: &["shadow", "native_pending", "native_ready", "delegated_to_parallel_slice"],
};

const CANVAS_SURFACE_KIND: EnumSpec = EnumSpec {
    name: "CanvasSurfaceKind",
    variants: &[
        "drop_canvas",
        "webexplorer_webview",
        "banger_native_child",
        "profile_surface",
        "llm_providers",
        "brain_surface",
        "product_section",
    ],
};

const CANVAS_SURFACE_STATUS: EnumSpec = EnumSpec {
    name: "CanvasSurfaceStatus",
    variants: &["shadow", "native_pending", "native_ready", "delegated", "ipc_ready"],
};

const IPC_ERROR: InterfaceSpec = InterfaceSpec {
    name: "IpcError",
    fields: &[
        FieldSpec { name: "code", ts_type: "IpcErrorCode", optional: false },
        FieldSpec { name: "message", ts_type: "string", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const HEADER_COMMAND_BASE: InterfaceSpec = InterfaceSpec {
    name: "HeaderCommandBase",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "cancelToken", ts_type: "string", optional: true },
    ],
};

const HEADER_COMMAND_RESULT: InterfaceSpec = InterfaceSpec {
    name: "HeaderCommandResult",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "accepted", ts_type: "boolean", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "event", ts_type: "HeaderCommandResultEvent", optional: false },
        FieldSpec { name: "error", ts_type: "IpcError", optional: true },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const SIDEBAR_COMMAND_BASE: InterfaceSpec = InterfaceSpec {
    name: "SidebarCommandBase",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "cancelToken", ts_type: "string", optional: true },
    ],
};

const SIDEBAR_COMMAND_RESULT: InterfaceSpec = InterfaceSpec {
    name: "SidebarCommandResult",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "accepted", ts_type: "boolean", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "event", ts_type: "SidebarCommandResultEvent", optional: false },
        FieldSpec { name: "error", ts_type: "IpcError", optional: true },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const HEADER_CONTROL: InterfaceSpec = InterfaceSpec {
    name: "HeaderControl",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "icon", ts_type: "string", optional: false },
        FieldSpec { name: "command", ts_type: "HeaderCommandKind", optional: false },
        FieldSpec { name: "route", ts_type: "NativeSection | \"sessions\" | \"right-panel\"", optional: true },
        FieldSpec { name: "selected", ts_type: "boolean", optional: false },
        FieldSpec { name: "visible", ts_type: "boolean", optional: false },
        FieldSpec { name: "nativeAuthority", ts_type: "NativeAuthority", optional: false },
    ],
};

const SIDEBAR_SESSION_ITEM: InterfaceSpec = InterfaceSpec {
    name: "SidebarSessionItem",
    fields: &[
        FieldSpec { name: "sessionId", ts_type: "string", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "date", ts_type: "string", optional: false },
        FieldSpec { name: "section", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "workspaceLabel", ts_type: "string", optional: true },
        FieldSpec { name: "rowVisible", ts_type: "boolean", optional: false },
        FieldSpec { name: "pinned", ts_type: "boolean", optional: false },
        FieldSpec { name: "working", ts_type: "boolean", optional: false },
        FieldSpec { name: "automated", ts_type: "boolean", optional: false },
        FieldSpec { name: "archived", ts_type: "boolean", optional: false },
        FieldSpec { name: "parallelGroupId", ts_type: "string", optional: true },
        FieldSpec { name: "parallelLaneIndex", ts_type: "number", optional: true },
        FieldSpec { name: "parallelLaneCount", ts_type: "number", optional: true },
        FieldSpec { name: "parallelPeerSessionIds", ts_type: "string[]", optional: true },
    ],
};

const SIDEBAR_TOOL_CONTROL: InterfaceSpec = InterfaceSpec {
    name: "SidebarToolControl",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "icon", ts_type: "string", optional: false },
        FieldSpec { name: "drawer", ts_type: "string", optional: false },
        FieldSpec { name: "visible", ts_type: "boolean", optional: false },
        FieldSpec { name: "hidden", ts_type: "boolean", optional: false },
        FieldSpec { name: "selected", ts_type: "boolean", optional: false },
        FieldSpec { name: "nativeAuthority", ts_type: "NativeAuthority", optional: false },
    ],
};

const PROFILE_MENU_ITEM: InterfaceSpec = InterfaceSpec {
    name: "ProfileMenuItem",
    fields: &[
        FieldSpec { name: "id", ts_type: "ProfileCanvas", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "detail", ts_type: "string", optional: false },
        FieldSpec { name: "iconLabel", ts_type: "string", optional: false },
    ],
};

const ARCHIVE_CONFIRM_STATE: InterfaceSpec = InterfaceSpec {
    name: "ArchiveConfirmState",
    fields: &[
        FieldSpec { name: "open", ts_type: "boolean", optional: false },
        FieldSpec { name: "candidateId", ts_type: "string", optional: false },
        FieldSpec { name: "candidateLabel", ts_type: "string", optional: false },
        FieldSpec { name: "candidateDate", ts_type: "string", optional: false },
        FieldSpec { name: "candidateSection", ts_type: "NativeSection", optional: false },
    ],
};

const HEADER_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "HeaderSnapshot",
    fields: &[
        FieldSpec { name: "schema", ts_type: "\"ingen.electron.header.snapshot.v1\"", optional: false },
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "activeSection", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "sectionTitle", ts_type: "string", optional: false },
        FieldSpec { name: "profileCanvas", ts_type: "\"\" | \"sessions\" | \"brain\" | \"profile\" | \"llm\"", optional: false },
        FieldSpec { name: "leftPanelOpen", ts_type: "boolean", optional: false },
        FieldSpec { name: "rightPanelOpen", ts_type: "boolean", optional: false },
        FieldSpec { name: "macChrome", ts_type: "boolean", optional: false },
        FieldSpec { name: "cpuLabel", ts_type: "string", optional: false },
        FieldSpec { name: "gpuLabel", ts_type: "string", optional: false },
        FieldSpec { name: "topControls", ts_type: "HeaderControl[]", optional: false },
        FieldSpec { name: "workspaceControls", ts_type: "HeaderControl[]", optional: false },
        FieldSpec { name: "nativeSurfaceContracts", ts_type: "{ banger: \"native-child-surface\"; webexplorer: \"rust-owned-webview\" }", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const SIDEBAR_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "SidebarSnapshot",
    fields: &[
        FieldSpec { name: "schema", ts_type: "\"ingen.electron.sidebar.snapshot.v1\"", optional: false },
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "activeSection", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "profileCanvas", ts_type: "ProfileCanvas", optional: false },
        FieldSpec { name: "activeDrawer", ts_type: "string", optional: false },
        FieldSpec { name: "profileOpen", ts_type: "boolean", optional: false },
        FieldSpec { name: "sessionsMenuMode", ts_type: "SessionsMenuMode", optional: false },
        FieldSpec { name: "recentSessionId", ts_type: "string", optional: false },
        FieldSpec { name: "hasArchivedSession", ts_type: "boolean", optional: false },
        FieldSpec { name: "recentItems", ts_type: "SidebarSessionItem[]", optional: false },
        FieldSpec { name: "archivedItems", ts_type: "SidebarSessionItem[]", optional: false },
        FieldSpec { name: "toolControls", ts_type: "SidebarToolControl[]", optional: false },
        FieldSpec { name: "profileMenuItems", ts_type: "ProfileMenuItem[]", optional: false },
        FieldSpec { name: "archiveConfirm", ts_type: "ArchiveConfirmState", optional: false },
        FieldSpec { name: "profileCanvasSummary", ts_type: "string", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const HEADER_SURFACE_SLOT: InterfaceSpec = InterfaceSpec {
    name: "HeaderSurfaceSlot",
    fields: &[
        FieldSpec { name: "x", ts_type: "number", optional: false },
        FieldSpec { name: "y", ts_type: "number", optional: false },
        FieldSpec { name: "width", ts_type: "number", optional: false },
        FieldSpec { name: "height", ts_type: "number", optional: false },
    ],
};

const HEADER_SURFACE_CONTRACT: InterfaceSpec = InterfaceSpec {
    name: "HeaderSurfaceContract",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "kind", ts_type: "HeaderSurfaceKind", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "route", ts_type: "NativeSection | \"sessions\" | \"right-panel\"", optional: false },
        FieldSpec { name: "authority", ts_type: "NativeAuthority", optional: false },
        FieldSpec { name: "status", ts_type: "HeaderSurfaceStatus", optional: false },
        FieldSpec { name: "slot", ts_type: "HeaderSurfaceSlot", optional: false },
        FieldSpec { name: "nativeContract", ts_type: "string", optional: false },
        FieldSpec { name: "sourceComponent", ts_type: "string", optional: false },
        FieldSpec { name: "summary", ts_type: "string", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const HEADER_SURFACE_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "HeaderSurfaceSnapshot",
    fields: &[
        FieldSpec { name: "schema", ts_type: "\"ingen.electron.header.surface_snapshot.v1\"", optional: false },
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "activeSection", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "profileCanvas", ts_type: "\"\" | \"sessions\" | \"brain\" | \"profile\" | \"llm\"", optional: false },
        FieldSpec { name: "surfaces", ts_type: "HeaderSurfaceContract[]", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const STATUS_DOCK_LINE: InterfaceSpec = InterfaceSpec {
    name: "StatusDockLine",
    fields: &[
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "value", ts_type: "string", optional: false },
        FieldSpec { name: "source", ts_type: "\"NativeStateKernel::projection\" | \"NativeServiceSnapshot\"", optional: false },
    ],
};

const STATUS_DOCK_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "StatusDockSnapshot",
    fields: &[
        FieldSpec { name: "visible", ts_type: "boolean", optional: false },
        FieldSpec { name: "title", ts_type: "string", optional: false },
        FieldSpec { name: "lines", ts_type: "StatusDockLine[]", optional: false },
        FieldSpec { name: "primaryAction", ts_type: "string", optional: false },
    ],
};

const TRANSCRIPT_MESSAGE: InterfaceSpec = InterfaceSpec {
    name: "TranscriptMessage",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "role", ts_type: "\"user\" | \"assistant\" | \"system\"", optional: false },
        FieldSpec { name: "text", ts_type: "string", optional: false },
        FieldSpec { name: "attachments", ts_type: "ComposerUploadPreview[]", optional: true },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const PARALLEL_CHAT_LANE: InterfaceSpec = InterfaceSpec {
    name: "ParallelChatLane",
    fields: &[
        FieldSpec { name: "index", ts_type: "number", optional: false },
        FieldSpec { name: "sessionId", ts_type: "string", optional: false },
        FieldSpec { name: "transcript", ts_type: "TranscriptMessage[]", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const LLM_PROVIDER_STATE: InterfaceSpec = InterfaceSpec {
    name: "LlmProviderState",
    fields: &[
        FieldSpec { name: "provider", ts_type: "\"openai\" | \"anthropic\" | \"openrouter\"", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "connected", ts_type: "boolean", optional: false },
        FieldSpec { name: "active", ts_type: "boolean", optional: false },
        FieldSpec { name: "account", ts_type: "string", optional: false },
        FieldSpec { name: "proof", ts_type: "string", optional: false },
    ],
};

const COMPOSER_UPLOAD_PREVIEW: InterfaceSpec = InterfaceSpec {
    name: "ComposerUploadPreview",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "name", ts_type: "string", optional: false },
        FieldSpec { name: "kind", ts_type: "\"image\" | \"video\" | \"model3d\" | \"pdf\" | \"spreadsheet\" | \"text\" | \"chart\" | \"file\"", optional: false },
        FieldSpec { name: "url", ts_type: "string", optional: false },
        FieldSpec { name: "textPreview", ts_type: "string", optional: false },
        FieldSpec { name: "tablePreview", ts_type: "string[][]", optional: false },
    ],
};

const PARALLEL_CHAT_DRAFT: InterfaceSpec = InterfaceSpec {
    name: "ParallelChatDraft",
    fields: &[
        FieldSpec { name: "parallelSessionIndex", ts_type: "number", optional: false },
        FieldSpec { name: "value", ts_type: "string", optional: false },
    ],
};

const COMPOSER_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "ComposerSnapshot",
    fields: &[
        FieldSpec { name: "chatText", ts_type: "string", optional: false },
        FieldSpec { name: "splitPrompts", ts_type: "boolean", optional: false },
        FieldSpec { name: "permissionMode", ts_type: "\"ask-permissions\" | \"auto-accept-edits\" | \"full-autonomy\" | \"self-directed\"", optional: false },
        FieldSpec { name: "permissionModeOpen", ts_type: "boolean", optional: false },
        FieldSpec { name: "selectedProvider", ts_type: "\"openai\" | \"anthropic\" | \"openrouter\"", optional: false },
        FieldSpec { name: "assistantBusy", ts_type: "boolean", optional: true },
        FieldSpec { name: "providers", ts_type: "LlmProviderState[]", optional: false },
        FieldSpec { name: "modelLabel", ts_type: "string", optional: false },
        FieldSpec { name: "reasoningLabel", ts_type: "string", optional: false },
        FieldSpec { name: "uploadStatus", ts_type: "string", optional: false },
        FieldSpec { name: "uploadPreviewLabel", ts_type: "string", optional: false },
        FieldSpec { name: "uploadPreviewKind", ts_type: "string", optional: false },
        FieldSpec { name: "uploadCount", ts_type: "number", optional: false },
        FieldSpec { name: "uploadErrorText", ts_type: "string", optional: false },
        FieldSpec { name: "uploadPreviews", ts_type: "ComposerUploadPreview[]", optional: false },
    ],
};

const BOTTOM_CONTROL: InterfaceSpec = InterfaceSpec {
    name: "BottomControl",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "kind", ts_type: "PanelsChatBottomCommandKind", optional: false },
        FieldSpec { name: "enabled", ts_type: "boolean", optional: false },
        FieldSpec { name: "nativeAuthority", ts_type: "NativeAuthority", optional: false },
    ],
};

const CANVAS_SURFACE_SUMMARY: InterfaceSpec = InterfaceSpec {
    name: "CanvasSurfaceSummary",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "kind", ts_type: "CanvasSurfaceKind", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "route", ts_type: "NativeSection | ProfileCanvas", optional: false },
        FieldSpec { name: "status", ts_type: "CanvasSurfaceStatus", optional: false },
        FieldSpec { name: "sourceComponent", ts_type: "string", optional: false },
        FieldSpec { name: "nativeContract", ts_type: "string", optional: false },
        FieldSpec { name: "authority", ts_type: "NativeAuthority", optional: false },
        FieldSpec { name: "headline", ts_type: "string", optional: false },
        FieldSpec { name: "detail", ts_type: "string", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const CANVAS_SURFACES_COMMAND_BASE: InterfaceSpec = InterfaceSpec {
    name: "CanvasSurfacesCommandBase",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "cancelToken", ts_type: "string", optional: true },
    ],
};

const CANVAS_SURFACES_COMMAND_RESULT: InterfaceSpec = InterfaceSpec {
    name: "CanvasSurfacesCommandResult",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "accepted", ts_type: "boolean", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "event", ts_type: "CanvasSurfacesCommandResultEvent", optional: false },
        FieldSpec { name: "error", ts_type: "IpcError", optional: true },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const CANVAS_SURFACES_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "CanvasSurfacesSnapshot",
    fields: &[
        FieldSpec { name: "schema", ts_type: "\"ingen.electron.canvas_surfaces.snapshot.v1\"", optional: false },
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "activeSection", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "profileCanvas", ts_type: "ProfileCanvas", optional: false },
        FieldSpec { name: "activeSurfaceId", ts_type: "string", optional: false },
        FieldSpec { name: "surfaces", ts_type: "CanvasSurfaceSummary[]", optional: false },
        FieldSpec { name: "nativeSurfacePolicy", ts_type: "{ banger: \"child-window\"; webexplorer: \"rust-owned-webview\" }", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const RIGHT_PANEL_LINE: InterfaceSpec = InterfaceSpec {
    name: "RightPanelLine",
    fields: &[
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "value", ts_type: "string", optional: false },
        FieldSpec { name: "severity", ts_type: "\"info\" | \"ok\" | \"warn\" | \"error\"", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const RIGHT_PANEL_ACTION: InterfaceSpec = InterfaceSpec {
    name: "RightPanelAction",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "command", ts_type: "RightPanelCommandKind", optional: false },
        FieldSpec { name: "enabled", ts_type: "boolean", optional: false },
    ],
};

const RIGHT_PANEL_TAB: InterfaceSpec = InterfaceSpec {
    name: "RightPanelTab",
    fields: &[
        FieldSpec { name: "id", ts_type: "string", optional: false },
        FieldSpec { name: "label", ts_type: "string", optional: false },
        FieldSpec { name: "selected", ts_type: "boolean", optional: false },
        FieldSpec { name: "count", ts_type: "number", optional: false },
    ],
};

const RIGHT_PANEL_COMMAND_BASE: InterfaceSpec = InterfaceSpec {
    name: "RightPanelCommandBase",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "cancelToken", ts_type: "string", optional: true },
    ],
};

const RIGHT_PANEL_COMMAND_RESULT: InterfaceSpec = InterfaceSpec {
    name: "RightPanelCommandResult",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "accepted", ts_type: "boolean", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "event", ts_type: "RightPanelCommandResultEvent", optional: false },
        FieldSpec { name: "error", ts_type: "IpcError", optional: true },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const RIGHT_PANEL_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "RightPanelSnapshot",
    fields: &[
        FieldSpec { name: "schema", ts_type: "\"ingen.electron.right_panel.snapshot.v1\"", optional: false },
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "activeSection", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "profileCanvas", ts_type: "ProfileCanvas", optional: false },
        FieldSpec { name: "open", ts_type: "boolean", optional: false },
        FieldSpec { name: "activeTab", ts_type: "string", optional: false },
        FieldSpec { name: "title", ts_type: "string", optional: false },
        FieldSpec { name: "summary", ts_type: "string", optional: false },
        FieldSpec { name: "tabs", ts_type: "RightPanelTab[]", optional: false },
        FieldSpec { name: "lines", ts_type: "RightPanelLine[]", optional: false },
        FieldSpec { name: "actions", ts_type: "RightPanelAction[]", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const PANELS_CHAT_BOTTOM_COMMAND_BASE: InterfaceSpec = InterfaceSpec {
    name: "PanelsChatBottomCommandBase",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "cancelToken", ts_type: "string", optional: true },
    ],
};

const PANELS_CHAT_BOTTOM_COMMAND_RESULT: InterfaceSpec = InterfaceSpec {
    name: "PanelsChatBottomCommandResult",
    fields: &[
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "requestId", ts_type: "string", optional: false },
        FieldSpec { name: "accepted", ts_type: "boolean", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "event", ts_type: "PanelsChatBottomCommandResultEvent", optional: false },
        FieldSpec { name: "error", ts_type: "IpcError", optional: true },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const PANELS_CHAT_BOTTOM_SNAPSHOT: InterfaceSpec = InterfaceSpec {
    name: "PanelsChatBottomSnapshot",
    fields: &[
        FieldSpec { name: "schema", ts_type: "\"ingen.electron.panels_chat_bottom.snapshot.v1\"", optional: false },
        FieldSpec { name: "version", ts_type: "typeof FORGE_ELECTRON_IPC_VERSION", optional: false },
        FieldSpec { name: "mode", ts_type: "FrontSliceMode", optional: false },
        FieldSpec { name: "activeSection", ts_type: "NativeSection", optional: false },
        FieldSpec { name: "activeSessionId", ts_type: "string", optional: false },
        FieldSpec { name: "profileCanvas", ts_type: "\"\" | \"sessions\" | \"brain\" | \"profile\" | \"llm\"", optional: false },
        FieldSpec { name: "rightPanelOpen", ts_type: "boolean", optional: false },
        FieldSpec { name: "statusDock", ts_type: "StatusDockSnapshot", optional: false },
        FieldSpec { name: "transcript", ts_type: "TranscriptMessage[]", optional: false },
        FieldSpec { name: "parallelLanes", ts_type: "ParallelChatLane[]", optional: false },
        FieldSpec { name: "agentSurfaceStatus", ts_type: "string", optional: false },
        FieldSpec { name: "composer", ts_type: "ComposerSnapshot", optional: false },
        FieldSpec { name: "bottomControls", ts_type: "BottomControl[]", optional: false },
        FieldSpec { name: "proofHash", ts_type: "string", optional: false },
    ],
};

const FORGE_SHELL_API: InterfaceSpec = InterfaceSpec {
    name: "ForgeShellApi",
    fields: &[
        FieldSpec { name: "getCutover", ts_type: "(slice: \"header\" | \"sidebar\" | \"panels_chat_bottom\" | \"canvas_surfaces\") => Promise<FrontSliceMode>", optional: false },
        FieldSpec { name: "getHeaderSnapshot", ts_type: "() => Promise<HeaderSnapshot>", optional: false },
        FieldSpec { name: "getHeaderSurfaceSnapshot", ts_type: "() => Promise<HeaderSurfaceSnapshot>", optional: false },
        FieldSpec { name: "dispatchHeaderCommand", ts_type: "(command: HeaderCommand) => Promise<HeaderCommandResult>", optional: false },
        FieldSpec { name: "getSidebarSnapshot", ts_type: "() => Promise<SidebarSnapshot>", optional: false },
        FieldSpec { name: "dispatchSidebarCommand", ts_type: "(command: SidebarCommand) => Promise<SidebarCommandResult>", optional: false },
        FieldSpec { name: "getPanelsChatBottomSnapshot", ts_type: "() => Promise<PanelsChatBottomSnapshot>", optional: false },
        FieldSpec { name: "dispatchPanelsChatBottomCommand", ts_type: "(command: PanelsChatBottomCommand) => Promise<PanelsChatBottomCommandResult>", optional: false },
        FieldSpec { name: "getCanvasSurfacesSnapshot", ts_type: "() => Promise<CanvasSurfacesSnapshot>", optional: false },
        FieldSpec { name: "dispatchCanvasSurfacesCommand", ts_type: "(command: CanvasSurfacesCommand) => Promise<CanvasSurfacesCommandResult>", optional: false },
        FieldSpec { name: "getRightPanelSnapshot", ts_type: "() => Promise<RightPanelSnapshot>", optional: false },
        FieldSpec { name: "dispatchRightPanelCommand", ts_type: "(command: RightPanelCommand) => Promise<RightPanelCommandResult>", optional: false },
    ],
};

fn render_enum(spec: EnumSpec) -> String {
    let union = spec
        .variants
        .iter()
        .map(|variant| format!("\"{variant}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    let values = spec
        .variants
        .iter()
        .map(|variant| format!("  \"{variant}\","))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "export type {name} = {union};\nexport const {values_name} = [\n{values}\n] as const;\n",
        name = spec.name,
        union = union,
        values_name = screaming_snake(spec.name),
        values = values
    )
}

fn render_interface(spec: InterfaceSpec) -> String {
    let fields = spec
        .fields
        .iter()
        .map(|field| {
            format!(
                "  {name}{optional}: {ts_type};",
                name = field.name,
                optional = if field.optional { "?" } else { "" },
                ts_type = field.ts_type
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("export interface {} {{\n{}\n}}\n", spec.name, fields)
}

fn screaming_snake(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("contract crate must live under examples/ingen_electron_shell/contract")
        .to_path_buf()
}

fn read_brain_str_const(brain_source: &str, name: &str) -> String {
    let prefix = format!("pub const {name}: &str = \"");
    let start = brain_source
        .find(&prefix)
        .unwrap_or_else(|| panic!("missing {name} in src/brain.rs"))
        + prefix.len();
    let rest = &brain_source[start..];
    let end = rest
        .find("\";")
        .unwrap_or_else(|| panic!("malformed {name} in src/brain.rs"));
    rest[..end].to_string()
}

fn ts_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn render_brain_codeact_projection() -> String {
    let brain_path = repo_root().join("src").join("brain.rs");
    let brain_source = fs::read_to_string(&brain_path).expect("read src/brain.rs for CodeAct projection");
    let mut out = String::new();
    out.push_str("// CodeAct command identities are projected from src/brain.rs. Do not define them in Electron files.\n");
    for name in BRAIN_STR_CONSTS {
        let value = read_brain_str_const(&brain_source, name);
        out.push_str(&format!(
            "export const {name} = {} as const;\n",
            ts_string_literal(&value)
        ));
    }
    out.push_str(
        "\nexport const BRAIN_CODEACT_COMMANDS = [\n\
  BRAIN_SEARCHARCHIVE_COMMAND,\n\
  BRAIN_RENAME_SESSION_COMMAND,\n\
  BRAIN_WEBSEARCH_COMMAND,\n\
  BRAIN_CODEDOCS_COMMAND,\n\
  BRAIN_GITHUB_MCP_COMMAND,\n\
  BRAIN_WEBACT_COMMAND,\n\
  BRAIN_SECURITYSCAN_COMMAND,\n\
  BRAIN_GOOGLEWEB_COMMAND,\n\
  BRAIN_SCRAPERS_COMMAND,\n\
  BRAIN_MAPS_COMMAND,\n\
  BRAIN_GMAIL_COMMAND,\n\
  BRAIN_GMAIL_COM_COMMAND,\n\
  BRAIN_AIRBNB_COMMAND,\n\
  BRAIN_NEWIMAGE_COMMAND,\n\
  BRAIN_EDITIMAGE_COMMAND,\n\
  BRAIN_QUESTIONNAIRE_COMMAND,\n\
  BRAIN_SCIENCE_COMMAND,\n\
  BRAIN_CODING_COMMAND,\n\
  BRAIN_NEWBRAIN_COMMAND,\n\
  BRAIN_MODIFY_NAMED_BRAIN_COMMAND,\n\
  BRAIN_WORKSPACE_COMMAND,\n\
  BRAIN_CAPABILITIES_COMMAND,\n\
  BRAIN_CODING_LIVE_PREVIEW_COMMAND,\n\
  BRAIN_NEWCOMPUTE_COMMAND,\n\
  BRAIN_SELECTCOMPUTE_COMMAND,\n\
  BRAIN_NAMED_COMPUTE_COMMAND,\n\
  BRAIN_NEWOBJECT_COMMAND,\n\
  BRAIN_WEB_COMMAND,\n\
  BRAIN_FRONTDESIGN_COMMAND,\n\
  BRAIN_GOOGLE_AGENDA_COMMAND,\n\
  BRAIN_BRAIN_COMMAND,\n\
  BRAIN_NEWMODULE_COMMAND,\n\
  BRAIN_RUST_PORT_ADAPTER_COMMAND,\n\
  BRAIN_RUST_STATE_STORE_COMMAND,\n\
] as const;\n\
export type BrainCodeActCommand = (typeof BRAIN_CODEACT_COMMANDS)[number];\n\n",
    );
    out.push_str("export const BRAIN_CODEACT_COMMAND_DESCRIPTIONS = [\n");
    for (command, description) in BRAIN_COMMAND_DESCRIPTION_PAIRS {
        out.push_str(&format!(
            "  {{ command: {command}, description: {description} }},\n"
        ));
    }
    out.push_str("] as const;\n\n");
    out
}

fn render_typescript() -> String {
    let mut out = String::new();
    out.push_str("// @generated by examples/ingen_electron_shell/contract/src/main.rs\n");
    out.push_str("// Do not edit by hand. Run `npm run generate:ipc`.\n\n");
    out.push_str(&format!("export const FORGE_ELECTRON_IPC_VERSION = {IPC_VERSION} as const;\n"));
    out.push_str("export const FORGE_ELECTRON_IPC_CONTRACT_SOURCE = \"rust:ingen-electron-ipc-contract\" as const;\n\n");
    out.push_str(&render_brain_codeact_projection());
    for spec in [
        FRONT_SLICE_MODE,
        NATIVE_SECTION,
        HEADER_COMMAND_KIND,
        SIDEBAR_COMMAND_KIND,
        PANELS_CHAT_BOTTOM_COMMAND_KIND,
        CANVAS_SURFACES_COMMAND_KIND,
        RIGHT_PANEL_COMMAND_KIND,
        IPC_ERROR_CODE,
        PROFILE_CANVAS,
        SESSIONS_MENU_MODE,
        HEADER_RESULT_EVENT,
        SIDEBAR_RESULT_EVENT,
        PANELS_CHAT_BOTTOM_RESULT_EVENT,
        CANVAS_SURFACES_RESULT_EVENT,
        RIGHT_PANEL_RESULT_EVENT,
        NATIVE_AUTHORITY,
        HEADER_SURFACE_KIND,
        HEADER_SURFACE_STATUS,
        CANVAS_SURFACE_KIND,
        CANVAS_SURFACE_STATUS,
    ] {
        out.push_str(&render_enum(spec));
        out.push('\n');
    }
    for spec in [
        IPC_ERROR,
        HEADER_COMMAND_BASE,
        HEADER_COMMAND_RESULT,
        SIDEBAR_COMMAND_BASE,
        SIDEBAR_COMMAND_RESULT,
        HEADER_CONTROL,
        SIDEBAR_SESSION_ITEM,
        SIDEBAR_TOOL_CONTROL,
        PROFILE_MENU_ITEM,
        ARCHIVE_CONFIRM_STATE,
        HEADER_SNAPSHOT,
        SIDEBAR_SNAPSHOT,
        HEADER_SURFACE_SLOT,
        HEADER_SURFACE_CONTRACT,
        HEADER_SURFACE_SNAPSHOT,
        STATUS_DOCK_LINE,
        STATUS_DOCK_SNAPSHOT,
        TRANSCRIPT_MESSAGE,
        PARALLEL_CHAT_LANE,
        LLM_PROVIDER_STATE,
        COMPOSER_UPLOAD_PREVIEW,
        PARALLEL_CHAT_DRAFT,
        COMPOSER_SNAPSHOT,
        BOTTOM_CONTROL,
        PANELS_CHAT_BOTTOM_COMMAND_BASE,
        PANELS_CHAT_BOTTOM_COMMAND_RESULT,
        PANELS_CHAT_BOTTOM_SNAPSHOT,
        CANVAS_SURFACE_SUMMARY,
        CANVAS_SURFACES_COMMAND_BASE,
        CANVAS_SURFACES_COMMAND_RESULT,
        CANVAS_SURFACES_SNAPSHOT,
        RIGHT_PANEL_LINE,
        RIGHT_PANEL_ACTION,
        RIGHT_PANEL_TAB,
        RIGHT_PANEL_COMMAND_BASE,
        RIGHT_PANEL_COMMAND_RESULT,
        RIGHT_PANEL_SNAPSHOT,
        FORGE_SHELL_API,
    ] {
        out.push_str(&render_interface(spec));
        out.push('\n');
    }
    out.push_str(
        "export type HeaderCommand =\n  | (HeaderCommandBase & { kind: Exclude<HeaderCommandKind, \"navigate_workspace\"> })\n  | (HeaderCommandBase & { kind: \"navigate_workspace\"; section: NativeSection });\n",
    );
    out.push_str(
        "\nexport type SidebarCommand =\n  | (SidebarCommandBase & { kind: \"navigate\"; section: NativeSection })\n  | (SidebarCommandBase & { kind: \"open_session\"; sessionId: string; section: NativeSection })\n  | (SidebarCommandBase & { kind: \"rename_session\"; sessionId: string; label: string })\n  | (SidebarCommandBase & { kind: \"open_profile_canvas\"; canvas: ProfileCanvas })\n  | (SidebarCommandBase & { kind: \"archive_session\"; sessionId: string })\n  | (SidebarCommandBase & { kind: \"activate_control\"; label: string })\n  | (SidebarCommandBase & { kind: \"switch_sessions_mode\"; mode: SessionsMenuMode })\n  | (SidebarCommandBase & { kind: \"toggle_profile_menu\" })\n  | (SidebarCommandBase & { kind: \"set_active_drawer\"; drawer: string })\n  | (SidebarCommandBase & { kind: \"hide_tool\"; toolId: string })\n  | (SidebarCommandBase & { kind: \"restore_tool\"; toolId: string })\n  | (SidebarCommandBase & { kind: \"pin_session\"; sessionId: string; label: string; section: NativeSection })\n  | (SidebarCommandBase & { kind: \"confirm_archive\" })\n  | (SidebarCommandBase & { kind: \"cancel_archive\" });\n",
    );
    out.push_str(
        "\nexport type PanelsChatBottomCommand = PanelsChatBottomCommandBase & {\n  kind: PanelsChatBottomCommandKind;\n  value?: string;\n  provider?: \"openai\" | \"anthropic\" | \"openrouter\";\n  direction?: number;\n  attachmentIds?: string[];\n  filePaths?: string[];\n  moduleId?: string;\n  parallelSessionIndex?: number;\n  parallelDrafts?: ParallelChatDraft[];\n  internalPrompt?: boolean;\n  replaceAssistantMessageId?: string;\n  userFirstName?: string;\n  agentFirstName?: string;\n  userHomeLocation?: string;\n};\n",
    );
    out.push_str(
        "\nexport type CanvasSurfacesCommand = CanvasSurfacesCommandBase & {\n  kind: CanvasSurfacesCommandKind;\n  target?: string;\n  value?: string;\n  section?: NativeSection;\n  canvas?: ProfileCanvas;\n};\n",
    );
    out.push_str(
        "\nexport type RightPanelCommand = RightPanelCommandBase & {\n  kind: RightPanelCommandKind;\n  target?: string;\n  value?: string;\n};\n",
    );
    out
}

fn main() {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: export <path-to-forge-ipc.generated.ts>");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).expect("create output directory");
    }
    fs::write(&output, render_typescript()).expect("write generated TypeScript contract");
    println!("{}", output.display());
}
