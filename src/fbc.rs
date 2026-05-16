//! Forge Native Bytecode / FBC v0.
//!
//! This module is intentionally small: a deterministic interpreter, a strict
//! preflight verifier, capability handles, and a compact proof envelope. It is
//! not a replacement for KASM v0 yet; it is the narrow bridge for KASM2 trials.

use sha2::{Digest, Sha256};

pub const FBC_VERSION: u16 = 0;
pub const FBC_VERIFIER_VERSION: &str = "forge-fbc-verifier-v0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeBytecodeProgram {
    pub name: String,
    pub version: u16,
    pub capabilities: Vec<ForgeCapability>,
    pub hostcalls: Vec<ForgeHostCall>,
    pub ops: Vec<ForgeOpcode>,
    pub expected_output_schema: String,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeOpcode {
    PushBytes(Vec<u8>),
    PushText(String),
    PushCapability([u8; 32]),
    ReadCapability,
    HashTop,
    CsvProfileTiny { max_rows: u32, max_cols: u16 },
    ToolCellProjectTiny {
        tool_id: String,
        command: String,
        query: String,
        focus: Vec<String>,
        limit: u16,
    },
    KernelProject {
        op: String,
        payload_json: String,
    },
    JobReadProjection {
        job_id: String,
        max_records: u16,
    },
    UiIntentTransition { from: String, intent: String },
    EmitProjection { label: String },
    RawFilesystemProbe(String),
    RawNetworkProbe(String),
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeCapability {
    pub kind: ForgeCapabilityKind,
    pub scope: String,
    pub sealed_hash: [u8; 32],
    pub content_hash: Option<[u8; 32]>,
    pub limit_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ForgeCapabilityKind {
    FileHash,
    ArtifactHash,
    MemoryScope,
    NetworkSource,
    EventSchema,
    UiProjection,
    GpuBudget,
    ModelProviderScope,
    RawFilesystem,
    RawNetwork,
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ForgeHostCall {
    HashBytes,
    CsvProfileTiny,
    UiProjectEvent,
    KernelProject,
    JobReadProjection,
    ToolCellRun,
    MemoryRecall,
    ArtifactReadHash,
    UiEmitProjection,
    NetworkFetchSourceId,
    ReadCapability,
    RawFilesystem,
    RawNetwork,
    ReadSecret,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVerifierReport {
    pub ok: bool,
    pub verifier_hash: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub declared_hostcalls: Vec<ForgeHostCall>,
    pub capability_summary: Vec<String>,
    pub max_fuel: u64,
    pub max_memory_bytes: u64,
    pub program_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeRunProof {
    pub program_hash: String,
    pub bytecode_hash: String,
    pub verifier_hash: String,
    pub input_hash: String,
    pub output_hash: String,
    pub capability_hash: String,
    pub hostcall_hash: String,
    pub fuel_used: u64,
    pub memory_peak: u64,
    pub backend: String,
    pub deterministic_replay_hash: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVmConfig {
    pub max_fuel: u64,
    pub max_memory_bytes: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub backend: String,
    pub forbidden_opcodes: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVmOutput {
    pub status: ForgeVmStatus,
    pub bytes: Vec<u8>,
    pub preview: String,
    pub fuel_used: u64,
    pub memory_peak: u64,
    pub proof: ForgeRunProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeOptimizerReport {
    pub original_program_hash: String,
    pub optimized_program_hash: String,
    pub fuel_before: u64,
    pub fuel_after: u64,
    pub fused_hash_ops: u32,
    pub fused_capability_hash_ops: u32,
    pub changed: bool,
    pub optimizer_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeBackendSelection {
    pub requested: String,
    pub selected: String,
    pub reason: String,
    pub selector_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgePipelineOutput {
    pub original_program_hash: String,
    pub optimized_program_hash: String,
    pub optimizer: ForgeOptimizerReport,
    pub backend: ForgeBackendSelection,
    pub verifier: ForgeVerifierReport,
    pub vm_output: ForgeVmOutput,
    pub proof_projection: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeToolCellSpec {
    pub id: String,
    pub command: String,
    pub query: String,
    pub focus: Vec<String>,
    pub permissions: Vec<String>,
    pub denied: Vec<String>,
    pub input_schema_hash: String,
    pub output_schema_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeToolCellRegistry {
    pub schema_version: u16,
    pub default_engine: String,
    pub registry_hash: String,
    pub input_schema_hash: String,
    pub output_schema_hash: String,
    pub permissions: Vec<String>,
    pub denied: Vec<String>,
    pub cells: Vec<ForgeToolCellSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeAppRegistry {
    pub registry_hash: String,
    pub section_count: usize,
    pub sensitive_command_count: usize,
    pub cells: Vec<ForgeToolCellSpec>,
    pub graph_jsonl: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeCapabilityBinding {
    pub sealed_hash: [u8; 32],
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForgeHostContext {
    pub bindings: Vec<ForgeCapabilityBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeCompiledToolCell {
    pub program: ForgeBytecodeProgram,
    pub host_context: ForgeHostContext,
    pub manifest_hash: String,
    pub graph_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeProofLedgerEntry {
    pub sequence: u64,
    pub status: String,
    pub program_hash: String,
    pub proof_hash: String,
    pub verifier_hash: String,
    pub capability_hash: String,
    pub backend: String,
    pub fuel_used: u64,
    pub memory_peak: u64,
    pub ledger_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeToolCellBatchRecord {
    pub tool_id: String,
    pub command: String,
    pub status: String,
    pub program_hash: String,
    pub proof_hash: String,
    pub ledger_hash: String,
    pub output_hash: String,
    pub selected_evidence_count: usize,
    pub ranked_action_count: usize,
    pub projection_json: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeToolCellBatchOutput {
    pub graph_hash: String,
    pub tool_count: usize,
    pub ok_count: usize,
    pub denied_count: usize,
    pub records: Vec<ForgeToolCellBatchRecord>,
    pub ledger_root_hash: String,
    pub projection_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeVmStatus {
    Ok,
    VerifierDenied,
    FuelExhausted,
    MemoryLimitExceeded,
    RuntimeError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeVmError {
    VerifierDenied(ForgeVerifierReport),
    FuelExhausted { used: u64, max: u64 },
    MemoryLimitExceeded { peak: u64, max: u64 },
    OutputLimitExceeded { bytes: u64, max: u64 },
    StackUnderflow(&'static str),
    CapabilityDenied(String),
    MissingEnd,
    Parse(String),
}

impl Default for ForgeVmConfig {
    fn default() -> Self {
        Self {
            max_fuel: 128,
            max_memory_bytes: 64 * 1024,
            max_input_bytes: 64 * 1024,
            max_output_bytes: 64 * 1024,
            backend: "fbc_interpreter".to_string(),
            forbidden_opcodes: Vec::new(),
        }
    }
}

impl ForgeBytecodeProgram {
    pub fn v0(name: impl Into<String>, ops: Vec<ForgeOpcode>) -> Self {
        let mut hostcalls = Vec::new();
        for op in &ops {
            for call in required_hostcalls(op) {
                if !hostcalls.contains(&call) {
                    hostcalls.push(call);
                }
            }
        }
        Self {
            name: name.into(),
            version: FBC_VERSION,
            capabilities: Vec::new(),
            hostcalls,
            ops,
            expected_output_schema: "bytes".to_string(),
            deterministic: true,
        }
    }

    pub fn with_capability(mut self, capability: ForgeCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    pub fn with_hostcall(mut self, hostcall: ForgeHostCall) -> Self {
        if !self.hostcalls.contains(&hostcall) {
            self.hostcalls.push(hostcall);
        }
        self
    }
}

impl ForgeHostContext {
    pub fn with_binding(mut self, capability: &ForgeCapability, bytes: Vec<u8>) -> Self {
        self.bindings.push(ForgeCapabilityBinding {
            sealed_hash: capability.sealed_hash,
            bytes,
        });
        self
    }

    fn read_binding(&self, sealed_hash: &[u8; 32]) -> Option<&[u8]> {
        self.bindings
            .iter()
            .find(|binding| &binding.sealed_hash == sealed_hash)
            .map(|binding| binding.bytes.as_slice())
    }
}

impl ForgeCapability {
    pub fn sealed(
        kind: ForgeCapabilityKind,
        scope: impl AsRef<str>,
        content: Option<&[u8]>,
        limit_bytes: u64,
    ) -> Self {
        let scope = scope.as_ref().to_string();
        let content_hash = content.map(sha256);
        let mut h = Sha256::new();
        h.update(b"forge-fbc-capability-v0\n");
        write_cap_kind(&mut h, kind);
        h.update(b"\n");
        h.update(scope.as_bytes());
        h.update(b"\n");
        h.update(limit_bytes.to_le_bytes());
        if let Some(content_hash) = content_hash {
            h.update(content_hash);
        }
        Self {
            kind,
            scope,
            sealed_hash: h.finalize().into(),
            content_hash,
            limit_bytes,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "cap:{}:{}:{}:{}",
            cap_kind_name(self.kind),
            self.scope,
            hex(&self.sealed_hash),
            self.limit_bytes
        )
    }
}

pub fn hash_bytes_program(name: &str, bytes: &[u8]) -> ForgeBytecodeProgram {
    ForgeBytecodeProgram::v0(
        name,
        vec![
            ForgeOpcode::PushBytes(bytes.to_vec()),
            ForgeOpcode::HashTop,
            ForgeOpcode::End,
        ],
    )
}

pub fn csv_profile_tiny_program(name: &str, csv: &str) -> ForgeBytecodeProgram {
    let mut program = ForgeBytecodeProgram::v0(
        name,
        vec![
            ForgeOpcode::PushText(csv.to_string()),
            ForgeOpcode::CsvProfileTiny {
                max_rows: 16,
                max_cols: 16,
            },
            ForgeOpcode::End,
        ],
    );
    program.expected_output_schema = "csv_profile_tiny_v0".to_string();
    program
}

pub fn ui_intent_transition_program(name: &str, from: &str, intent: &str) -> ForgeBytecodeProgram {
    let mut program = ForgeBytecodeProgram::v0(
        name,
        vec![
            ForgeOpcode::UiIntentTransition {
                from: from.to_string(),
                intent: intent.to_string(),
            },
            ForgeOpcode::EmitProjection {
                label: "ui_projection".to_string(),
            },
            ForgeOpcode::End,
        ],
    );
    program.expected_output_schema = "ui_projection_v0".to_string();
    program.capabilities.push(ForgeCapability::sealed(
        ForgeCapabilityKind::UiProjection,
        "ui:projection:bounded",
        Some(b"ui_projection_v0"),
        4096,
    ));
    program
}

pub fn kernel_project_program(name: &str, op: &str, payload_json: &str) -> ForgeBytecodeProgram {
    let mut program = ForgeBytecodeProgram::v0(
        name,
        vec![
            ForgeOpcode::KernelProject {
                op: op.to_string(),
                payload_json: payload_json.to_string(),
            },
            ForgeOpcode::End,
        ],
    );
    program.expected_output_schema = "kernel_projection_v0".to_string();
    program.capabilities.push(ForgeCapability::sealed(
        ForgeCapabilityKind::UiProjection,
        "kernel:projection:bounded",
        Some(b"kernel_projection_v0"),
        8192,
    ));
    program
}

pub fn job_read_projection_program(name: &str, job_id: &str, max_records: u16) -> ForgeBytecodeProgram {
    let mut program = ForgeBytecodeProgram::v0(
        name,
        vec![
            ForgeOpcode::JobReadProjection {
                job_id: job_id.to_string(),
                max_records,
            },
            ForgeOpcode::End,
        ],
    );
    program.expected_output_schema = "job_projection_v0".to_string();
    program.capabilities.push(ForgeCapability::sealed(
        ForgeCapabilityKind::ArtifactHash,
        "job:projection:bounded",
        Some(b"job_projection_v0"),
        8192,
    ));
    program
}

pub fn hostcall_abi_v0() -> Vec<ForgeHostCall> {
    vec![
        ForgeHostCall::KernelProject,
        ForgeHostCall::JobReadProjection,
        ForgeHostCall::ToolCellRun,
        ForgeHostCall::MemoryRecall,
        ForgeHostCall::ArtifactReadHash,
        ForgeHostCall::UiEmitProjection,
        ForgeHostCall::NetworkFetchSourceId,
        ForgeHostCall::ReadCapability,
    ]
}

pub fn compile_tool_cell_program(cell: &ForgeToolCellSpec) -> ForgeBytecodeProgram {
    compile_tool_cell_bundle(cell).program
}

pub fn compile_tool_cell_bundle(cell: &ForgeToolCellSpec) -> ForgeCompiledToolCell {
    compile_tool_cell_bundle_with_graph(cell, b"")
}

pub fn compile_tool_cell_bundle_with_graph(
    cell: &ForgeToolCellSpec,
    graph_jsonl: &[u8],
) -> ForgeCompiledToolCell {
    let manifest = tool_cell_manifest(cell);
    let manifest_capability = ForgeCapability::sealed(
        ForgeCapabilityKind::EventSchema,
        format!("toolcell:{}:manifest", cell.id),
        Some(manifest.as_bytes()),
        manifest.len() as u64,
    );
    let graph_capability = ForgeCapability::sealed(
        ForgeCapabilityKind::ArtifactHash,
        format!("toolcell:{}:living_dataflow_graph", cell.id),
        Some(graph_jsonl),
        graph_jsonl.len().max(1) as u64,
    );
    let mut program = ForgeBytecodeProgram::v0(
        format!("toolcell:{}", cell.id),
        vec![
            ForgeOpcode::PushCapability(manifest_capability.sealed_hash),
            ForgeOpcode::ReadCapability,
            ForgeOpcode::HashTop,
            ForgeOpcode::PushCapability(graph_capability.sealed_hash),
            ForgeOpcode::ReadCapability,
            ForgeOpcode::ToolCellProjectTiny {
                tool_id: cell.id.clone(),
                command: cell.command.clone(),
                query: cell.query.clone(),
                focus: cell.focus.clone(),
                limit: 96,
            },
            ForgeOpcode::End,
        ],
    )
    .with_hostcall(ForgeHostCall::ReadCapability);
    program.expected_output_schema = "forge_tool_cell_projection_v0".to_string();
    program.capabilities = cell
        .permissions
        .iter()
        .map(|permission| capability_from_permission(permission))
        .collect();
    program.capabilities.push(manifest_capability.clone());
    program.capabilities.push(graph_capability.clone());
    ForgeCompiledToolCell {
        program,
        host_context: ForgeHostContext::default()
            .with_binding(&manifest_capability, manifest.as_bytes().to_vec())
            .with_binding(&graph_capability, graph_jsonl.to_vec()),
        manifest_hash: hex(&sha256(manifest.as_bytes())),
        graph_hash: hex(&sha256(graph_jsonl)),
    }
}

pub fn parse_program_v0(text: &str) -> Result<ForgeBytecodeProgram, ForgeVmError> {
    let mut name = "parsed-fbc-program".to_string();
    let mut hostcalls = Vec::new();
    let mut capabilities = Vec::new();
    let mut ops = Vec::new();
    let mut expected_output_schema = "bytes".to_string();
    let mut deterministic = true;

    for (line_no, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("name=") {
            name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("schema=") {
            expected_output_schema = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("deterministic=") {
            deterministic = match value.trim() {
                "true" => true,
                "false" => false,
                other => {
                    return Err(ForgeVmError::Parse(format!(
                        "line {}: bad deterministic value {other}",
                        line_no + 1
                    )))
                }
            };
        } else if let Some(value) = line.strip_prefix("hostcall=") {
            hostcalls.push(parse_hostcall(value.trim()).ok_or_else(|| {
                ForgeVmError::Parse(format!("line {}: unknown hostcall", line_no + 1))
            })?);
        } else if let Some(value) = line.strip_prefix("cap=") {
            let mut parts = value.split('|');
            let kind = parts
                .next()
                .and_then(parse_cap_kind)
                .ok_or_else(|| ForgeVmError::Parse(format!("line {}: bad cap kind", line_no + 1)))?;
            let scope = parts
                .next()
                .ok_or_else(|| ForgeVmError::Parse(format!("line {}: missing cap scope", line_no + 1)))?;
            let limit = parts
                .next()
                .and_then(|item| item.parse::<u64>().ok())
                .unwrap_or(0);
            capabilities.push(ForgeCapability::sealed(kind, scope, None, limit));
        } else if let Some(value) = line.strip_prefix("op=") {
            ops.push(parse_opcode(value.trim(), line_no + 1)?);
        } else {
            return Err(ForgeVmError::Parse(format!(
                "line {}: unknown directive",
                line_no + 1
            )));
        }
    }

    if hostcalls.is_empty() {
        for op in &ops {
            for call in required_hostcalls(op) {
                if !hostcalls.contains(&call) {
                    hostcalls.push(call);
                }
            }
        }
    }

    Ok(ForgeBytecodeProgram {
        name,
        version: FBC_VERSION,
        capabilities,
        hostcalls,
        ops,
        expected_output_schema,
        deterministic,
    })
}

pub fn hash_program(program: &ForgeBytecodeProgram) -> String {
    hex(&sha256(&encode_program(program)))
}

pub fn verify_program(program: &ForgeBytecodeProgram, config: &ForgeVmConfig) -> ForgeVerifierReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let program_hash = hash_program(program);

    if program.version != FBC_VERSION {
        errors.push(format!("unsupported FBC version {}", program.version));
    }
    if program.ops.is_empty() {
        errors.push("program has no opcodes".to_string());
    }
    if program.ops.len() as u64 > config.max_fuel {
        errors.push(format!(
            "program op count {} exceeds max fuel {}",
            program.ops.len(),
            config.max_fuel
        ));
    }
    if !matches!(program.ops.last(), Some(ForgeOpcode::End)) {
        errors.push("program must end with End".to_string());
    }

    let declared: Vec<ForgeHostCall> = program.hostcalls.clone();
    for capability in &program.capabilities {
        match capability.kind {
            ForgeCapabilityKind::RawFilesystem => {
                errors.push("raw filesystem capability denied".to_string())
            }
            ForgeCapabilityKind::RawNetwork => errors.push("raw network capability denied".to_string()),
            ForgeCapabilityKind::Secret => errors.push("secret capability denied in FBC v0".to_string()),
            _ => {}
        }
        if capability.scope.contains('\\') || capability.scope.starts_with('/') || capability.scope.contains("..") {
            errors.push(format!(
                "capability scope must not contain raw path syntax: {}",
                capability.scope
            ));
        }
        if capability.limit_bytes > config.max_input_bytes {
            errors.push(format!(
                "capability {} limit {} exceeds max input {}",
                capability.scope, capability.limit_bytes, config.max_input_bytes
            ));
        }
    }

    let mut estimated_bytes = 0_u64;
    for op in &program.ops {
        let op_name = opcode_name(op);
        if config.forbidden_opcodes.iter().any(|forbidden| forbidden == &op_name) {
            errors.push(format!("opcode {op_name} is forbidden by context"));
        }
        if let ForgeOpcode::PushCapability(sealed_hash) = op {
            if !program
                .capabilities
                .iter()
                .any(|capability| capability.sealed_hash == *sealed_hash)
            {
                errors.push(format!(
                    "capability handle {} is not declared",
                    hex(sealed_hash)
                ));
            }
        }
        for required in required_hostcalls(op) {
            if !declared.contains(&required) {
                errors.push(format!(
                    "opcode {op_name} requires undeclared hostcall {}",
                    hostcall_name(required)
                ));
            }
            match required {
                ForgeHostCall::RawFilesystem => errors.push("raw filesystem hostcall denied".to_string()),
                ForgeHostCall::RawNetwork => errors.push("raw network hostcall denied".to_string()),
                ForgeHostCall::ReadSecret => errors.push("secret hostcall denied".to_string()),
                _ => {}
            }
        }
        estimated_bytes = estimated_bytes.saturating_add(match op {
            ForgeOpcode::PushBytes(bytes) => bytes.len() as u64,
            ForgeOpcode::PushText(text) => text.len() as u64,
            ForgeOpcode::PushCapability(_) => 32,
            ForgeOpcode::ToolCellProjectTiny {
                tool_id,
                command,
                query,
                focus,
                ..
            } => {
                (tool_id.len()
                    + command.len()
                    + query.len()
                    + focus.iter().map(String::len).sum::<usize>()) as u64
            }
            ForgeOpcode::KernelProject { op, payload_json } => (op.len() + payload_json.len()) as u64,
            ForgeOpcode::JobReadProjection { job_id, .. } => job_id.len() as u64 + 8,
            ForgeOpcode::UiIntentTransition { from, intent } => (from.len() + intent.len()) as u64,
            ForgeOpcode::EmitProjection { label } => label.len() as u64,
            ForgeOpcode::RawFilesystemProbe(path) => path.len() as u64,
            ForgeOpcode::RawNetworkProbe(url) => url.len() as u64,
            _ => 8,
        });
    }
    if estimated_bytes > config.max_memory_bytes {
        errors.push(format!(
            "estimated memory {} exceeds max {}",
            estimated_bytes, config.max_memory_bytes
        ));
    }
    if !program.deterministic {
        warnings.push("program declares non-determinism; replay hash is advisory".to_string());
    }

    let capability_summary = program
        .capabilities
        .iter()
        .map(ForgeCapability::summary)
        .collect::<Vec<_>>();
    let verifier_hash = verifier_hash(program, config, &errors, &warnings);
    ForgeVerifierReport {
        ok: errors.is_empty(),
        verifier_hash,
        errors,
        warnings,
        declared_hostcalls: declared,
        capability_summary,
        max_fuel: config.max_fuel,
        max_memory_bytes: config.max_memory_bytes,
        program_hash,
    }
}

pub fn optimize_program_v0(program: &ForgeBytecodeProgram) -> (ForgeBytecodeProgram, ForgeOptimizerReport) {
    optimize_program_v0_inner(program, None)
}

pub fn optimize_program_v0_with_context(
    program: &ForgeBytecodeProgram,
    host_context: &ForgeHostContext,
) -> (ForgeBytecodeProgram, ForgeOptimizerReport) {
    optimize_program_v0_inner(program, Some(host_context))
}

fn optimize_program_v0_inner(
    program: &ForgeBytecodeProgram,
    host_context: Option<&ForgeHostContext>,
) -> (ForgeBytecodeProgram, ForgeOptimizerReport) {
    let original_program_hash = hash_program(program);
    let mut ops = Vec::with_capacity(program.ops.len());
    let mut idx = 0;
    let mut fused_hash_ops = 0_u32;
    let mut fused_capability_hash_ops = 0_u32;

    while idx < program.ops.len() {
        match (
            program.ops.get(idx),
            program.ops.get(idx + 1),
            program.ops.get(idx + 2),
        ) {
            (
                Some(ForgeOpcode::PushCapability(sealed_hash)),
                Some(ForgeOpcode::ReadCapability),
                Some(ForgeOpcode::HashTop),
            ) if host_context
                .and_then(|context| verified_capability_bytes(program, context, sealed_hash))
                .is_some() =>
            {
                let bytes = host_context
                    .and_then(|context| verified_capability_bytes(program, context, sealed_hash))
                    .unwrap();
                ops.push(ForgeOpcode::PushBytes(hex(&sha256(bytes)).into_bytes()));
                fused_capability_hash_ops += 1;
                idx += 3;
            }
            _ => match (program.ops.get(idx), program.ops.get(idx + 1)) {
            (
                Some(ForgeOpcode::PushCapability(sealed_hash)),
                Some(ForgeOpcode::ReadCapability),
            ) if host_context
                .and_then(|context| verified_capability_bytes(program, context, sealed_hash))
                .map(|bytes| bytes.is_empty())
                .unwrap_or(false) =>
            {
                ops.push(ForgeOpcode::PushBytes(Vec::new()));
                idx += 2;
            }
            (Some(ForgeOpcode::PushBytes(bytes)), Some(ForgeOpcode::HashTop)) => {
                ops.push(ForgeOpcode::PushBytes(hex(&sha256(bytes)).into_bytes()));
                fused_hash_ops += 1;
                idx += 2;
            }
            (Some(ForgeOpcode::PushText(text)), Some(ForgeOpcode::HashTop)) => {
                ops.push(ForgeOpcode::PushBytes(
                    hex(&sha256(text.as_bytes())).into_bytes(),
                ));
                fused_hash_ops += 1;
                idx += 2;
            }
            (Some(ForgeOpcode::PushCapability(_)), Some(ForgeOpcode::HashTop)) => {
                ops.push(program.ops[idx].clone());
                ops.push(program.ops[idx + 1].clone());
                idx += 2;
            }
            (Some(op), _) => {
                ops.push(op.clone());
                idx += 1;
            }
            _ => break,
            },
        };
    }

    let mut optimized = ForgeBytecodeProgram::v0(program.name.clone(), ops);
    optimized.version = program.version;
    optimized.capabilities = program.capabilities.clone();
    optimized.expected_output_schema = program.expected_output_schema.clone();
    optimized.deterministic = program.deterministic;
    for hostcall in &program.hostcalls {
        if *hostcall == ForgeHostCall::ReadCapability && !optimized.hostcalls.contains(hostcall) {
            optimized.hostcalls.push(*hostcall);
        }
    }

    let optimized_program_hash = hash_program(&optimized);
    let changed = original_program_hash != optimized_program_hash;
    let fuel_before = program.ops.len() as u64;
    let fuel_after = optimized.ops.len() as u64;
    let optimizer_hash = stable_hash(&[
        "forge-fbc-optimizer-v0",
        &original_program_hash,
        &optimized_program_hash,
        &fuel_before.to_string(),
        &fuel_after.to_string(),
        &fused_hash_ops.to_string(),
        &fused_capability_hash_ops.to_string(),
    ]);
    (
        optimized,
        ForgeOptimizerReport {
            original_program_hash,
            optimized_program_hash,
            fuel_before,
            fuel_after,
            fused_hash_ops,
            fused_capability_hash_ops,
            changed,
            optimizer_hash,
        },
    )
}

pub fn select_backend(program: &ForgeBytecodeProgram, config: &ForgeVmConfig) -> ForgeBackendSelection {
    let requested = config.backend.clone();
    let selected = if requested == "auto" {
        if program
            .capabilities
            .iter()
            .any(|capability| capability.kind == ForgeCapabilityKind::GpuBudget)
        {
            "fbc_interpreter".to_string()
        } else {
            "fbc_interpreter".to_string()
        }
    } else {
        requested.clone()
    };
    let reason = if requested == "auto" {
        "v0 routes all verified programs to deterministic interpreter; GPU/native remain proof-compatible future backends"
            .to_string()
    } else {
        "explicit backend requested by config".to_string()
    };
    let selector_hash = stable_hash(&[
        "forge-fbc-backend-selector-v0",
        &hash_program(program),
        &requested,
        &selected,
        &reason,
    ]);
    ForgeBackendSelection {
        requested,
        selected,
        reason,
        selector_hash,
    }
}

pub fn execute_program_pipeline(
    program: &ForgeBytecodeProgram,
    config: &ForgeVmConfig,
) -> Result<ForgePipelineOutput, ForgeVmError> {
    execute_program_pipeline_with_context(program, config, &ForgeHostContext::default())
}

pub fn execute_program_pipeline_with_context(
    program: &ForgeBytecodeProgram,
    config: &ForgeVmConfig,
    host_context: &ForgeHostContext,
) -> Result<ForgePipelineOutput, ForgeVmError> {
    let original_report = verify_program(program, config);
    if !original_report.ok {
        return Err(ForgeVmError::VerifierDenied(original_report));
    }

    let (optimized, optimizer) = optimize_program_v0_with_context(program, host_context);
    let backend = select_backend(&optimized, config);
    let mut selected_config = config.clone();
    selected_config.backend = backend.selected.clone();
    let verifier = verify_program(&optimized, &selected_config);
    if !verifier.ok {
        return Err(ForgeVmError::VerifierDenied(verifier));
    }
    let vm_output = execute_program_interpreter_with_context(&optimized, &selected_config, host_context)?;
    let proof_projection = proof_projection_json(
        &vm_output.proof,
        &verifier,
        Some(&optimizer),
        Some(&backend),
    );
    Ok(ForgePipelineOutput {
        original_program_hash: optimizer.original_program_hash.clone(),
        optimized_program_hash: optimizer.optimized_program_hash.clone(),
        optimizer,
        backend,
        verifier,
        vm_output,
        proof_projection,
    })
}

pub fn execute_program_interpreter(
    program: &ForgeBytecodeProgram,
    config: &ForgeVmConfig,
) -> Result<ForgeVmOutput, ForgeVmError> {
    execute_program_interpreter_with_context(program, config, &ForgeHostContext::default())
}

pub fn execute_program_interpreter_with_context(
    program: &ForgeBytecodeProgram,
    config: &ForgeVmConfig,
    host_context: &ForgeHostContext,
) -> Result<ForgeVmOutput, ForgeVmError> {
    let report = verify_program(program, config);
    if !report.ok {
        return Err(ForgeVmError::VerifierDenied(report));
    }

    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut fuel_used = 0_u64;
    let mut memory_peak = 0_u64;
    let mut output = Vec::new();
    let mut ended = false;

    for op in &program.ops {
        fuel_used = fuel_used.saturating_add(1);
        if fuel_used > config.max_fuel {
            return Err(ForgeVmError::FuelExhausted {
                used: fuel_used,
                max: config.max_fuel,
            });
        }
        match op {
            ForgeOpcode::PushBytes(bytes) => stack.push(bytes.clone()),
            ForgeOpcode::PushText(text) => stack.push(text.as_bytes().to_vec()),
            ForgeOpcode::PushCapability(sealed_hash) => stack.push(hex(sealed_hash).into_bytes()),
            ForgeOpcode::ReadCapability => {
                let handle = stack
                    .pop()
                    .ok_or(ForgeVmError::StackUnderflow("ReadCapability"))?;
                let handle = String::from_utf8(handle)
                    .map_err(|_| ForgeVmError::CapabilityDenied("capability handle is not utf8".to_string()))?;
                let sealed_hash = parse_hash_hex(&handle)
                    .map_err(|err| ForgeVmError::CapabilityDenied(err))?;
                let capability = program
                    .capabilities
                    .iter()
                    .find(|capability| capability.sealed_hash == sealed_hash)
                    .ok_or_else(|| {
                        ForgeVmError::CapabilityDenied(format!(
                            "capability {} is not declared by program",
                            handle
                        ))
                    })?;
                let bytes = host_context
                    .read_binding(&sealed_hash)
                    .ok_or_else(|| {
                        ForgeVmError::CapabilityDenied(format!(
                            "capability {} has no host binding",
                            handle
                        ))
                    })?;
                if bytes.len() as u64 > capability.limit_bytes {
                    return Err(ForgeVmError::CapabilityDenied(format!(
                        "capability {} binding exceeds byte limit",
                        handle
                    )));
                }
                if let Some(expected) = capability.content_hash {
                    let actual = sha256(bytes);
                    if actual != expected {
                        return Err(ForgeVmError::CapabilityDenied(format!(
                            "capability {} content hash mismatch",
                            handle
                        )));
                    }
                }
                stack.push(bytes.to_vec());
            }
            ForgeOpcode::HashTop => {
                let bytes = stack.pop().ok_or(ForgeVmError::StackUnderflow("HashTop"))?;
                stack.push(hex(&sha256(&bytes)).into_bytes());
            }
            ForgeOpcode::CsvProfileTiny { max_rows, max_cols } => {
                let bytes = stack
                    .pop()
                    .ok_or(ForgeVmError::StackUnderflow("CsvProfileTiny"))?;
                stack.push(csv_profile_tiny(&bytes, *max_rows, *max_cols).into_bytes());
            }
            ForgeOpcode::ToolCellProjectTiny {
                tool_id,
                command,
                query,
                focus,
                limit,
            } => {
                let graph = stack
                    .pop()
                    .ok_or(ForgeVmError::StackUnderflow("ToolCellProjectTiny.graph"))?;
                let manifest_hash = stack
                    .pop()
                    .ok_or(ForgeVmError::StackUnderflow("ToolCellProjectTiny.manifest"))?;
                let manifest_hash = String::from_utf8_lossy(&manifest_hash);
                stack.push(
                    tool_cell_project_tiny(
                        tool_id,
                        command,
                        query,
                        focus,
                        *limit,
                        manifest_hash.as_ref(),
                        &graph,
                    )
                    .into_bytes(),
                );
            }
            ForgeOpcode::UiIntentTransition { from, intent } => {
                stack.push(ui_projection(from, intent).into_bytes());
            }
            ForgeOpcode::KernelProject { op, payload_json } => {
                stack.push(kernel_projection(op, payload_json).into_bytes());
            }
            ForgeOpcode::JobReadProjection { job_id, max_records } => {
                stack.push(job_projection(job_id, *max_records).into_bytes());
            }
            ForgeOpcode::EmitProjection { label } => {
                let bytes = stack
                    .pop()
                    .ok_or(ForgeVmError::StackUnderflow("EmitProjection"))?;
                let projection = format!(
                    "{{\"label\":\"{}\",\"bytes\":{},\"outputHash\":\"{}\"}}",
                    escape_json(label),
                    bytes.len(),
                    hex(&sha256(&bytes))
                );
                stack.push(projection.into_bytes());
            }
            ForgeOpcode::RawFilesystemProbe(_) | ForgeOpcode::RawNetworkProbe(_) => {
                unreachable!("raw probes must be rejected by verifier")
            }
            ForgeOpcode::End => {
                output = stack.pop().unwrap_or_default();
                ended = true;
                break;
            }
        }
        memory_peak = memory_peak.max(stack_bytes(&stack));
        if memory_peak > config.max_memory_bytes {
            return Err(ForgeVmError::MemoryLimitExceeded {
                peak: memory_peak,
                max: config.max_memory_bytes,
            });
        }
    }

    if !ended {
        return Err(ForgeVmError::MissingEnd);
    }
    if output.len() as u64 > config.max_output_bytes {
        return Err(ForgeVmError::OutputLimitExceeded {
            bytes: output.len() as u64,
            max: config.max_output_bytes,
        });
    }
    memory_peak = memory_peak.max(output.len() as u64);

    let proof = build_proof(program, &report, &output, fuel_used, memory_peak, &config.backend);
    Ok(ForgeVmOutput {
        status: ForgeVmStatus::Ok,
        preview: preview(&output, 160),
        bytes: output,
        fuel_used,
        memory_peak,
        proof,
    })
}

pub fn build_proof(
    program: &ForgeBytecodeProgram,
    report: &ForgeVerifierReport,
    output: &[u8],
    fuel_used: u64,
    memory_peak: u64,
    backend: &str,
) -> ForgeRunProof {
    let encoded = encode_program(program);
    let program_hash = report.program_hash.clone();
    let bytecode_hash = hex(&sha256(&encoded));
    let input_hash = input_hash(program);
    let output_hash = hex(&sha256(output));
    let capability_hash = capability_hash(&program.capabilities);
    let hostcall_hash = hostcall_hash(&program.hostcalls);
    let deterministic_replay_hash = stable_hash(&[
        "fbc-replay-v0",
        &program_hash,
        &input_hash,
        &output_hash,
        &fuel_used.to_string(),
        &memory_peak.to_string(),
        backend,
    ]);
    let proof_hash = stable_hash(&[
        "fbc-proof-v0",
        &program_hash,
        &bytecode_hash,
        &report.verifier_hash,
        &input_hash,
        &output_hash,
        &capability_hash,
        &hostcall_hash,
        &fuel_used.to_string(),
        &memory_peak.to_string(),
        backend,
        &deterministic_replay_hash,
    ]);
    ForgeRunProof {
        program_hash,
        bytecode_hash,
        verifier_hash: report.verifier_hash.clone(),
        input_hash,
        output_hash,
        capability_hash,
        hostcall_hash,
        fuel_used,
        memory_peak,
        backend: backend.to_string(),
        deterministic_replay_hash,
        proof_hash,
    }
}

pub fn build_denial_proof(
    program: &ForgeBytecodeProgram,
    report: &ForgeVerifierReport,
    backend: &str,
) -> ForgeRunProof {
    let output = report.errors.join("|");
    let output_hash = hex(&sha256(output.as_bytes()));
    let program_hash = report.program_hash.clone();
    let bytecode_hash = hex(&sha256(&encode_program(program)));
    let input_hash = input_hash(program);
    let capability_hash = capability_hash(&program.capabilities);
    let hostcall_hash = hostcall_hash(&program.hostcalls);
    let deterministic_replay_hash = stable_hash(&[
        "fbc-denial-replay-v0",
        &program_hash,
        &input_hash,
        &output_hash,
        backend,
    ]);
    let proof_hash = stable_hash(&[
        "fbc-denial-proof-v0",
        &program_hash,
        &bytecode_hash,
        &report.verifier_hash,
        &input_hash,
        &output_hash,
        &capability_hash,
        &hostcall_hash,
        backend,
        &deterministic_replay_hash,
    ]);
    ForgeRunProof {
        program_hash,
        bytecode_hash,
        verifier_hash: report.verifier_hash.clone(),
        input_hash,
        output_hash,
        capability_hash,
        hostcall_hash,
        fuel_used: 0,
        memory_peak: 0,
        backend: backend.to_string(),
        deterministic_replay_hash,
        proof_hash,
    }
}

pub fn build_runtime_error_proof(
    program: &ForgeBytecodeProgram,
    report: &ForgeVerifierReport,
    error: &ForgeVmError,
    backend: &str,
) -> ForgeRunProof {
    let output = format!("{error:?}");
    let output_hash = hex(&sha256(output.as_bytes()));
    let program_hash = report.program_hash.clone();
    let bytecode_hash = hex(&sha256(&encode_program(program)));
    let input_hash = input_hash(program);
    let capability_hash = capability_hash(&program.capabilities);
    let hostcall_hash = hostcall_hash(&program.hostcalls);
    let deterministic_replay_hash = stable_hash(&[
        "fbc-runtime-error-replay-v0",
        &program_hash,
        &input_hash,
        &output_hash,
        backend,
    ]);
    let proof_hash = stable_hash(&[
        "fbc-runtime-error-proof-v0",
        &program_hash,
        &bytecode_hash,
        &report.verifier_hash,
        &input_hash,
        &output_hash,
        &capability_hash,
        &hostcall_hash,
        backend,
        &deterministic_replay_hash,
    ]);
    ForgeRunProof {
        program_hash,
        bytecode_hash,
        verifier_hash: report.verifier_hash.clone(),
        input_hash,
        output_hash,
        capability_hash,
        hostcall_hash,
        fuel_used: 0,
        memory_peak: 0,
        backend: backend.to_string(),
        deterministic_replay_hash,
        proof_hash,
    }
}

pub fn proof_projection_json(
    proof: &ForgeRunProof,
    verifier: &ForgeVerifierReport,
    optimizer: Option<&ForgeOptimizerReport>,
    backend: Option<&ForgeBackendSelection>,
) -> String {
    let optimizer_json = optimizer
        .map(|report| {
            format!(
                "{{\"changed\":{},\"fuelBefore\":{},\"fuelAfter\":{},\"fusedHashOps\":{},\"fusedCapabilityHashOps\":{},\"optimizerHash\":\"{}\"}}",
                report.changed,
                report.fuel_before,
                report.fuel_after,
                report.fused_hash_ops,
                report.fused_capability_hash_ops,
                report.optimizer_hash
            )
        })
        .unwrap_or_else(|| "null".to_string());
    let backend_json = backend
        .map(|selection| {
            format!(
                "{{\"requested\":\"{}\",\"selected\":\"{}\",\"selectorHash\":\"{}\",\"reason\":\"{}\"}}",
                escape_json(&selection.requested),
                escape_json(&selection.selected),
                escape_json(&selection.selector_hash),
                escape_json(&selection.reason)
            )
        })
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"kind\":\"forge_fbc_proof_projection_v0\",\"programHash\":\"{}\",\"bytecodeHash\":\"{}\",\"verifierHash\":\"{}\",\"inputHash\":\"{}\",\"outputHash\":\"{}\",\"capabilityHash\":\"{}\",\"hostcallHash\":\"{}\",\"fuelUsed\":{},\"memoryPeak\":{},\"backend\":\"{}\",\"deterministicReplayHash\":\"{}\",\"proofHash\":\"{}\",\"verifierStatus\":\"{}\",\"optimizer\":{},\"backendSelection\":{}}}",
        proof.program_hash,
        proof.bytecode_hash,
        proof.verifier_hash,
        proof.input_hash,
        proof.output_hash,
        proof.capability_hash,
        proof.hostcall_hash,
        proof.fuel_used,
        proof.memory_peak,
        escape_json(&proof.backend),
        proof.deterministic_replay_hash,
        proof.proof_hash,
        if verifier.ok { "ok" } else { "denied" },
        optimizer_json,
        backend_json
    )
}

pub fn proof_ledger_entry(
    sequence: u64,
    status: &str,
    proof: &ForgeRunProof,
) -> ForgeProofLedgerEntry {
    let ledger_hash = stable_hash(&[
        "forge-fbc-proof-ledger-entry-v0",
        &sequence.to_string(),
        status,
        &proof.program_hash,
        &proof.proof_hash,
        &proof.verifier_hash,
        &proof.capability_hash,
        &proof.backend,
        &proof.fuel_used.to_string(),
        &proof.memory_peak.to_string(),
    ]);
    ForgeProofLedgerEntry {
        sequence,
        status: status.to_string(),
        program_hash: proof.program_hash.clone(),
        proof_hash: proof.proof_hash.clone(),
        verifier_hash: proof.verifier_hash.clone(),
        capability_hash: proof.capability_hash.clone(),
        backend: proof.backend.clone(),
        fuel_used: proof.fuel_used,
        memory_peak: proof.memory_peak,
        ledger_hash,
    }
}

pub fn proof_ledger_projection_json(entry: &ForgeProofLedgerEntry) -> String {
    format!(
        "{{\"kind\":\"forge_fbc_proof_ledger_entry_v0\",\"sequence\":{},\"status\":\"{}\",\"programHash\":\"{}\",\"proofHash\":\"{}\",\"verifierHash\":\"{}\",\"capabilityHash\":\"{}\",\"backend\":\"{}\",\"fuelUsed\":{},\"memoryPeak\":{},\"ledgerHash\":\"{}\"}}",
        entry.sequence,
        escape_json(&entry.status),
        entry.program_hash,
        entry.proof_hash,
        entry.verifier_hash,
        entry.capability_hash,
        escape_json(&entry.backend),
        entry.fuel_used,
        entry.memory_peak,
        entry.ledger_hash
    )
}

pub fn tool_cell_output_artifact_json(
    record: &ForgeToolCellBatchRecord,
    graph_hash: &str,
    registry_hash: &str,
    ledger_root_hash: &str,
) -> String {
    let projection = if record.projection_json.trim().is_empty() {
        "{}"
    } else {
        record.projection_json.as_str()
    };
    format!(
        "{{\"toolId\":\"{}\",\"command\":\"{}\",\"status\":\"{}\",\"proofHash\":\"{}\",\"graphProofHash\":\"{}\",\"summary\":{{\"engine\":\"forge_bytecode_v0\",\"registryHash\":\"{}\",\"programHash\":\"{}\",\"ledgerHash\":\"{}\",\"ledgerRootHash\":\"{}\",\"outputHash\":\"{}\",\"selectedEvidenceCount\":{},\"rankedActionCount\":{},\"error\":\"{}\"}},\"fbcProjection\":{},\"evidenceRefs\":{},\"rankedActions\":{}}}",
        escape_json(&record.tool_id),
        escape_json(&record.command),
        escape_json(&record.status),
        record.proof_hash,
        graph_hash,
        registry_hash,
        record.program_hash,
        record.ledger_hash,
        ledger_root_hash,
        record.output_hash,
        record.selected_evidence_count,
        record.ranked_action_count,
        escape_json(&record.error),
        projection,
        json_array_from_projection(projection, "evidenceRefs"),
        json_array_from_projection(projection, "rankedActions")
    )
}

pub fn execute_tool_cell_batch(
    cells: &[ForgeToolCellSpec],
    graph_jsonl: &[u8],
    config: &ForgeVmConfig,
) -> ForgeToolCellBatchOutput {
    let graph_hash = hex(&sha256(graph_jsonl));
    let mut records = Vec::with_capacity(cells.len());
    let mut ledger_hashes = Vec::with_capacity(cells.len());

    for (idx, cell) in cells.iter().enumerate() {
        let bundle = compile_tool_cell_bundle_with_graph(cell, graph_jsonl);
        let sequence = idx as u64 + 1;
        match execute_program_pipeline_with_context(&bundle.program, config, &bundle.host_context) {
            Ok(output) => {
                let projection = String::from_utf8_lossy(&output.vm_output.bytes);
                let selected_evidence_count =
                    json_usize_field(&projection, "selectedEvidenceCount").unwrap_or(0);
                let ranked_action_count =
                    json_usize_field(&projection, "rankedActionCount").unwrap_or(0);
                let projection_json = projection.to_string();
                let ledger = proof_ledger_entry(sequence, "ok", &output.vm_output.proof);
                ledger_hashes.push(ledger.ledger_hash.clone());
                records.push(ForgeToolCellBatchRecord {
                    tool_id: cell.id.clone(),
                    command: cell.command.clone(),
                    status: "ok".to_string(),
                    program_hash: output.vm_output.proof.program_hash,
                    proof_hash: output.vm_output.proof.proof_hash,
                    ledger_hash: ledger.ledger_hash,
                    output_hash: output.vm_output.proof.output_hash,
                    selected_evidence_count,
                    ranked_action_count,
                    projection_json,
                    error: String::new(),
                });
            }
            Err(ForgeVmError::VerifierDenied(report)) => {
                let proof = build_denial_proof(&bundle.program, &report, &config.backend);
                let ledger = proof_ledger_entry(sequence, "denied", &proof);
                ledger_hashes.push(ledger.ledger_hash.clone());
                records.push(ForgeToolCellBatchRecord {
                    tool_id: cell.id.clone(),
                    command: cell.command.clone(),
                    status: "denied".to_string(),
                    program_hash: proof.program_hash,
                    proof_hash: proof.proof_hash,
                    ledger_hash: ledger.ledger_hash,
                    output_hash: proof.output_hash,
                    selected_evidence_count: 0,
                    ranked_action_count: 0,
                    projection_json: String::new(),
                    error: report.errors.join("|"),
                });
            }
            Err(error) => {
                let report = verify_program(&bundle.program, config);
                let proof = build_runtime_error_proof(&bundle.program, &report, &error, &config.backend);
                let ledger = proof_ledger_entry(sequence, "runtime_error", &proof);
                ledger_hashes.push(ledger.ledger_hash.clone());
                records.push(ForgeToolCellBatchRecord {
                    tool_id: cell.id.clone(),
                    command: cell.command.clone(),
                    status: "runtime_error".to_string(),
                    program_hash: proof.program_hash,
                    proof_hash: proof.proof_hash,
                    ledger_hash: ledger.ledger_hash,
                    output_hash: proof.output_hash,
                    selected_evidence_count: 0,
                    ranked_action_count: 0,
                    projection_json: String::new(),
                    error: format!("{error:?}"),
                });
            }
        }
    }

    let ok_count = records.iter().filter(|record| record.status == "ok").count();
    let denied_count = records
        .iter()
        .filter(|record| record.status != "ok")
        .count();
    let ledger_root_hash = ledger_root_hash(&ledger_hashes);
    let projection_json =
        tool_cell_batch_projection_json(&graph_hash, records.as_slice(), &ledger_root_hash);
    ForgeToolCellBatchOutput {
        graph_hash,
        tool_count: cells.len(),
        ok_count,
        denied_count,
        records,
        ledger_root_hash,
        projection_json,
    }
}

pub fn parse_tool_cell_registry_v0(json: &str) -> Result<ForgeToolCellRegistry, ForgeVmError> {
    let schema_version = json_number_field(json, "schemaVersion")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let defaults = json_object_field(json, "defaults")
        .ok_or_else(|| ForgeVmError::Parse("missing defaults object".to_string()))?;
    let default_engine = json_field(defaults, "engine").unwrap_or_default();
    let permissions = json_string_array_field(defaults, "permissions").unwrap_or_default();
    let denied = json_string_array_field(defaults, "denied").unwrap_or_default();
    let input_schema = json_object_field(defaults, "inputSchema").unwrap_or("{}");
    let output_schema = json_object_field(defaults, "outputSchema").unwrap_or("{}");
    let input_schema_hash = hex(&sha256(input_schema.as_bytes()));
    let output_schema_hash = hex(&sha256(output_schema.as_bytes()));
    let tool_cells = json_array_field(json, "toolCells")
        .ok_or_else(|| ForgeVmError::Parse("missing toolCells array".to_string()))?;
    let mut cells = Vec::new();
    for object in split_top_level_objects(tool_cells) {
        let id = json_field(object, "id")
            .ok_or_else(|| ForgeVmError::Parse("toolCell missing id".to_string()))?;
        let query = json_field(object, "query")
            .ok_or_else(|| ForgeVmError::Parse(format!("toolCell {id} missing query")))?;
        let focus = json_string_array_field(object, "focus")
            .ok_or_else(|| ForgeVmError::Parse(format!("toolCell {id} missing focus")))?;
        cells.push(ForgeToolCellSpec {
            command: format!("/{}_", id.replace('-', "_")),
            id,
            query,
            focus,
            permissions: permissions.clone(),
            denied: denied.clone(),
            input_schema_hash: input_schema_hash.clone(),
            output_schema_hash: output_schema_hash.clone(),
        });
    }
    let registry_hash = hex(&sha256(json.as_bytes()));
    Ok(ForgeToolCellRegistry {
        schema_version,
        default_engine,
        registry_hash,
        input_schema_hash,
        output_schema_hash,
        permissions,
        denied,
        cells,
    })
}

pub fn execute_tool_cell_registry_batch(
    registry_json: &str,
    graph_jsonl: &[u8],
    config: &ForgeVmConfig,
) -> Result<ForgeToolCellBatchOutput, ForgeVmError> {
    let registry = parse_tool_cell_registry_v0(registry_json)?;
    Ok(execute_tool_cell_batch(&registry.cells, graph_jsonl, config))
}

pub fn parse_app_section_registry_v0(json: &str) -> Result<ForgeAppRegistry, ForgeVmError> {
    let sections = json_array_field(json, "sections")
        .ok_or_else(|| ForgeVmError::Parse("missing sections array".to_string()))?;
    let sensitive = json_array_field(json, "sensitiveCommands").unwrap_or("[]");
    let mut cells = Vec::new();
    let mut graph_lines = Vec::new();
    let mut section_count = 0_usize;
    let mut sensitive_command_count = 0_usize;

    for section in split_top_level_objects(sections) {
        let id = json_field(section, "id")
            .ok_or_else(|| ForgeVmError::Parse("section missing id".to_string()))?;
        let owner = json_field(section, "owner").unwrap_or_else(|| id.clone());
        let lifecycle = json_field(section, "lifecycle").unwrap_or_else(|| "section".to_string());
        let files = json_string_array_field(section, "files").unwrap_or_default();
        let native_present = json_string_array_field(section, "nativePresentCommands").unwrap_or_default();
        let native_hide = json_string_array_field(section, "nativeHideCommands").unwrap_or_default();
        let mut focus = vec!["section".to_string(), "ui_projection".to_string(), lifecycle.clone()];
        if !native_present.is_empty() || !native_hide.is_empty() {
            focus.push("native_bridge".to_string());
        }
        cells.push(ForgeToolCellSpec {
            id: format!("app-section-{id}"),
            command: format!("/app_section_{}_", id.replace('-', "_")),
            query: format!("app_section_projection:{id}"),
            focus,
            permissions: vec![
                "read:section_registry".to_string(),
                "read:forge_kernel_projection".to_string(),
                "write:ui_projection".to_string(),
            ],
            denied: default_app_denied(),
            input_schema_hash: hex(&sha256(b"forge_app_section_input_v0")),
            output_schema_hash: hex(&sha256(b"forge_app_section_output_v0")),
        });
        graph_lines.push(format!(
            "{{\"kind\":\"dataflow_node\",\"id\":\"app-section-{}\",\"type\":\"section\",\"label\":\"{}\",\"recordHash\":\"{}\",\"confidence\":0.93}}",
            escape_json(&id),
            escape_json(&owner),
            hex(&sha256(section.as_bytes()))
        ));
        graph_lines.push(format!(
            "{{\"kind\":\"dataflow_node\",\"id\":\"app-lifecycle-{}\",\"type\":\"{}\",\"label\":\"{}\",\"recordHash\":\"{}\",\"confidence\":0.88}}",
            escape_json(&id),
            escape_json(&lifecycle),
            escape_json(&lifecycle),
            hex(&sha256(lifecycle.as_bytes()))
        ));
        for file in files.iter().take(16) {
            graph_lines.push(format!(
                "{{\"kind\":\"dataflow_edge\",\"id\":\"app-file-{}-{}\",\"from\":\"app-section-{}\",\"to\":\"{}\",\"relation\":\"owns_file\",\"recordHash\":\"{}\",\"confidence\":0.86}}",
                escape_json(&id),
                hex(&sha256(file.as_bytes())).chars().take(12).collect::<String>(),
                escape_json(&id),
                escape_json(file),
                hex(&sha256(file.as_bytes()))
            ));
        }
        section_count += 1;
    }

    for command in split_top_level_objects(sensitive) {
        let command_name = json_field(command, "command")
            .ok_or_else(|| ForgeVmError::Parse("sensitive command missing command".to_string()))?;
        let owner = json_field(command, "owner").unwrap_or_else(|| "shell".to_string());
        cells.push(ForgeToolCellSpec {
            id: format!("app-command-{command_name}"),
            command: format!("/app_command_{}_", command_name.replace('-', "_")),
            query: format!("app_sensitive_command:{command_name}"),
            focus: vec![
                "native_bridge".to_string(),
                "sensitive_command".to_string(),
                owner.clone(),
            ],
            permissions: vec![
                "read:section_registry".to_string(),
                "read:bridge_contract".to_string(),
                "write:proof_ledger".to_string(),
            ],
            denied: default_app_denied(),
            input_schema_hash: hex(&sha256(b"forge_app_command_input_v0")),
            output_schema_hash: hex(&sha256(b"forge_app_command_output_v0")),
        });
        graph_lines.push(format!(
            "{{\"kind\":\"dataflow_node\",\"id\":\"app-command-{}\",\"type\":\"sensitive_command\",\"label\":\"{}\",\"recordHash\":\"{}\",\"confidence\":0.97}}",
            escape_json(&command_name),
            escape_json(&command_name),
            hex(&sha256(command.as_bytes()))
        ));
        graph_lines.push(format!(
            "{{\"kind\":\"dataflow_edge\",\"id\":\"app-command-owner-{}\",\"from\":\"app-command-{}\",\"to\":\"app-section-{}\",\"relation\":\"owned_by_section\",\"recordHash\":\"{}\",\"confidence\":0.91}}",
            escape_json(&command_name),
            escape_json(&command_name),
            escape_json(&owner),
            hex(&sha256(format!("{command_name}:{owner}").as_bytes()))
        ));
        sensitive_command_count += 1;
    }

    Ok(ForgeAppRegistry {
        registry_hash: hex(&sha256(json.as_bytes())),
        section_count,
        sensitive_command_count,
        cells,
        graph_jsonl: graph_lines.join("\n").into_bytes(),
    })
}

pub fn execute_app_registry_batch(
    section_ownership_json: &str,
    config: &ForgeVmConfig,
) -> Result<ForgeToolCellBatchOutput, ForgeVmError> {
    let registry = parse_app_section_registry_v0(section_ownership_json)?;
    Ok(execute_tool_cell_batch(&registry.cells, &registry.graph_jsonl, config))
}

pub fn encode_program(program: &ForgeBytecodeProgram) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("forge-fbc-program-v0\n");
    out.push_str(&format!("name={}\n", escape_line(&program.name)));
    out.push_str(&format!("version={}\n", program.version));
    out.push_str(&format!("deterministic={}\n", program.deterministic));
    out.push_str(&format!("schema={}\n", escape_line(&program.expected_output_schema)));
    let mut capabilities = program.capabilities.clone();
    capabilities.sort_by_key(|cap| cap.summary());
    for cap in &capabilities {
        out.push_str("cap=");
        out.push_str(&cap.summary());
        out.push('\n');
    }
    let mut hostcalls = program.hostcalls.clone();
    hostcalls.sort();
    for hostcall in hostcalls {
        out.push_str(&format!("hostcall={}\n", hostcall_name(hostcall)));
    }
    for op in &program.ops {
        out.push_str("op=");
        encode_opcode(&mut out, op);
        out.push('\n');
    }
    out.into_bytes()
}

fn parse_opcode(value: &str, line_no: usize) -> Result<ForgeOpcode, ForgeVmError> {
    if let Some(text) = value.strip_prefix("push_text:") {
        Ok(ForgeOpcode::PushText(text.replace("\\n", "\n")))
    } else if let Some(hex_bytes) = value.strip_prefix("push_hex:") {
        Ok(ForgeOpcode::PushBytes(parse_hex(hex_bytes).map_err(|err| {
            ForgeVmError::Parse(format!("line {line_no}: {err}"))
        })?))
    } else if let Some(handle) = value.strip_prefix("push_cap:") {
        Ok(ForgeOpcode::PushCapability(
            parse_hash_hex(handle)
                .map_err(|err| ForgeVmError::Parse(format!("line {line_no}: {err}")))?,
        ))
    } else if value == "read_capability" {
        Ok(ForgeOpcode::ReadCapability)
    } else if value == "hash_top" {
        Ok(ForgeOpcode::HashTop)
    } else if let Some(rest) = value.strip_prefix("csv_profile_tiny:") {
        let mut parts = rest.split(':');
        let max_rows = parts
            .next()
            .and_then(|item| item.parse::<u32>().ok())
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: bad max_rows")))?;
        let max_cols = parts
            .next()
            .and_then(|item| item.parse::<u16>().ok())
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: bad max_cols")))?;
        Ok(ForgeOpcode::CsvProfileTiny { max_rows, max_cols })
    } else if let Some(rest) = value.strip_prefix("toolcell_project_tiny:") {
        let mut parts = rest.splitn(5, ':');
        let tool_id = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing tool_id")))?
            .to_string();
        let command = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing command")))?
            .to_string();
        let query = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing query")))?
            .to_string();
        let limit = parts
            .next()
            .and_then(|item| item.parse::<u16>().ok())
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: bad limit")))?;
        let focus = parts
            .next()
            .unwrap_or("")
            .split(',')
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect();
        Ok(ForgeOpcode::ToolCellProjectTiny {
            tool_id,
            command,
            query,
            focus,
            limit,
        })
    } else if let Some(rest) = value.strip_prefix("ui_intent_transition:") {
        let mut parts = rest.splitn(2, ':');
        let from = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing from")))?;
        let intent = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing intent")))?;
        Ok(ForgeOpcode::UiIntentTransition {
            from: from.to_string(),
            intent: intent.to_string(),
        })
    } else if let Some(rest) = value.strip_prefix("kernel_project:") {
        let mut parts = rest.splitn(2, ':');
        let op = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing kernel op")))?;
        let payload_json = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing kernel payload")))?;
        Ok(ForgeOpcode::KernelProject {
            op: op.to_string(),
            payload_json: payload_json.replace("\\n", "\n"),
        })
    } else if let Some(rest) = value.strip_prefix("job_read_projection:") {
        let mut parts = rest.splitn(2, ':');
        let job_id = parts
            .next()
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: missing job id")))?;
        let max_records = parts
            .next()
            .and_then(|item| item.parse::<u16>().ok())
            .ok_or_else(|| ForgeVmError::Parse(format!("line {line_no}: bad max_records")))?;
        Ok(ForgeOpcode::JobReadProjection {
            job_id: job_id.to_string(),
            max_records,
        })
    } else if let Some(label) = value.strip_prefix("emit_projection:") {
        Ok(ForgeOpcode::EmitProjection {
            label: label.to_string(),
        })
    } else if let Some(path) = value.strip_prefix("raw_fs:") {
        Ok(ForgeOpcode::RawFilesystemProbe(path.to_string()))
    } else if let Some(url) = value.strip_prefix("raw_network:") {
        Ok(ForgeOpcode::RawNetworkProbe(url.to_string()))
    } else if value == "end" {
        Ok(ForgeOpcode::End)
    } else {
        Err(ForgeVmError::Parse(format!(
            "line {line_no}: unknown opcode {value}"
        )))
    }
}

fn required_hostcalls(op: &ForgeOpcode) -> Vec<ForgeHostCall> {
    match op {
        ForgeOpcode::ReadCapability => vec![ForgeHostCall::ReadCapability],
        ForgeOpcode::HashTop => vec![ForgeHostCall::HashBytes],
        ForgeOpcode::CsvProfileTiny { .. } => vec![ForgeHostCall::CsvProfileTiny],
        ForgeOpcode::ToolCellProjectTiny { .. } => vec![ForgeHostCall::ReadCapability],
        ForgeOpcode::KernelProject { .. } => vec![ForgeHostCall::KernelProject],
        ForgeOpcode::JobReadProjection { .. } => vec![ForgeHostCall::JobReadProjection],
        ForgeOpcode::UiIntentTransition { .. } | ForgeOpcode::EmitProjection { .. } => {
            vec![ForgeHostCall::UiProjectEvent]
        }
        ForgeOpcode::RawFilesystemProbe(_) => vec![ForgeHostCall::RawFilesystem],
        ForgeOpcode::RawNetworkProbe(_) => vec![ForgeHostCall::RawNetwork],
        _ => Vec::new(),
    }
}

fn verifier_hash(
    program: &ForgeBytecodeProgram,
    config: &ForgeVmConfig,
    errors: &[String],
    warnings: &[String],
) -> String {
    let mut h = Sha256::new();
    h.update(FBC_VERIFIER_VERSION.as_bytes());
    h.update(b"\n");
    h.update(hash_program(program).as_bytes());
    h.update(b"\n");
    h.update(config.max_fuel.to_le_bytes());
    h.update(config.max_memory_bytes.to_le_bytes());
    h.update(config.max_input_bytes.to_le_bytes());
    h.update(config.max_output_bytes.to_le_bytes());
    h.update(config.backend.as_bytes());
    for error in errors {
        h.update(b"\nerror=");
        h.update(error.as_bytes());
    }
    for warning in warnings {
        h.update(b"\nwarning=");
        h.update(warning.as_bytes());
    }
    hex(&h.finalize())
}

fn input_hash(program: &ForgeBytecodeProgram) -> String {
    let mut h = Sha256::new();
    h.update(b"forge-fbc-inputs-v0\n");
    for op in &program.ops {
        match op {
            ForgeOpcode::PushBytes(bytes) => {
                h.update(b"push-bytes:");
                h.update((bytes.len() as u64).to_le_bytes());
                h.update(sha256(bytes));
            }
            ForgeOpcode::PushText(text) => {
                h.update(b"push-text:");
                h.update((text.len() as u64).to_le_bytes());
                h.update(sha256(text.as_bytes()));
            }
            ForgeOpcode::PushCapability(sealed_hash) => {
                h.update(b"push-capability:");
                h.update(sealed_hash);
            }
            _ => {}
        }
    }
    hex(&h.finalize())
}

fn capability_hash(capabilities: &[ForgeCapability]) -> String {
    let mut items = capabilities
        .iter()
        .map(ForgeCapability::summary)
        .collect::<Vec<_>>();
    items.sort();
    stable_hash_with_domain("forge-fbc-capabilities-v0", &items)
}

fn hostcall_hash(hostcalls: &[ForgeHostCall]) -> String {
    let mut names = hostcalls
        .iter()
        .map(|call| hostcall_name(*call).to_string())
        .collect::<Vec<_>>();
    names.sort();
    stable_hash_with_domain("forge-fbc-hostcalls-v0", &names)
}

fn verified_capability_bytes<'a>(
    program: &ForgeBytecodeProgram,
    host_context: &'a ForgeHostContext,
    sealed_hash: &[u8; 32],
) -> Option<&'a [u8]> {
    let capability = program
        .capabilities
        .iter()
        .find(|capability| capability.sealed_hash == *sealed_hash)?;
    let bytes = host_context.read_binding(sealed_hash)?;
    if bytes.len() as u64 > capability.limit_bytes {
        return None;
    }
    if let Some(expected) = capability.content_hash {
        if sha256(bytes) != expected {
            return None;
        }
    }
    Some(bytes)
}

fn stable_hash(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part.as_bytes());
    }
    hex(&h.finalize())
}

fn stable_hash_with_domain(domain: &str, parts: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    for part in parts {
        h.update(b"\n");
        h.update(part.as_bytes());
    }
    hex(&h.finalize())
}

fn csv_profile_tiny(bytes: &[u8], max_rows: u32, max_cols: u16) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if rows.len() >= max_rows as usize {
            break;
        }
        let cols = line
            .split(',')
            .take(max_cols as usize)
            .map(|col| col.trim().to_string())
            .collect::<Vec<_>>();
        rows.push(cols);
    }
    let row_count = rows.len();
    let col_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let headers = rows
        .first()
        .map(|row| row.join("|"))
        .unwrap_or_else(String::new);
    let mut numeric_cells = 0_usize;
    let mut empty_cells = 0_usize;
    for row in rows.iter().skip(1) {
        for cell in row {
            if cell.is_empty() {
                empty_cells += 1;
            } else if cell.parse::<f64>().is_ok() {
                numeric_cells += 1;
            }
        }
    }
    format!(
        "csv_profile_tiny_v0\nrows={row_count}\ncols={col_count}\nheaders={headers}\nnumericCells={numeric_cells}\nemptyCells={empty_cells}\ninputHash={}",
        hex(&sha256(bytes))
    )
}

fn tool_cell_manifest(cell: &ForgeToolCellSpec) -> String {
    let mut focus = cell.focus.clone();
    focus.sort();
    let mut permissions = cell.permissions.clone();
    permissions.sort();
    let mut denied = cell.denied.clone();
    denied.sort();
    format!(
        "toolcell_v0\nid={}\ncommand={}\nquery={}\nfocus={}\npermissions={}\ndenied={}\ninputSchemaHash={}\noutputSchemaHash={}",
        escape_line(&cell.id),
        escape_line(&cell.command),
        escape_line(&cell.query),
        focus.join(","),
        permissions.join(","),
        denied.join(","),
        escape_line(&cell.input_schema_hash),
        escape_line(&cell.output_schema_hash)
    )
}

fn tool_cell_project_tiny(
    tool_id: &str,
    command: &str,
    query: &str,
    focus: &[String],
    limit: u16,
    manifest_hash: &str,
    graph_jsonl: &[u8],
) -> String {
    let graph = String::from_utf8_lossy(graph_jsonl);
    let mut evidence = Vec::new();
    let mut ranked = Vec::new();
    let focus = focus.iter().map(String::as_str).collect::<Vec<_>>();
    let limit = limit as usize;

    for line in graph.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if evidence.len() >= limit {
            break;
        }
        let kind = json_field(line, "kind").unwrap_or_default();
        if kind != "dataflow_node" && kind != "dataflow_edge" {
            continue;
        }
        let record_type = json_field(line, "type")
            .or_else(|| json_field(line, "relation"))
            .unwrap_or_default();
        if kind == "dataflow_node" && !focus.iter().any(|item| *item == record_type) {
            continue;
        }
        let id = json_field(line, "id")
            .or_else(|| json_field(line, "from"))
            .unwrap_or_else(|| format!("record-{}", evidence.len() + 1));
        let record_hash = json_field(line, "recordHash").unwrap_or_else(|| hex(&sha256(line.as_bytes())));
        let confidence = json_number_field(line, "confidence").unwrap_or_else(|| "0".to_string());
        evidence.push(format!(
            "{{\"id\":\"{}\",\"type\":\"{}\",\"recordHash\":\"{}\",\"confidence\":{}}}",
            escape_json(&id),
            escape_json(&record_type),
            escape_json(&record_hash),
            confidence
        ));
        if ranked.len() < 12
            && (record_type == "action" || record_type == "score" || record_type == "intelPack")
        {
            let label = json_field(line, "label").unwrap_or_else(|| id.clone());
            ranked.push(format!(
                "{{\"rank\":{},\"id\":\"{}\",\"label\":\"{}\",\"confidence\":{},\"reason\":\"{}:{}\"}}",
                ranked.len() + 1,
                escape_json(&id),
                escape_json(&label),
                confidence,
                escape_json(query),
                escape_json(&record_type)
            ));
        }
    }

    let evidence_json = evidence.join(",");
    let ranked_json = ranked.join(",");
    let projection_seed = format!(
        "{tool_id}|{command}|{query}|{manifest_hash}|{}|{}|{}",
        hex(&sha256(graph_jsonl)),
        evidence_json,
        ranked_json
    );
    format!(
        "{{\"kind\":\"forge_tool_cell_projection_v0\",\"toolId\":\"{}\",\"command\":\"{}\",\"query\":\"{}\",\"manifestHash\":\"{}\",\"graphHash\":\"{}\",\"selectedEvidenceCount\":{},\"rankedActionCount\":{},\"evidenceRefs\":[{}],\"rankedActions\":[{}],\"projectionHash\":\"{}\"}}",
        escape_json(tool_id),
        escape_json(command),
        escape_json(query),
        escape_json(manifest_hash),
        hex(&sha256(graph_jsonl)),
        evidence.len(),
        ranked.len(),
        evidence_json,
        ranked_json,
        hex(&sha256(projection_seed.as_bytes()))
    )
}

fn capability_from_permission(permission: &str) -> ForgeCapability {
    let kind = if permission == "filesystem:raw_client_files" || permission.starts_with("raw_fs:") {
        ForgeCapabilityKind::RawFilesystem
    } else if permission == "network:direct" || permission.starts_with("raw_network:") {
        ForgeCapabilityKind::RawNetwork
    } else if permission == "secret:read" || permission.starts_with("secret:") {
        ForgeCapabilityKind::Secret
    } else if permission.starts_with("read:") {
        ForgeCapabilityKind::MemoryScope
    } else if permission.starts_with("write:") {
        ForgeCapabilityKind::ArtifactHash
    } else {
        ForgeCapabilityKind::EventSchema
    };
    let scope = permission
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == ':' || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    ForgeCapability::sealed(kind, scope, Some(permission.as_bytes()), 64 * 1024)
}

fn default_app_denied() -> Vec<String> {
    vec![
        "network:direct".to_string(),
        "filesystem:raw_client_files".to_string(),
        "secret:read".to_string(),
        "shell:direct".to_string(),
    ]
}

fn ui_projection(from: &str, intent: &str) -> String {
    let to = match intent {
        "open_real_estate" | "agence_immo" => "real-estate-main",
        "open_webexplorer" | "webexplorer" => "webexplorer",
        "open_trading" | "trading" => "trading",
        "open_banger" | "banger" => "banger",
        _ => from,
    };
    format!(
        "{{\"kind\":\"ui_intent_transition_v0\",\"from\":\"{}\",\"intent\":\"{}\",\"to\":\"{}\",\"action\":\"set_surface_active\"}}",
        escape_json(from),
        escape_json(intent),
        escape_json(to)
    )
}

fn kernel_projection(op: &str, payload_json: &str) -> String {
    format!(
        "{{\"kind\":\"kernel_projection_v0\",\"op\":\"{}\",\"payloadHash\":\"{}\",\"payload\":{}}}",
        escape_json(op),
        hex(&sha256(payload_json.as_bytes())),
        payload_json
    )
}

fn job_projection(job_id: &str, max_records: u16) -> String {
    format!(
        "{{\"kind\":\"job_projection_v0\",\"jobId\":\"{}\",\"maxRecords\":{},\"queryHash\":\"{}\",\"rawDataReturned\":false}}",
        escape_json(job_id),
        max_records,
        hex(&sha256(format!("{job_id}:{max_records}").as_bytes()))
    )
}

fn json_field(line: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = line.find(&marker)?;
    let after_key = &line[start + marker.len()..];
    let colon = after_key.find(':')?;
    let mut value = after_key[colon + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }
    value = &value[1..];
    let mut out = String::new();
    let mut escape = false;
    for ch in value.chars() {
        if escape {
            out.push(ch);
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

fn json_object_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    json_compound_field(text, key, '{', '}')
}

fn json_array_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    json_compound_field(text, key, '[', ']')
}

fn json_array_from_projection(text: &str, key: &str) -> String {
    json_array_field(text, key).unwrap_or("[]").to_string()
}

fn json_compound_field<'a>(
    text: &'a str,
    key: &str,
    open: char,
    close: char,
) -> Option<&'a str> {
    let marker = format!("\"{key}\"");
    let start = text.find(&marker)?;
    let after_key = &text[start + marker.len()..];
    let colon = after_key.find(':')?;
    let value_start = start + marker.len() + colon + 1;
    let relative_open = text[value_start..].find(open)?;
    let open_idx = value_start + relative_open;
    let close_idx = matching_delimiter(text, open_idx, open, close)?;
    Some(&text[open_idx..=close_idx])
}

fn matching_delimiter(text: &str, open_idx: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escape = false;
    for (idx, ch) in text[open_idx..].char_indices() {
        let absolute = open_idx + idx;
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(absolute);
            }
        }
    }
    None
}

fn json_string_array_field(text: &str, key: &str) -> Option<Vec<String>> {
    let array = json_array_field(text, key)?;
    let mut out = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    let mut current = String::new();
    for ch in array.chars() {
        if escape {
            if in_string {
                current.push(ch);
            }
            escape = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                out.push(current.clone());
                current.clear();
                in_string = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_string = true;
        }
    }
    Some(out)
}

fn split_top_level_objects(array: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    let mut depth = 0_i32;
    let mut start = None;
    for (idx, ch) in array.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = start.take() {
                        out.push(&array[start..=idx]);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn json_number_field(line: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = line.find(&marker)?;
    let after_key = &line[start + marker.len()..];
    let colon = after_key.find(':')?;
    let value = after_key[colon + 1..].trim_start();
    let number = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == '-')
        .collect::<String>();
    if number.is_empty() {
        None
    } else {
        Some(number)
    }
}

fn json_usize_field(line: &str, key: &str) -> Option<usize> {
    json_number_field(line, key)?.parse::<usize>().ok()
}

fn ledger_root_hash(ledger_hashes: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(b"forge-fbc-proof-ledger-root-v0\n");
    for (idx, ledger_hash) in ledger_hashes.iter().enumerate() {
        h.update((idx as u64 + 1).to_le_bytes());
        h.update(ledger_hash.as_bytes());
        h.update(b"\n");
    }
    hex(&h.finalize())
}

fn tool_cell_batch_projection_json(
    graph_hash: &str,
    records: &[ForgeToolCellBatchRecord],
    ledger_root_hash: &str,
) -> String {
    let records_json = records
        .iter()
        .map(|record| {
            format!(
                "{{\"toolId\":\"{}\",\"command\":\"{}\",\"status\":\"{}\",\"programHash\":\"{}\",\"proofHash\":\"{}\",\"ledgerHash\":\"{}\",\"outputHash\":\"{}\",\"selectedEvidenceCount\":{},\"rankedActionCount\":{},\"projectionHash\":\"{}\",\"error\":\"{}\"}}",
                escape_json(&record.tool_id),
                escape_json(&record.command),
                escape_json(&record.status),
                record.program_hash,
                record.proof_hash,
                record.ledger_hash,
                record.output_hash,
                record.selected_evidence_count,
                record.ranked_action_count,
                hex(&sha256(record.projection_json.as_bytes())),
                escape_json(&record.error)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let ok_count = records.iter().filter(|record| record.status == "ok").count();
    let denied_count = records.len().saturating_sub(ok_count);
    format!(
        "{{\"kind\":\"forge_fbc_tool_cell_batch_projection_v0\",\"graphHash\":\"{}\",\"toolCount\":{},\"okCount\":{},\"deniedCount\":{},\"ledgerRootHash\":\"{}\",\"records\":[{}]}}",
        graph_hash,
        records.len(),
        ok_count,
        denied_count,
        ledger_root_hash,
        records_json
    )
}

fn stack_bytes(stack: &[Vec<u8>]) -> u64 {
    stack.iter().map(|item| item.len() as u64).sum()
}

fn preview(bytes: &[u8], max: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = text.chars().take(max).collect::<String>();
    if text.chars().count() > max {
        out.push_str("...");
    }
    out
}

fn opcode_name(op: &ForgeOpcode) -> &'static str {
    match op {
        ForgeOpcode::PushBytes(_) => "push_bytes",
        ForgeOpcode::PushText(_) => "push_text",
        ForgeOpcode::PushCapability(_) => "push_capability",
        ForgeOpcode::ReadCapability => "read_capability",
        ForgeOpcode::HashTop => "hash_top",
        ForgeOpcode::CsvProfileTiny { .. } => "csv_profile_tiny",
        ForgeOpcode::ToolCellProjectTiny { .. } => "toolcell_project_tiny",
        ForgeOpcode::KernelProject { .. } => "kernel_project",
        ForgeOpcode::JobReadProjection { .. } => "job_read_projection",
        ForgeOpcode::UiIntentTransition { .. } => "ui_intent_transition",
        ForgeOpcode::EmitProjection { .. } => "emit_projection",
        ForgeOpcode::RawFilesystemProbe(_) => "raw_filesystem_probe",
        ForgeOpcode::RawNetworkProbe(_) => "raw_network_probe",
        ForgeOpcode::End => "end",
    }
}

fn encode_opcode(out: &mut String, op: &ForgeOpcode) {
    match op {
        ForgeOpcode::PushBytes(bytes) => out.push_str(&format!("push_hex:{}", hex(bytes))),
        ForgeOpcode::PushText(text) => {
            out.push_str("push_text:");
            out.push_str(&escape_line(text));
        }
        ForgeOpcode::PushCapability(sealed_hash) => {
            out.push_str("push_cap:");
            out.push_str(&hex(sealed_hash));
        }
        ForgeOpcode::ReadCapability => out.push_str("read_capability"),
        ForgeOpcode::HashTop => out.push_str("hash_top"),
        ForgeOpcode::CsvProfileTiny { max_rows, max_cols } => {
            out.push_str(&format!("csv_profile_tiny:{max_rows}:{max_cols}"));
        }
        ForgeOpcode::ToolCellProjectTiny {
            tool_id,
            command,
            query,
            focus,
            limit,
        } => {
            out.push_str(&format!(
                "toolcell_project_tiny:{}:{}:{}:{}:{}",
                escape_line(tool_id),
                escape_line(command),
                escape_line(query),
                limit,
                focus.join(",")
            ));
        }
        ForgeOpcode::UiIntentTransition { from, intent } => {
            out.push_str(&format!(
                "ui_intent_transition:{}:{}",
                escape_line(from),
                escape_line(intent)
            ));
        }
        ForgeOpcode::KernelProject { op, payload_json } => {
            out.push_str(&format!(
                "kernel_project:{}:{}",
                escape_line(op),
                escape_line(payload_json)
            ));
        }
        ForgeOpcode::JobReadProjection { job_id, max_records } => {
            out.push_str(&format!(
                "job_read_projection:{}:{}",
                escape_line(job_id),
                max_records
            ));
        }
        ForgeOpcode::EmitProjection { label } => {
            out.push_str("emit_projection:");
            out.push_str(&escape_line(label));
        }
        ForgeOpcode::RawFilesystemProbe(path) => {
            out.push_str("raw_fs:");
            out.push_str(&escape_line(path));
        }
        ForgeOpcode::RawNetworkProbe(url) => {
            out.push_str("raw_network:");
            out.push_str(&escape_line(url));
        }
        ForgeOpcode::End => out.push_str("end"),
    }
}

fn parse_cap_kind(value: &str) -> Option<ForgeCapabilityKind> {
    match value {
        "file_hash" | "cap:file:hash" => Some(ForgeCapabilityKind::FileHash),
        "artifact_hash" | "cap:artifact:hash" => Some(ForgeCapabilityKind::ArtifactHash),
        "memory_scope" | "cap:memory:scope" => Some(ForgeCapabilityKind::MemoryScope),
        "network_source" | "cap:network:source_id" => Some(ForgeCapabilityKind::NetworkSource),
        "event_schema" | "cap:event:schema" => Some(ForgeCapabilityKind::EventSchema),
        "ui_projection" | "cap:ui:projection" => Some(ForgeCapabilityKind::UiProjection),
        "gpu_budget" | "cap:gpu:budget" => Some(ForgeCapabilityKind::GpuBudget),
        "model_provider_scope" | "cap:model:provider_scope" => {
            Some(ForgeCapabilityKind::ModelProviderScope)
        }
        "raw_filesystem" => Some(ForgeCapabilityKind::RawFilesystem),
        "raw_network" => Some(ForgeCapabilityKind::RawNetwork),
        "secret" => Some(ForgeCapabilityKind::Secret),
        _ => None,
    }
}

fn cap_kind_name(kind: ForgeCapabilityKind) -> &'static str {
    match kind {
        ForgeCapabilityKind::FileHash => "file_hash",
        ForgeCapabilityKind::ArtifactHash => "artifact_hash",
        ForgeCapabilityKind::MemoryScope => "memory_scope",
        ForgeCapabilityKind::NetworkSource => "network_source",
        ForgeCapabilityKind::EventSchema => "event_schema",
        ForgeCapabilityKind::UiProjection => "ui_projection",
        ForgeCapabilityKind::GpuBudget => "gpu_budget",
        ForgeCapabilityKind::ModelProviderScope => "model_provider_scope",
        ForgeCapabilityKind::RawFilesystem => "raw_filesystem",
        ForgeCapabilityKind::RawNetwork => "raw_network",
        ForgeCapabilityKind::Secret => "secret",
    }
}

fn parse_hostcall(value: &str) -> Option<ForgeHostCall> {
    match value {
        "hash_bytes" => Some(ForgeHostCall::HashBytes),
        "csv_profile_tiny" => Some(ForgeHostCall::CsvProfileTiny),
        "ui_project_event" => Some(ForgeHostCall::UiProjectEvent),
        "kernel_project" => Some(ForgeHostCall::KernelProject),
        "job_read_projection" => Some(ForgeHostCall::JobReadProjection),
        "toolcell_run" => Some(ForgeHostCall::ToolCellRun),
        "memory_recall" => Some(ForgeHostCall::MemoryRecall),
        "artifact_read_hash" => Some(ForgeHostCall::ArtifactReadHash),
        "ui_emit_projection" => Some(ForgeHostCall::UiEmitProjection),
        "network_fetch_source_id" => Some(ForgeHostCall::NetworkFetchSourceId),
        "read_capability" => Some(ForgeHostCall::ReadCapability),
        "raw_filesystem" => Some(ForgeHostCall::RawFilesystem),
        "raw_network" => Some(ForgeHostCall::RawNetwork),
        "read_secret" => Some(ForgeHostCall::ReadSecret),
        _ => None,
    }
}

fn hostcall_name(hostcall: ForgeHostCall) -> &'static str {
    match hostcall {
        ForgeHostCall::HashBytes => "hash_bytes",
        ForgeHostCall::CsvProfileTiny => "csv_profile_tiny",
        ForgeHostCall::UiProjectEvent => "ui_project_event",
        ForgeHostCall::KernelProject => "kernel_project",
        ForgeHostCall::JobReadProjection => "job_read_projection",
        ForgeHostCall::ToolCellRun => "toolcell_run",
        ForgeHostCall::MemoryRecall => "memory_recall",
        ForgeHostCall::ArtifactReadHash => "artifact_read_hash",
        ForgeHostCall::UiEmitProjection => "ui_emit_projection",
        ForgeHostCall::NetworkFetchSourceId => "network_fetch_source_id",
        ForgeHostCall::ReadCapability => "read_capability",
        ForgeHostCall::RawFilesystem => "raw_filesystem",
        ForgeHostCall::RawNetwork => "raw_network",
        ForgeHostCall::ReadSecret => "read_secret",
    }
}

fn write_cap_kind(h: &mut Sha256, kind: ForgeCapabilityKind) {
    h.update(cap_kind_name(kind).as_bytes());
}

fn escape_line(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn parse_hex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("hex length must be even".to_string());
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    let bytes = value.as_bytes();
    for idx in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[idx]).ok_or_else(|| "bad hex byte".to_string())?;
        let lo = hex_nibble(bytes[idx + 1]).ok_or_else(|| "bad hex byte".to_string())?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn parse_hash_hex(value: &str) -> Result<[u8; 32], String> {
    let bytes = parse_hex(value)?;
    if bytes.len() != 32 {
        return Err(format!("expected 32-byte hash, got {}", bytes.len()));
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch_ok_cell() -> ForgeToolCellSpec {
        ForgeToolCellSpec {
            id: "pilotage-agence".to_string(),
            command: "/pilotage_agence_".to_string(),
            query: "agency_control_tower".to_string(),
            focus: vec!["score".to_string(), "action".to_string()],
            permissions: vec!["read:living_dataflow_graph".to_string()],
            denied: Vec::new(),
            input_schema_hash: "input".to_string(),
            output_schema_hash: "output".to_string(),
        }
    }

    fn batch_denied_cell() -> ForgeToolCellSpec {
        ForgeToolCellSpec {
            id: "bad-raw".to_string(),
            command: "/bad_raw_".to_string(),
            query: "bad".to_string(),
            focus: vec!["action".to_string()],
            permissions: vec!["filesystem:raw_client_files".to_string()],
            denied: Vec::new(),
            input_schema_hash: "input".to_string(),
            output_schema_hash: "output".to_string(),
        }
    }

    #[test]
    fn fbc_hostcall_abi_v0_is_compact_and_named() {
        let abi = hostcall_abi_v0();
        assert_eq!(abi.len(), 8);
        assert!(abi.contains(&ForgeHostCall::KernelProject));
        assert!(abi.contains(&ForgeHostCall::JobReadProjection));
        assert!(abi.contains(&ForgeHostCall::ReadCapability));
        let names = abi.iter().map(|call| hostcall_name(*call)).collect::<Vec<_>>();
        assert_eq!(names[0], "kernel_project");
        assert!(names.contains(&"ui_emit_projection"));
        assert!(names.contains(&"network_fetch_source_id"));
    }

    #[test]
    fn fbc_hash_bytes_has_stable_hash_and_proof() {
        let config = ForgeVmConfig::default();
        let program = hash_bytes_program("hash_bytes_test", b"forge-native-bytecode");
        let report = verify_program(&program, &config);
        assert!(report.ok, "{:?}", report.errors);

        let first = execute_program_interpreter(&program, &config).unwrap();
        let second = execute_program_interpreter(&program, &config).unwrap();
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.proof.proof_hash, second.proof.proof_hash);
        assert_eq!(first.proof.deterministic_replay_hash, second.proof.deterministic_replay_hash);
        assert_eq!(
            first.preview,
            "270bc23d60114300a9e1fe6086ef8f460b84668aade1b0809a4b9acf100877c5"
        );
    }

    #[test]
    fn fbc_csv_profile_tiny_is_deterministic() {
        let config = ForgeVmConfig::default();
        let program = csv_profile_tiny_program("csv_profile_tiny", "city,price,rooms\nLyon,240000,3\nParis,510000,2\n");
        let report = verify_program(&program, &config);
        assert!(report.ok, "{:?}", report.errors);

        let output = execute_program_interpreter(&program, &config).unwrap();
        let text = String::from_utf8(output.bytes).unwrap();
        assert!(text.contains("rows=3"));
        assert!(text.contains("cols=3"));
        assert!(text.contains("headers=city|price|rooms"));
        assert!(text.contains("numericCells=4"));
        assert_eq!(output.proof.output_hash.len(), 64);
    }

    #[test]
    fn fbc_ui_intent_transition_emits_projection() {
        let config = ForgeVmConfig::default();
        let program = ui_intent_transition_program("ui_intent_transition", "alpha", "open_real_estate");
        let report = verify_program(&program, &config);
        assert!(report.ok, "{:?}", report.errors);

        let output = execute_program_interpreter(&program, &config).unwrap();
        assert!(output.preview.contains("ui_projection"));
        assert!(output.preview.contains("outputHash"));
        assert_eq!(output.proof.fuel_used, 3);
        assert_eq!(output.proof.proof_hash, execute_program_interpreter(&program, &config).unwrap().proof.proof_hash);
    }

    #[test]
    fn fbc_kernel_project_emits_action_payload() {
        let config = ForgeVmConfig::default();
        let program = kernel_project_program(
            "kernel_project",
            "set_surface_active",
            "{\"section\":\"trading\",\"active\":true}",
        );
        let report = verify_program(&program, &config);
        assert!(report.ok, "{:?}", report.errors);
        assert!(report.declared_hostcalls.contains(&ForgeHostCall::KernelProject));
        let output = execute_program_interpreter(&program, &config).unwrap();
        let text = String::from_utf8(output.bytes).unwrap();
        assert!(text.contains("\"kind\":\"kernel_projection_v0\""));
        assert!(text.contains("\"op\":\"set_surface_active\""));
        assert!(text.contains("\"section\":\"trading\""));
        assert_eq!(output.proof.proof_hash, execute_program_interpreter(&program, &config).unwrap().proof.proof_hash);
    }

    #[test]
    fn fbc_job_read_projection_emits_bounded_query() {
        let config = ForgeVmConfig::default();
        let program = job_read_projection_program("job_read_projection", "latest", 8);
        let report = verify_program(&program, &config);
        assert!(report.ok, "{:?}", report.errors);
        assert!(report.declared_hostcalls.contains(&ForgeHostCall::JobReadProjection));
        let output = execute_program_interpreter(&program, &config).unwrap();
        let text = String::from_utf8(output.bytes).unwrap();
        assert!(text.contains("\"kind\":\"job_projection_v0\""));
        assert!(text.contains("\"jobId\":\"latest\""));
        assert!(text.contains("\"rawDataReturned\":false"));
        assert_eq!(output.proof.proof_hash, execute_program_interpreter(&program, &config).unwrap().proof.proof_hash);
    }

    #[test]
    fn fbc_verifier_denies_raw_filesystem_capability_and_opcode() {
        let config = ForgeVmConfig::default();
        let program = ForgeBytecodeProgram::v0(
            "deny_raw_fs",
            vec![
                ForgeOpcode::RawFilesystemProbe("C:\\Users\\quent\\Documents\\EVE\\MAP".to_string()),
                ForgeOpcode::End,
            ],
        )
        .with_capability(ForgeCapability::sealed(
            ForgeCapabilityKind::RawFilesystem,
            "C:\\Users\\quent\\Documents\\EVE\\MAP",
            None,
            1,
        ));
        let report = verify_program(&program, &config);
        assert!(!report.ok);
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("raw filesystem")));
        let proof = build_denial_proof(&program, &report, &config.backend);
        assert_eq!(
            proof.proof_hash,
            build_denial_proof(&program, &report, &config.backend).proof_hash
        );
        assert_eq!(proof.fuel_used, 0);
        assert!(matches!(
            execute_program_interpreter(&program, &config),
            Err(ForgeVmError::VerifierDenied(_))
        ));
    }

    #[test]
    fn fbc_audit_denies_undeclared_hostcall_forbidden_opcode_and_raw_network() {
        let mut config = ForgeVmConfig::default();
        config.forbidden_opcodes.push("csv_profile_tiny");

        let mut program = csv_profile_tiny_program("audit_denied", "a,b\n1,2\n");
        program.hostcalls.clear();
        program.ops.push(ForgeOpcode::RawNetworkProbe(
            "https://example.invalid/raw".to_string(),
        ));

        let report = verify_program(&program, &config);
        assert!(!report.ok);
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("requires undeclared hostcall csv_profile_tiny")));
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("opcode csv_profile_tiny is forbidden")));
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("raw network hostcall denied")));
    }

    #[test]
    fn fbc_optimizer_fuses_static_hash_and_preserves_output() {
        let config = ForgeVmConfig::default();
        let program = hash_bytes_program("optimizer_hash", b"frontier");
        let direct = execute_program_interpreter(&program, &config).unwrap();
        let (optimized, report) = optimize_program_v0(&program);
        let optimized_output = execute_program_interpreter(&optimized, &config).unwrap();

        assert!(report.changed);
        assert_eq!(report.fused_hash_ops, 1);
        assert!(report.fuel_after < report.fuel_before);
        assert_eq!(direct.bytes, optimized_output.bytes);
    }

    #[test]
    fn fbc_pipeline_compiles_tool_cell_to_proof_projection() {
        let mut config = ForgeVmConfig::default();
        config.backend = "auto".to_string();
        let cell = ForgeToolCellSpec {
            id: "pilotage-agence".to_string(),
            command: "/pilotage_agence_".to_string(),
            query: "agency_control_tower".to_string(),
            focus: vec!["score".to_string(), "action".to_string()],
            permissions: vec![
                "read:living_dataflow_graph".to_string(),
                "write:tool_cell_outputs".to_string(),
            ],
            denied: vec![
                "network:direct".to_string(),
                "filesystem:raw_client_files".to_string(),
                "secret:read".to_string(),
            ],
            input_schema_hash: "input-schema".to_string(),
            output_schema_hash: "output-schema".to_string(),
        };
        let graph = br#"{"kind":"dataflow_node","id":"score-1","type":"score","label":"Pipeline","recordHash":"hash-score","confidence":0.91}
{"kind":"dataflow_node","id":"action-1","type":"action","label":"Call seller","recordHash":"hash-action","confidence":0.88}
{"kind":"dataflow_node","id":"source-1","type":"source","label":"Ignored","recordHash":"hash-source","confidence":0.5}
"#;
        let bundle = compile_tool_cell_bundle_with_graph(&cell, graph);
        let program = bundle.program;
        let output = execute_program_pipeline_with_context(&program, &config, &bundle.host_context).unwrap();

        assert_eq!(output.backend.selected, "fbc_interpreter");
        assert!(output.optimizer.changed);
        assert_eq!(output.optimizer.fused_hash_ops, 0);
        assert_eq!(output.optimizer.fused_capability_hash_ops, 1);
        assert!(output.optimizer.fuel_after < output.optimizer.fuel_before);
        assert!(output.proof_projection.contains("forge_fbc_proof_projection_v0"));
        assert!(output.proof_projection.contains("\"verifierStatus\":\"ok\""));
        let projection = String::from_utf8(output.vm_output.bytes.clone()).unwrap();
        assert!(projection.contains("\"selectedEvidenceCount\":2"));
        assert!(projection.contains("\"rankedActionCount\":2"));
        assert!(projection.contains("hash-score"));
        assert!(projection.contains("hash-action"));
        assert!(!projection.contains("hash-source"));
        assert_eq!(
            output.vm_output.proof.proof_hash,
            execute_program_pipeline_with_context(&program, &config, &bundle.host_context)
                .unwrap()
                .vm_output
                .proof
                .proof_hash
        );
    }

    #[test]
    fn fbc_tool_cell_graph_capability_tamper_is_denied() {
        let config = ForgeVmConfig::default();
        let cell = ForgeToolCellSpec {
            id: "pilotage-agence".to_string(),
            command: "/pilotage_agence_".to_string(),
            query: "agency_control_tower".to_string(),
            focus: vec!["score".to_string()],
            permissions: vec!["read:living_dataflow_graph".to_string()],
            denied: Vec::new(),
            input_schema_hash: "input-schema".to_string(),
            output_schema_hash: "output-schema".to_string(),
        };
        let mut bundle = compile_tool_cell_bundle_with_graph(
            &cell,
            br#"{"kind":"dataflow_node","id":"score-1","type":"score","recordHash":"hash-score","confidence":1}"#,
        );
        let graph_capability = bundle
            .program
            .capabilities
            .iter()
            .find(|capability| capability.scope.ends_with(":living_dataflow_graph"))
            .unwrap()
            .clone();
        bundle.host_context = ForgeHostContext::default()
            .with_binding(
                bundle
                    .program
                    .capabilities
                    .iter()
                    .find(|capability| capability.scope.ends_with(":manifest"))
                    .unwrap(),
                b"toolcell_v0".to_vec(),
            )
            .with_binding(&graph_capability, b"tampered".to_vec());

        assert!(matches!(
            execute_program_pipeline_with_context(&bundle.program, &config, &bundle.host_context),
            Err(ForgeVmError::CapabilityDenied(_))
        ));
    }

    #[test]
    fn fbc_tool_cell_batch_chains_ledger_for_ok_and_denied_cells() {
        let mut config = ForgeVmConfig::default();
        config.backend = "auto".to_string();
        let graph = br#"{"kind":"dataflow_node","id":"score-1","type":"score","label":"Pipeline","recordHash":"hash-score","confidence":0.91}
{"kind":"dataflow_node","id":"action-1","type":"action","label":"Call seller","recordHash":"hash-action","confidence":0.88}
{"kind":"dataflow_node","id":"source-1","type":"source","label":"Ignored","recordHash":"hash-source","confidence":0.5}
"#;
        let batch = execute_tool_cell_batch(&[batch_ok_cell(), batch_denied_cell()], graph, &config);

        assert_eq!(batch.tool_count, 2);
        assert_eq!(batch.ok_count, 1);
        assert_eq!(batch.denied_count, 1);
        assert_eq!(batch.records[0].selected_evidence_count, 2);
        assert_eq!(batch.records[0].ranked_action_count, 2);
        assert_eq!(batch.records[1].status, "denied");
        assert!(batch.records[1].error.contains("raw filesystem"));
        assert!(batch.projection_json.contains("forge_fbc_tool_cell_batch_projection_v0"));
        assert!(batch.projection_json.contains("\"ledgerRootHash\""));
        assert_eq!(
            batch.ledger_root_hash,
            execute_tool_cell_batch(&[batch_ok_cell(), batch_denied_cell()], graph, &config)
                .ledger_root_hash
        );
    }

    #[test]
    fn fbc_registry_parser_compiles_tool_cells_from_json() {
        let registry_json = r#"{
          "schemaVersion": 1,
          "kind": "forge_real_estate_tool_cells",
          "defaults": {
            "engine": "kasm_dataflow_query_v1",
            "inputSchema": { "type": "object" },
            "outputSchema": { "type": "object" },
            "permissions": ["read:living_dataflow_graph", "write:tool_cell_outputs"],
            "denied": ["network:direct", "filesystem:raw_client_files", "secret:read"]
          },
          "toolCells": [
            { "id": "pilotage-agence", "group": "Pilotage", "focus": ["score", "action"], "query": "agency_control_tower" },
            { "id": "prospects", "group": "Contacts", "focus": ["entity", "action"], "query": "prospect_prioritization" }
          ]
        }"#;
        let registry = parse_tool_cell_registry_v0(registry_json).unwrap();

        assert_eq!(registry.schema_version, 1);
        assert_eq!(registry.default_engine, "kasm_dataflow_query_v1");
        assert_eq!(registry.cells.len(), 2);
        assert_eq!(registry.cells[0].command, "/pilotage_agence_");
        assert_eq!(registry.cells[1].permissions, registry.permissions);
        assert_eq!(registry.denied.len(), 3);
        assert_eq!(registry.registry_hash.len(), 64);
    }

    #[test]
    fn fbc_registry_batch_executes_all_registry_cells() {
        let mut config = ForgeVmConfig::default();
        config.backend = "auto".to_string();
        let registry_json = r#"{
          "schemaVersion": 1,
          "defaults": {
            "engine": "forge_bytecode_v0",
            "inputSchema": { "type": "object" },
            "outputSchema": { "type": "object" },
            "permissions": ["read:living_dataflow_graph", "write:tool_cell_outputs"],
            "denied": ["network:direct"]
          },
          "toolCells": [
            { "id": "pilotage-agence", "focus": ["score", "action"], "query": "agency_control_tower" },
            { "id": "reputation", "focus": ["score"], "query": "agency_reputation" }
          ]
        }"#;
        let graph = br#"{"kind":"dataflow_node","id":"score-1","type":"score","label":"Pipeline","recordHash":"hash-score","confidence":0.91}
{"kind":"dataflow_node","id":"action-1","type":"action","label":"Call seller","recordHash":"hash-action","confidence":0.88}
"#;
        let batch = execute_tool_cell_registry_batch(registry_json, graph, &config).unwrap();

        assert_eq!(batch.tool_count, 2);
        assert_eq!(batch.ok_count, 2);
        assert_eq!(batch.denied_count, 0);
        assert_eq!(batch.records[0].selected_evidence_count, 2);
        assert_eq!(batch.records[1].selected_evidence_count, 1);
        assert_eq!(
            batch.ledger_root_hash,
            execute_tool_cell_registry_batch(registry_json, graph, &config)
                .unwrap()
                .ledger_root_hash
        );
    }

    #[test]
    fn fbc_tool_cell_output_artifact_matches_ui_contract_shape() {
        let mut config = ForgeVmConfig::default();
        config.backend = "auto".to_string();
        let graph = br#"{"kind":"dataflow_node","id":"score-1","type":"score","label":"Pipeline","recordHash":"hash-score","confidence":0.91}
{"kind":"dataflow_node","id":"action-1","type":"action","label":"Call seller","recordHash":"hash-action","confidence":0.88}
"#;
        let batch = execute_tool_cell_batch(&[batch_ok_cell()], graph, &config);
        let artifact = tool_cell_output_artifact_json(
            &batch.records[0],
            &batch.graph_hash,
            "registry-hash",
            &batch.ledger_root_hash,
        );

        assert!(artifact.contains("\"toolId\":\"pilotage-agence\""));
        assert!(artifact.contains("\"status\":\"ok\""));
        assert!(artifact.contains("\"summary\""));
        assert!(artifact.contains("\"engine\":\"forge_bytecode_v0\""));
        assert!(artifact.contains("\"evidenceRefs\""));
        assert!(artifact.contains("\"rankedActions\""));
        assert!(artifact.contains("\"fbcProjection\""));
        assert!(artifact.contains("hash-score"));
        assert!(artifact.contains("hash-action"));
    }

    #[test]
    fn fbc_app_registry_parses_sections_and_sensitive_commands() {
        let ownership = r#"{
          "version": 1,
          "sections": [
            { "id": "shell", "owner": "Forge shell", "files": ["ui/src/shell/surface.ts"], "lifecycle": "always-active" },
            { "id": "trading", "owner": "Trading workspace", "files": ["ui/src/sections/trading/surface.ts"], "lifecycle": "open-close", "nativePresentCommands": ["bloomberg_live_native_present"] }
          ],
          "sensitiveCommands": [
            { "command": "bloomberg_live_native_present", "owner": "trading", "requiresBridge": true },
            { "command": "get_hardware_info", "owner": "shell", "requiresBridge": true }
          ]
        }"#;
        let registry = parse_app_section_registry_v0(ownership).unwrap();

        assert_eq!(registry.section_count, 2);
        assert_eq!(registry.sensitive_command_count, 2);
        assert_eq!(registry.cells.len(), 4);
        assert!(String::from_utf8_lossy(&registry.graph_jsonl).contains("app-section-trading"));
        assert!(String::from_utf8_lossy(&registry.graph_jsonl).contains("sensitive_command"));
    }

    #[test]
    fn fbc_app_registry_batch_covers_sections_and_commands() {
        let mut config = ForgeVmConfig::default();
        config.backend = "auto".to_string();
        let ownership = r#"{
          "version": 1,
          "sections": [
            { "id": "shell", "owner": "Forge shell", "files": ["ui/src/shell/surface.ts"], "lifecycle": "always-active" },
            { "id": "banger", "owner": "Banger viewport", "files": ["ui/src/sections/banger/surface.ts"], "lifecycle": "open-close" }
          ],
          "sensitiveCommands": [
            { "command": "get_hardware_info", "owner": "shell", "requiresBridge": true }
          ]
        }"#;
        let batch = execute_app_registry_batch(ownership, &config).unwrap();

        assert_eq!(batch.tool_count, 3);
        assert_eq!(batch.ok_count, 3);
        assert_eq!(batch.denied_count, 0);
        assert!(batch.records.iter().any(|record| record.tool_id == "app-section-shell"));
        assert!(batch.records.iter().any(|record| record.tool_id == "app-command-get_hardware_info"));
        assert!(batch.records.iter().all(|record| record.selected_evidence_count > 0));
        assert_eq!(
            batch.ledger_root_hash,
            execute_app_registry_batch(ownership, &config)
                .unwrap()
                .ledger_root_hash
        );
    }

    #[test]
    fn fbc_tool_cell_compiler_denies_raw_permission_authority() {
        let config = ForgeVmConfig::default();
        let cell = ForgeToolCellSpec {
            id: "bad-tool".to_string(),
            command: "/bad_tool_".to_string(),
            query: "bad".to_string(),
            focus: vec!["action".to_string()],
            permissions: vec!["filesystem:raw_client_files".to_string()],
            denied: Vec::new(),
            input_schema_hash: "input".to_string(),
            output_schema_hash: "output".to_string(),
        };
        let program = compile_tool_cell_program(&cell);
        let report = verify_program(&program, &config);

        assert!(!report.ok);
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("raw filesystem capability denied")));
    }

    #[test]
    fn fbc_capability_read_requires_host_binding_and_content_hash() {
        let config = ForgeVmConfig::default();
        let capability = ForgeCapability::sealed(
            ForgeCapabilityKind::EventSchema,
            "test:manifest",
            Some(b"expected"),
            32,
        );
        let program = ForgeBytecodeProgram::v0(
            "capability_read",
            vec![
                ForgeOpcode::PushCapability(capability.sealed_hash),
                ForgeOpcode::ReadCapability,
                ForgeOpcode::HashTop,
                ForgeOpcode::End,
            ],
        )
        .with_capability(capability.clone());

        assert!(matches!(
            execute_program_interpreter_with_context(&program, &config, &ForgeHostContext::default()),
            Err(ForgeVmError::CapabilityDenied(_))
        ));

        let bad_context = ForgeHostContext::default().with_binding(&capability, b"wrong".to_vec());
        assert!(matches!(
            execute_program_interpreter_with_context(&program, &config, &bad_context),
            Err(ForgeVmError::CapabilityDenied(_))
        ));

        let good_context = ForgeHostContext::default().with_binding(&capability, b"expected".to_vec());
        let output = execute_program_interpreter_with_context(&program, &config, &good_context).unwrap();
        assert_eq!(
            output.preview,
            "cea23dd4b87e8b00d19fb9ccaaef93e97353c7353e2070f3baf05aeb3995dff4"
        );
    }

    #[test]
    fn fbc_ledger_entry_is_stable_for_ok_and_denied_runs() {
        let config = ForgeVmConfig::default();
        let ok_program = hash_bytes_program("ledger_ok", b"ledger");
        let ok = execute_program_interpreter(&ok_program, &config).unwrap();
        let first = proof_ledger_entry(7, "ok", &ok.proof);
        let second = proof_ledger_entry(7, "ok", &ok.proof);
        assert_eq!(first.ledger_hash, second.ledger_hash);
        assert!(proof_ledger_projection_json(&first).contains("forge_fbc_proof_ledger_entry_v0"));

        let denied = ForgeBytecodeProgram::v0(
            "ledger_denied",
            vec![ForgeOpcode::RawNetworkProbe("https://example.invalid".to_string()), ForgeOpcode::End],
        );
        let report = verify_program(&denied, &config);
        let denial_proof = build_denial_proof(&denied, &report, &config.backend);
        let denied_entry = proof_ledger_entry(8, "denied", &denial_proof);
        assert_eq!(
            denied_entry.ledger_hash,
            proof_ledger_entry(8, "denied", &denial_proof).ledger_hash
        );
    }

    #[test]
    fn fbc_parse_constructs_v0_program() {
        let source = "name=parsed\nop=push_text:hello\nop=hash_top\nop=end\n";
        let config = ForgeVmConfig::default();
        let program = parse_program_v0(source).unwrap();
        assert_eq!(program.name, "parsed");
        assert!(verify_program(&program, &config).ok);
        let output = execute_program_interpreter(&program, &config).unwrap();
        assert_eq!(
            output.preview,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
