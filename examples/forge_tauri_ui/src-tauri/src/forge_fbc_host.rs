#![allow(dead_code)]

use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use scan::fbc::{
    execute_program_interpreter, job_read_projection_program, kernel_project_program,
    ui_intent_transition_program, ForgeRunProof, ForgeVmConfig,
};

const FBC_HOST_LEDGER_FILE: &str = "fbc_host_ledger.jsonl";

#[derive(Debug, Clone)]
pub struct ForgeFbcHostResponse {
    pub kind: String,
    pub emitted_op: Option<String>,
    pub emitted_payload: Option<JsonValue>,
    pub output: JsonValue,
    pub proof: JsonValue,
    pub ledger_hash: Option<String>,
}

pub fn execute_kernel_project(
    store_path: Option<&Path>,
    from_section: &str,
    op: &str,
    payload: &JsonValue,
) -> Result<ForgeFbcHostResponse, String> {
    let payload_json = bounded_json(payload, 2 * 1024, "kernel payload")?;
    let program = kernel_project_program("forge_kernel_project_v0", op, &payload_json);
    let vm = execute_program_interpreter(&program, &host_config(8 * 1024))
        .map_err(|err| format!("FBC kernel project denied {op}: {err:?}"))?;
    let emitted = serde_json::from_slice::<JsonValue>(&vm.bytes)
        .map_err(|err| format!("decode FBC kernel project output: {err}"))?;
    let emitted_op = emitted
        .get("op")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "FBC kernel project missing op".to_string())?
        .to_string();
    let emitted_payload = emitted
        .get("payload")
        .cloned()
        .ok_or_else(|| "FBC kernel project missing payload".to_string())?;
    let proof = proof_json(
        "forge_kernel_fbc_project_v0",
        Some(json!({
            "op": op,
            "emittedOp": &emitted_op,
            "fromSection": from_section,
        })),
        &vm.proof,
    );
    let ledger_hash = append_fbc_host_ledger(
        store_path,
        "kernel.project",
        &proof,
        &json!({ "op": op, "emittedOp": &emitted_op }),
    )?;
    Ok(ForgeFbcHostResponse {
        kind: "kernel_project".to_string(),
        emitted_op: Some(emitted_op),
        emitted_payload: Some(emitted_payload),
        output: emitted,
        proof,
        ledger_hash,
    })
}

pub fn execute_sensitive_guard(
    store_path: Option<&Path>,
    action: &str,
    payload: &JsonValue,
) -> Result<ForgeFbcHostResponse, String> {
    let payload_json = bounded_json(payload, 768, "sensitive action payload")?;
    let intent = format!("sensitive.{action}:{payload_json}");
    let program = ui_intent_transition_program("forge_sensitive_hostcall_guard_v0", "app", &intent);
    let vm = execute_program_interpreter(&program, &host_config(8 * 1024))
        .map_err(|err| format!("FBC sensitive action denied {action}: {err:?}"))?;
    let proof = proof_json(
        "forge_sensitive_fbc_guard_v0",
        Some(json!({ "action": action })),
        &vm.proof,
    );
    let ledger_hash = append_fbc_host_ledger(
        store_path,
        "sensitive.guard",
        &proof,
        &json!({ "action": action }),
    )?;
    Ok(ForgeFbcHostResponse {
        kind: "sensitive_guard".to_string(),
        emitted_op: None,
        emitted_payload: None,
        output: serde_json::from_slice(&vm.bytes).unwrap_or_else(|_| json!({ "bytes": vm.bytes.len() })),
        proof,
        ledger_hash,
    })
}

pub fn execute_job_read_projection(
    store_path: Option<&Path>,
    job_id: &str,
    max_records: u16,
) -> Result<ForgeFbcHostResponse, String> {
    let program = job_read_projection_program("forge_job_read_projection_v0", job_id, max_records);
    let vm = execute_program_interpreter(&program, &host_config(8 * 1024))
        .map_err(|err| format!("FBC job read projection denied {job_id}: {err:?}"))?;
    let query = serde_json::from_slice::<JsonValue>(&vm.bytes)
        .map_err(|err| format!("decode FBC job projection query: {err}"))?;
    let proof = proof_json(
        "forge_job_read_projection_fbc_v0",
        Some(json!({ "jobId": job_id, "maxRecords": max_records })),
        &vm.proof,
    );
    let ledger_hash = append_fbc_host_ledger(
        store_path,
        "job.read_projection",
        &proof,
        &json!({ "jobId": job_id, "maxRecords": max_records }),
    )?;
    Ok(ForgeFbcHostResponse {
        kind: "job_read_projection".to_string(),
        emitted_op: None,
        emitted_payload: None,
        output: query,
        proof,
        ledger_hash,
    })
}

fn host_config(max_output_bytes: u64) -> ForgeVmConfig {
    let mut config = ForgeVmConfig::default();
    config.max_output_bytes = max_output_bytes;
    config
}

fn bounded_json(value: &JsonValue, max_bytes: usize, label: &str) -> Result<String, String> {
    let text = value.to_string();
    if text.len() > max_bytes {
        return Err(format!("{label} too large: {} > {max_bytes} bytes", text.len()));
    }
    Ok(text)
}

fn proof_json(kind: &str, context: Option<JsonValue>, proof: &ForgeRunProof) -> JsonValue {
    json!({
        "kind": kind,
        "context": context.unwrap_or(JsonValue::Null),
        "programHash": &proof.program_hash,
        "verifierHash": &proof.verifier_hash,
        "inputHash": &proof.input_hash,
        "outputHash": &proof.output_hash,
        "capabilityHash": &proof.capability_hash,
        "hostcallHash": &proof.hostcall_hash,
        "fuelUsed": proof.fuel_used,
        "memoryPeak": proof.memory_peak,
        "backend": &proof.backend,
        "deterministicReplayHash": &proof.deterministic_replay_hash,
        "proofHash": &proof.proof_hash,
        "rawInputReturned": false,
        "capabilityOnly": true,
    })
}

fn append_fbc_host_ledger(
    store_path: Option<&Path>,
    event_type: &str,
    proof: &JsonValue,
    summary: &JsonValue,
) -> Result<Option<String>, String> {
    let Some(store_path) = store_path else {
        return Ok(None);
    };
    fs::create_dir_all(store_path).map_err(|err| format!("create FBC host ledger dir: {err}"))?;
    let path: PathBuf = store_path.join(FBC_HOST_LEDGER_FILE);
    let at_ms = now_ms();
    let mut event = json!({
        "kind": "forge_fbc_host_ledger_event_v0",
        "eventType": event_type,
        "atMs": at_ms,
        "proof": proof,
        "summary": summary,
        "rawDataReturned": false,
    });
    let ledger_hash = hash_json("forge_fbc_host_ledger_event_v0", &event);
    if let Some(object) = event.as_object_mut() {
        object.insert("ledgerHash".to_string(), json!(ledger_hash));
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("open FBC host ledger '{}': {err}", path.display()))?;
    let line = serde_json::to_string(&event).map_err(|err| format!("encode FBC host ledger: {err}"))?;
    writeln!(file, "{line}").map_err(|err| format!("append FBC host ledger: {err}"))?;
    Ok(Some(ledger_hash))
}

fn hash_json(domain: &str, value: &JsonValue) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update(b"\n");
    h.update(value.to_string().as_bytes());
    format!("{:x}", h.finalize())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
