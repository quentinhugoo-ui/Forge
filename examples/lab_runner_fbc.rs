//! Forge Native Bytecode v0 lab runner.
//!
//! Prints a compact verifier/proof manifest and replays the same program to
//! prove deterministic output without exposing raw files or host resources.

use scan::fbc::{
    build_denial_proof, compile_tool_cell_bundle_with_graph, compile_tool_cell_program,
    csv_profile_tiny_program, execute_program_interpreter, execute_program_pipeline_with_context,
    execute_app_registry_batch, execute_tool_cell_batch, execute_tool_cell_registry_batch,
    hash_bytes_program, parse_app_section_registry_v0, parse_tool_cell_registry_v0,
    proof_ledger_entry, proof_ledger_projection_json, tool_cell_output_artifact_json,
    ui_intent_transition_program, verify_program, ForgeBytecodeProgram, ForgeCapability,
    ForgeCapabilityKind, ForgeHostContext, ForgeToolCellBatchOutput, ForgeToolCellSpec,
    ForgeVmConfig, ForgeVmError,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let mode = env::args().nth(1).unwrap_or_else(|| "hash-bytes".to_string());
    let config = ForgeVmConfig::default();
    if matches!(mode.as_str(), "batch" | "toolcell-batch") {
        run_batch(config);
        return;
    }
    if matches!(mode.as_str(), "registry" | "registry-batch") {
        run_registry_batch(config, false);
        return;
    }
    if matches!(mode.as_str(), "registry-write" | "write-registry") {
        run_registry_batch(config, true);
        return;
    }
    if matches!(mode.as_str(), "app" | "app-registry") {
        run_app_batch(config, false);
        return;
    }
    if matches!(mode.as_str(), "app-write" | "write-app") {
        run_app_batch(config, true);
        return;
    }
    let program = match mode.as_str() {
        "toolcell" | "tool-cell" => compile_tool_cell_program(&sample_tool_cell()),
        "csv" | "csv-profile" => {
            csv_profile_tiny_program("lab_csv_profile_tiny", "city,price,rooms\nLyon,240000,3\nParis,510000,2\n")
        }
        "ui" | "ui-intent" => {
            ui_intent_transition_program("lab_ui_intent_transition", "alpha", "open_real_estate")
        }
        "deny" | "deny-raw" => denied_program(),
        _ => hash_bytes_program("lab_hash_bytes", b"forge-native-bytecode"),
    };

    if matches!(mode.as_str(), "toolcell" | "tool-cell") {
        let bundle = compile_tool_cell_bundle_with_sample_graph(&sample_tool_cell());
        run_pipeline(bundle.program, bundle.host_context, config);
        return;
    }

    let report = verify_program(&program, &config);
    println!("[fbc-lab] programHash={}", report.program_hash);
    println!("[fbc-lab] verifierStatus={}", if report.ok { "ok" } else { "denied" });
    println!("[fbc-lab] verifierHash={}", report.verifier_hash);
    println!(
        "[fbc-lab] capabilitySummary={}",
        if report.capability_summary.is_empty() {
            "none".to_string()
        } else {
            report.capability_summary.join(",")
        }
    );

    match execute_program_interpreter(&program, &config) {
        Ok(output) => {
            let replay = execute_program_interpreter(&program, &config)
                .expect("verified FBC replay must execute");
            println!("[fbc-lab] fuelUsed={}", output.fuel_used);
            println!("[fbc-lab] memoryPeak={}", output.memory_peak);
            println!("[fbc-lab] proofHash={}", output.proof.proof_hash);
            println!(
                "[fbc-lab] replayResult={}",
                if output.proof.proof_hash == replay.proof.proof_hash {
                    "stable"
                } else {
                    "drift"
                }
            );
            println!("[fbc-lab] preview={}", output.preview.replace('\n', "\\n"));
        }
        Err(ForgeVmError::VerifierDenied(report)) => {
            let proof = build_denial_proof(&program, &report, &config.backend);
            let replay = build_denial_proof(&program, &report, &config.backend);
            println!("[fbc-lab] fuelUsed=0");
            println!("[fbc-lab] memoryPeak=0");
            println!("[fbc-lab] proofHash={}", proof.proof_hash);
            println!(
                "[fbc-lab] replayResult={}",
                if proof.proof_hash == replay.proof_hash {
                    "stable-denial"
                } else {
                    "drift"
                }
            );
            println!("[fbc-lab] verifierErrors={}", report.errors.join("|"));
        }
        Err(error) => {
            println!("[fbc-lab] runtimeError={error:?}");
            std::process::exit(1);
        }
    }
}

fn run_batch(mut config: ForgeVmConfig) {
    config.backend = "auto".to_string();
    let cells = sample_tool_cell_batch();
    let batch = execute_tool_cell_batch(&cells, sample_graph_jsonl().as_bytes(), &config);
    print_batch("[fbc-lab] batch", &batch);
}

fn run_registry_batch(mut config: ForgeVmConfig, write_outputs: bool) {
    config.backend = "auto".to_string();
    let registry_path = repo_path("examples/forge_tauri_ui/source-registry/real-estate-tool-cells.json");
    let registry_json = fs::read_to_string(&registry_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", registry_path.display()));
    let registry = parse_tool_cell_registry_v0(&registry_json).expect("registry must parse");
    let graph = fs::read(repo_path(
        "examples/forge_tauri_ui/.forge-data/real-estate-harvester/data/living_dataflow_graph.jsonl",
    ))
    .unwrap_or_else(|_| sample_graph_jsonl().into_bytes());
    let batch = execute_tool_cell_registry_batch(&registry_json, &graph, &config)
        .expect("registry batch must execute");
    print_batch("[fbc-lab] registry", &batch);
    println!("[fbc-lab] registryHash={}", registry.registry_hash);
    println!("[fbc-lab] registryCellCount={}", registry.cells.len());
    println!("[fbc-lab] registryEngine={}", registry.default_engine);
    if write_outputs {
        write_registry_outputs(&registry.registry_hash, &batch);
    }
}

fn run_app_batch(mut config: ForgeVmConfig, write_outputs: bool) {
    config.backend = "auto".to_string();
    let ownership_path = repo_path("examples/forge_tauri_ui/ui/SECTION_OWNERSHIP.json");
    let ownership_json = fs::read_to_string(&ownership_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", ownership_path.display()));
    let registry = parse_app_section_registry_v0(&ownership_json).expect("app ownership must parse");
    let batch = execute_app_registry_batch(&ownership_json, &config)
        .expect("app registry batch must execute");
    print_batch("[fbc-lab] app", &batch);
    println!("[fbc-lab] appRegistryHash={}", registry.registry_hash);
    println!("[fbc-lab] appSectionCount={}", registry.section_count);
    println!(
        "[fbc-lab] appSensitiveCommandCount={}",
        registry.sensitive_command_count
    );
    println!("[fbc-lab] appCellCount={}", registry.cells.len());
    if write_outputs {
        write_app_outputs(&registry.registry_hash, &batch);
    }
}

fn print_batch(prefix: &str, batch: &ForgeToolCellBatchOutput) {
    println!("{prefix}GraphHash={}", batch.graph_hash);
    println!("{prefix}ToolCount={}", batch.tool_count);
    println!("{prefix}OkCount={}", batch.ok_count);
    println!("{prefix}DeniedCount={}", batch.denied_count);
    println!("{prefix}LedgerRootHash={}", batch.ledger_root_hash);
    for record in &batch.records {
        println!(
            "{prefix}Record tool={} status={} evidence={} ranked={} proof={} ledger={}",
            record.tool_id,
            record.status,
            record.selected_evidence_count,
            record.ranked_action_count,
            record.proof_hash,
            record.ledger_hash
        );
    }
    println!("{prefix}Projection={}", batch.projection_json);
}

fn write_registry_outputs(registry_hash: &str, batch: &ForgeToolCellBatchOutput) {
    let output_dir = repo_path(
        "examples/forge_tauri_ui/.forge-data/real-estate-harvester/data/tool_cell_outputs",
    );
    fs::create_dir_all(&output_dir)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", output_dir.display()));
    for record in &batch.records {
        let path = output_dir.join(format!("{}.fbc.json", record.command.trim_matches('/').trim_end_matches('_')));
        let artifact = tool_cell_output_artifact_json(
            record,
            &batch.graph_hash,
            registry_hash,
            &batch.ledger_root_hash,
        );
        fs::write(&path, format!("{artifact}\n"))
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    }
    let manifest_path = output_dir.join("fbc_registry_batch.json");
    fs::write(&manifest_path, format!("{}\n", batch.projection_json))
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", manifest_path.display()));
    println!("[fbc-lab] registryOutputDir={}", output_dir.display());
    println!("[fbc-lab] registryManifest={}", manifest_path.display());
}

fn write_app_outputs(registry_hash: &str, batch: &ForgeToolCellBatchOutput) {
    let output_dir = repo_path("examples/forge_tauri_ui/.forge-data/forge-app/fbc_outputs");
    fs::create_dir_all(&output_dir)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", output_dir.display()));
    for record in &batch.records {
        let safe_name = record.command.trim_matches('/').trim_end_matches('_');
        let path = output_dir.join(format!("{safe_name}.json"));
        let artifact = tool_cell_output_artifact_json(
            record,
            &batch.graph_hash,
            registry_hash,
            &batch.ledger_root_hash,
        );
        fs::write(&path, format!("{artifact}\n"))
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    }
    let manifest_path = output_dir.join("app_fbc_registry_batch.json");
    fs::write(&manifest_path, format!("{}\n", batch.projection_json))
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", manifest_path.display()));
    println!("[fbc-lab] appOutputDir={}", output_dir.display());
    println!("[fbc-lab] appManifest={}", manifest_path.display());
}

fn compile_tool_cell_bundle_with_sample_graph(
    cell: &ForgeToolCellSpec,
) -> scan::fbc::ForgeCompiledToolCell {
    compile_tool_cell_bundle_with_graph(cell, sample_graph_jsonl().as_bytes())
}

fn run_pipeline(program: ForgeBytecodeProgram, host_context: ForgeHostContext, mut config: ForgeVmConfig) {
    config.backend = "auto".to_string();
    match execute_program_pipeline_with_context(&program, &config, &host_context) {
        Ok(output) => {
            let ledger = proof_ledger_entry(1, "ok", &output.vm_output.proof);
            println!("[fbc-lab] originalProgramHash={}", output.original_program_hash);
            println!("[fbc-lab] optimizedProgramHash={}", output.optimized_program_hash);
            println!("[fbc-lab] optimizerHash={}", output.optimizer.optimizer_hash);
            println!("[fbc-lab] optimizerChanged={}", output.optimizer.changed);
            println!("[fbc-lab] fuelBefore={}", output.optimizer.fuel_before);
            println!("[fbc-lab] fuelAfter={}", output.optimizer.fuel_after);
            println!("[fbc-lab] fusedHashOps={}", output.optimizer.fused_hash_ops);
            println!(
                "[fbc-lab] fusedCapabilityHashOps={}",
                output.optimizer.fused_capability_hash_ops
            );
            println!("[fbc-lab] backend={}", output.backend.selected);
            println!("[fbc-lab] backendSelectorHash={}", output.backend.selector_hash);
            println!("[fbc-lab] verifierStatus={}", if output.verifier.ok { "ok" } else { "denied" });
            println!("[fbc-lab] fuelUsed={}", output.vm_output.fuel_used);
            println!("[fbc-lab] memoryPeak={}", output.vm_output.memory_peak);
            println!("[fbc-lab] proofHash={}", output.vm_output.proof.proof_hash);
            println!("[fbc-lab] replayHash={}", output.vm_output.proof.deterministic_replay_hash);
            println!("[fbc-lab] ledgerHash={}", ledger.ledger_hash);
            println!("[fbc-lab] proofProjection={}", output.proof_projection);
            println!("[fbc-lab] ledgerProjection={}", proof_ledger_projection_json(&ledger));
        }
        Err(ForgeVmError::VerifierDenied(report)) => {
            let proof = build_denial_proof(&program, &report, &config.backend);
            let ledger = proof_ledger_entry(1, "denied", &proof);
            println!("[fbc-lab] verifierStatus=denied");
            println!("[fbc-lab] proofHash={}", proof.proof_hash);
            println!("[fbc-lab] ledgerHash={}", ledger.ledger_hash);
            println!("[fbc-lab] verifierErrors={}", report.errors.join("|"));
        }
        Err(error) => {
            println!("[fbc-lab] runtimeError={error:?}");
            std::process::exit(1);
        }
    }
}

fn sample_tool_cell() -> ForgeToolCellSpec {
    ForgeToolCellSpec {
        id: "pilotage-agence".to_string(),
        command: "/pilotage_agence_".to_string(),
        query: "agency_control_tower".to_string(),
        focus: vec![
            "score".to_string(),
            "action".to_string(),
            "memoryFact".to_string(),
        ],
        permissions: vec![
            "read:living_dataflow_graph".to_string(),
            "read:intel_packs".to_string(),
            "read:kasm_metric_seeds".to_string(),
            "write:tool_cell_outputs".to_string(),
        ],
        denied: vec![
            "network:direct".to_string(),
            "filesystem:raw_client_files".to_string(),
            "secret:read".to_string(),
        ],
        input_schema_hash: "toolcell-input-schema-v0".to_string(),
        output_schema_hash: "toolcell-output-schema-v0".to_string(),
    }
}

fn repo_path(path: &str) -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.push(path);
    root
}

fn sample_tool_cell_batch() -> Vec<ForgeToolCellSpec> {
    vec![
        sample_tool_cell(),
        ForgeToolCellSpec {
            id: "prospects".to_string(),
            command: "/prospects_".to_string(),
            query: "prospect_prioritization".to_string(),
            focus: vec!["action".to_string(), "score".to_string()],
            permissions: vec![
                "read:living_dataflow_graph".to_string(),
                "write:tool_cell_outputs".to_string(),
            ],
            denied: vec![
                "network:direct".to_string(),
                "filesystem:raw_client_files".to_string(),
                "secret:read".to_string(),
            ],
            input_schema_hash: "toolcell-input-schema-v0".to_string(),
            output_schema_hash: "toolcell-output-schema-v0".to_string(),
        },
        ForgeToolCellSpec {
            id: "bad-raw-files".to_string(),
            command: "/bad_raw_files_".to_string(),
            query: "bad_raw_files".to_string(),
            focus: vec!["action".to_string()],
            permissions: vec!["filesystem:raw_client_files".to_string()],
            denied: Vec::new(),
            input_schema_hash: "toolcell-input-schema-v0".to_string(),
            output_schema_hash: "toolcell-output-schema-v0".to_string(),
        },
    ]
}

fn sample_graph_jsonl() -> String {
    [
        r#"{"kind":"dataflow_node","id":"score-agency-1","type":"score","label":"Pipeline pressure","recordHash":"score-hash-1","confidence":0.92}"#,
        r#"{"kind":"dataflow_node","id":"action-followup-1","type":"action","label":"Relancer vendeurs tiedes","recordHash":"action-hash-1","confidence":0.87}"#,
        r#"{"kind":"dataflow_node","id":"memory-coach-1","type":"memoryFact","label":"Coaching cadence","recordHash":"memory-hash-1","confidence":0.81}"#,
        r#"{"kind":"dataflow_node","id":"ignored-source-1","type":"source","label":"Raw source","recordHash":"source-hash-1","confidence":0.7}"#,
        r#"{"kind":"dataflow_edge","id":"edge-1","from":"score-agency-1","to":"action-followup-1","relation":"supports","recordHash":"edge-hash-1","confidence":0.83}"#,
    ]
    .join("\n")
}

fn denied_program() -> ForgeBytecodeProgram {
    hash_bytes_program("lab_denied_raw_capability", b"denied").with_capability(
        ForgeCapability::sealed(
            ForgeCapabilityKind::RawFilesystem,
            "C:\\Users\\quent\\Documents\\EVE\\MAP",
            None,
            1,
        ),
    )
}
