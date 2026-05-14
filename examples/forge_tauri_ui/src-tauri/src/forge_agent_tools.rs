use serde_json::{json, Map, Value};
use scan::kasm::Program;
use scan::{Hash, MemoryGovernor, MonsterNode, Store};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 80;
const MAX_COMPACT_ARRAY: usize = 8;
const MAX_COMPACT_STRING: usize = 320;
const MAX_PLY_POINTS_ANALYZED: usize = 1_000_000;
const MAX_3D_MODEL_ROWS: usize = 1_000_000;
const DEFAULT_3D_MODEL_POINTS: usize = 1_000_000;
const BRAIN_DEFAULT_SAMPLES: usize = 8;
const BRAIN_MAX_SAMPLES: usize = 64;
const BRAIN_MAX_NOTE_CHARS: usize = 16 * 1024;
const BRAIN_NOTE_PREVIEW_CHARS: usize = 960;
const BRAIN_MAX_PROGRAM_BATCH: usize = 64;
const BRAIN_LLM_NOTE_LATEST_REF: &str = "refs/brain/llm/latest";
const BRAIN_LLM_NOTE_LAYER_REF_PREFIX: &str = "refs/brain/llm/by_layer/";
const BRAIN_LLM_NOTE_INDEX_REF_PREFIX: &str = "refs/brain/llm/index/";
const BRAIN_LLM_FACT_REF_PREFIX: &str = "refs/brain/llm/fact/";
const BRAIN_AUTO_COMMIT_REF_PREFIX: &str = "refs/brain/llm/autocommit/";
const BRAIN_AUTO_COMMIT_DEFAULT_THRESHOLD: f64 = 0.62;
const BRAIN_MEMORY_LAYERS: [&str; 3] = ["semantic", "episodic", "procedural"];
const BRAIN_NOTE_INDEX_LIMIT: usize = 32;
const BRAIN_RECALL_DEFAULT_LIMIT: usize = 8;

#[derive(Clone, Copy)]
struct Point3d {
    x: f64,
    y: f64,
    z: f64,
    r: f64,
    g: f64,
    b: f64,
    size: f64,
}

#[derive(Clone, Copy)]
struct AxisStats {
    min: f64,
    max: f64,
    mean: f64,
    std: f64,
}

#[derive(Clone)]
struct VoxelAgg {
    count: usize,
    sum: [f64; 3],
}

#[derive(Clone)]
struct VoxelComponent {
    id: usize,
    voxel_count: usize,
    point_count: usize,
    min_idx: [usize; 3],
    max_idx: [usize; 3],
    centroid: [f64; 3],
}

struct MetricRequest {
    label: String,
    key: String,
    window: usize,
}

struct CsvSeries {
    headers: Vec<String>,
    delimiter: char,
    source_path: PathBuf,
    row_count: usize,
    truncated: bool,
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
    extras: HashMap<String, Vec<f64>>,
}

#[allow(dead_code)]
pub fn resolve_store_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("FORGE_JOBS_DIR") {
        if let Some(parent) = PathBuf::from(path).parent() {
            return Ok(parent.to_path_buf());
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata)
            .join("com.forge.ui")
            .join("forge-store"));
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(".forge-store"))
        .map_err(|e| format!("resolve Forge store dir: {e}"))
}

pub fn call_internal_tool(
    store_path: &Path,
    tool: &str,
    args: &Value,
    active_job_id: Option<&str>,
) -> Result<Value, String> {
    let result = match tool {
        "forge_3d_metric_catalog" | "mapping_metric_catalog" | "metric_catalog_3d" => {
            three_d_metric_catalog(store_path, args, active_job_id)
        }
        "forge_model_3d_mapping" | "model_mapping" | "mapping_model" | "forge_3d_model_view" => {
            model_3d_mapping(store_path, args, active_job_id)
        }
        "forge_run_visual_program" | "visual_program_run" | "run_visual_program" => {
            run_visual_program(store_path, args, active_job_id)
        }
        "forge_analyze_3d_mapping" | "analyze_mapping" | "mapping_analysis" => {
            analyze_3d_mapping(store_path, args, active_job_id)
        }
        "forge_interpret_visual_mapping" | "mapping" | "visual_mapping" => {
            interpret_visual_mapping(store_path, args, active_job_id)
        }
        "forge_profile_settings" | "profile" | "settings" => profile_settings(store_path, args),
        "forge_list_sessions" | "sessions" | "history" => list_sessions(store_path, args),
        "forge_list_documents" | "documents" | "docs" => list_documents(store_path, args),
        "forge_atlas_overview" | "atlas" => atlas_overview(store_path, args),
        "forge_upsert_geonode" | "upsert_geonode" | "geonode_upsert" => {
            upsert_geonode(store_path, args)
        }
        "forge_update_session" | "update_session" => update_session(store_path, args),
        "forge_brain_recall" | "brain_recall" | "recall_memory" => {
            brain_recall(store_path, args)
        }
        "forge_brain_commit" | "brain_commit" | "commit_memory" => {
            brain_commit(store_path, args)
        }
        "forge_brain_compare" | "brain_compare" | "compare_memory" => {
            brain_compare(store_path, args)
        }
        "forge_brain_sleep" | "brain_sleep" | "memory_sleep" => brain_sleep(store_path, args),
        "forge_brain_explain" | "brain_explain" | "explain_memory" => {
            brain_explain(store_path, args)
        }
        _ => Err(format!("unknown Forge internal tool: {tool}")),
    }?;
    Ok(attach_brain_context(store_path, tool, args, result))
}

fn attach_brain_context(store_path: &Path, tool: &str, args: &Value, mut result: Value) -> Value {
    if is_brain_tool(tool) {
        return result;
    }
    let auto_commit = brain_auto_commit_observation(store_path, tool, args, &result)
        .unwrap_or_else(|err| json!({ "status": "error", "error": err }));
    let Ok(mut context) = brain_context_capsule(store_path, tool, args) else {
        return result;
    };
    if let Value::Object(context_map) = &mut context {
        context_map.insert("auto_commit".to_string(), auto_commit);
    }
    match &mut result {
        Value::Object(map) => {
            map.entry("brain_context".to_string()).or_insert(context);
            result
        }
        _ => json!({
            "result": result,
            "brain_context": context
        }),
    }
}

fn is_brain_tool(tool: &str) -> bool {
    tool.contains("brain")
        || tool.contains("memory")
        || matches!(
            tool,
            "recall_memory" | "commit_memory" | "compare_memory" | "explain_memory"
        )
}

fn brain_auto_commit_observation(
    store_path: &Path,
    tool: &str,
    args: &Value,
    result: &Value,
) -> Result<Value, String> {
    if brain_autocommit_disabled(args) {
        return Ok(json!({ "status": "disabled" }));
    }
    let scope = brain_scope_for_tool(tool, args);
    let args_summary = compact_value_for_memory(args, 0);
    let result_summary = compact_value_for_memory(result, 0);
    let stable_observation = json!({
        "schema": "forge-brain-autocommit-key-v1",
        "scope": scope,
        "tool": tool,
        "args": args_summary,
        "result": result_summary
    });
    let stable_observation_bytes = serde_json::to_vec(&stable_observation)
        .map_err(|e| format!("serialize stable brain observation: {e}"))?;
    let observation_hash = Hash::for_blob(&stable_observation_bytes);
    let mut importance = brain_observation_importance(tool, args, result);
    if args
        .get("brain_autocommit_force")
        .or_else(|| args.get("brain_auto_commit_force"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        importance = 1.0;
    }
    let threshold = args
        .get("brain_autocommit_threshold")
        .or_else(|| args.get("brain_auto_commit_threshold"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(BRAIN_AUTO_COMMIT_DEFAULT_THRESHOLD)
        .clamp(0.0, 1.0);
    let observation = json!({
        "schema": "forge-brain-autocommit-v1",
        "scope": scope,
        "memory_layer": "episodic",
        "tool": tool,
        "observed_ms": now_ms(),
        "observation_hash": observation_hash.as_hex(),
        "importance": importance,
        "threshold": threshold,
        "write_policy": "importance_threshold_and_hash_dedup",
        "redaction": "sensitive_keys_and_secret_like_strings",
        "args": args_summary,
        "result": result_summary
    });
    let observation_ref = format!(
        "{}{}/{}",
        BRAIN_AUTO_COMMIT_REF_PREFIX,
        scope,
        observation_hash.as_hex()
    );

    if importance < threshold {
        return Ok(json!({
            "status": "skipped_low_importance",
            "importance": importance,
            "threshold": threshold,
            "observation_hash": observation_hash.as_hex(),
            "ref": observation_ref
        }));
    }

    let node = brain_node(store_path)?;
    if let Some(existing) = node.store().lookup_ref(&observation_ref) {
        return Ok(json!({
            "status": "skipped_duplicate",
            "importance": importance,
            "threshold": threshold,
            "observation_hash": observation_hash.as_hex(),
            "ref": observation_ref,
            "note_hash": existing.as_hex()
        }));
    }

    let observation_text = format!(
        "forge-brain-autocommit-v1\nobservation_hash={}\ntool={}\nscope={}\nimportance={importance:.3}\nthreshold={threshold:.3}\n\n{}",
        observation_hash.as_hex(),
        sanitize_meta_value(tool),
        scope,
        compact_json_text(&observation, 12 * 1024)?
    );
    let note_args = json!({
        "scope": scope,
        "kind": "tool_observation",
        "source": tool,
        "memory_layer": "episodic",
        "confidence": importance,
        "importance": importance,
        "observation_hash": observation_hash.as_hex(),
        "evidence_hash": observation_hash.as_hex(),
        "valid_from_ms": now_ms(),
        "trust": "tool_output",
        "retention": "adaptive"
    });
    let note = store_brain_note(&node, &note_args, &observation_text)?;
    let note_hash = note
        .get("hash")
        .and_then(Value::as_str)
        .and_then(Hash::from_hex)
        .ok_or_else(|| "brain auto-commit note hash missing".to_string())?;
    node.store()
        .write_ref(&observation_ref, &note_hash, "brain auto-commit observation")
        .map_err(|e| format!("write brain auto-commit ref: {e}"))?;
    let latest_ref = format!("{}{}/latest", BRAIN_AUTO_COMMIT_REF_PREFIX, scope);
    node.store()
        .write_ref(&latest_ref, &note_hash, "brain latest auto-commit observation")
        .map_err(|e| format!("write brain latest auto-commit ref: {e}"))?;

    Ok(json!({
        "status": "committed",
        "importance": importance,
        "threshold": threshold,
        "observation_hash": observation_hash.as_hex(),
        "note_hash": note_hash.as_hex(),
        "ref": observation_ref,
        "latest_ref": latest_ref
    }))
}

fn brain_autocommit_disabled(args: &Value) -> bool {
    if std::env::var("FORGE_BRAIN_AUTOCOMMIT")
        .map(|value| matches!(value.trim(), "0" | "false" | "off" | "disabled"))
        .unwrap_or(false)
    {
        return true;
    }
    args.get("brain_autocommit")
        .or_else(|| args.get("brain_auto_commit"))
        .and_then(Value::as_bool)
        == Some(false)
}

fn brain_observation_importance(tool: &str, args: &Value, result: &Value) -> f64 {
    let mut score: f64 = 0.20;
    let tool_lower = tool.to_ascii_lowercase();
    if tool_lower.contains("run")
        || tool_lower.contains("model")
        || tool_lower.contains("analyze")
        || tool_lower.contains("upsert")
        || tool_lower.contains("update")
    {
        score += 0.16;
    }
    if tool_lower.contains("profile") || tool_lower.contains("settings") {
        score += 0.10;
    }
    if args.get("note").is_some() || args.get("title").is_some() || args.get("goal").is_some() {
        score += 0.12;
    }
    let text = serde_json::to_string(result)
        .unwrap_or_default()
        .to_ascii_lowercase();
    for (needle, weight) in [
        ("proof_hash", 0.20),
        ("proofhash", 0.20),
        ("artifact_path", 0.16),
        ("artifactpath", 0.16),
        ("program_hash", 0.18),
        ("data_hash", 0.18),
        ("evidence_hash", 0.18),
        ("job_id", 0.14),
        ("jobid", 0.14),
        ("pack_id", 0.14),
        ("error", 0.20),
        ("failed", 0.20),
        ("completed", 0.12),
        ("changed", 0.10),
    ] {
        if text.contains(needle) {
            score += weight;
        }
    }
    score.clamp(0.0, 1.0)
}

fn compact_value_for_memory(value: &Value, depth: usize) -> Value {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        Value::String(text) => json!(redact_memory_string(text)),
        Value::Array(items) => {
            let preview = if depth >= 2 {
                Vec::new()
            } else {
                items
                    .iter()
                    .take(4)
                    .map(|item| compact_value_for_memory(item, depth + 1))
                    .collect::<Vec<_>>()
            };
            json!({
                "len": items.len(),
                "preview": preview
            })
        }
        Value::Object(map) => {
            let mut out = Map::new();
            for (idx, (key, item)) in map.iter().enumerate() {
                if idx >= 32 {
                    out.insert("_truncated_keys".to_string(), json!(map.len() - idx));
                    break;
                }
                if key == "brain_context" {
                    continue;
                }
                if is_sensitive_memory_key(key) {
                    out.insert(key.clone(), json!("<redacted>"));
                    continue;
                }
                out.insert(key.clone(), compact_value_for_memory(item, depth + 1));
            }
            Value::Object(out)
        }
    }
}

fn is_sensitive_memory_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "secret",
        "token",
        "password",
        "passwd",
        "authorization",
        "auth",
        "cookie",
        "credential",
        "private_key",
        "client_secret",
        "gemini_api_key",
        "openai_api_key",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn redact_memory_string(text: &str) -> String {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    let looks_secret = lower.starts_with("bearer ")
        || lower.starts_with("sk-")
        || lower.starts_with("xoxb-")
        || lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || (trimmed.len() >= 48
            && trimmed
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/')));
    if looks_secret {
        "<redacted>".to_string()
    } else {
        trimmed.chars().take(260).collect::<String>()
    }
}

fn compact_json_text(value: &Value, max_chars: usize) -> Result<String, String> {
    let text = serde_json::to_string(value).map_err(|e| format!("serialize compact json: {e}"))?;
    Ok(text.chars().take(max_chars).collect())
}

fn brain_context_capsule(store_path: &Path, tool: &str, args: &Value) -> Result<Value, String> {
    fs::create_dir_all(store_path)
        .map_err(|e| format!("create Forge brain context store '{}': {e}", store_path.display()))?;
    let store = Store::open(store_path.to_path_buf())
        .map_err(|e| format!("open Forge brain context store '{}': {e}", store_path.display()))?;
    let scope = brain_scope_for_tool(tool, args);
    let scoped_note_ref = format!("refs/brain/llm/{scope}/latest");
    let scoped_auto_commit_ref = format!("{}{}/latest", BRAIN_AUTO_COMMIT_REF_PREFIX, scope);
    Ok(json!({
        "kind": "forge_brain_context_v1",
        "scope": scope,
        "tool": tool,
        "memory_layers": BRAIN_MEMORY_LAYERS,
        "layer_refs": brain_layer_ref_summaries(&store, &scope),
        "history_refs": brain_layer_index_ref_summaries(&store, &scope),
        "retrieval_policy": {
            "primary": "scoped_layer_ref_when_requested",
            "fallback": "scoped_latest_then_global_latest",
            "write_path": "hot_path_importance_threshold_with_hash_dedup"
        },
        "state_hash": store.lookup_ref(scan::BRAIN_STATE_REF).map(|h| h.as_hex()),
        "latest_memory_hash": store.lookup_ref(scan::BRAIN_HEAD_REF).map(|h| h.as_hex()),
        "latest_active_hash": store.lookup_ref(scan::BRAIN_LATEST_ACTIVE_REF).map(|h| h.as_hex()),
        "latest_note_hash": store.lookup_ref(BRAIN_LLM_NOTE_LATEST_REF).map(|h| h.as_hex()),
        "scoped_note_ref": scoped_note_ref,
        "scoped_note_hash": store.lookup_ref(&scoped_note_ref).map(|h| h.as_hex()),
        "scoped_auto_commit_ref": scoped_auto_commit_ref,
        "scoped_auto_commit_hash": store.lookup_ref(&scoped_auto_commit_ref).map(|h| h.as_hex()),
        "next_tools": {
            "recall": "brain_recall",
            "commit": "brain_commit",
            "compare": "brain_compare",
            "sleep": "brain_sleep",
            "explain": "brain_explain"
        }
    }))
}

fn brain_layer_ref_summaries(store: &Store, scope: &str) -> Value {
    let mut layers = Map::new();
    for layer in BRAIN_MEMORY_LAYERS {
        let note_ref = scoped_layer_note_ref(scope, layer);
        layers.insert(layer.to_string(), ref_summary(&note_ref, store.lookup_ref(&note_ref)));
    }
    Value::Object(layers)
}

fn brain_layer_index_ref_summaries(store: &Store, scope: &str) -> Value {
    let mut layers = Map::new();
    for layer in BRAIN_MEMORY_LAYERS {
        let index_ref = scoped_layer_index_ref(scope, layer);
        layers.insert(layer.to_string(), ref_summary(&index_ref, store.lookup_ref(&index_ref)));
    }
    Value::Object(layers)
}

fn brain_scope_for_tool(tool: &str, args: &Value) -> String {
    let explicit = clean_optional_string(args.get("scope").or_else(|| args.get("section")))
        .map(|value| clean_ref_segment(&value))
        .filter(|value| !value.is_empty());
    if let Some(scope) = explicit {
        return scope;
    }
    let tool = tool.to_ascii_lowercase();
    if tool.contains("trading") {
        "trading".to_string()
    } else if tool.contains("banger") || tool.contains("visual") || tool.contains("mapping") {
        "banger".to_string()
    } else if tool.contains("document") || tool.contains("docs") {
        "google_suite".to_string()
    } else if tool.contains("real_estate") || tool.contains("immo") || tool.contains("geonode") {
        "agence_immo".to_string()
    } else if tool.contains("profile") || tool.contains("session") {
        "basique".to_string()
    } else {
        "global".to_string()
    }
}

pub fn brain_recall(store_path: &Path, args: &Value) -> Result<Value, String> {
    let node = brain_node(store_path)?;
    let scope = brain_scope(args);
    let limit = bounded_limit(args.get("limit"), BRAIN_RECALL_DEFAULT_LIMIT, BRAIN_NOTE_INDEX_LIMIT);
    let include_expired = args
        .get("include_expired")
        .or_else(|| args.get("includeExpired"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let state_hash = ref_hash(node.store(), scan::BRAIN_STATE_REF);
    let latest_memory_hash = ref_hash(node.store(), scan::BRAIN_HEAD_REF);
    let latest_active_hash = ref_hash(node.store(), scan::BRAIN_LATEST_ACTIVE_REF);
    let scoped_note_ref = format!("refs/brain/llm/{scope}/latest");
    let scoped_auto_commit_ref = format!("{}{}/latest", BRAIN_AUTO_COMMIT_REF_PREFIX, scope);
    let requested_layer = brain_memory_layer_arg(args);
    let scoped_layer_ref = requested_layer
        .as_deref()
        .map(|layer| scoped_layer_note_ref(&scope, layer));
    let scoped_note_hash = ref_hash(node.store(), &scoped_note_ref);
    let scoped_auto_commit_hash = ref_hash(node.store(), &scoped_auto_commit_ref);
    let scoped_layer_hash = scoped_layer_ref
        .as_deref()
        .and_then(|note_ref| ref_hash(node.store(), note_ref));
    let latest_note_hash = ref_hash(node.store(), BRAIN_LLM_NOTE_LATEST_REF);
    let mut result = Map::new();
    result.insert("tool".to_string(), json!("forge_brain_recall"));
    result.insert("scope".to_string(), json!(scope));
    result.insert("memory_layer".to_string(), json!(requested_layer));
    result.insert("limit".to_string(), json!(limit));
    result.insert("include_expired".to_string(), json!(include_expired));
    result.insert(
        "memory_layers".to_string(),
        json!({
            "available": BRAIN_MEMORY_LAYERS,
            "refs": brain_layer_ref_summaries(node.store(), &scope),
            "history_refs": brain_layer_index_ref_summaries(node.store(), &scope)
        }),
    );
    result.insert(
        "refs".to_string(),
        json!({
            "state": ref_summary(scan::BRAIN_STATE_REF, state_hash),
            "latest_memory": ref_summary(scan::BRAIN_HEAD_REF, latest_memory_hash),
            "latest_active": ref_summary(scan::BRAIN_LATEST_ACTIVE_REF, latest_active_hash),
            "latest_llm_note": ref_summary(BRAIN_LLM_NOTE_LATEST_REF, latest_note_hash),
            "scoped_llm_note": ref_summary(&scoped_note_ref, scoped_note_hash),
            "scoped_layer_note": scoped_layer_ref
                .as_deref()
                .map(|note_ref| ref_summary(note_ref, scoped_layer_hash)),
            "scoped_auto_commit": ref_summary(&scoped_auto_commit_ref, scoped_auto_commit_hash)
        }),
    );

    if let Some(hash) = optional_hash_arg(args, &["program_hash", "hash"])? {
        let resolved = scan::resolve_program_hash(&node, hash);
        let summary = node
            .store()
            .load(&resolved)
            .map(|bytes| explain_brain_blob(resolved, &bytes))
            .unwrap_or_else(|| json!({ "type": "missing", "hash": resolved.as_hex() }));
        result.insert(
            "program".to_string(),
            json!({
                "requested_hash": hash.as_hex(),
                "resolved_hash": resolved.as_hex(),
                "substitution_ref": scan::brain_substitution_ref(hash),
                "summary": summary
            }),
        );
    }

    if let Some(note_hash) = scoped_layer_hash.or(scoped_note_hash).or(latest_note_hash) {
        if let Some(bytes) = node.store().load(&note_hash) {
            result.insert("latest_note".to_string(), explain_brain_blob(note_hash, &bytes));
        }
    }
    result.insert(
        "recent_notes".to_string(),
        recent_brain_notes(&node, &scope, requested_layer.as_deref(), limit, include_expired),
    );
    result.insert("content_policy".to_string(), compact_content_policy());
    result.insert(
        "token_safety".to_string(),
        token_safety("Brain recall returns refs, hashes, compact summaries and bounded previews only."),
    );
    Ok(Value::Object(result))
}

pub fn brain_commit(store_path: &Path, args: &Value) -> Result<Value, String> {
    let node = brain_node(store_path)?;
    let samples = bounded_limit(
        args.get("samples"),
        BRAIN_DEFAULT_SAMPLES,
        BRAIN_MAX_SAMPLES,
    );
    let note_text = brain_note_text(args);
    let program_hash = optional_hash_arg(args, &["program_hash", "function_hash"])?;
    if note_text.is_none() && program_hash.is_none() {
        return Err("brain_commit requires text/observation or program_hash".to_string());
    }

    let mut result = Map::new();
    result.insert("tool".to_string(), json!("forge_brain_commit"));
    result.insert("samples".to_string(), json!(samples));

    if let Some(text) = note_text {
        let note = store_brain_note(&node, args, &text)?;
        result.insert("note".to_string(), note);
    }

    if let Some(hash) = program_hash {
        let program = load_brain_program(&node, hash)?;
        let before_nodes = program.nodes().len();
        let before_bytes = program.bytes().len();
        let tightened = scan::tighten_program_for_execution(&node, hash, program, samples);
        let tightened_hash = Hash::for_blob(tightened.bytes());
        let attractor = scan::publish_semantic_attractor(&node, tightened_hash, &tightened, samples)
            .map_err(|e| format!("publish semantic attractor: {e}"))?;
        let final_hash = attractor.unwrap_or(tightened_hash);
        result.insert(
            "program".to_string(),
            json!({
                "action": if final_hash == hash { "already_active_or_tight" } else { "committed_verified_substitution" },
                "from": hash.as_hex(),
                "tightened": tightened_hash.as_hex(),
                "active": final_hash.as_hex(),
                "nodes_before": before_nodes,
                "nodes_after": tightened.nodes().len(),
                "bytes_before": before_bytes,
                "bytes_after": tightened.bytes().len(),
                "semantic_fingerprint": program_fingerprint_hex(&tightened).ok()
            }),
        );
    }

    result.insert("content_policy".to_string(), compact_content_policy());
    result.insert(
        "token_safety".to_string(),
        token_safety("Brain commit stores bounded observations or verified KASM substitutions in the CAS."),
    );
    Ok(Value::Object(result))
}

pub fn brain_compare(store_path: &Path, args: &Value) -> Result<Value, String> {
    let node = brain_node(store_path)?;
    let samples = bounded_limit(
        args.get("samples"),
        BRAIN_DEFAULT_SAMPLES,
        BRAIN_MAX_SAMPLES,
    );
    let left = required_hash_arg(args, &["left_hash", "a", "from"])?;
    let right = required_hash_arg(args, &["right_hash", "b", "to"])?;
    let left_resolved = scan::resolve_program_hash(&node, left);
    let right_resolved = scan::resolve_program_hash(&node, right);
    let left_program = load_brain_program(&node, left_resolved)?;
    let right_program = load_brain_program(&node, right_resolved)?;
    let left_fp = program_fingerprint_hex(&left_program)?;
    let right_fp = program_fingerprint_hex(&right_program)?;
    let equivalent = left_fp == right_fp;
    let mut attractor_hash = None;
    if equivalent {
        let _ = scan::publish_semantic_attractor(&node, left_resolved, &left_program, samples)
            .map_err(|e| format!("publish left semantic attractor: {e}"))?;
        attractor_hash =
            scan::publish_semantic_attractor(&node, right_resolved, &right_program, samples)
                .map_err(|e| format!("publish right semantic attractor: {e}"))?;
        let left_after = scan::resolve_program_hash(&node, left_resolved);
        let right_after = scan::resolve_program_hash(&node, right_resolved);
        if left_after == right_after {
            attractor_hash = Some(left_after);
        }
    }

    Ok(json!({
        "tool": "forge_brain_compare",
        "equivalent_by_semantic_fingerprint": equivalent,
        "samples": samples,
        "left": {
            "requested_hash": left.as_hex(),
            "resolved_hash": left_resolved.as_hex(),
            "nodes": left_program.nodes().len(),
            "bytes": left_program.bytes().len(),
            "semantic_fingerprint": left_fp
        },
        "right": {
            "requested_hash": right.as_hex(),
            "resolved_hash": right_resolved.as_hex(),
            "nodes": right_program.nodes().len(),
            "bytes": right_program.bytes().len(),
            "semantic_fingerprint": right_fp
        },
        "attractor_hash": attractor_hash.map(|h| h.as_hex()),
        "content_policy": compact_content_policy(),
        "token_safety": token_safety("Brain compare returns compact semantic proofs and may publish a verified attractor ref.")
    }))
}

pub fn brain_sleep(store_path: &Path, args: &Value) -> Result<Value, String> {
    let node = brain_node(store_path)?;
    let samples = bounded_limit(
        args.get("samples"),
        BRAIN_DEFAULT_SAMPLES,
        BRAIN_MAX_SAMPLES,
    );
    let hashes = program_hash_batch(args)?;
    if hashes.is_empty() {
        let brain = scan::ForgeBrain::rehydrate_with_samples(&node, samples)
            .map_err(|e| format!("rehydrate brain: {e}"))?;
        return Ok(json!({
            "tool": "forge_brain_sleep",
            "action": "rehydrated_only",
            "active_count": brain.active_count(),
            "latest_memory_hash": brain.latest_memory_hash().map(|h| h.as_hex()),
            "samples": samples,
            "note": "Provide program_hashes to run a bounded semantic sleep pass.",
            "content_policy": compact_content_policy(),
            "token_safety": token_safety("Brain sleep without a batch only rehydrates compact state.")
        }));
    }

    let mut changed = 0usize;
    let mut items = Vec::new();
    for hash in hashes {
        let resolved = scan::resolve_program_hash(&node, hash);
        let program = load_brain_program(&node, resolved)?;
        let before_nodes = program.nodes().len();
        let before_bytes = program.bytes().len();
        let tightened = scan::tighten_program_for_execution(&node, resolved, program, samples);
        let tightened_hash = Hash::for_blob(tightened.bytes());
        let attractor = scan::publish_semantic_attractor(&node, tightened_hash, &tightened, samples)
            .map_err(|e| format!("publish semantic attractor during sleep: {e}"))?;
        let final_hash = attractor.unwrap_or(tightened_hash);
        if final_hash != resolved {
            changed += 1;
        }
        items.push(json!({
            "requested_hash": hash.as_hex(),
            "resolved_hash": resolved.as_hex(),
            "final_hash": final_hash.as_hex(),
            "changed": final_hash != resolved,
            "nodes_before": before_nodes,
            "nodes_after": tightened.nodes().len(),
            "bytes_before": before_bytes,
            "bytes_after": tightened.bytes().len(),
            "semantic_fingerprint": program_fingerprint_hex(&tightened).ok()
        }));
    }

    Ok(json!({
        "tool": "forge_brain_sleep",
        "action": "bounded_semantic_sleep",
        "samples": samples,
        "checked": items.len(),
        "changed": changed,
        "items": items,
        "content_policy": compact_content_policy(),
        "token_safety": token_safety("Brain sleep rewrites only verified KASM programs supplied by hash.")
    }))
}

pub fn brain_explain(store_path: &Path, args: &Value) -> Result<Value, String> {
    let node = brain_node(store_path)?;
    let target_hash = if let Some(ref_name) = clean_optional_string(args.get("ref")) {
        if !ref_name.starts_with("refs/brain/") {
            return Err("brain_explain only accepts refs under refs/brain/".to_string());
        }
        node.store()
            .lookup_ref(&ref_name)
            .ok_or_else(|| format!("brain ref not found: {ref_name}"))?
    } else if clean_optional_string(args.get("kind")).as_deref() == Some("state") {
        node.store()
            .lookup_ref(scan::BRAIN_STATE_REF)
            .ok_or_else(|| "brain state ref not found".to_string())?
    } else {
        required_hash_arg(args, &["hash", "program_hash", "memory_hash"])?
    };
    let bytes = node
        .store()
        .load(&target_hash)
        .ok_or_else(|| format!("brain object not found: {}", target_hash.as_hex()))?;
    Ok(json!({
        "tool": "forge_brain_explain",
        "hash": target_hash.as_hex(),
        "explanation": explain_brain_blob(target_hash, &bytes),
        "content_policy": compact_content_policy(),
        "token_safety": token_safety("Brain explain returns metadata and bounded previews only.")
    }))
}

pub fn list_sessions(store_path: &Path, args: &Value) -> Result<Value, String> {
    let limit = bounded_limit(args.get("limit"), DEFAULT_LIMIT, MAX_LIMIT);
    let query = clean_optional_string(args.get("query"));
    let status = clean_optional_string(args.get("status"));
    let sessions = list_job_summaries(store_path, limit, query.as_deref(), status.as_deref())?;
    Ok(json!({
        "sessions": sessions,
        "limit": limit,
        "query": query,
        "status": status,
        "content_policy": compact_content_policy(),
        "token_safety": token_safety("Session history returns compact manifests, refs, counts and hashes only.")
    }))
}

pub fn list_documents(store_path: &Path, args: &Value) -> Result<Value, String> {
    let limit = bounded_limit(args.get("limit"), DEFAULT_LIMIT, MAX_LIMIT);
    let query = clean_optional_string(args.get("query"));
    let type_filter = clean_optional_string(args.get("type")).map(|v| v.to_ascii_lowercase());
    let jobs = list_job_summaries(store_path, MAX_LIMIT * 4, query.as_deref(), None)?;
    let mut documents = Vec::new();
    for job in jobs {
        let Some(files) = job.get("source_refs").and_then(Value::as_array) else {
            continue;
        };
        if files.is_empty() {
            continue;
        }
        for source in files {
            let label = source
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| job.get("title").and_then(Value::as_str))
                .unwrap_or("Forge document")
                .to_string();
            let file_type = document_file_type(&label);
            if let Some(kind) = &type_filter {
                if &file_type != kind {
                    continue;
                }
            }
            documents.push(json!({
                "document_id": format!(
                    "{}:{}",
                    job.get("job_id").and_then(Value::as_str).unwrap_or("unknown"),
                    documents.len()
                ),
                "job_id": job.get("job_id").cloned().unwrap_or(Value::Null),
                "title": label,
                "type": file_type,
                "status": job.get("status").cloned().unwrap_or(Value::Null),
                "source_ref": source,
                "session_title": job.get("title").cloned().unwrap_or(Value::Null),
                "created_ms": job.get("created_ms").cloned().unwrap_or(Value::Null),
                "last_modified_ms": job.get("last_modified_ms").cloned().unwrap_or(Value::Null),
                "content_included": false
            }));
            if documents.len() >= limit {
                break;
            }
        }
        if documents.len() >= limit {
            break;
        }
    }
    Ok(json!({
        "documents": documents,
        "limit": limit,
        "query": query,
        "type": type_filter,
        "content_policy": compact_content_policy(),
        "token_safety": token_safety("Document library returns source references and metadata only. Use a bounded preview or a compute program for content.")
    }))
}

pub fn three_d_metric_catalog(
    store_path: &Path,
    args: &Value,
    active_job_id: Option<&str>,
) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .or(active_job_id)
        .ok_or_else(|| "forge_3d_metric_catalog requires job_id or an active session".to_string())?;
    validate_job_id(job_id)?;
    let (manifest_path, job) = read_job_value(store_path, job_id)?;
    let source_paths = source_file_paths(&job);
    let source = source_paths
        .iter()
        .find(|path| {
            path.extension()
                .and_then(|v| v.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("csv"))
                .unwrap_or(false)
        })
        .or_else(|| source_paths.first())
        .ok_or_else(|| "this session has no source file reference for 3D metric catalog".to_string())?;
    let schema = inspect_csv_metric_schema(source, 96)?;
    Ok(json!({
        "job_id": job_id,
        "manifest_path": manifest_path.display().to_string(),
        "source": {
            "path": source.display().to_string(),
            "name": source.file_name().and_then(|v| v.to_str()).unwrap_or("source"),
            "content_included": false
        },
        "csv_schema": schema,
        "metric_recipe_schema": {
            "axis_fields": ["x", "y", "z"],
            "visual_fields": ["color", "size"],
            "accepted_metric_forms": [
                "source column name, e.g. close or volume",
                "derived metric, e.g. return_1, momentum_24, volatility_48, rsi_14, forward_return_6",
                { "metric": "volatility", "window": 48 }
            ],
            "example": {
                "mode": "agent_edge_map",
                "objective": "separate regimes that precede directional edge",
                "axes": { "x": "time_index", "y": "momentum_24", "z": "volatility_48" },
                "color": "forward_return_6",
                "size": "volume_z_48",
                "transform": "robust"
            }
        },
        "derived_metrics": derived_metric_catalog(),
        "content_policy": {
            "source_content_included": false,
            "sample_values_included": false,
            "column_names_included": true,
            "raw_rows_returned": false
        },
        "token_safety": token_safety("3D metric catalog inspects source columns locally and returns only names, type hints and allowed metric recipes.")
    }))
}

pub fn model_3d_mapping(
    store_path: &Path,
    args: &Value,
    active_job_id: Option<&str>,
) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .or(active_job_id)
        .ok_or_else(|| "forge_model_3d_mapping requires job_id or an active session".to_string())?;
    validate_job_id(job_id)?;
    let (manifest_path, mut job) = read_job_value(store_path, job_id)?;
    let recipe = args.get("recipe").unwrap_or(args);
    let mode = clean_optional_string(recipe.get("mode"))
        .or_else(|| clean_optional_string(args.get("mode")))
        .unwrap_or_else(|| "agent_model".to_string());
    let mode_name = safe_artifact_token(&mode, "agent_model");
    let objective = clean_optional_string(recipe.get("objective"))
        .or_else(|| clean_optional_string(args.get("objective")))
        .unwrap_or_else(|| "surface useful market structure in 3D without exposing source rows".to_string());
    let axes = recipe.get("axes");
    let x_req = metric_request(
        axes.and_then(|value| value.get("x"))
            .or_else(|| recipe.get("x"))
            .or_else(|| recipe.get("x_metric")),
        "time_index",
    );
    let y_req = metric_request(
        axes.and_then(|value| value.get("y"))
            .or_else(|| recipe.get("y"))
            .or_else(|| recipe.get("y_metric")),
        "close",
    );
    let z_req = metric_request(
        axes.and_then(|value| value.get("z"))
            .or_else(|| recipe.get("z"))
            .or_else(|| recipe.get("z_metric")),
        "volatility_24",
    );
    let color_req = metric_request(
        recipe.get("color")
            .or_else(|| recipe.get("color_metric"))
            .or_else(|| recipe.get("colour")),
        "forward_return_6",
    );
    let size_req = metric_request(recipe.get("size").or_else(|| recipe.get("size_metric")), "volume_z_48");
    let transform = clean_optional_string(recipe.get("transform")).unwrap_or_else(|| "robust".to_string());
    let max_points = args
        .get("max_points")
        .or_else(|| recipe.get("max_points"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_3D_MODEL_POINTS)
        .clamp(64, MAX_3D_MODEL_ROWS);
    let source_paths = source_file_paths(&job);
    let source = source_paths
        .iter()
        .find(|path| {
            path.extension()
                .and_then(|v| v.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("csv"))
                .unwrap_or(false)
        })
        .or_else(|| source_paths.first())
        .ok_or_else(|| "this session has no source file reference for 3D modeling".to_string())?;

    let requested = [&x_req, &y_req, &z_req, &color_req, &size_req];
    let series = read_csv_series_for_metrics(source, &requested, MAX_3D_MODEL_ROWS)?;
    if series.row_count < 2 {
        return Err("source file does not contain enough numeric rows to model a 3D mapping".to_string());
    }

    let x_raw = evaluate_metric(&x_req, &series)?;
    let y_raw = evaluate_metric(&y_req, &series)?;
    let z_raw = evaluate_metric(&z_req, &series)?;
    let color_raw = evaluate_metric(&color_req, &series)?;
    let size_raw = evaluate_metric(&size_req, &series)?;
    let x = normalize_metric_values(&x_raw, &transform, metric_is_time_like(&x_req.key));
    let y = normalize_metric_values(&y_raw, &transform, false);
    let z = normalize_metric_values(&z_raw, &transform, false);
    let color = normalize_metric_values(&color_raw, "robust", false);
    let size = normalize_metric_values(&size_raw, "robust", false);
    let stride = series.row_count.div_ceil(max_points).max(1);

    let mut points = Vec::<Point3d>::new();
    for index in (0..series.row_count).step_by(stride) {
        let px = x.get(index).copied().unwrap_or(f64::NAN);
        let py = y.get(index).copied().unwrap_or(f64::NAN);
        let pz = z.get(index).copied().unwrap_or(f64::NAN);
        if !(px.is_finite() && py.is_finite() && pz.is_finite()) {
            continue;
        }
        let c = color.get(index).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        let s = size.get(index).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        let (r, g, b) = color_ramp(c);
        points.push(Point3d {
            x: px,
            y: py,
            z: pz,
            r,
            g,
            b,
            size: 2.0 + (s + 1.0) * 3.5,
        });
    }
    if points.is_empty() {
        return Err("3D recipe produced no finite points".to_string());
    }

    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| store_path.join("jobs"));
    let artifacts_dir = manifest_dir.join(format!("{job_id}.artifacts"));
    fs::create_dir_all(&artifacts_dir)
        .map_err(|e| format!("create 3D artifact dir '{}': {e}", artifacts_dir.display()))?;
    let ply_path = artifacts_dir.join(format!("{job_id}.3d.{mode_name}.ply"));
    let ply_bytes = model_3d_ply_bytes(&mode_name, &points)?;
    fs::write(&ply_path, &ply_bytes)
        .map_err(|e| format!("write modeled 3D PLY '{}': {e}", ply_path.display()))?;
    let ply_hash = quick_file_hash_path(&ply_path)?;

    let recipe_doc = json!({
        "version": "forge.3d_recipe.v1",
        "mode": mode_name,
        "objective": objective,
        "source": {
            "path": series.source_path.display().to_string(),
            "rows_seen": series.row_count,
            "truncated": series.truncated,
            "delimiter": series.delimiter.to_string(),
            "column_count": series.headers.len(),
            "content_included": false
        },
        "axes": {
            "x": metric_summary(&x_req),
            "y": metric_summary(&y_req),
            "z": metric_summary(&z_req)
        },
        "color": metric_summary(&color_req),
        "size": metric_summary(&size_req),
        "transform": transform,
        "stride": stride,
        "points_materialized": points.len(),
        "raw_rows_returned": false
    });
    let artifact = json!({
        "kind": "visualization_3d",
        "artifact_type": "point_cloud",
        "mode": mode_name,
        "format": "ply",
        "mime": "model/x.ply",
        "path": ply_path.display().to_string(),
        "bytes": ply_bytes.len() as u64,
        "hash_algorithm": "forge_fnv1a64",
        "hash": format!("{ply_hash:016x}"),
        "point_count": points.len() as u64,
        "draw_mode": "points",
        "point_size": 1.0,
        "legend": {
            "generated_by": "forge_model_3d_mapping",
            "recipe": recipe_doc,
            "color_metric": metric_summary(&color_req),
            "size_metric": metric_summary(&size_req)
        },
        "mcp_injectable": true,
        "download": {
            "delivery": "by_reference",
            "path": ply_path.display().to_string(),
            "do_not_inline": true
        }
    });
    upsert_3d_artifact(&mut job, artifact.clone());
    update_visual_mapping_doc(&manifest_dir, job_id, &mut job, &artifact, &recipe_doc)?;
    let obj = ensure_object(&mut job);
    obj.insert("active_3d_recipe".to_string(), recipe_doc.clone());
    obj.insert("last_modified_ms".to_string(), json!(now_ms()));
    write_job_value(&manifest_path, &job)?;

    let analysis_args = json!({
        "job_id": job_id,
        "mode": mode_name,
        "voxel_resolution": args.get("voxel_resolution").and_then(Value::as_u64).unwrap_or(40)
    });
    let compact_analysis = analyze_3d_mapping(store_path, &analysis_args, Some(job_id))
        .map(|value| compact_json_value(&value, 4))
        .unwrap_or_else(|err| json!({ "status": "analysis_unavailable", "error": err }));

    Ok(json!({
        "job_id": job_id,
        "status": "modeled_3d_mapping_created",
        "mode": mode_name,
        "artifact": {
            "path": ply_path.display().to_string(),
            "bytes": ply_bytes.len() as u64,
            "hash_algorithm": "forge_fnv1a64",
            "hash": format!("{ply_hash:016x}"),
            "point_count": points.len() as u64,
            "source_rows_seen": series.row_count,
            "stride": stride
        },
        "recipe": recipe_doc,
        "compact_analysis": compact_analysis,
        "ui_instruction": {
            "action": "refresh_3d_mapping",
            "mode": mode_name,
            "reason": "A new local 3D mapping artifact was generated from the agent recipe."
        },
        "content_policy": {
            "source_content_included": false,
            "point_cloud_content_included": false,
            "raw_arrays_included": false,
            "raw_rows_returned": false,
            "download_by_reference_only": true
        },
        "token_safety": token_safety("The LLM supplied only a metric recipe. Forge read the full source locally, modeled the 3D map, persisted a PLY artifact and returned compact diagnostics.")
    }))
}

pub fn run_visual_program(
    store_path: &Path,
    args: &Value,
    active_job_id: Option<&str>,
) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .or(active_job_id)
        .ok_or_else(|| "forge_run_visual_program requires job_id or an active session".to_string())?;
    validate_job_id(job_id)?;
    let views = args
        .get("views")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if views.is_empty() {
        return Err("visual_program execution requires at least one 2D, 3D or Planet view".to_string());
    }
    let metrics = args
        .get("metrics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let program_hash = clean_optional_string(args.get("program_hash"));
    let program_title = clean_optional_string(args.get("program_title"))
        .unwrap_or_else(|| "Forge visual program".to_string());
    let program_goal = clean_optional_string(args.get("program_goal"))
        .unwrap_or_else(|| "materialize configured 2D and 3D views without returning raw data".to_string());

    let mut views_2d = Vec::new();
    let mut views_3d = Vec::new();
    let mut errors = Vec::new();
    let mut artifact_bytes = 0usize;
    for (idx, view) in views.iter().enumerate() {
        let view_type = view
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("3d")
            .trim()
            .to_ascii_lowercase();
        if view_type == "2d" {
            match materialize_2d_visual_view(store_path, job_id, view, &metrics, args, idx) {
                Ok(value) => {
                    artifact_bytes = artifact_bytes.saturating_add(
                        value
                            .pointer("/artifact/bytes")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize,
                    );
                    views_2d.push(value);
                }
                Err(err) => errors.push(json!({
                    "view_index": idx,
                    "view_id": view.get("id").cloned().unwrap_or(Value::Null),
                    "type": "2d",
                    "error": err
                })),
            }
        } else {
            let recipe = visual_view_to_3d_recipe(view, args, idx);
            let call_args = json!({
                "job_id": job_id,
                "recipe": recipe,
                "max_points": args.get("max_points").cloned().unwrap_or(Value::Null),
                "voxel_resolution": args.get("voxel_resolution").cloned().unwrap_or(Value::Null)
            });
            match model_3d_mapping(store_path, &call_args, Some(job_id)) {
                Ok(value) => {
                    artifact_bytes = artifact_bytes.saturating_add(
                        value
                            .pointer("/artifact/bytes")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize,
                    );
                    views_3d.push(json!({
                        "view_id": view.get("id").cloned().unwrap_or_else(|| json!(format!("view_{}", idx + 1))),
                        "title": view.get("title").cloned().unwrap_or(Value::Null),
                        "type": "3d",
                        "status": value.get("status").cloned().unwrap_or(Value::Null),
                        "mode": value.get("mode").cloned().unwrap_or(Value::Null),
                        "artifact": value.get("artifact").cloned().unwrap_or(Value::Null),
                        "recipe": value.get("recipe").cloned().unwrap_or(Value::Null),
                        "compact_analysis": value.get("compact_analysis").cloned().unwrap_or(Value::Null),
                        "content_policy": value.get("content_policy").cloned().unwrap_or_else(compact_content_policy)
                    }));
                }
                Err(err) => errors.push(json!({
                    "view_index": idx,
                    "view_id": view.get("id").cloned().unwrap_or(Value::Null),
                    "type": "3d",
                    "error": err
                })),
            }
        }
    }

    let (manifest_path, mut job) = read_job_value(store_path, job_id)?;
    let obj = ensure_object(&mut job);
    obj.insert(
        "visual_program".to_string(),
        json!({
            "available": true,
            "program_hash": program_hash,
            "program_title": program_title,
            "program_goal": program_goal,
            "metric_count": metrics.len(),
            "view_count": views.len(),
            "views_declared": views,
            "raw_input_returned": false,
            "raw_series_returned": false,
            "point_cloud_returned": false
        }),
    );
    obj.insert("views_2d".to_string(), json!(views_2d));
    obj.insert("views_3d".to_string(), json!(views_3d));
    obj.insert("last_modified_ms".to_string(), json!(now_ms()));
    write_job_value(&manifest_path, &job)?;

    let refreshed = read_json_value(&manifest_path).unwrap_or(job);
    let visual_mapping = refreshed.get("visual_mapping").cloned().unwrap_or(Value::Null);
    let visual_mapping_path = refreshed
        .get("visual_mapping_path")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let visual_mapping_artifact = visual_mapping_path
        .as_ref()
        .map(|path| visual_mapping_artifact_value(path))
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(json!({
        "job_id": job_id,
        "stage": "visual_program_materialized",
        "status": if errors.is_empty() { "completed" } else { "completed_with_visual_view_errors" },
        "program_hash": program_hash,
        "program_title": program_title,
        "metric_count": metrics.len(),
        "view_count": views.len(),
        "view_2d_count": refreshed.get("views_2d").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "view_3d_count": refreshed.get("views_3d").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "failed_count": errors.len(),
        "errors": errors,
        "views_2d": refreshed.get("views_2d").cloned().unwrap_or_else(|| json!([])),
        "views_3d": refreshed.get("views_3d").cloned().unwrap_or_else(|| json!([])),
        "artifacts_2d": refreshed.get("artifacts_2d").cloned().unwrap_or_else(|| json!([])),
        "artifacts_3d": refreshed.get("artifacts_3d").cloned().unwrap_or_else(|| json!([])),
        "visual_mapping": visual_mapping,
        "visual_mapping_artifact": visual_mapping_artifact,
        "artifact_bytes": artifact_bytes,
        "compute_avoided": {
            "mode": "local_visual_program_executor",
            "program_kind": "visual_program_run",
            "metric_count": metrics.len(),
            "view_count": views.len(),
            "view_2d_count": refreshed.get("views_2d").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "view_3d_count": refreshed.get("views_3d").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "operations_unit": "view_materializations",
            "raw_input_returned": false,
            "raw_series_returned": false,
            "point_cloud_returned": false
        },
        "content_policy": {
            "source_content_included": false,
            "raw_rows_returned": false,
            "raw_series_returned": false,
            "point_cloud_content_included": false,
            "download_by_reference_only": true
        },
        "token_safety": token_safety("Visual programs are recipes. Forge reads and models files locally, persists 2D/3D artifacts and returns only compact refs, hashes and diagnostics.")
    }))
}

pub fn update_session(store_path: &Path, args: &Value) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "forge_update_session requires job_id".to_string())?;
    validate_job_id(job_id)?;
    let (path, mut job) = read_job_value(store_path, job_id)?;
    let obj = ensure_object(&mut job);
    let mut changed = Vec::new();

    if let Some(title) = clean_optional_string(args.get("title")) {
        obj.insert("title".to_string(), json!(title));
        changed.push("title");
    }
    for (field, key) in [("pinned", "pinned"), ("protected", "protected")] {
        if let Some(value) = args.get(field).and_then(Value::as_bool) {
            obj.insert(key.to_string(), json!(value));
            changed.push(key);
        }
    }
    if let Some(status) = clean_optional_string(args.get("status")) {
        let status = status.to_ascii_lowercase();
        let allowed = [
            "pending",
            "running",
            "completed",
            "failed",
            "cancelled",
            "canceled",
            "archived",
        ];
        if !allowed.iter().any(|item| *item == status) {
            return Err(format!("unsupported session status '{status}'"));
        }
        obj.insert("status".to_string(), json!(status));
        changed.push("status");
    }
    if let Some(archived) = args.get("archived").and_then(Value::as_bool) {
        if archived {
            obj.insert("status".to_string(), json!("archived"));
            changed.push("archived");
        }
    }
    if let Some(tags) = args.get("tags").and_then(Value::as_array) {
        let clean = tags
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .take(24)
            .map(|v| json!(v))
            .collect::<Vec<_>>();
        obj.insert("tags".to_string(), Value::Array(clean));
        changed.push("tags");
    }
    if let Some(note) = clean_optional_string(args.get("note")) {
        obj.insert(
            "agent_note".to_string(),
            json!({
                "updated_ms": now_ms(),
                "text": note
            }),
        );
        changed.push("agent_note");
    }
    obj.insert("last_modified_ms".to_string(), json!(now_ms()));
    write_job_value(&path, &job)?;
    Ok(json!({
        "job_id": job_id,
        "changed": changed,
        "manifest_path": path.display().to_string(),
        "session": compact_job_summary(&job, Some(&path)),
        "content_policy": compact_content_policy()
    }))
}

pub fn interpret_visual_mapping(
    store_path: &Path,
    args: &Value,
    active_job_id: Option<&str>,
) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .or(active_job_id)
        .ok_or_else(|| "forge_interpret_visual_mapping requires job_id or an active session".to_string())?;
    validate_job_id(job_id)?;
    let (manifest_path, job) = read_job_value(store_path, job_id)?;
    let mode_filter = clean_optional_string(args.get("mode")).map(|v| v.to_ascii_lowercase());
    let mapping_doc = read_visual_mapping_doc(&job).unwrap_or(Value::Null);
    let mapping_views = mapping_doc
        .get("views")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let artifacts = job
        .get("artifacts_3d")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut views = Vec::new();
    let mut raw_artifact_bytes = 0u64;
    for artifact in artifacts {
        let mode = artifact
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("view")
            .to_string();
        if let Some(filter) = &mode_filter {
            if mode.to_ascii_lowercase() != *filter {
                continue;
            }
        }
        raw_artifact_bytes = raw_artifact_bytes.saturating_add(
            artifact
                .get("bytes")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        );
        let mapping_view = find_mapping_view_for_mode(&mapping_views, &mode);
        views.push(summarize_3d_view(&artifact, mapping_view.as_ref()));
    }
    if views.is_empty() && !mapping_views.is_empty() {
        for view in mapping_views {
            let mode = mapping_mode_name(&view);
            if let Some(filter) = &mode_filter {
                if mode.to_ascii_lowercase() != *filter {
                    continue;
                }
            }
            views.push(json!({
                "mode": mode,
                "mapping_view": compact_json_value(&view, 3),
                "interpretation": mode_interpretation(&mode)
            }));
        }
    }
    let selection = summarize_visual_selection(args, &views);
    Ok(json!({
        "job_id": job_id,
        "manifest_path": manifest_path.display().to_string(),
        "title": job_label(&job),
        "bars": value_at_any(&job, &["bars", "bar_count", "barCount"]).cloned().unwrap_or(Value::Null),
        "visualization_3d": compact_json_value(job.get("visualization_3d").unwrap_or(&Value::Null), 3),
        "visual_mapping": compact_json_value(job.get("visual_mapping").unwrap_or(&Value::Null), 3),
        "visual_mapping_doc": compact_json_value(&mapping_doc, 2),
        "views": views,
        "selection": selection,
        "agent_interpretation": "Each 3D view is an addressable artifact. Interpret the legend, axes and mode summary here, then ask Forge for bounded metrics if exact point-level values are needed.",
        "raw_artifact_bytes_not_returned": raw_artifact_bytes,
        "content_policy": {
            "source_content_included": false,
            "point_cloud_content_included": false,
            "raw_arrays_included": false,
            "download_by_reference_only": true
        },
        "token_safety": token_safety("3D mapping interpretation returns only mode summaries, legends, axes, hashes and selected index hints. PLY point clouds stay on disk.")
    }))
}

pub fn analyze_3d_mapping(
    store_path: &Path,
    args: &Value,
    active_job_id: Option<&str>,
) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .or(active_job_id)
        .ok_or_else(|| "forge_analyze_3d_mapping requires job_id or an active session".to_string())?;
    validate_job_id(job_id)?;
    let (manifest_path, job) = read_job_value(store_path, job_id)?;
    let mode_filter = clean_optional_string(args.get("mode")).map(|v| v.to_ascii_lowercase());
    let voxel_resolution = args
        .get("voxel_resolution")
        .or_else(|| args.get("resolution"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(40)
        .clamp(12, 96);
    let max_hotspots = bounded_limit(args.get("max_hotspots"), 8, 24);
    let max_clusters = bounded_limit(args.get("max_clusters"), 8, 24);
    let Some(artifact) = select_3d_artifact(&job, mode_filter.as_deref()) else {
        return Ok(json!({
            "job_id": job_id,
            "status": "needs_3d_export",
            "message": "No exported 3D point-cloud artifact is attached to this session yet.",
            "next_action": "Open the 3D mapping view/export for this session, then call forge_analyze_3d_mapping again.",
            "raw_points_returned": false,
            "content_policy": {
                "source_content_included": false,
                "point_cloud_content_included": false,
                "raw_arrays_included": false
            }
        }));
    };
    let artifact_path = artifact
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "selected 3D artifact has no path".to_string())?;
    let mode = artifact
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("view")
        .to_string();
    let points = read_ply_points(Path::new(artifact_path), MAX_PLY_POINTS_ANALYZED)?;
    if points.is_empty() {
        return Err(format!("3D artifact '{}' has no readable points", artifact_path));
    }

    let axes = axis_stats(&points);
    let covariance = covariance_matrix(&points, &axes);
    let eigenvalues = eigenvalues_symmetric_3x3(covariance);
    let variance_total = eigenvalues.iter().sum::<f64>().max(f64::EPSILON);
    let variance_ratio = [
        eigenvalues[0] / variance_total,
        eigenvalues[1] / variance_total,
        eigenvalues[2] / variance_total,
    ];
    let corr = correlation_matrix(covariance, &axes);
    let visual_channels = visual_channel_stats(&points);
    let voxel = analyze_voxels(
        &points,
        &axes,
        voxel_resolution,
        max_hotspots,
        max_clusters,
    );
    let trajectory = analyze_trajectory(
        &points,
        &axes,
        voxel_resolution,
        voxel.get("component_by_key"),
    );
    let shape = classify_3d_shape(&variance_ratio, &voxel);
    let outliers = analyze_outliers(&points, &axes, voxel.get("sparse_point_count"));
    let visual_read = build_visual_read(&shape, &mode, &voxel, &outliers);
    let math_read = build_math_read(&variance_ratio, &corr, &trajectory);

    Ok(json!({
        "job_id": job_id,
        "manifest_path": manifest_path.display().to_string(),
        "title": job_label(&job),
        "mode": mode,
        "artifact": {
            "path": artifact_path,
            "format": artifact.get("format").cloned().unwrap_or_else(|| json!("ply")),
            "bytes": artifact.get("bytes").cloned().unwrap_or(Value::Null),
            "hash_algorithm": artifact.get("hash_algorithm").cloned().unwrap_or(Value::Null),
            "hash": artifact.get("hash").cloned().unwrap_or(Value::Null),
            "declared_point_count": artifact.get("point_count").cloned().unwrap_or(Value::Null)
        },
        "points_analyzed": points.len(),
        "points_cap": MAX_PLY_POINTS_ANALYZED,
        "voxel_resolution": voxel_resolution,
        "axis_stats": {
            "x": axis_stats_json(axes[0]),
            "y": axis_stats_json(axes[1]),
            "z": axis_stats_json(axes[2])
        },
        "pca_variance_ratio": {
            "primary": round6(variance_ratio[0]),
            "secondary": round6(variance_ratio[1]),
            "residual": round6(variance_ratio[2])
        },
        "axis_correlation": matrix_json(corr),
        "visual_channels": visual_channels,
        "visual_signature": {
            "main_shape": shape,
            "mode_interpretation": mode_interpretation(&mode),
            "visual_read": visual_read,
            "math_read": math_read
        },
        "voxel_density": strip_component_lookup(voxel),
        "outliers": outliers,
        "trajectory": trajectory,
        "recommended_next_probes": [
            "Run a cluster-vs-forward-return Forge program if the user wants market meaning, not only geometry.",
            "Ask for a zoomed cluster id to compute a bounded local diagnostic.",
            "Compare this mode with another mode (phase/heightmap/manifold/lattice) using the same tool."
        ],
        "content_policy": {
            "source_content_included": false,
            "point_cloud_content_included": false,
            "raw_arrays_included": false,
            "raw_points_returned": false,
            "download_by_reference_only": true
        },
        "token_safety": token_safety("Forge analyzed the 3D point cloud locally and returned only compact statistics, clusters, density descriptors and interpretation hints.")
    }))
}

pub fn atlas_overview(store_path: &Path, args: &Value) -> Result<Value, String> {
    let max_entries = bounded_limit(args.get("max_entries"), 24, 80);
    let query = clean_optional_string(args.get("query")).map(|value| value.to_ascii_lowercase());
    let kind_filter = clean_optional_string(args.get("kind")).map(|value| value.to_ascii_lowercase());
    let jobs_count = count_json_files(&store_path.join("jobs"));
    let programs_count = count_json_files(&store_path.join("programs"));
    let my_atlas = load_my_atlas_overview(
        store_path,
        max_entries,
        query.as_deref(),
        kind_filter.as_deref(),
    )?;
    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    let mut scanned = 0usize;
    scan_store_entries(
        store_path,
        0,
        2,
        max_entries,
        &mut scanned,
        &mut total_bytes,
        &mut entries,
    );
    Ok(json!({
        "atlas": {
            "configured": true,
            "ui_tab_configured": true,
            "status": "My Atlas is the local reusable memory for created programs, metric tags and completed content-addressed runs.",
            "store_path": store_path.display().to_string()
        },
        "my_atlas": my_atlas,
        "store_overview": {
            "jobs_count": jobs_count,
            "programs_count": programs_count,
            "scanned_entries": scanned,
            "scanned_bytes": total_bytes,
            "sample_entries": entries
        },
        "available_actions": [
            "reuse a metric tag from my_atlas.metric_tags before creating a new one",
            "reuse a program from my_atlas.programs before creating a near-duplicate",
            "if my_atlas.runs contains the same run_hash, use the job/artifact refs directly instead of recomputing",
            "filter with query and kind=program|metric_tag|run to choose a specific reusable Atlas item",
            "list sessions with forge_list_sessions",
            "list documents with forge_list_documents",
            "list programs with forge_list_programs or programs/read tools",
            "interpret 3D mappings with forge_interpret_visual_mapping",
            "analyze 3D point-cloud geometry locally with forge_analyze_3d_mapping"
        ],
        "content_policy": compact_content_policy(),
        "token_safety": token_safety("Atlas overview reports counts and file refs only. It does not dump store contents.")
    }))
}

fn atlas_item_matches(value: &Value, query: Option<&str>) -> bool {
    let Some(query) = query.map(str::trim).filter(|v| !v.is_empty()) else {
        return true;
    };
    serde_json::to_string(value)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains(query)
}

fn filter_atlas_array(value: &mut Value, key: &str, query: Option<&str>, limit: usize) {
    if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
        items.retain(|item| atlas_item_matches(item, query));
        items.truncate(limit);
    }
}

fn load_my_atlas_overview(
    store_path: &Path,
    limit: usize,
    query: Option<&str>,
    kind_filter: Option<&str>,
) -> Result<Value, String> {
    ensure_builtin_mars_geonodes(store_path)?;
    let path = store_path.join("atlas").join("my_atlas.json");
    let mut atlas = if path.exists() {
        read_json_value(&path)?
    } else {
        json!({
            "schema": "forge_my_atlas_v1",
            "programs": [],
            "metric_tags": [],
            "runs": [],
            "web_blocks": []
        })
    };
    let mut fallback_programs = synthesize_programs_for_my_atlas(store_path, limit)?;
    let mut fallback_tags = Vec::new();
    for program in &fallback_programs {
        if let Some(nodes) = program.get("metric_nodes").and_then(Value::as_array) {
            for node in nodes.iter().take(32) {
                let tag = node
                    .get("tag")
                    .or_else(|| node.get("id"))
                    .or_else(|| node.get("name"))
                    .cloned()
                    .unwrap_or(Value::Null);
                fallback_tags.push(json!({
                    "kind": "metric_tag",
                    "tag": tag,
                    "node_kind": node.get("kind").cloned().unwrap_or(Value::Null),
                    "op": node.get("op").cloned().unwrap_or(Value::Null),
                    "dtype": node.get("dtype").cloned().unwrap_or(Value::Null),
                    "domain": node.get("domain").cloned().unwrap_or(Value::Null),
                    "formula": node.get("formula").or_else(|| node.get("expression")).cloned().unwrap_or(Value::Null),
                    "algorithm": node.get("algorithm").or_else(|| node.get("description")).cloned().unwrap_or(Value::Null),
                    "program_hash": program.get("program_hash").cloned().unwrap_or(Value::Null),
                    "program_title": program.get("title").cloned().unwrap_or(Value::Null),
                    "reusable": true,
                    "content_addressed": true,
                    "source_content_included": false
                }));
            }
        }
    }

    let programs_empty = atlas
        .get("programs")
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true);
    if programs_empty {
        atlas["programs"] = Value::Array(fallback_programs);
    } else if let Some(programs) = atlas.get_mut("programs").and_then(Value::as_array_mut) {
        programs.truncate(limit);
        fallback_programs.clear();
    }
    let tags_empty = atlas
        .get("metric_tags")
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true);
    if tags_empty {
        fallback_tags.truncate(limit.saturating_mul(2));
        atlas["metric_tags"] = Value::Array(fallback_tags);
    } else if let Some(tags) = atlas.get_mut("metric_tags").and_then(Value::as_array_mut) {
        tags.truncate(limit.saturating_mul(2));
    }
    if let Some(runs) = atlas.get_mut("runs").and_then(Value::as_array_mut) {
        runs.truncate(limit);
    }
    if let Some(web_blocks) = atlas.get_mut("web_blocks").and_then(Value::as_array_mut) {
        web_blocks.truncate(limit.saturating_mul(2));
    }
    filter_atlas_array(&mut atlas, "programs", query, limit);
    filter_atlas_array(&mut atlas, "metric_tags", query, limit.saturating_mul(2));
    filter_atlas_array(&mut atlas, "runs", query, limit);
    filter_atlas_array(&mut atlas, "web_blocks", query, limit.saturating_mul(2));
    match kind_filter {
        Some("program") | Some("programs") => {
            atlas["metric_tags"] = json!([]);
            atlas["runs"] = json!([]);
            atlas["web_blocks"] = json!([]);
        }
        Some("geonode") | Some("geonodes") | Some("geo_node") | Some("geo_nodes") => {
            atlas["programs"] = json!([]);
            atlas["runs"] = json!([]);
            atlas["web_blocks"] = json!([]);
            if let Some(tags) = atlas.get_mut("metric_tags").and_then(Value::as_array_mut) {
                tags.retain(atlas_value_is_geonode);
            }
        }
        Some("minigeonode")
        | Some("minigeonodes")
        | Some("mini_geonode")
        | Some("mini_geonodes")
        | Some("mini_geo_node")
        | Some("mini_geo_nodes") => {
            atlas["programs"] = json!([]);
            atlas["runs"] = json!([]);
            atlas["web_blocks"] = json!([]);
            if let Some(tags) = atlas.get_mut("metric_tags").and_then(Value::as_array_mut) {
                tags.retain(atlas_value_is_mini_geonode);
            }
        }
        Some("metric") | Some("metric_tag") | Some("metric_tags") | Some("tag") | Some("tags") => {
            atlas["programs"] = json!([]);
            atlas["runs"] = json!([]);
            atlas["web_blocks"] = json!([]);
        }
        Some("run") | Some("runs") | Some("result") | Some("results") => {
            atlas["programs"] = json!([]);
            atlas["metric_tags"] = json!([]);
            atlas["web_blocks"] = json!([]);
        }
        Some("web_block") | Some("web_blocks") | Some("web") => {
            atlas["programs"] = json!([]);
            atlas["metric_tags"] = json!([]);
            atlas["runs"] = json!([]);
        }
        _ => {}
    }

    let program_count = atlas.get("programs").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    let tag_count = atlas.get("metric_tags").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    let run_count = atlas.get("runs").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    let web_block_count = atlas.get("web_blocks").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    Ok(json!({
        "index_path": path.display().to_string(),
        "program_count": program_count,
        "metric_tag_count": tag_count,
        "run_count": run_count,
        "web_block_count": web_block_count,
        "query": query,
        "kind_filter": kind_filter,
        "programs": atlas.get("programs").cloned().unwrap_or_else(|| json!([])),
        "metric_tags": atlas.get("metric_tags").cloned().unwrap_or_else(|| json!([])),
        "runs": atlas.get("runs").cloned().unwrap_or_else(|| json!([])),
        "web_blocks": atlas.get("web_blocks").cloned().unwrap_or_else(|| json!([])),
        "reuse_policy": {
            "first_run_materializes_result": true,
            "same_program_same_inputs_same_params": "instant Atlas hit",
            "tags_can_seed_new_programs": true,
            "raw_content_included": false
        }
    }))
}

fn atlas_value_is_geonode(item: &Value) -> bool {
    let fields = [
        item.get("kind"),
        item.get("node_kind"),
        item.get("geonode_level"),
        item.get("dtype"),
        item.get("domain"),
        item.get("op"),
        item.get("tag"),
        item.get("title"),
        item.get("formula"),
        item.get("algorithm"),
    ];
    fields.iter().flatten().any(|value| {
        value
            .as_str()
            .map(|text| {
                let text = text.to_ascii_lowercase();
                text.contains("geo_node")
                    || text.contains("minigeonode")
                    || text.contains("geonode")
                    || text.contains("geo_anchor")
                    || text.contains("geo_path")
                    || text.contains("geo_region")
                    || text.contains("geo_heatmap")
                    || text.contains("geojson")
            })
            .unwrap_or(false)
    }) || (item.get("lat").is_some() && item.get("lon").is_some())
}

fn atlas_value_is_mini_geonode(item: &Value) -> bool {
    ["node_kind", "geonode_level", "class"].iter().any(|key| {
        item.get(*key)
            .and_then(Value::as_str)
            .map(|text| {
                let text = text.to_ascii_lowercase();
                text.contains("mini_geo_node")
                    || text.contains("minigeonode")
                    || text == "mini"
                    || text == "sub_region"
            })
            .unwrap_or(false)
    }) || item.get("parent_geonode").and_then(Value::as_str).is_some()
}

pub fn upsert_geonode(store_path: &Path, args: &Value) -> Result<Value, String> {
    ensure_builtin_mars_geonodes(store_path)?;
    let name = clean_optional_string(args.get("name").or_else(|| args.get("title")))
        .ok_or_else(|| "forge_upsert_geonode requires name".to_string())?;
    let body = clean_optional_string(args.get("body"))
        .unwrap_or_else(|| "mars".to_string())
        .to_ascii_lowercase();
    let lat = args
        .get("lat")
        .or_else(|| args.get("latitude"))
        .and_then(Value::as_f64);
    let lon = args
        .get("lon")
        .or_else(|| args.get("longitude"))
        .and_then(Value::as_f64);
    let ra = args
        .get("ra")
        .or_else(|| args.get("right_ascension"))
        .and_then(Value::as_f64);
    let dec = args
        .get("dec")
        .or_else(|| args.get("declination"))
        .and_then(Value::as_f64);
    if lat.is_none() && ra.is_none() {
        return Err("forge_upsert_geonode requires either lat/lon for surface coordinates or ra/dec for astronomical coordinates".to_string());
    }
    if lat.is_some() != lon.is_some() {
        return Err("forge_upsert_geonode lat and lon must be provided together".to_string());
    }
    if ra.is_some() != dec.is_some() {
        return Err("forge_upsert_geonode ra and dec must be provided together".to_string());
    }
    if let Some(value) = lat {
        if !(-90.0..=90.0).contains(&value) {
            return Err("forge_upsert_geonode lat must be between -90 and 90".to_string());
        }
    }
    if let Some(value) = lon {
        if !(-360.0..=360.0).contains(&value) {
            return Err("forge_upsert_geonode lon must be between -360 and 360".to_string());
        }
    }
    if let Some(value) = ra {
        if !(0.0..=360.0).contains(&value) {
            return Err("forge_upsert_geonode ra must be between 0 and 360 degrees".to_string());
        }
    }
    if let Some(value) = dec {
        if !(-90.0..=90.0).contains(&value) {
            return Err("forge_upsert_geonode dec must be between -90 and 90".to_string());
        }
    }
    let coordinate_system = clean_optional_string(
        args.get("coordinate_system")
            .or_else(|| args.get("frame"))
            .or_else(|| args.get("coords")),
    )
    .unwrap_or_else(|| {
        if lat.is_some() {
            "planetocentric_latlon".to_string()
        } else {
            "icrs_ra_dec".to_string()
        }
    });
    let distance = args
        .get("distance")
        .or_else(|| args.get("distance_ly"))
        .or_else(|| args.get("distance_parsec"))
        .and_then(Value::as_f64);
    let distance_unit = clean_optional_string(args.get("distance_unit"))
        .unwrap_or_else(|| if args.get("distance_parsec").is_some() { "pc" } else { "ly" }.to_string());
    let parent_geonode = clean_optional_string(
        args.get("parent_geonode")
            .or_else(|| args.get("parent"))
            .or_else(|| args.get("parentGeoNode")),
    );
    let requested_kind = clean_optional_string(args.get("node_kind").or_else(|| args.get("kind")))
        .unwrap_or_default()
        .to_ascii_lowercase();
    let is_mini = requested_kind.contains("mini") || parent_geonode.is_some();
    let node_kind = if is_mini { "mini_geo_node" } else { "geo_node" };
    let geonode_level = if is_mini { "mini" } else { "region" };
    let tag = clean_optional_string(args.get("tag").or_else(|| args.get("id")))
        .map(|value| sanitize_geonode_tag(&value, &body))
        .unwrap_or_else(|| sanitize_geonode_tag(&name, &body));
    let coordinate_source = clean_optional_string(args.get("coordinate_source").or_else(|| args.get("source")))
        .unwrap_or_else(|| "llm_estimate".to_string());
    let confidence = args
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(0.55)
        .clamp(0.0, 1.0);
    let notes = clean_optional_string(args.get("notes").or_else(|| args.get("evidence")));
    let aliases = args
        .get("aliases")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .take(16)
                .map(Value::from)
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();

    let atlas_dir = store_path.join("atlas");
    fs::create_dir_all(&atlas_dir)
        .map_err(|e| format!("create atlas dir '{}': {e}", atlas_dir.display()))?;
    let path = atlas_dir.join("my_atlas.json");
    let mut atlas = if path.exists() {
        read_json_value(&path)?
    } else {
        json!({
            "schema": "forge_my_atlas_v1",
            "programs": [],
            "metric_tags": [],
            "runs": [],
            "web_blocks": []
        })
    };
    let obj = ensure_object(&mut atlas);
    obj.entry("schema".to_string()).or_insert_with(|| json!("forge_my_atlas_v1"));
    obj.entry("programs".to_string()).or_insert_with(|| json!([]));
    obj.entry("runs".to_string()).or_insert_with(|| json!([]));
    obj.entry("web_blocks".to_string()).or_insert_with(|| json!([]));
    let tags_value = obj.entry("metric_tags".to_string()).or_insert_with(|| json!([]));
    if !tags_value.is_array() {
        *tags_value = json!([]);
    }
    let tags = tags_value.as_array_mut().expect("metric_tags array");
    let now = now_ms();
    let parent_value = parent_geonode
        .as_ref()
        .map(|value| json!(value))
        .unwrap_or(Value::Null);
    let renderer_tool = if lat.is_some() { "planet_sphere" } else { "space_map" };
    let op = if lat.is_some() { "geo_anchor" } else { "astro_anchor" };
    let dtype = if lat.is_some() { "geojson" } else { "astrojson" };
    let formula = if let (Some(lat), Some(lon)) = (lat, lon) {
        format!("geo_anchor(body='{body}', lat={lat}, lon={lon}, coordinate_system='{coordinate_system}')")
    } else if let (Some(ra), Some(dec)) = (ra, dec) {
        format!("astro_anchor(body='{body}', ra={ra}, dec={dec}, coordinate_system='{coordinate_system}')")
    } else {
        format!("spatial_anchor(body='{body}', coordinate_system='{coordinate_system}')")
    };
    let mut geonode = json!({
        "kind": "metric_tag",
        "node_kind": node_kind,
        "tag": tag,
        "title": name,
        "domain": "geospatial",
        "op": op,
        "dtype": dtype,
        "body": body,
        "coordinate_system": coordinate_system,
        "lat": lat,
        "lon": lon,
        "ra": ra,
        "dec": dec,
        "distance": distance,
        "distance_unit": distance_unit,
        "parent": parent_value,
        "parent_geonode": parent_geonode,
        "geonode_level": geonode_level,
        "class": if is_mini { "user_mini_region" } else { "user_region" },
        "aliases": aliases,
        "metric_linkable": true,
        "accepts_metric_nodes": true,
        "metric_binding_fields": ["geo_ref", "geo_refs", "geonode", "geonode_tag", "parent_geonode"],
        "metric_binding_model": "Metric Nodes attach results to this spatial anchor with geo_ref=<geonode_or_minigeonode_tag>. Visual programs render metric_layers at the referenced anchors.",
        "linked_metric_tags": [],
        "formula": formula,
        "algorithm": "User/LLM-created spatial coordinate anchor saved from a Forge conversation.",
        "renderer_tool": renderer_tool,
        "coordinate_source": coordinate_source,
        "coordinate_confidence": confidence,
        "notes": notes,
        "reusable": true,
        "content_addressed": true,
        "source_content_included": false,
        "created_ms": now,
        "updated_ms": now
    });
    let tag_hash = format!("{:016x}", quick_hash_bytes(
        &serde_json::to_vec(&geonode).map_err(|e| format!("hash GeoNode: {e}"))?,
    ));
    geonode["tag_hash"] = json!(tag_hash);

    let mut created = true;
    if let Some(existing) = tags.iter_mut().find(|item| {
        item.get("tag")
            .and_then(Value::as_str)
            .map(|value| value.eq_ignore_ascii_case(&tag))
            .unwrap_or(false)
    }) {
        let created_ms = existing.get("created_ms").cloned().unwrap_or_else(|| json!(now));
        geonode["created_ms"] = created_ms;
        *existing = geonode.clone();
        created = false;
    } else {
        tags.push(geonode.clone());
    }
    let bytes = serde_json::to_vec_pretty(&atlas)
        .map_err(|e| format!("encode My Atlas: {e}"))?;
    fs::write(&path, bytes)
        .map_err(|e| format!("write My Atlas '{}': {e}", path.display()))?;
    Ok(json!({
        "status": if created { "created" } else { "updated" },
        "geonode": geonode,
        "atlas_path": path.display().to_string(),
        "source_content_included": false,
        "visual_program_hint": {
            "tool": renderer_tool,
            "body": body,
            "geonodes": [tag],
            "view": if renderer_tool == "planet_sphere" {
                format!("<view id='planet_view' type='planet' tool='planet_sphere' body='{body}' geonodes='{tag}' />")
            } else {
                format!("<view id='space_view' type='space' tool='space_map' body='{body}' geonodes='{tag}' />")
            }
        }
    }))
}

fn default_my_atlas_value(now: u64) -> Value {
    json!({
        "schema": "forge_my_atlas_v1",
        "created_ms": now,
        "updated_ms": now,
        "programs": [],
        "metric_tags": [],
        "runs": [],
        "web_blocks": [],
        "doctrine": {
            "content_addressed": true,
            "after_first_run": "same program_hash + same input hashes + same params returns an Atlas hit instead of recomputing",
            "agents_should": "reuse metric_tags and programs from My Atlas before creating new ones"
        }
    })
}

pub fn ensure_builtin_mars_geonodes(store_path: &Path) -> Result<(), String> {
    let atlas_dir = store_path.join("atlas");
    fs::create_dir_all(&atlas_dir)
        .map_err(|e| format!("create atlas dir '{}': {e}", atlas_dir.display()))?;
    let path = atlas_dir.join("my_atlas.json");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut atlas = if path.exists() {
        match read_json_value(&path) {
            Ok(value) => value,
            Err(err) => {
                let backup = atlas_dir.join(format!("my_atlas.corrupt-backup-{now}.json"));
                let _ = fs::rename(&path, &backup);
                eprintln!("[forge-atlas] repaired invalid my_atlas.json after decode failure: {err}");
                default_my_atlas_value(now)
            }
        }
    } else {
        default_my_atlas_value(now)
    };
    if !atlas.is_object() {
        atlas = default_my_atlas_value(now);
    }
    let obj = atlas.as_object_mut().expect("atlas object");
    obj.entry("schema".to_string()).or_insert_with(|| json!("forge_my_atlas_v1"));
    obj.entry("created_ms".to_string()).or_insert_with(|| json!(now));
    obj.entry("updated_ms".to_string()).or_insert_with(|| json!(now));
    obj.entry("programs".to_string()).or_insert_with(|| json!([]));
    obj.entry("runs".to_string()).or_insert_with(|| json!([]));
    obj.entry("web_blocks".to_string()).or_insert_with(|| json!([]));
    obj.entry("doctrine".to_string()).or_insert_with(|| {
        json!({
            "content_addressed": true,
            "after_first_run": "same program_hash + same input hashes + same params returns an Atlas hit instead of recomputing",
            "agents_should": "reuse metric_tags and programs from My Atlas before creating new ones"
        })
    });
    let tags_value = obj.entry("metric_tags".to_string()).or_insert_with(|| json!([]));
    if !tags_value.is_array() {
        *tags_value = json!([]);
    }
    let tags = tags_value.as_array_mut().expect("metric_tags array");
    let mut seen: HashSet<String> = tags
        .iter()
        .filter_map(|item| item.get("tag").and_then(Value::as_str).map(str::to_string))
        .collect();
    let geonodes = [
        ("mars_olympus_mons", "Olympus Mons", "major_region", 18.65, -133.8, Value::Null),
        ("mars_jezero_crater", "Jezero Crater", "major_region", 18.38, 77.58, Value::Null),
        ("mars_gale_crater", "Gale Crater", "major_region", -5.4, 137.8, Value::Null),
        ("mars_valles_marineris", "Valles Marineris", "major_region", -14.0, -59.0, Value::Null),
        ("mars_hellas_planitia", "Hellas Planitia", "major_region", -42.4, 70.5, Value::Null),
        ("mars_elysium_mons", "Elysium Mons", "major_region", 24.5, 146.7, Value::Null),
        ("mars_ascraeus_mons", "Ascraeus Mons", "major_region", 11.9, -104.1, Value::Null),
        ("mars_pavonis_mons", "Pavonis Mons", "major_region", 0.0, -113.0, Value::Null),
        ("mars_arsia_mons", "Arsia Mons", "major_region", -9.4, -121.0, Value::Null),
        ("mars_argyre_planitia", "Argyre Planitia", "major_region", -49.5, -40.0, Value::Null),
        ("mars_utopia_planitia", "Utopia Planitia", "major_region", 45.0, 110.0, Value::Null),
        ("mars_planum_boreum", "Planum Boreum", "major_region", 88.0, 15.0, Value::Null),
        ("mars_planum_australe", "Planum Australe", "major_region", -83.9, -160.0, Value::Null),
        ("mars_korolev_crater", "Korolev Crater", "major_region", 73.0, 165.0, Value::Null),
        ("mars_ius_chasma", "Ius Chasma", "sub_region", -7.0, -85.0, json!("mars_valles_marineris")),
        ("mars_tithonium_chasma", "Tithonium Chasma", "sub_region", -5.0, -84.0, json!("mars_valles_marineris")),
        ("mars_melas_chasma", "Melas Chasma", "sub_region", -10.0, -72.0, json!("mars_valles_marineris")),
        ("mars_coprates_chasma", "Coprates Chasma", "sub_region", -13.5, -61.0, json!("mars_valles_marineris")),
        ("mars_capri_chasma", "Capri Chasma", "sub_region", -14.0, -48.0, json!("mars_valles_marineris")),
        ("mars_eos_chasma", "Eos Chasma", "sub_region", -12.0, -42.0, json!("mars_valles_marineris")),
        ("mars_hebes_chasma", "Hebes Chasma", "sub_region", -1.0, -76.0, json!("mars_valles_marineris")),
        ("mars_ophir_chasma", "Ophir Chasma", "sub_region", -4.0, -72.5, json!("mars_valles_marineris")),
        ("mars_juventae_chasma", "Juventae Chasma", "sub_region", -4.5, -63.0, json!("mars_valles_marineris")),
        ("mars_ganges_chasma", "Ganges Chasma", "sub_region", -7.5, -49.0, json!("mars_valles_marineris")),
    ];
    let mut changed = false;
    for (tag, title, class, lat, lon, parent) in geonodes {
        let is_mini = class == "sub_region";
        let node_kind = if is_mini { "mini_geo_node" } else { "geo_node" };
        let geonode_level = if is_mini { "mini" } else { "region" };
        let parent_geonode = parent.clone();
        let sublocation_tags = if tag == "mars_valles_marineris" {
            json!([
                "mars_ius_chasma",
                "mars_tithonium_chasma",
                "mars_melas_chasma",
                "mars_coprates_chasma",
                "mars_capri_chasma",
                "mars_eos_chasma",
                "mars_hebes_chasma",
                "mars_ophir_chasma",
                "mars_juventae_chasma",
                "mars_ganges_chasma"
            ])
        } else {
            json!([])
        };
        if let Some(existing) = tags.iter_mut().find(|item| {
            item.get("tag")
                .and_then(Value::as_str)
                .map(|value| value == tag)
                .unwrap_or(false)
        }) {
            let before = existing.clone();
            if let Some(obj) = existing.as_object_mut() {
                obj.insert("node_kind".to_string(), json!(node_kind));
                obj.insert("geonode_level".to_string(), json!(geonode_level));
                obj.insert("parent_geonode".to_string(), parent_geonode);
                obj.insert("sublocation_tags".to_string(), sublocation_tags);
                obj.insert("metric_linkable".to_string(), json!(true));
                obj.insert("accepts_metric_nodes".to_string(), json!(true));
                obj.insert(
                    "metric_binding_fields".to_string(),
                    json!(["geo_ref", "geo_refs", "geonode", "geonode_tag", "parent_geonode"]),
                );
                obj.insert(
                    "metric_binding_model".to_string(),
                    json!("Metric Nodes attach results to this coordinate anchor with geo_ref=<geonode_or_minigeonode_tag>. Planet views render metric_layers at the referenced anchors."),
                );
                obj.entry("linked_metric_tags".to_string()).or_insert_with(|| json!([]));
            }
            if *existing != before {
                changed = true;
            }
            continue;
        }
        if !seen.insert(tag.to_string()) {
            continue;
        }
        tags.push(json!({
            "kind": "metric_tag",
            "node_kind": node_kind,
            "tag": tag,
            "title": title,
            "domain": "geospatial",
            "op": "geo_anchor",
            "dtype": "geojson",
            "body": "mars",
            "lat": lat,
            "lon": lon,
            "parent": parent,
            "parent_geonode": parent_geonode,
            "geonode_level": geonode_level,
            "class": class,
            "sublocation_tags": sublocation_tags,
            "metric_linkable": true,
            "accepts_metric_nodes": true,
            "metric_binding_fields": ["geo_ref", "geo_refs", "geonode", "geonode_tag", "parent_geonode"],
            "metric_binding_model": "Metric Nodes attach results to this coordinate anchor with geo_ref=<geonode_or_minigeonode_tag>. Planet views render metric_layers at the referenced anchors.",
            "linked_metric_tags": [],
            "formula": format!("geo_anchor(body='mars', lat={lat}, lon={lon})"),
            "algorithm": "Static Mars coordinate GeoNode seeded from the local Mars Planet bundle.",
            "renderer_tool": "planet_sphere",
            "source_bundle": "assets/lenses/mars-globe/mars-data.json",
            "reusable": true,
            "content_addressed": true,
            "source_content_included": false,
            "created_ms": now
        }));
        changed = true;
    }
    if changed || !path.exists() {
        let bytes = serde_json::to_vec_pretty(&atlas)
            .map_err(|e| format!("encode My Atlas: {e}"))?;
        fs::write(&path, bytes)
            .map_err(|e| format!("write My Atlas '{}': {e}", path.display()))?;
    }
    Ok(())
}

fn synthesize_programs_for_my_atlas(store_path: &Path, limit: usize) -> Result<Vec<Value>, String> {
    let dir = store_path.join("programs");
    let mut entries = Vec::<(SystemTime, PathBuf)>::new();
    match fs::read_dir(&dir) {
        Ok(read_dir) => {
            for entry in read_dir.filter_map(Result::ok) {
                if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(UNIX_EPOCH);
                entries.push((modified, entry.path()));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("read programs dir '{}': {err}", dir.display())),
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = Vec::new();
    for (_, path) in entries {
        let value = match read_json_value(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let metrics = value
            .pointer("/canonical/metrics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        out.push(json!({
            "kind": "program",
            "program_hash": value.get("program_hash").cloned().unwrap_or(Value::Null),
            "title": value.pointer("/canonical/title").cloned().unwrap_or(Value::Null),
            "domain": value.pointer("/canonical/domain").cloned().unwrap_or(Value::Null),
            "goal": value.pointer("/canonical/goal").cloned().unwrap_or(Value::Null),
            "program_kind": value.get("program_kind").cloned().unwrap_or(Value::Null),
            "status": value.get("status").cloned().unwrap_or(Value::Null),
            "metric_count": metrics.len(),
            "metric_tags": metrics
                .iter()
                .filter_map(|metric| {
                    metric
                        .get("tag")
                        .or_else(|| metric.get("id"))
                        .or_else(|| metric.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .take(32)
                .collect::<Vec<_>>(),
            "metric_nodes": metrics
                .iter()
                .take(32)
                .cloned()
                .collect::<Vec<_>>(),
            "created_ms": value.get("created_ms").cloned().unwrap_or(Value::Null),
            "reusable": true,
            "content_addressed": true,
            "source_content_included": false,
            "manifest_path": path.display().to_string()
        }));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

pub fn profile_settings(store_path: &Path, args: &Value) -> Result<Value, String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("get")
        .trim()
        .to_ascii_lowercase();
    let profile_path = profile_settings_path(store_path);
    let mut settings = read_profile_settings(&profile_path);
    let mut action_result = Value::Null;

    match action.as_str() {
        "get" | "status" => {}
        "update" | "patch" => {
            let patch = args
                .get("settings")
                .or_else(|| args.get("patch"))
                .ok_or_else(|| "profile update requires settings or patch".to_string())?;
            merge_redacted(&mut settings, patch);
            set_updated_ms(&mut settings);
            write_profile_settings(&profile_path, &settings)?;
            action_result = json!({ "updated": true });
        }
        "set_model" | "model" => {
            let provider = clean_optional_string(args.get("provider")).unwrap_or_else(|| "codex".to_string());
            let model = clean_optional_string(args.get("model_ref"))
                .or_else(|| clean_optional_string(args.get("model")))
                .ok_or_else(|| "set_model requires model_ref".to_string())?;
            validate_model_ref(&model)?;
            set_provider_field(&mut settings, &provider, "model_ref", json!(model));
            set_updated_ms(&mut settings);
            write_profile_settings(&profile_path, &settings)?;
            action_result = json!({ "updated": true, "provider": provider, "field": "model_ref" });
        }
        "set_reasoning" | "reasoning" | "set_reasoning_effort" => {
            let provider = clean_optional_string(args.get("provider")).unwrap_or_else(|| "codex".to_string());
            let effort = clean_optional_string(args.get("reasoning_effort"))
                .or_else(|| clean_optional_string(args.get("effort")))
                .ok_or_else(|| "set_reasoning requires reasoning_effort".to_string())?;
            let normalized = normalize_reasoning_effort(&effort)?;
            set_provider_field(&mut settings, &provider, "reasoning_effort", json!(normalized));
            set_updated_ms(&mut settings);
            write_profile_settings(&profile_path, &settings)?;
            action_result = json!({ "updated": true, "provider": provider, "field": "reasoning_effort" });
        }
        "save_gemini_key" | "set_gemini_api_key" => {
            let key = args
                .get("gemini_api_key")
                .or_else(|| args.get("api_key"))
                .and_then(Value::as_str)
                .ok_or_else(|| "save_gemini_key requires gemini_api_key".to_string())?;
            let path = gemini_env_path()?;
            dotenv_write_value(&path, "GEMINI_API_KEY", key.trim())?;
            action_result = json!({
                "saved": true,
                "provider": "gemini",
                "source": path.display().to_string(),
                "secret_returned": false
            });
        }
        "clear_gemini_key" => {
            let path = gemini_env_path()?;
            dotenv_remove_value(&path, "GEMINI_API_KEY")?;
            action_result = json!({
                "cleared": true,
                "provider": "gemini",
                "source": path.display().to_string(),
                "secret_returned": false
            });
        }
        "login_claude" | "connect_claude" => {
            action_result = start_claude_login()?;
        }
        "login_openai" | "connect_openai" | "login_codex" => {
            action_result = start_openai_login()?;
        }
        other => return Err(format!("unsupported profile action '{other}'")),
    }

    Ok(json!({
        "action": action,
        "result": action_result,
        "profile_path": profile_path.display().to_string(),
        "settings": redact_json(&settings),
        "providers": provider_config_status(),
        "content_policy": {
            "secrets_returned": false,
            "api_keys_returned": false,
            "tokens_returned": false
        },
        "token_safety": token_safety("Profile tool returns provider status and redacted settings. Secrets can be written when supplied by the user, never read back.")
    }))
}

fn list_job_summaries(
    store_path: &Path,
    limit: usize,
    query: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<Value>, String> {
    let mut entries = Vec::<(SystemTime, PathBuf)>::new();
    for dir in discover_job_dirs(store_path) {
        match fs::read_dir(&dir) {
            Ok(read_dir) => {
                for entry in read_dir.filter_map(Result::ok) {
                    if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                        continue;
                    }
                    let modified = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(UNIX_EPOCH);
                    entries.push((modified, entry.path()));
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) => {}
            Err(err) => return Err(format!("read jobs dir '{}': {err}", dir.display())),
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    let query = query.map(|v| v.to_ascii_lowercase());
    let status = status.map(|v| v.to_ascii_lowercase());
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (_, path) in entries {
        let job_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if job_id.is_empty() || !seen.insert(job_id.clone()) {
            continue;
        }
        let value = match read_json_value(&path) {
            Ok(value) => value,
            Err(error) => {
                out.push(json!({
                    "job_id": job_id,
                    "status": "decode_error",
                    "manifest_path": path.display().to_string(),
                    "error": error
                }));
                continue;
            }
        };
        if let Some(expected) = &status {
            let actual = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            if &actual != expected {
                continue;
            }
        }
        if let Some(q) = &query {
            let haystack = job_search_text(&value);
            if !haystack.contains(q) {
                continue;
            }
        }
        out.push(compact_job_summary(&value, Some(&path)));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

fn compact_job_summary(job: &Value, manifest_path: Option<&Path>) -> Value {
    let paths = source_file_paths(job);
    let source_refs = paths
        .iter()
        .enumerate()
        .take(8)
        .map(|(index, path)| file_ref_value("source", path, index, job))
        .collect::<Vec<_>>();
    json!({
        "job_id": value_at_any(job, &["job_id", "jobId"]).cloned().unwrap_or(Value::Null),
        "title": job_label(job),
        "status": value_at_any(job, &["status"]).cloned().unwrap_or(Value::Null),
        "kind": value_at_any(job, &["kind", "type"]).cloned().unwrap_or(Value::Null),
        "pinned": value_at_any(job, &["pinned", "is_pinned", "isPinned"]).cloned().unwrap_or(Value::Bool(false)),
        "protected": value_at_any(job, &["protected"]).cloned().unwrap_or(Value::Bool(false)),
        "created_ms": value_at_any(job, &["created_ms", "createdMs"]).cloned().unwrap_or(Value::Null),
        "last_modified_ms": value_at_any(job, &["last_modified_ms", "lastModifiedMs"]).cloned().unwrap_or_else(|| {
            manifest_path
                .and_then(|path| fs::metadata(path).ok())
                .and_then(|meta| meta.modified().ok())
                .map(system_time_ms)
                .map(Value::from)
                .unwrap_or(Value::Null)
        }),
        "bars": value_at_any(job, &["bars", "bar_count", "barCount"]).cloned().unwrap_or(Value::Null),
        "file_count": paths.len(),
        "source_refs": source_refs,
        "program_hash": value_at_any(job, &["program_hash", "programHash"]).cloned().unwrap_or(Value::Null),
        "strategy_hash": value_at_any(job, &["strategy_hash", "strategyHash"]).cloned().unwrap_or(Value::Null),
        "visualization_3d": compact_json_value(job.get("visualization_3d").unwrap_or(&Value::Null), 2),
        "visual_mapping": compact_json_value(job.get("visual_mapping").unwrap_or(&Value::Null), 2),
        "manifest_path": manifest_path.map(|path| path.display().to_string()),
        "content_included": false
    })
}

fn summarize_3d_view(artifact: &Value, mapping_view: Option<&Value>) -> Value {
    let mode = artifact
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("view")
        .to_string();
    let axes = mapping_view
        .and_then(|view| view.get("axes"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "mode": mode,
        "artifact": {
            "path": artifact.get("path").cloned().unwrap_or(Value::Null),
            "format": artifact.get("format").cloned().unwrap_or(Value::Null),
            "bytes": artifact.get("bytes").cloned().unwrap_or(Value::Null),
            "hash_algorithm": artifact.get("hash_algorithm").cloned().unwrap_or(Value::Null),
            "hash": artifact.get("hash").cloned().unwrap_or(Value::Null),
            "point_count": artifact.get("point_count").cloned().unwrap_or(Value::Null),
            "draw_mode": artifact.get("draw_mode").cloned().unwrap_or(Value::Null)
        },
        "legend": compact_json_value(artifact.get("legend").unwrap_or(&Value::Null), 3),
        "axes": compact_json_value(&axes, 3),
        "mapping_view": mapping_view.map(|view| compact_json_value(view, 2)).unwrap_or(Value::Null),
        "interpretation": mode_interpretation(&mode),
        "raw_points_included": false
    })
}

fn select_3d_artifact(job: &Value, mode_filter: Option<&str>) -> Option<Value> {
    let artifacts = job.get("artifacts_3d").and_then(Value::as_array)?;
    artifacts
        .iter()
        .find(|artifact| {
            let has_path = artifact
                .get("path")
                .and_then(Value::as_str)
                .map(|path| !path.trim().is_empty())
                .unwrap_or(false);
            if !has_path {
                return false;
            }
            if let Some(filter) = mode_filter {
                artifact
                    .get("mode")
                    .and_then(Value::as_str)
                    .map(|mode| mode.eq_ignore_ascii_case(filter))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .cloned()
        .or_else(|| {
            if mode_filter.is_some() {
                None
            } else {
                artifacts
                    .iter()
                    .find(|artifact| artifact.get("path").and_then(Value::as_str).is_some())
                    .cloned()
            }
        })
}

fn read_ply_points(path: &Path, max_points: usize) -> Result<Vec<Point3d>, String> {
    let file = fs::File::open(path).map_err(|e| format!("open PLY '{}': {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut vertex_count = None;
    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read PLY header '{}': {e}", path.display()))?;
        if n == 0 {
            return Err(format!("PLY '{}' ended before end_header", path.display()));
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("element vertex ") {
            vertex_count = rest.trim().parse::<usize>().ok();
        }
        if trimmed == "end_header" {
            break;
        }
    }
    let expected = vertex_count.unwrap_or(max_points).min(max_points);
    let mut points = Vec::with_capacity(expected.min(100_000));
    for _ in 0..expected {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read PLY point '{}': {e}", path.display()))?;
        if n == 0 {
            break;
        }
        let mut nums = line.split_whitespace();
        let Some(x) = nums.next().and_then(|v| v.parse::<f64>().ok()) else {
            continue;
        };
        let Some(y) = nums.next().and_then(|v| v.parse::<f64>().ok()) else {
            continue;
        };
        let Some(z) = nums.next().and_then(|v| v.parse::<f64>().ok()) else {
            continue;
        };
        let r = nums
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(200.0);
        let g = nums
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(200.0);
        let b = nums
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(200.0);
        let size = nums
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0);
        points.push(Point3d {
            x,
            y,
            z,
            r,
            g,
            b,
            size,
        });
    }
    Ok(points)
}

fn axis_stats(points: &[Point3d]) -> [AxisStats; 3] {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut sum = [0.0; 3];
    for point in points {
        let values = [point.x, point.y, point.z];
        for axis in 0..3 {
            min[axis] = min[axis].min(values[axis]);
            max[axis] = max[axis].max(values[axis]);
            sum[axis] += values[axis];
        }
    }
    let n = points.len().max(1) as f64;
    let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
    let mut var = [0.0; 3];
    for point in points {
        let values = [point.x, point.y, point.z];
        for axis in 0..3 {
            let delta = values[axis] - mean[axis];
            var[axis] += delta * delta;
        }
    }
    [
        AxisStats {
            min: min[0],
            max: max[0],
            mean: mean[0],
            std: (var[0] / n).sqrt(),
        },
        AxisStats {
            min: min[1],
            max: max[1],
            mean: mean[1],
            std: (var[1] / n).sqrt(),
        },
        AxisStats {
            min: min[2],
            max: max[2],
            mean: mean[2],
            std: (var[2] / n).sqrt(),
        },
    ]
}

fn axis_stats_json(stats: AxisStats) -> Value {
    json!({
        "min": round6(stats.min),
        "max": round6(stats.max),
        "mean": round6(stats.mean),
        "std": round6(stats.std),
        "range": round6(stats.max - stats.min)
    })
}

fn visual_channel_stats(points: &[Point3d]) -> Value {
    let mut min = [f64::INFINITY; 4];
    let mut max = [f64::NEG_INFINITY; 4];
    let mut sum = [0.0; 4];
    for point in points {
        let values = [point.r, point.g, point.b, point.size];
        for i in 0..4 {
            min[i] = min[i].min(values[i]);
            max[i] = max[i].max(values[i]);
            sum[i] += values[i];
        }
    }
    let n = points.len().max(1) as f64;
    let mean = [sum[0] / n, sum[1] / n, sum[2] / n, sum[3] / n];
    let mut var = [0.0; 4];
    for point in points {
        let values = [point.r, point.g, point.b, point.size];
        for i in 0..4 {
            let delta = values[i] - mean[i];
            var[i] += delta * delta;
        }
    }
    let stats = [
        AxisStats {
            min: min[0],
            max: max[0],
            mean: mean[0],
            std: (var[0] / n).sqrt(),
        },
        AxisStats {
            min: min[1],
            max: max[1],
            mean: mean[1],
            std: (var[1] / n).sqrt(),
        },
        AxisStats {
            min: min[2],
            max: max[2],
            mean: mean[2],
            std: (var[2] / n).sqrt(),
        },
        AxisStats {
            min: min[3],
            max: max[3],
            mean: mean[3],
            std: (var[3] / n).sqrt(),
        },
    ];
    json!({
        "rgb": {
            "red": axis_stats_json(stats[0]),
            "green": axis_stats_json(stats[1]),
            "blue": axis_stats_json(stats[2]),
            "mean_rgb": [round6(mean[0]), round6(mean[1]), round6(mean[2])]
        },
        "point_size": axis_stats_json(stats[3]),
        "raw_color_arrays_returned": false
    })
}

fn covariance_matrix(points: &[Point3d], axes: &[AxisStats; 3]) -> [[f64; 3]; 3] {
    let n = points.len().max(1) as f64;
    let mut cov = [[0.0; 3]; 3];
    for point in points {
        let values = [
            point.x - axes[0].mean,
            point.y - axes[1].mean,
            point.z - axes[2].mean,
        ];
        for row in 0..3 {
            for col in row..3 {
                cov[row][col] += values[row] * values[col];
            }
        }
    }
    for row in 0..3 {
        for col in row..3 {
            cov[row][col] /= n;
            cov[col][row] = cov[row][col];
        }
    }
    cov
}

fn correlation_matrix(cov: [[f64; 3]; 3], axes: &[AxisStats; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for row in 0..3 {
        for col in 0..3 {
            let denom = axes[row].std * axes[col].std;
            out[row][col] = if denom > f64::EPSILON {
                (cov[row][col] / denom).clamp(-1.0, 1.0)
            } else if row == col {
                1.0
            } else {
                0.0
            };
        }
    }
    out
}

fn matrix_json(matrix: [[f64; 3]; 3]) -> Value {
    json!([
        [round6(matrix[0][0]), round6(matrix[0][1]), round6(matrix[0][2])],
        [round6(matrix[1][0]), round6(matrix[1][1]), round6(matrix[1][2])],
        [round6(matrix[2][0]), round6(matrix[2][1]), round6(matrix[2][2])]
    ])
}

fn eigenvalues_symmetric_3x3(a: [[f64; 3]; 3]) -> [f64; 3] {
    let p1 = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
    if p1 <= f64::EPSILON {
        let mut values = [a[0][0].max(0.0), a[1][1].max(0.0), a[2][2].max(0.0)];
        values.sort_by(|left, right| right.total_cmp(left));
        return values;
    }
    let q = (a[0][0] + a[1][1] + a[2][2]) / 3.0;
    let p2 = (a[0][0] - q).powi(2)
        + (a[1][1] - q).powi(2)
        + (a[2][2] - q).powi(2)
        + 2.0 * p1;
    let p = (p2 / 6.0).sqrt();
    if p <= f64::EPSILON {
        return [q.max(0.0), q.max(0.0), q.max(0.0)];
    }
    let mut b = [[0.0; 3]; 3];
    for row in 0..3 {
        for col in 0..3 {
            b[row][col] = (a[row][col] - if row == col { q } else { 0.0 }) / p;
        }
    }
    let r = determinant_3x3(b) / 2.0;
    let phi = if r <= -1.0 {
        std::f64::consts::PI / 3.0
    } else if r >= 1.0 {
        0.0
    } else {
        r.acos() / 3.0
    };
    let eig1 = q + 2.0 * p * phi.cos();
    let eig3 = q + 2.0 * p * (phi + 2.0 * std::f64::consts::PI / 3.0).cos();
    let eig2 = 3.0 * q - eig1 - eig3;
    let mut values = [eig1.max(0.0), eig2.max(0.0), eig3.max(0.0)];
    values.sort_by(|left, right| right.total_cmp(left));
    values
}

fn determinant_3x3(m: [[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

fn analyze_voxels(
    points: &[Point3d],
    axes: &[AxisStats; 3],
    resolution: usize,
    max_hotspots: usize,
    max_components: usize,
) -> Value {
    let mut voxels = HashMap::<u64, VoxelAgg>::new();
    for point in points {
        let idx = point_voxel(point, axes, resolution);
        let key = voxel_key(idx);
        let agg = voxels.entry(key).or_insert(VoxelAgg {
            count: 0,
            sum: [0.0; 3],
        });
        agg.count += 1;
        agg.sum[0] += point.x;
        agg.sum[1] += point.y;
        agg.sum[2] += point.z;
    }
    let (mut components, component_lookup) = voxel_components(&voxels, resolution);
    let mut component_by_key = Map::new();
    for (key, component_id) in component_lookup {
        component_by_key.insert(key.to_string(), json!(component_id));
    }
    components.sort_by(|left, right| right.point_count.cmp(&left.point_count));
    let component_json = components
        .iter()
        .take(max_components)
        .map(|component| {
            json!({
                "id": component.id,
                "point_count": component.point_count,
                "point_fraction": round6(component.point_count as f64 / points.len().max(1) as f64),
                "voxel_count": component.voxel_count,
                "bbox_index": {
                    "min": component.min_idx,
                    "max": component.max_idx
                },
                "centroid": component.centroid.map(round6),
                "density_points_per_voxel": round6(component.point_count as f64 / component.voxel_count.max(1) as f64)
            })
        })
        .collect::<Vec<_>>();
    let mut voxel_counts = voxels
        .iter()
        .map(|(key, agg)| (*key, agg.count))
        .collect::<Vec<_>>();
    voxel_counts.sort_by(|a, b| b.1.cmp(&a.1));
    let hotspots = voxel_counts
        .iter()
        .take(max_hotspots)
        .map(|(key, count)| {
            let idx = decode_voxel_key(*key);
            json!({
                "voxel": idx,
                "center_normalized": [
                    round6((idx[0] as f64 + 0.5) / resolution as f64),
                    round6((idx[1] as f64 + 0.5) / resolution as f64),
                    round6((idx[2] as f64 + 0.5) / resolution as f64)
                ],
                "point_count": count,
                "point_fraction": round6(*count as f64 / points.len().max(1) as f64)
            })
        })
        .collect::<Vec<_>>();
    let occupied = voxels.len();
    let max_density = voxel_counts.first().map(|(_, count)| *count).unwrap_or(0);
    let entropy = normalized_entropy(voxel_counts.iter().map(|(_, count)| *count));
    let sparse_point_count = voxels
        .values()
        .filter(|agg| agg.count <= 1)
        .map(|agg| agg.count)
        .sum::<usize>();
    json!({
        "occupied_voxels": occupied,
        "possible_voxels": resolution.saturating_mul(resolution).saturating_mul(resolution),
        "occupancy_ratio": round6(occupied as f64 / resolution.saturating_mul(resolution).saturating_mul(resolution).max(1) as f64),
        "max_points_in_voxel": max_density,
        "density_entropy": round6(entropy),
        "sparse_point_count": sparse_point_count,
        "sparse_point_fraction": round6(sparse_point_count as f64 / points.len().max(1) as f64),
        "component_count": components.len(),
        "components": component_json,
        "hotspots": hotspots,
        "component_by_key": Value::Object(component_by_key)
    })
}

fn voxel_components(
    voxels: &HashMap<u64, VoxelAgg>,
    resolution: usize,
) -> (Vec<VoxelComponent>, HashMap<u64, usize>) {
    let mut visited = HashSet::<u64>::new();
    let mut out = Vec::new();
    let mut component_by_key = HashMap::<u64, usize>::new();
    for key in voxels.keys().copied() {
        if visited.contains(&key) {
            continue;
        }
        let id = out.len();
        let mut queue = VecDeque::from([key]);
        visited.insert(key);
        let mut voxel_count = 0usize;
        let mut point_count = 0usize;
        let mut min_idx = [usize::MAX; 3];
        let mut max_idx = [0usize; 3];
        let mut centroid_sum = [0.0; 3];
        while let Some(current) = queue.pop_front() {
            let idx = decode_voxel_key(current);
            if let Some(agg) = voxels.get(&current) {
                component_by_key.insert(current, id);
                voxel_count += 1;
                point_count += agg.count;
                for axis in 0..3 {
                    min_idx[axis] = min_idx[axis].min(idx[axis]);
                    max_idx[axis] = max_idx[axis].max(idx[axis]);
                    centroid_sum[axis] += agg.sum[axis];
                }
            }
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    for dz in -1i32..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        let nx = idx[0] as i32 + dx;
                        let ny = idx[1] as i32 + dy;
                        let nz = idx[2] as i32 + dz;
                        if nx < 0
                            || ny < 0
                            || nz < 0
                            || nx >= resolution as i32
                            || ny >= resolution as i32
                            || nz >= resolution as i32
                        {
                            continue;
                        }
                        let next = voxel_key([nx as usize, ny as usize, nz as usize]);
                        if voxels.contains_key(&next) && visited.insert(next) {
                            queue.push_back(next);
                        }
                    }
                }
            }
        }
        let denom = point_count.max(1) as f64;
        out.push(VoxelComponent {
            id,
            voxel_count,
            point_count,
            min_idx,
            max_idx,
            centroid: [
                centroid_sum[0] / denom,
                centroid_sum[1] / denom,
                centroid_sum[2] / denom,
            ],
        });
    }
    (out, component_by_key)
}

fn analyze_trajectory(
    points: &[Point3d],
    axes: &[AxisStats; 3],
    resolution: usize,
    component_by_key: Option<&Value>,
) -> Value {
    if points.len() < 2 {
        return Value::Null;
    }
    let mut steps = Vec::with_capacity(points.len().saturating_sub(1));
    let mut max_step = 0.0f64;
    for pair in points.windows(2) {
        let dist = distance3(pair[0], pair[1]);
        max_step = max_step.max(dist);
        steps.push(dist);
    }
    steps.sort_by(|left, right| left.total_cmp(right));
    let mean_step = steps.iter().sum::<f64>() / steps.len().max(1) as f64;
    let p95 = percentile_sorted(&steps, 0.95);
    let first = slice_centroid(&points[..(points.len() / 10).max(1)]);
    let last = slice_centroid(&points[points.len().saturating_sub((points.len() / 10).max(1))..]);
    let drift = [last[0] - first[0], last[1] - first[1], last[2] - first[2]];
    let diag = bounding_diag(axes).max(f64::EPSILON);
    let component_transitions = component_by_key
        .and_then(Value::as_object)
        .map(|lookup| {
            let mut transitions = 0usize;
            let mut previous = None::<u64>;
            for point in points {
                let key = voxel_key(point_voxel(point, axes, resolution)).to_string();
                let current = lookup.get(&key).and_then(Value::as_u64);
                if let (Some(prev), Some(cur)) = (previous, current) {
                    if prev != cur {
                        transitions += 1;
                    }
                }
                previous = current;
            }
            transitions
        })
        .unwrap_or(0);
    json!({
        "ordered_by_source_index": true,
        "mean_step": round6(mean_step),
        "p95_step": round6(p95),
        "max_step": round6(max_step),
        "mean_step_over_bbox_diag": round6(mean_step / diag),
        "start_to_end_drift": drift.map(round6),
        "start_to_end_drift_over_bbox_diag": round6(length3(drift) / diag),
        "component_transitions": component_transitions,
        "component_transitions_per_1000_points": round6(component_transitions as f64 * 1000.0 / points.len().max(1) as f64),
        "interpretation": if component_transitions > points.len() / 20 {
            "The ordered path frequently switches regimes/components."
        } else {
            "The ordered path is mostly continuous through the detected geometry."
        }
    })
}

fn analyze_outliers(points: &[Point3d], axes: &[AxisStats; 3], sparse_points: Option<&Value>) -> Value {
    let mut radial = Vec::with_capacity(points.len());
    for point in points {
        let dx = zscore(point.x, axes[0]);
        let dy = zscore(point.y, axes[1]);
        let dz = zscore(point.z, axes[2]);
        radial.push((dx * dx + dy * dy + dz * dz).sqrt());
    }
    radial.sort_by(|left, right| left.total_cmp(right));
    let strong = radial.iter().filter(|v| **v >= 3.5).count();
    let moderate = radial.iter().filter(|v| **v >= 2.5).count();
    json!({
        "moderate_z_radius_count": moderate,
        "strong_z_radius_count": strong,
        "moderate_fraction": round6(moderate as f64 / points.len().max(1) as f64),
        "strong_fraction": round6(strong as f64 / points.len().max(1) as f64),
        "p95_z_radius": round6(percentile_sorted(&radial, 0.95)),
        "p99_z_radius": round6(percentile_sorted(&radial, 0.99)),
        "sparse_voxel_point_count": sparse_points.cloned().unwrap_or(Value::Null),
        "raw_outlier_points_returned": false
    })
}

fn classify_3d_shape(variance_ratio: &[f64; 3], voxel: &Value) -> String {
    let component_count = voxel
        .get("component_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let largest_fraction = voxel
        .get("components")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("point_fraction"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    if component_count >= 4 && largest_fraction < 0.72 {
        "separated regime islands".to_string()
    } else if variance_ratio[0] > 0.78 && variance_ratio[1] < 0.18 {
        "dominant ridge or tube".to_string()
    } else if variance_ratio[2] < 0.08 && variance_ratio[1] > 0.18 {
        "flat shelf or folded surface".to_string()
    } else if variance_ratio[2] > 0.22 {
        "volumetric cloud".to_string()
    } else {
        "curved manifold".to_string()
    }
}

fn build_visual_read(shape: &str, mode: &str, voxel: &Value, outliers: &Value) -> Vec<String> {
    let component_count = voxel
        .get("component_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let occupancy = voxel
        .get("occupancy_ratio")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let strong_outlier_fraction = outliers
        .get("strong_fraction")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    vec![
        format!("The {mode} projection reads as a {shape}."),
        format!("Density is concentrated into {component_count} connected voxel component(s) with occupancy ratio {occupancy:.4}."),
        if strong_outlier_fraction > 0.02 {
            format!("Strong outliers are visible enough to deserve a focused local probe ({:.2}% of points).", strong_outlier_fraction * 100.0)
        } else {
            "Outliers are present but not dominant in the global geometry.".to_string()
        },
    ]
}

fn build_math_read(variance_ratio: &[f64; 3], corr: &[[f64; 3]; 3], trajectory: &Value) -> Vec<String> {
    let primary = variance_ratio[0];
    let residual = variance_ratio[2];
    let strongest_corr = [
        ("X/Y", corr[0][1].abs()),
        ("X/Z", corr[0][2].abs()),
        ("Y/Z", corr[1][2].abs()),
    ]
    .into_iter()
    .max_by(|left, right| left.1.total_cmp(&right.1))
    .unwrap_or(("X/Y", 0.0));
    let transitions = trajectory
        .get("component_transitions_per_1000_points")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    vec![
        format!("Primary variance explains {:.1}% of the geometry; residual 3D variance is {:.1}%.", primary * 100.0, residual * 100.0),
        format!("Strongest absolute axis coupling is {} at {:.3}.", strongest_corr.0, strongest_corr.1),
        format!("Ordered path crosses detected components at {:.2} transitions per 1000 points.", transitions),
    ]
}

fn strip_component_lookup(mut voxel: Value) -> Value {
    if let Value::Object(ref mut obj) = voxel {
        obj.remove("component_by_key");
    }
    voxel
}

fn point_voxel(point: &Point3d, axes: &[AxisStats; 3], resolution: usize) -> [usize; 3] {
    [
        quantize_axis(point.x, axes[0], resolution),
        quantize_axis(point.y, axes[1], resolution),
        quantize_axis(point.z, axes[2], resolution),
    ]
}

fn quantize_axis(value: f64, axis: AxisStats, resolution: usize) -> usize {
    let range = (axis.max - axis.min).max(f64::EPSILON);
    let normalized = ((value - axis.min) / range).clamp(0.0, 0.999_999);
    (normalized * resolution as f64).floor() as usize
}

fn voxel_key(idx: [usize; 3]) -> u64 {
    ((idx[0] as u64) << 42) | ((idx[1] as u64) << 21) | idx[2] as u64
}

fn decode_voxel_key(key: u64) -> [usize; 3] {
    [
        ((key >> 42) & 0x1f_ffff) as usize,
        ((key >> 21) & 0x1f_ffff) as usize,
        (key & 0x1f_ffff) as usize,
    ]
}

fn normalized_entropy<I>(counts: I) -> f64
where
    I: IntoIterator<Item = usize>,
{
    let counts = counts.into_iter().filter(|count| *count > 0).collect::<Vec<_>>();
    let total = counts.iter().sum::<usize>() as f64;
    if total <= 0.0 || counts.len() <= 1 {
        return 0.0;
    }
    let entropy = counts
        .iter()
        .map(|count| {
            let p = *count as f64 / total;
            -p * p.ln()
        })
        .sum::<f64>();
    entropy / (counts.len() as f64).ln().max(f64::EPSILON)
}

fn zscore(value: f64, axis: AxisStats) -> f64 {
    if axis.std > f64::EPSILON {
        (value - axis.mean) / axis.std
    } else {
        0.0
    }
}

fn distance3(left: Point3d, right: Point3d) -> f64 {
    ((left.x - right.x).powi(2) + (left.y - right.y).powi(2) + (left.z - right.z).powi(2))
        .sqrt()
}

fn length3(value: [f64; 3]) -> f64 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn bounding_diag(axes: &[AxisStats; 3]) -> f64 {
    length3([
        axes[0].max - axes[0].min,
        axes[1].max - axes[1].min,
        axes[2].max - axes[2].min,
    ])
}

fn slice_centroid(points: &[Point3d]) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for point in points {
        sum[0] += point.x;
        sum[1] += point.y;
        sum[2] += point.z;
    }
    let n = points.len().max(1) as f64;
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

fn percentile_sorted(values: &[f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let idx = ((values.len() - 1) as f64 * percentile.clamp(0.0, 1.0)).round() as usize;
    values[idx.min(values.len() - 1)]
}

fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
}

fn inspect_csv_metric_schema(path: &Path, sample_rows: usize) -> Result<Value, String> {
    let file = fs::File::open(path).map_err(|e| format!("open source csv '{}': {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|e| format!("read csv header '{}': {e}", path.display()))?;
    if header.trim().is_empty() {
        return Err(format!("source csv '{}' has no header", path.display()));
    }
    let delimiter = detect_delimiter(&header);
    let headers = parse_csv_line(&header, delimiter);
    let mut numeric_counts = vec![0usize; headers.len()];
    let mut sampled = 0usize;
    let mut line = String::new();
    while sampled < sample_rows {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read csv sample '{}': {e}", path.display()))?;
        if n == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(&line, delimiter);
        for (idx, count) in numeric_counts.iter_mut().enumerate() {
            if fields.get(idx).and_then(|v| parse_number(v, delimiter)).is_some() {
                *count += 1;
            }
        }
        sampled += 1;
    }
    let columns = headers
        .iter()
        .enumerate()
        .map(|(idx, name)| {
            let role = guess_column_role(name);
            json!({
                "index": idx,
                "name": name,
                "normalized": normalize_metric_key(name),
                "numeric_sample_ratio": if sampled > 0 { round6(numeric_counts[idx] as f64 / sampled as f64) } else { 0.0 },
                "role_guess": role,
                "raw_values_returned": false
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "delimiter": delimiter.to_string(),
        "column_count": headers.len(),
        "sampled_rows_for_type_inference": sampled,
        "columns": columns,
        "raw_rows_returned": false
    }))
}

fn derived_metric_catalog() -> Value {
    json!([
        { "name": "row_index", "meaning": "ordered source row coordinate", "params": [] },
        { "name": "time_index", "meaning": "normalized chronological coordinate", "params": [] },
        { "name": "open/high/low/close/volume", "meaning": "direct OHLCV source columns when present", "params": [] },
        { "name": "return_1", "meaning": "one-bar close-to-close return", "params": [] },
        { "name": "forward_return_N", "meaning": "future close return over N bars for edge coloring", "params": ["window"] },
        { "name": "momentum_N", "meaning": "close change over N bars", "params": ["window"] },
        { "name": "volatility_N", "meaning": "rolling standard deviation of returns", "params": ["window"] },
        { "name": "rsi_N", "meaning": "rolling RSI-like oscillator", "params": ["window"] },
        { "name": "volume_z_N", "meaning": "rolling volume z-score", "params": ["window"] },
        { "name": "cvd", "meaning": "cumulative signed volume proxy", "params": [] },
        { "name": "drawdown_N", "meaning": "close distance from rolling high", "params": ["window"] },
        { "name": "range", "meaning": "high minus low", "params": [] },
        { "name": "body", "meaning": "close minus open", "params": [] },
        { "name": "candle_balance", "meaning": "body divided by range", "params": [] }
    ])
}

fn metric_request(value: Option<&Value>, default: &str) -> MetricRequest {
    let (label, explicit_window) = match value {
        Some(Value::String(text)) => (text.trim().to_string(), None),
        Some(Value::Object(obj)) => {
            let metric = obj
                .get("metric")
                .or_else(|| obj.get("name"))
                .or_else(|| obj.get("column"))
                .and_then(Value::as_str)
                .unwrap_or(default)
                .trim()
                .to_string();
            let window = obj.get("window").and_then(Value::as_u64).map(|v| v as usize);
            (metric, window)
        }
        _ => (default.to_string(), None),
    };
    let (key, parsed_window) = normalize_metric_with_window(&label);
    MetricRequest {
        label,
        key,
        window: explicit_window.or(parsed_window).unwrap_or(14).clamp(1, 10_000),
    }
}

fn metric_summary(metric: &MetricRequest) -> Value {
    json!({
        "label": metric.label,
        "key": metric.key,
        "window": metric.window
    })
}

fn read_csv_series_for_metrics(
    path: &Path,
    metrics: &[&MetricRequest],
    max_rows: usize,
) -> Result<CsvSeries, String> {
    let file = fs::File::open(path).map_err(|e| format!("open source csv '{}': {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|e| format!("read csv header '{}': {e}", path.display()))?;
    if header.trim().is_empty() {
        return Err(format!("source csv '{}' has no header", path.display()));
    }
    let delimiter = detect_delimiter(&header);
    let headers = parse_csv_line(&header, delimiter);
    let mut by_key = HashMap::<String, usize>::new();
    for (idx, name) in headers.iter().enumerate() {
        by_key.insert(normalize_metric_key(name), idx);
    }
    let open_idx = find_column(&by_key, &["open", "o"]);
    let high_idx = find_column(&by_key, &["high", "h"]);
    let low_idx = find_column(&by_key, &["low", "l"]);
    let close_idx = find_column(&by_key, &["close", "c", "price", "last"]);
    let volume_idx = find_column(&by_key, &["volume", "vol", "tick_volume", "real_volume"]);
    let mut extra_indices = HashMap::<String, usize>::new();
    for metric in metrics {
        if matches!(
            metric.key.as_str(),
            "open" | "high" | "low" | "close" | "volume" | "price"
        ) {
            continue;
        }
        if let Some(idx) = by_key.get(&metric.key).copied() {
            extra_indices.insert(metric.key.clone(), idx);
        }
    }
    let mut series = CsvSeries {
        headers,
        delimiter,
        source_path: path.to_path_buf(),
        row_count: 0,
        truncated: false,
        open: Vec::new(),
        high: Vec::new(),
        low: Vec::new(),
        close: Vec::new(),
        volume: Vec::new(),
        extras: extra_indices
            .keys()
            .map(|key| (key.clone(), Vec::<f64>::new()))
            .collect(),
    };
    let mut line = String::new();
    while series.row_count < max_rows {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("read csv row '{}': {e}", path.display()))?;
        if n == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(&line, delimiter);
        series.open.push(field_number(&fields, open_idx, delimiter));
        series.high.push(field_number(&fields, high_idx, delimiter));
        series.low.push(field_number(&fields, low_idx, delimiter));
        series.close.push(field_number(&fields, close_idx, delimiter));
        series.volume.push(field_number(&fields, volume_idx, delimiter));
        for (key, idx) in &extra_indices {
            if let Some(values) = series.extras.get_mut(key) {
                values.push(field_number(&fields, Some(*idx), delimiter));
            }
        }
        series.row_count += 1;
    }
    let mut probe = String::new();
    series.truncated = reader.read_line(&mut probe).unwrap_or(0) > 0;
    fill_market_fallbacks(&mut series);
    Ok(series)
}

fn evaluate_metric(metric: &MetricRequest, series: &CsvSeries) -> Result<Vec<f64>, String> {
    let n = series.row_count;
    let key = metric.key.as_str();
    let out = match key {
        "row_index" => (0..n).map(|i| i as f64).collect(),
        "time_index" | "time" => (0..n)
            .map(|i| if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 })
            .collect(),
        "open" => series.open.clone(),
        "high" => series.high.clone(),
        "low" => series.low.clone(),
        "close" | "price" => series.close.clone(),
        "volume" => series.volume.clone(),
        "range" | "volatility_range" => series
            .high
            .iter()
            .zip(series.low.iter())
            .map(|(h, l)| h - l)
            .collect(),
        "body" | "candle_body" => series
            .close
            .iter()
            .zip(series.open.iter())
            .map(|(c, o)| c - o)
            .collect(),
        "hlc3" | "typical_price" => (0..n)
            .map(|i| (series.high[i] + series.low[i] + series.close[i]) / 3.0)
            .collect(),
        "return" | "return_1" | "log_return" => returns(&series.close, 1, key == "log_return"),
        "forward_return" | "future_return" | "edge" => forward_returns(&series.close, metric.window),
        "momentum" => momentum(&series.close, metric.window),
        "volatility" | "realized_volatility" => rolling_std(&returns(&series.close, 1, false), metric.window),
        "rsi" => rsi(&series.close, metric.window),
        "volume_z" | "volume_zscore" => rolling_zscore(&series.volume, metric.window),
        "cvd" | "cumulative_delta_volume" => cvd(&series.open, &series.close, &series.volume),
        "drawdown" => drawdown(&series.close, metric.window),
        "candle_balance" => (0..n)
            .map(|i| {
                let range = (series.high[i] - series.low[i]).abs();
                if range > f64::EPSILON {
                    (series.close[i] - series.open[i]) / range
                } else {
                    0.0
                }
            })
            .collect(),
        other => series
            .extras
            .get(other)
            .cloned()
            .ok_or_else(|| format!("unknown 3D metric '{other}'"))?,
    };
    Ok(out)
}

fn normalize_metric_values(values: &[f64], transform: &str, time_like: bool) -> Vec<f64> {
    if time_like {
        let finite = values.iter().copied().filter(|v| v.is_finite()).collect::<Vec<_>>();
        let min = finite.iter().copied().fold(f64::INFINITY, f64::min);
        let max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let span = (max - min).max(f64::EPSILON);
        return values
            .iter()
            .map(|v| if v.is_finite() { ((*v - min) / span) * 2.0 - 1.0 } else { f64::NAN })
            .collect();
    }
    let mut finite = values.iter().copied().filter(|v| v.is_finite()).collect::<Vec<_>>();
    if finite.is_empty() {
        return vec![f64::NAN; values.len()];
    }
    finite.sort_by(|a, b| a.total_cmp(b));
    let lower = if transform.eq_ignore_ascii_case("minmax") {
        *finite.first().unwrap_or(&0.0)
    } else {
        percentile_sorted(&finite, 0.02)
    };
    let upper = if transform.eq_ignore_ascii_case("minmax") {
        *finite.last().unwrap_or(&1.0)
    } else {
        percentile_sorted(&finite, 0.98)
    };
    let span = (upper - lower).abs().max(f64::EPSILON);
    values
        .iter()
        .map(|v| {
            if v.is_finite() {
                (((*v - lower) / span) * 2.0 - 1.0).clamp(-1.0, 1.0)
            } else {
                f64::NAN
            }
        })
        .collect()
}

fn model_3d_ply_bytes(mode: &str, points: &[Point3d]) -> Result<Vec<u8>, String> {
    if points.is_empty() {
        return Err("cannot write empty 3D model".to_string());
    }
    let mut text = String::with_capacity(256 + points.len().saturating_mul(64));
    text.push_str("ply\n");
    text.push_str("format ascii 1.0\n");
    text.push_str("comment Forge agent-modeled 3D mapping artifact\n");
    text.push_str(&format!("comment mode {}\n", safe_artifact_token(mode, "agent_model")));
    text.push_str(&format!("element vertex {}\n", points.len()));
    text.push_str("property float x\n");
    text.push_str("property float y\n");
    text.push_str("property float z\n");
    text.push_str("property uchar red\n");
    text.push_str("property uchar green\n");
    text.push_str("property uchar blue\n");
    text.push_str("property float size\n");
    text.push_str("end_header\n");
    for point in points {
        text.push_str(&format!(
            "{:.6} {:.6} {:.6} {} {} {} {:.3}\n",
            point.x,
            point.y,
            point.z,
            point.r.round().clamp(0.0, 255.0) as u8,
            point.g.round().clamp(0.0, 255.0) as u8,
            point.b.round().clamp(0.0, 255.0) as u8,
            point.size
        ));
    }
    Ok(text.into_bytes())
}

fn upsert_3d_artifact(job: &mut Value, artifact: Value) {
    let mode = artifact.get("mode").and_then(Value::as_str).unwrap_or("");
    let obj = ensure_object(job);
    let entry = obj
        .entry("artifacts_3d".to_string())
        .or_insert_with(|| json!([]));
    if !entry.is_array() {
        *entry = json!([]);
    }
    let items = entry.as_array_mut().expect("artifacts_3d is array");
    items.retain(|item| item.get("mode").and_then(Value::as_str) != Some(mode));
    items.push(artifact);
}

fn visual_view_to_3d_recipe(view: &Value, args: &Value, idx: usize) -> Value {
    let view_id = view
        .get("id")
        .and_then(Value::as_str)
        .map(|value| safe_artifact_token(value, "visual_view"))
        .unwrap_or_else(|| format!("visual_view_{}", idx + 1));
    let axes = view.get("axes").cloned().unwrap_or_else(|| {
        json!({
            "x": view.get("x").cloned().unwrap_or_else(|| json!("time_index")),
            "y": view.get("y").cloned().unwrap_or_else(|| json!("close")),
            "z": view.get("z").cloned().unwrap_or_else(|| json!("volatility_24"))
        })
    });
    json!({
        "mode": view_id,
        "view_id": view.get("id").cloned().unwrap_or(Value::Null),
        "title": view.get("title").cloned().unwrap_or(Value::Null),
        "objective": clean_optional_string(view.get("objective"))
            .or_else(|| clean_optional_string(args.get("program_goal")))
            .unwrap_or_else(|| "materialize the configured 3D visual program view locally".to_string()),
        "axes": axes,
        "color": view.get("color")
            .or_else(|| view.get("color_metric"))
            .or_else(|| view.get("colour"))
            .cloned()
            .unwrap_or_else(|| json!("forward_return_6")),
        "size": view.get("size")
            .or_else(|| view.get("size_metric"))
            .cloned()
            .unwrap_or_else(|| json!("volume_z_48")),
        "transform": view.get("transform").cloned().unwrap_or_else(|| json!("robust")),
        "params": view.get("params").cloned().unwrap_or(Value::Null),
        "filters": view.get("filters").cloned().unwrap_or(Value::Null)
    })
}

fn materialize_2d_visual_view(
    store_path: &Path,
    job_id: &str,
    view: &Value,
    metrics: &[Value],
    args: &Value,
    idx: usize,
) -> Result<Value, String> {
    let (manifest_path, mut job) = read_job_value(store_path, job_id)?;
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| store_path.join("jobs"));
    let artifacts_dir = manifest_dir.join(format!("{job_id}.artifacts"));
    fs::create_dir_all(&artifacts_dir)
        .map_err(|e| format!("create 2D artifact dir '{}': {e}", artifacts_dir.display()))?;
    let view_id = view
        .get("id")
        .and_then(Value::as_str)
        .map(|value| safe_artifact_token(value, "visual_2d"))
        .unwrap_or_else(|| format!("visual_2d_{}", idx + 1));
    let title = view
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Forge 2D visual view");
    let source_refs = source_file_paths(&job)
        .iter()
        .enumerate()
        .map(|(source_idx, path)| file_ref_value("source", path, source_idx, &job))
        .collect::<Vec<_>>();
    let doc = json!({
        "version": "forge.visual_view_2d.v1",
        "kind": "visualization_2d_contract",
        "job_id": job_id,
        "view_id": view_id,
        "title": title,
        "program_hash": args.get("program_hash").cloned().unwrap_or(Value::Null),
        "created_ms": now_ms(),
        "source_refs": source_refs,
        "axes": view.get("axes").cloned().unwrap_or_else(|| json!({ "x": "time_index", "y": "close" })),
        "overlays": view.get("overlays").cloned().unwrap_or_else(|| json!([])),
        "encodings": {
            "color": view.get("color").cloned().unwrap_or(Value::Null),
            "size": view.get("size").cloned().unwrap_or(Value::Null)
        },
        "metrics": compact_visual_metric_refs(metrics),
        "params": view.get("params").cloned().unwrap_or(Value::Null),
        "filters": view.get("filters").cloned().unwrap_or(Value::Null),
        "render_contract": {
            "viewer": "Forge 2D/3D file viewer",
            "view_is_recipe_not_data": true,
            "compute_series_locally": true,
            "raw_series_returned": false,
            "raw_input_returned": false
        },
        "selection_contract": {
            "select_returns": ["job_id", "program_hash", "view_id", "source_ref", "row_index_when_available", "metric_refs"],
            "raw_input_returned": false
        },
        "content_policy": {
            "source_content_included": false,
            "raw_rows_returned": false,
            "raw_series_returned": false,
            "download_by_reference_only": true
        }
    });
    let path = artifacts_dir.join(format!("{job_id}.2d.{view_id}.json"));
    let bytes =
        serde_json::to_vec_pretty(&doc).map_err(|e| format!("encode 2D view artifact: {e}"))?;
    fs::write(&path, &bytes)
        .map_err(|e| format!("write 2D view artifact '{}': {e}", path.display()))?;
    let hash = quick_file_hash_path(&path)?;
    let artifact = json!({
        "kind": "visualization_2d",
        "artifact_type": "view_contract",
        "view_id": view_id,
        "title": title,
        "format": "json",
        "mime": "application/json",
        "path": path.display().to_string(),
        "bytes": bytes.len() as u64,
        "hash_algorithm": "forge_fnv1a64",
        "hash": format!("{hash:016x}"),
        "mcp_injectable": true,
        "download": {
            "delivery": "by_reference",
            "path": path.display().to_string(),
            "do_not_inline": true
        }
    });
    upsert_2d_artifact(&mut job, artifact.clone());
    update_visual_2d_mapping_doc(&manifest_dir, job_id, &mut job, &artifact, view, metrics)?;
    let obj = ensure_object(&mut job);
    obj.insert("last_modified_ms".to_string(), json!(now_ms()));
    write_job_value(&manifest_path, &job)?;
    Ok(json!({
        "view_id": view_id,
        "title": title,
        "type": "2d",
        "status": "view_contract_created",
        "artifact": {
            "path": path.display().to_string(),
            "bytes": bytes.len() as u64,
            "hash_algorithm": "forge_fnv1a64",
            "hash": format!("{hash:016x}")
        },
        "axes": view.get("axes").cloned().unwrap_or(Value::Null),
        "metric_count": metrics.len(),
        "content_policy": {
            "source_content_included": false,
            "raw_rows_returned": false,
            "raw_series_returned": false,
            "download_by_reference_only": true
        }
    }))
}

fn compact_visual_metric_refs(metrics: &[Value]) -> Value {
    Value::Array(
        metrics
            .iter()
            .take(128)
            .map(|metric| {
                json!({
                    "id": metric.get("id").cloned().unwrap_or(Value::Null),
                    "tag": metric.get("tag").cloned().unwrap_or(Value::Null),
                    "name": metric.get("name").cloned().unwrap_or(Value::Null),
                    "op": metric.get("op").cloned().unwrap_or(Value::Null),
                    "inputs": metric.get("inputs").cloned().unwrap_or_else(|| json!([])),
                    "output": metric.get("output").cloned().unwrap_or(Value::Null),
                    "dtype": metric.get("dtype").cloned().unwrap_or(Value::Null),
                    "params": compact_json_value(&metric.get("params").cloned().unwrap_or(Value::Null), 3),
                    "content_included": false
                })
            })
            .collect::<Vec<_>>(),
    )
}

fn upsert_2d_artifact(job: &mut Value, artifact: Value) {
    let view_id = artifact.get("view_id").and_then(Value::as_str).unwrap_or("");
    let obj = ensure_object(job);
    let entry = obj
        .entry("artifacts_2d".to_string())
        .or_insert_with(|| json!([]));
    if !entry.is_array() {
        *entry = json!([]);
    }
    let items = entry.as_array_mut().expect("artifacts_2d is array");
    items.retain(|item| item.get("view_id").and_then(Value::as_str) != Some(view_id));
    items.push(artifact);
}

fn update_visual_2d_mapping_doc(
    manifest_dir: &Path,
    job_id: &str,
    job: &mut Value,
    artifact: &Value,
    view: &Value,
    metrics: &[Value],
) -> Result<(), String> {
    let artifacts_dir = manifest_dir.join(format!("{job_id}.artifacts"));
    fs::create_dir_all(&artifacts_dir)
        .map_err(|e| format!("create visual artifact dir '{}': {e}", artifacts_dir.display()))?;
    let visual_mapping_path = job
        .get("visual_mapping_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| artifacts_dir.join(format!("{job_id}.visual_mapping.json")));
    let mut mapping_doc = read_json_value(&visual_mapping_path).unwrap_or_else(|_| {
        json!({
            "version": "forge.visual_mapping.v1",
            "kind": "agent_modeled_visual_mapping",
            "job_id": job_id,
            "views": [],
            "content_policy": {
                "raw_input_included": false,
                "raw_series_inlined_to_llm": false,
                "point_cloud_content_inlined_to_llm": false,
                "download_by_reference_only": true
            }
        })
    });
    let view_id = artifact
        .get("view_id")
        .and_then(Value::as_str)
        .unwrap_or("visual_2d");
    let mapping_view = json!({
        "id": format!("visual_2d_{view_id}"),
        "title": artifact.get("title").cloned().unwrap_or(Value::Null),
        "type": "line_or_overlay_contract",
        "artifact_path": artifact.get("path").cloned().unwrap_or(Value::Null),
        "artifact_hash": artifact.get("hash").cloned().unwrap_or(Value::Null),
        "format": "json",
        "axes": view.get("axes").cloned().unwrap_or(Value::Null),
        "overlays": view.get("overlays").cloned().unwrap_or_else(|| json!([])),
        "metric_refs": compact_visual_metric_refs(metrics),
        "selection_contract": {
            "select_returns": ["job_id", "view_id", "artifact_path", "artifact_hash", "metric_refs"],
            "raw_input_returned": false,
            "raw_series_returned": false
        }
    });
    let obj = ensure_object(&mut mapping_doc);
    let views = obj.entry("views".to_string()).or_insert_with(|| json!([]));
    if !views.is_array() {
        *views = json!([]);
    }
    let items = views.as_array_mut().expect("views is array");
    let mapping_id = mapping_view.get("id").and_then(Value::as_str).unwrap_or("");
    items.retain(|item| item.get("id").and_then(Value::as_str) != Some(mapping_id));
    items.push(mapping_view);
    let bytes =
        serde_json::to_vec_pretty(&mapping_doc).map_err(|e| format!("encode visual mapping: {e}"))?;
    fs::write(&visual_mapping_path, &bytes)
        .map_err(|e| format!("write visual mapping '{}': {e}", visual_mapping_path.display()))?;
    let mapping_hash = quick_file_hash_path(&visual_mapping_path)?;
    let job_obj = ensure_object(job);
    job_obj.insert(
        "visualization_2d".to_string(),
        json!({
            "available": true,
            "view_count": job_obj.get("artifacts_2d").and_then(Value::as_array).map(Vec::len).unwrap_or(1),
            "download_by_reference_only": true,
            "mcp_injectable": true
        }),
    );
    job_obj.insert(
        "visual_mapping_path".to_string(),
        json!(visual_mapping_path.display().to_string()),
    );
    job_obj.insert(
        "visual_mapping".to_string(),
        json!({
            "available": true,
            "version": "forge.visual_mapping.v1",
            "kind": "agent_modeled_visual_mapping",
            "path": visual_mapping_path.display().to_string(),
            "bytes": bytes.len() as u64,
            "hash_algorithm": "forge_fnv1a64",
            "hash": format!("{mapping_hash:016x}"),
            "download_by_reference_only": true,
            "mcp_injectable": true
        }),
    );
    Ok(())
}

fn visual_mapping_artifact_value(path: &Path) -> Result<Value, String> {
    let exists = path.exists();
    let bytes = if exists {
        Some(
            fs::metadata(path)
                .map_err(|e| format!("metadata visual mapping '{}': {e}", path.display()))?
                .len(),
        )
    } else {
        None
    };
    let hash = if exists {
        Some(quick_file_hash_path(path)?)
    } else {
        None
    };
    Ok(json!({
        "kind": "visual_mapping",
        "path": path.display().to_string(),
        "exists": exists,
        "bytes": bytes,
        "hash_algorithm": "forge_fnv1a64",
        "hash": hash.map(|value| format!("{value:016x}")),
        "download_by_reference_only": true
    }))
}

fn update_visual_mapping_doc(
    manifest_dir: &Path,
    job_id: &str,
    job: &mut Value,
    artifact: &Value,
    recipe: &Value,
) -> Result<(), String> {
    let artifacts_dir = manifest_dir.join(format!("{job_id}.artifacts"));
    fs::create_dir_all(&artifacts_dir)
        .map_err(|e| format!("create 3D artifact dir '{}': {e}", artifacts_dir.display()))?;
    let visual_mapping_path = job
        .get("visual_mapping_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| artifacts_dir.join(format!("{job_id}.visual_mapping.json")));
    let mut mapping_doc = read_json_value(&visual_mapping_path).unwrap_or_else(|_| {
        json!({
            "version": "forge.visual_mapping.v1",
            "kind": "agent_modeled_visual_mapping",
            "job_id": job_id,
            "views": [],
            "content_policy": {
                "raw_input_included": false,
                "point_cloud_content_inlined_to_llm": false,
                "download_by_reference_only": true
            }
        })
    });
    let mode = artifact.get("mode").and_then(Value::as_str).unwrap_or("agent_model");
    let view = json!({
        "id": format!("agent_3d_{mode}"),
        "title": format!("Agent 3D {mode}"),
        "type": "point_cloud",
        "artifact_path": artifact.get("path").cloned().unwrap_or(Value::Null),
        "artifact_hash": artifact.get("hash").cloned().unwrap_or(Value::Null),
        "format": "ply",
        "point_count": artifact.get("point_count").cloned().unwrap_or(Value::Null),
        "legend": artifact.get("legend").cloned().unwrap_or(Value::Null),
        "axes": [
            { "axis": "x", "metric": recipe.pointer("/axes/x").cloned().unwrap_or(Value::Null) },
            { "axis": "y", "metric": recipe.pointer("/axes/y").cloned().unwrap_or(Value::Null) },
            { "axis": "z", "metric": recipe.pointer("/axes/z").cloned().unwrap_or(Value::Null) }
        ],
        "selection_contract": {
            "select_returns": ["job_id", "mode", "artifact_path", "artifact_hash", "bar_index_when_available"],
            "raw_input_returned": false
        }
    });
    let obj = ensure_object(&mut mapping_doc);
    let views = obj.entry("views".to_string()).or_insert_with(|| json!([]));
    if !views.is_array() {
        *views = json!([]);
    }
    let items = views.as_array_mut().expect("views is array");
    let view_id = view.get("id").and_then(Value::as_str).unwrap_or("");
    items.retain(|item| item.get("id").and_then(Value::as_str) != Some(view_id));
    items.push(view);
    let bytes =
        serde_json::to_vec_pretty(&mapping_doc).map_err(|e| format!("encode visual mapping: {e}"))?;
    fs::write(&visual_mapping_path, &bytes)
        .map_err(|e| format!("write visual mapping '{}': {e}", visual_mapping_path.display()))?;
    let mapping_hash = quick_file_hash_path(&visual_mapping_path)?;
    let job_obj = ensure_object(job);
    job_obj.insert(
        "visualization_3d".to_string(),
        json!({
            "available": true,
            "mode_count": job_obj.get("artifacts_3d").and_then(Value::as_array).map(|v| v.len()).unwrap_or(1),
            "download_by_reference_only": true,
            "mcp_injectable": true
        }),
    );
    job_obj.insert(
        "visual_mapping_path".to_string(),
        json!(visual_mapping_path.display().to_string()),
    );
    job_obj.insert(
        "visual_mapping".to_string(),
        json!({
            "available": true,
            "version": "forge.visual_mapping.v1",
            "kind": "agent_modeled_visual_mapping",
            "path": visual_mapping_path.display().to_string(),
            "bytes": bytes.len() as u64,
            "hash_algorithm": "forge_fnv1a64",
            "hash": format!("{mapping_hash:016x}"),
            "download_by_reference_only": true,
            "mcp_injectable": true
        }),
    );
    Ok(())
}

fn summarize_visual_selection(args: &Value, views: &[Value]) -> Value {
    let vertex = args
        .get("vertex_index")
        .or_else(|| args.pointer("/selection/vertex_index"))
        .or_else(|| args.pointer("/selection/vertexIndex"))
        .and_then(Value::as_u64);
    let Some(vertex_index) = vertex else {
        return Value::Null;
    };
    let mode = args
        .get("mode")
        .and_then(Value::as_str)
        .or_else(|| args.pointer("/selection/mode").and_then(Value::as_str))
        .unwrap_or("selected_view");
    let point_count = views
        .iter()
        .find(|view| view.get("mode").and_then(Value::as_str) == Some(mode))
        .or_else(|| views.first())
        .and_then(|view| view.pointer("/artifact/point_count"))
        .and_then(Value::as_u64);
    json!({
        "mode": mode,
        "vertex_index": vertex_index,
        "inside_view": point_count.map(|count| vertex_index < count).unwrap_or(true),
        "bar_index_hint": vertex_index,
        "raw_point_values_returned": false,
        "note": "For exported Forge 3D views, point order normally follows the source bar order unless the selected view says otherwise. Ask Forge for a bounded metric lookup if exact OHLC/feature values are needed."
    })
}

fn read_visual_mapping_doc(job: &Value) -> Result<Value, String> {
    let path = job
        .get("visual_mapping_path")
        .and_then(Value::as_str)
        .or_else(|| job.pointer("/visual_mapping/path").and_then(Value::as_str))
        .ok_or_else(|| "job has no visual_mapping_path".to_string())?;
    read_json_value(Path::new(path))
}

fn find_mapping_view_for_mode(views: &[Value], mode: &str) -> Option<Value> {
    let needle = mode.to_ascii_lowercase();
    views
        .iter()
        .find(|view| mapping_mode_name(view).to_ascii_lowercase() == needle)
        .cloned()
        .or_else(|| {
            views.iter().find(|view| {
                let text = [
                    view.get("id").and_then(Value::as_str).unwrap_or(""),
                    view.get("title").and_then(Value::as_str).unwrap_or(""),
                ]
                .join(" ")
                .to_ascii_lowercase();
                text.contains(&needle)
            })
            .cloned()
        })
}

fn mapping_mode_name(view: &Value) -> String {
    view.get("id")
        .and_then(Value::as_str)
        .and_then(|id| id.rsplit('_').next())
        .or_else(|| view.get("mode").and_then(Value::as_str))
        .or_else(|| view.get("title").and_then(Value::as_str))
        .unwrap_or("view")
        .to_string()
}

fn detect_delimiter(header: &str) -> char {
    let comma = header.matches(',').count();
    let semi = header.matches(';').count();
    let tab = header.matches('\t').count();
    if tab > comma.max(semi) {
        '\t'
    } else if semi > comma {
        ';'
    } else {
        ','
    }
}

fn parse_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.trim_end_matches(['\r', '\n']).chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            if quoted && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else {
                quoted = !quoted;
            }
        } else if ch == delimiter && !quoted {
            fields.push(current.trim().trim_matches('"').to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().trim_matches('"').to_string());
    fields
}

fn parse_number(raw: &str, delimiter: char) -> Option<f64> {
    let mut text = raw.trim().trim_matches('"').replace(' ', "");
    if delimiter == ';' {
        text = text.replace(',', ".");
    }
    if text.is_empty() || matches!(text.as_str(), "NaN" | "nan" | "null" | "NULL") {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn normalize_metric_key(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn normalize_metric_with_window(raw: &str) -> (String, Option<usize>) {
    let key = normalize_metric_key(raw);
    let aliases = [
        ("future_return", "forward_return"),
        ("fwd_return", "forward_return"),
        ("edge_return", "forward_return"),
        ("returns", "return_1"),
        ("ret", "return_1"),
        ("price", "close"),
        ("vol", "volume"),
        ("vol_z", "volume_z"),
    ];
    let mut canonical = aliases
        .iter()
        .find_map(|(from, to)| if key == *from { Some((*to).to_string()) } else { None })
        .unwrap_or(key);
    let mut window = None;
    if let Some((base, suffix)) = canonical.rsplit_once('_') {
        if let Ok(parsed) = suffix.parse::<usize>() {
            canonical = base.to_string();
            window = Some(parsed);
        }
    } else {
        let split_at = canonical
            .char_indices()
            .rev()
            .find(|(_, ch)| !ch.is_ascii_digit())
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(0);
        if split_at > 0 && split_at < canonical.len() {
            if let Ok(parsed) = canonical[split_at..].parse::<usize>() {
                canonical = canonical[..split_at].trim_end_matches('_').to_string();
                window = Some(parsed);
            }
        }
    }
    (canonical, window)
}

fn guess_column_role(name: &str) -> String {
    match normalize_metric_key(name).as_str() {
        "time" | "timestamp" | "date" | "datetime" => "time",
        "open" | "o" => "open",
        "high" | "h" => "high",
        "low" | "l" => "low",
        "close" | "c" | "price" | "last" => "close",
        "volume" | "vol" | "tick_volume" | "real_volume" => "volume",
        _ => "feature",
    }
    .to_string()
}

fn find_column(columns: &HashMap<String, usize>, aliases: &[&str]) -> Option<usize> {
    aliases
        .iter()
        .find_map(|alias| columns.get(&normalize_metric_key(alias)).copied())
}

fn field_number(fields: &[String], idx: Option<usize>, delimiter: char) -> f64 {
    idx.and_then(|i| fields.get(i))
        .and_then(|value| parse_number(value, delimiter))
        .unwrap_or(f64::NAN)
}

fn fill_market_fallbacks(series: &mut CsvSeries) {
    for i in 0..series.row_count {
        if !series.close[i].is_finite() {
            series.close[i] = [series.open[i], series.high[i], series.low[i]]
                .into_iter()
                .find(|v| v.is_finite())
                .unwrap_or(i as f64);
        }
        if !series.open[i].is_finite() {
            series.open[i] = series.close[i];
        }
        if !series.high[i].is_finite() {
            series.high[i] = series.open[i].max(series.close[i]);
        }
        if !series.low[i].is_finite() {
            series.low[i] = series.open[i].min(series.close[i]);
        }
        if !series.volume[i].is_finite() {
            series.volume[i] = 0.0;
        }
    }
}

fn returns(values: &[f64], window: usize, log_return: bool) -> Vec<f64> {
    let n = values.len();
    let w = window.max(1);
    (0..n)
        .map(|i| {
            if i < w || !values[i].is_finite() || !values[i - w].is_finite() {
                0.0
            } else if log_return && values[i] > 0.0 && values[i - w] > 0.0 {
                (values[i] / values[i - w]).ln()
            } else if values[i - w].abs() > f64::EPSILON {
                values[i] / values[i - w] - 1.0
            } else {
                values[i] - values[i - w]
            }
        })
        .collect()
}

fn forward_returns(values: &[f64], window: usize) -> Vec<f64> {
    let n = values.len();
    let w = window.max(1);
    (0..n)
        .map(|i| {
            if i + w >= n || !values[i].is_finite() || !values[i + w].is_finite() {
                0.0
            } else if values[i].abs() > f64::EPSILON {
                values[i + w] / values[i] - 1.0
            } else {
                values[i + w] - values[i]
            }
        })
        .collect()
}

fn momentum(values: &[f64], window: usize) -> Vec<f64> {
    let w = window.max(1);
    (0..values.len())
        .map(|i| if i < w { 0.0 } else { values[i] - values[i - w] })
        .collect()
}

fn rolling_std(values: &[f64], window: usize) -> Vec<f64> {
    let w = window.max(2);
    let mut out = vec![0.0; values.len()];
    for i in 0..values.len() {
        let start = i.saturating_sub(w - 1);
        let finite = values[start..=i]
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if finite.len() < 2 {
            continue;
        }
        let mean = finite.iter().sum::<f64>() / finite.len() as f64;
        let var = finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / finite.len() as f64;
        out[i] = var.sqrt();
    }
    out
}

fn rolling_zscore(values: &[f64], window: usize) -> Vec<f64> {
    let w = window.max(2);
    let mut out = vec![0.0; values.len()];
    for i in 0..values.len() {
        let start = i.saturating_sub(w - 1);
        let finite = values[start..=i]
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if finite.len() < 2 {
            continue;
        }
        let mean = finite.iter().sum::<f64>() / finite.len() as f64;
        let var = finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / finite.len() as f64;
        let std = var.sqrt();
        if std > f64::EPSILON {
            out[i] = (values[i] - mean) / std;
        }
    }
    out
}

fn rsi(close: &[f64], window: usize) -> Vec<f64> {
    let w = window.max(2);
    let mut out = vec![50.0; close.len()];
    for i in 1..close.len() {
        let start = i.saturating_sub(w - 1).max(1);
        let mut gain = 0.0;
        let mut loss = 0.0;
        for j in start..=i {
            let delta = close[j] - close[j - 1];
            if delta >= 0.0 {
                gain += delta;
            } else {
                loss -= delta;
            }
        }
        out[i] = if loss <= f64::EPSILON {
            100.0
        } else {
            100.0 - (100.0 / (1.0 + gain / loss))
        };
    }
    out
}

fn cvd(open: &[f64], close: &[f64], volume: &[f64]) -> Vec<f64> {
    let mut total = 0.0;
    let mut out = Vec::with_capacity(close.len());
    for i in 0..close.len() {
        let sign = if close[i] >= open[i] { 1.0 } else { -1.0 };
        total += sign * volume[i].max(0.0);
        out.push(total);
    }
    out
}

fn drawdown(close: &[f64], window: usize) -> Vec<f64> {
    let w = window.max(2);
    (0..close.len())
        .map(|i| {
            let start = i.saturating_sub(w - 1);
            let high = close[start..=i]
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(f64::NEG_INFINITY, f64::max);
            if high.is_finite() && high.abs() > f64::EPSILON {
                close[i] / high - 1.0
            } else {
                0.0
            }
        })
        .collect()
}

fn metric_is_time_like(key: &str) -> bool {
    matches!(key, "row_index" | "time_index" | "time")
}

fn color_ramp(value: f64) -> (f64, f64, f64) {
    let t = ((value.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0);
    let r = 40.0 + 215.0 * t;
    let g = 180.0 - 100.0 * (t - 0.5).abs() * 2.0;
    let b = 255.0 - 215.0 * t;
    (r, g.max(70.0), b)
}

fn safe_artifact_token(raw: &str, fallback: &str) -> String {
    let token = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if token.is_empty() {
        fallback.to_string()
    } else {
        token.chars().take(64).collect()
    }
}

fn mode_interpretation(mode: &str) -> &'static str {
    match mode.to_ascii_lowercase().as_str() {
        "phase" => "Phase view emphasizes cyclic position, local regime changes and repeated market structure over time.",
        "heightmap" => "Heightmap view projects the series as relief: high and low zones make trend, volatility and feature intensity easier to compare.",
        "manifold" => "Manifold view is for separation: clusters, folds and gaps suggest regimes or feature combinations worth testing.",
        "lattice" => "Lattice view organizes points into a structured grid so repeated states, buckets and transitions can be compared.",
        "candles3d" => "Candles3d view keeps the mental model close to the chart while using the third axis for the selected feature or metric.",
        _ => "This view is an addressable 3D projection of the result. Use its legend and axes, then run a compact lookup for exact values when needed.",
    }
}

fn atlas_entry_value(path: &Path, base: &Path, meta: &fs::Metadata) -> Value {
    json!({
        "path": path.display().to_string(),
        "relative_path": path.strip_prefix(base).unwrap_or(path).display().to_string(),
        "kind": if meta.is_dir() { "dir" } else { "file" },
        "bytes": if meta.is_file() { Some(meta.len()) } else { None },
        "modified_ms": meta.modified().ok().map(system_time_ms)
    })
}

fn scan_store_entries(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    scanned: &mut usize,
    total_bytes: &mut u64,
    entries: &mut Vec<Value>,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        *scanned = scanned.saturating_add(1);
        if meta.is_file() {
            *total_bytes = total_bytes.saturating_add(meta.len());
        }
        if entries.len() < max_entries {
            entries.push(atlas_entry_value(&path, dir, &meta));
        }
        if meta.is_dir() && depth < max_depth {
            scan_store_entries(
                &path,
                depth + 1,
                max_depth,
                max_entries,
                scanned,
                total_bytes,
                entries,
            );
        }
    }
}

fn count_json_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|read_dir| {
            read_dir
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("json"))
                .count()
        })
        .unwrap_or(0)
}

fn profile_settings_path(store_path: &Path) -> PathBuf {
    store_path.join("profile").join("settings.json")
}

fn read_profile_settings(path: &Path) -> Value {
    read_json_value(path).unwrap_or_else(|_| {
        json!({
            "version": "forge.profile.v1",
            "providers": {
                "codex": { "model_ref": "gpt-5.3-codex", "reasoning_effort": "medium" },
                "gemini": { "model_ref": "gemini-default", "reasoning_effort": "medium" },
                "claude": { "model_ref": "claude-code-default", "reasoning_effort": "medium" }
            }
        })
    })
}

fn write_profile_settings(path: &Path, settings: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create profile settings dir: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("encode profile settings: {e}"))?;
    fs::write(&tmp, bytes).map_err(|e| format!("write profile settings tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("replace profile settings: {e}"))
}

fn set_provider_field(settings: &mut Value, provider: &str, field: &str, value: Value) {
    let root = ensure_object(settings);
    let providers = root
        .entry("providers".to_string())
        .or_insert_with(|| json!({}));
    let providers = ensure_object(providers);
    let provider_value = providers
        .entry(provider.to_ascii_lowercase())
        .or_insert_with(|| json!({}));
    ensure_object(provider_value).insert(field.to_string(), value);
}

fn set_updated_ms(settings: &mut Value) {
    ensure_object(settings).insert("updated_ms".to_string(), json!(now_ms()));
}

fn merge_redacted(target: &mut Value, patch: &Value) {
    if let (Some(target_obj), Some(patch_obj)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            if is_secret_key(key) {
                target_obj.insert(key.clone(), json!("[redacted]"));
                continue;
            }
            if value.is_object() {
                let child = target_obj.entry(key.clone()).or_insert_with(|| json!({}));
                merge_redacted(child, value);
            } else {
                target_obj.insert(key.clone(), redact_json(value));
            }
        }
    } else {
        *target = redact_json(patch);
    }
}

fn provider_config_status() -> Value {
    json!({
        "codex": codex_status(),
        "gemini": gemini_status(),
        "claude": claude_status()
    })
}

fn codex_status() -> Value {
    let auth_path = home_dir().map(|home| home.join(".codex").join("auth.json"));
    let configured = auth_path
        .as_ref()
        .and_then(|path| read_json_value(path).ok())
        .and_then(|value| value.get("tokens").cloned())
        .map(|tokens| {
            tokens
                .get("access_token")
                .and_then(Value::as_str)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
                || tokens
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
        })
        .unwrap_or(false);
    json!({
        "configured": configured,
        "auth_source": auth_path.map(|path| path.display().to_string()).unwrap_or_else(|| "none".to_string()),
        "secret_returned": false
    })
}

fn gemini_status() -> Value {
    let env_key = env_value_present("GEMINI_API_KEY") || env_value_present("GOOGLE_API_KEY");
    let env_path = gemini_env_path().ok();
    let env_file_key = env_path
        .as_ref()
        .and_then(|path| dotenv_read_value(path, "GEMINI_API_KEY"))
        .is_some();
    let oauth = home_dir()
        .map(|home| home.join(".gemini").join("oauth_creds.json"))
        .map(|path| file_present(&path))
        .unwrap_or(false);
    json!({
        "configured": env_key || env_file_key || oauth,
        "api_key_configured": env_key || env_file_key,
        "oauth_configured": oauth,
        "auth_source": if env_key { "environment" } else if env_file_key { "~/.gemini/.env" } else if oauth { "~/.gemini/oauth_creds.json" } else { "none" },
        "env_path": env_path.map(|path| path.display().to_string()),
        "secret_returned": false
    })
}

fn claude_status() -> Value {
    let credentials_path = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".claude")))
        .map(|dir| dir.join(".credentials.json"));
    let credentials = credentials_path
        .as_ref()
        .map(|path| file_present(path))
        .unwrap_or(false);
    let oauth_env = env_value_present("CLAUDE_CODE_OAUTH_TOKEN");
    json!({
        "configured": credentials || oauth_env,
        "oauth_env_configured": oauth_env,
        "credentials_configured": credentials,
        "credentials_path": credentials_path.map(|path| path.display().to_string()),
        "cli_hint": command_binary_hint("claude"),
        "secret_returned": false
    })
}

fn start_claude_login() -> Result<Value, String> {
    let candidate = command_binary_hint("claude")
        .and_then(|raw| {
            if raw == "claude" {
                Some(PathBuf::from("claude"))
            } else {
                Some(PathBuf::from(raw))
            }
        })
        .unwrap_or_else(|| PathBuf::from("claude"));
    let command = format!("{} auth login", candidate.display());
    #[cfg(windows)]
    {
        let quoted = candidate.display().to_string().replace('\'', "''");
        let script = format!(
            "Write-Host 'Forge is starting Claude.ai subscription OAuth.'; & '{quoted}' auth login; Write-Host ''; Write-Host 'When login is finished, return to Forge and refresh providers.'"
        );
        Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                "powershell",
                "-NoExit",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .spawn()
            .map_err(|e| format!("failed to start Claude subscription login: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        Command::new(&candidate)
            .args(["auth", "login"])
            .spawn()
            .map_err(|e| format!("failed to start Claude subscription login: {e}"))?;
    }
    Ok(json!({
        "started": true,
        "command": command,
        "message": "Claude Code subscription login opened. Choose Claude.ai Pro/Max OAuth.",
        "secret_returned": false
    }))
}

fn start_openai_login() -> Result<Value, String> {
    let command = "codex login".to_string();
    #[cfg(windows)]
    {
        let script = "$ErrorActionPreference='Continue'; \
            $localCodex = Join-Path $env:USERPROFILE '.codex\\.sandbox-bin\\codex.exe'; \
            if (Test-Path -LiteralPath $localCodex) { \
                Write-Host 'Forge is starting Codex OAuth for your OpenAI subscription.'; \
                & $localCodex login; \
            } elseif (Get-Command codex -ErrorAction SilentlyContinue) { \
                Write-Host 'Forge is starting Codex OAuth for your OpenAI subscription.'; \
                codex login; \
            } else { \
                Write-Host 'Codex CLI was not found in PATH.'; \
            }; \
            Write-Host ''; \
            Write-Host 'Forge detects Codex OAuth at ~/.codex/auth.json.'";
        Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                "powershell",
                "-NoExit",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .spawn()
            .map_err(|e| format!("failed to start OpenAI sign-in: {e}"))?;
    }
    Ok(json!({
        "started": cfg!(windows),
        "command": command,
        "message": if cfg!(windows) { "Codex OAuth sign-in opened." } else { "Run `codex login` in a terminal, then refresh Forge provider status." },
        "secret_returned": false
    }))
}

fn command_binary_hint(name: &str) -> Option<String> {
    let env_key = match name {
        "claude" => "FORGE_CLAUDE_BIN",
        "gemini" => "FORGE_GEMINI_BIN",
        "codex" => "FORGE_CODEX_BIN",
        _ => "",
    };
    if !env_key.is_empty() {
        if let Some(path) = std::env::var_os(env_key) {
            return Some(PathBuf::from(path).display().to_string());
        }
    }
    let mut candidates = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let npm = PathBuf::from(appdata).join("npm");
        candidates.push(npm.join(format!("{name}.cmd")));
        candidates.push(npm.join(format!("{name}.exe")));
        candidates.push(npm.join(name));
    }
    if let Some(home) = home_dir() {
        let npm = home.join("AppData").join("Roaming").join("npm");
        candidates.push(npm.join(format!("{name}.cmd")));
        candidates.push(npm.join(format!("{name}.exe")));
        candidates.push(npm.join(name));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            candidates.push(dir.join(format!("{name}.cmd")));
            candidates.push(dir.join(format!("{name}.exe")));
            candidates.push(dir.join(name));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
        .or_else(|| Some(name.to_string()))
}

fn compact_json_value(value: &Value, depth: usize) -> Value {
    match value {
        Value::Array(items) => {
            if depth == 0 {
                return json!({ "type": "array", "len": items.len() });
            }
            if items.len() > MAX_COMPACT_ARRAY {
                json!({
                    "type": "array",
                    "len": items.len(),
                    "sample": items.iter().take(MAX_COMPACT_ARRAY).map(|item| compact_json_value(item, depth - 1)).collect::<Vec<_>>()
                })
            } else {
                Value::Array(
                    items
                        .iter()
                        .map(|item| compact_json_value(item, depth - 1))
                        .collect(),
                )
            }
        }
        Value::Object(obj) => {
            if depth == 0 {
                return json!({
                    "type": "object",
                    "keys": obj.keys().take(24).cloned().collect::<Vec<_>>()
                });
            }
            let mut out = Map::new();
            for (key, item) in obj.iter().take(48) {
                if matches!(
                    key.as_str(),
                    "positions" | "colors" | "sizes" | "points" | "vertices" | "rows" | "data"
                ) {
                    out.insert(
                        key.clone(),
                        match item {
                            Value::Array(items) => json!({ "type": "array", "len": items.len(), "omitted": true }),
                            _ => json!({ "omitted": true }),
                        },
                    );
                } else {
                    out.insert(key.clone(), compact_json_value(item, depth - 1));
                }
            }
            Value::Object(out)
        }
        Value::String(text) if text.len() > MAX_COMPACT_STRING => {
            json!({
                "text": text.chars().take(MAX_COMPACT_STRING).collect::<String>(),
                "truncated": true,
                "chars": text.chars().count()
            })
        }
        other => other.clone(),
    }
}

fn redact_json(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut out = Map::new();
            for (key, item) in obj {
                if is_secret_key(key) {
                    out.insert(key.clone(), json!("[redacted]"));
                } else {
                    out.insert(key.clone(), redact_json(item));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_json).collect()),
        other => other.clone(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["key", "token", "secret", "password", "credential", "access", "refresh"]
        .iter()
        .any(|needle| key.contains(needle))
}

fn compact_content_policy() -> Value {
    json!({
        "source_content_included": false,
        "artifact_content_included": false,
        "full_logs_included": false,
        "secrets_included": false,
        "large_values_are_references": true
    })
}

fn token_safety(note: &str) -> Value {
    json!({
        "raw_files_read_into_llm": false,
        "large_artifacts_inlined": false,
        "bounded_response": true,
        "note": note
    })
}

fn discover_job_dirs(store_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    push_unique_path(&mut dirs, store_path.join("jobs"));
    if let Some(path) = std::env::var_os("FORGE_JOBS_DIR") {
        push_unique_path(&mut dirs, PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        push_unique_path(&mut dirs, PathBuf::from(path).join("jobs"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        push_unique_path(
            &mut dirs,
            PathBuf::from(appdata)
                .join("com.forge.ui")
                .join("forge-store")
                .join("jobs"),
        );
    }
    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn read_job_value(store_path: &Path, job_id: &str) -> Result<(PathBuf, Value), String> {
    validate_job_id(job_id)?;
    let filename = format!("{job_id}.json");
    for dir in discover_job_dirs(store_path) {
        let path = dir.join(&filename);
        if path.exists() {
            return read_json_value(&path).map(|value| (path, value));
        }
    }
    Err(format!("Forge job '{job_id}' not found"))
}

fn write_job_value(path: &Path, value: &Value) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| format!("encode job manifest: {e}"))?;
    fs::write(&tmp, bytes).map_err(|e| format!("write job manifest tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("replace job manifest: {e}"))
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("read json '{}': {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("decode json '{}': {e}", path.display()))
}

fn validate_job_id(job_id: &str) -> Result<(), String> {
    if job_id.is_empty()
        || !job_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid job_id characters".to_string());
    }
    Ok(())
}

fn source_file_paths(job: &Value) -> Vec<PathBuf> {
    if let Some(paths) = job.get("file_paths").and_then(Value::as_array) {
        let out = paths
            .iter()
            .filter_map(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return out;
        }
    }
    job.get("file_path")
        .or_else(|| job.get("filePath"))
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(|path| vec![PathBuf::from(path)])
        .unwrap_or_default()
}

fn file_ref_value(kind: &str, path: &Path, index: usize, job: &Value) -> Value {
    let meta = fs::metadata(path).ok();
    json!({
        "kind": kind,
        "index": index,
        "path": path.display().to_string(),
        "name": path.file_name().and_then(|v| v.to_str()).unwrap_or("").to_string(),
        "extension": path.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase(),
        "exists": meta.is_some(),
        "bytes": meta.as_ref().map(|m| m.len()),
        "modified_ms": meta.and_then(|m| m.modified().ok()).map(system_time_ms),
        "manifest_file_hash": if index == 0 { job.get("file_hash").cloned().unwrap_or(Value::Null) } else { Value::Null },
        "content_included": false
    })
}

fn value_at_any<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn job_label(job: &Value) -> String {
    value_at_any(job, &["title", "original_file_name", "originalFileName"])
        .and_then(Value::as_str)
        .or_else(|| {
            job.get("original_file_names")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str)
        })
        .or_else(|| {
            job.get("file_path")
                .or_else(|| job.get("filePath"))
                .and_then(Value::as_str)
                .and_then(|path| Path::new(path).file_name())
                .and_then(|path| path.to_str())
        })
        .or_else(|| value_at_any(job, &["job_id", "jobId"]).and_then(Value::as_str))
        .unwrap_or("Forge session")
        .to_string()
}

fn job_search_text(job: &Value) -> String {
    let mut parts = vec![
        job_label(job),
        value_at_any(job, &["job_id", "jobId"])
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        job.get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    ];
    for path in source_file_paths(job) {
        parts.push(path.display().to_string());
    }
    parts.join(" ").to_ascii_lowercase()
}

fn document_file_type(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    if lower.ends_with(".csv") || lower.contains("csv") {
        "csv"
    } else if lower.ends_with(".json") {
        "json"
    } else if lower.ends_with(".pdf") {
        "pdf"
    } else if lower.ends_with(".txt") || lower.ends_with(".log") {
        "text"
    } else if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
        "spreadsheet"
    } else {
        "document"
    }
    .to_string()
}

fn brain_node(store_path: &Path) -> Result<MonsterNode, String> {
    fs::create_dir_all(store_path)
        .map_err(|e| format!("create Forge brain store '{}': {e}", store_path.display()))?;
    let store = Store::open(store_path.to_path_buf())
        .map_err(|e| format!("open Forge brain store '{}': {e}", store_path.display()))?;
    Ok(MonsterNode::new(
        store,
        MemoryGovernor::one_percent_assumed_host(),
    ))
}

fn brain_scope(args: &Value) -> String {
    clean_optional_string(args.get("scope").or_else(|| args.get("section")))
        .map(|value| clean_ref_segment(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "global".to_string())
}

fn brain_memory_layer(args: &Value, kind: &str, source: &str) -> String {
    if let Some(layer) = brain_memory_layer_arg(args) {
        return layer;
    }
    let hint = format!("{kind} {source}").to_ascii_lowercase();
    if hint.contains("instruction")
        || hint.contains("prompt")
        || hint.contains("policy")
        || hint.contains("procedure")
        || hint.contains("workflow")
        || hint.contains("rule")
    {
        "procedural".to_string()
    } else if hint.contains("profile")
        || hint.contains("fact")
        || hint.contains("knowledge")
        || hint.contains("preference")
        || hint.contains("entity")
    {
        "semantic".to_string()
    } else {
        "episodic".to_string()
    }
}

fn brain_memory_layer_arg(args: &Value) -> Option<String> {
    clean_optional_string(
        args.get("memory_layer")
            .or_else(|| args.get("memory_kind"))
            .or_else(|| args.get("layer")),
    )
    .and_then(|value| normalize_memory_layer(&value))
}

fn normalize_memory_layer(value: &str) -> Option<String> {
    let layer = clean_ref_segment(value);
    BRAIN_MEMORY_LAYERS
        .iter()
        .find(|candidate| **candidate == layer)
        .map(|candidate| (*candidate).to_string())
}

fn scoped_layer_note_ref(scope: &str, layer: &str) -> String {
    format!(
        "{}{}/{}/latest",
        BRAIN_LLM_NOTE_LAYER_REF_PREFIX,
        clean_ref_segment(scope),
        normalize_memory_layer(layer).unwrap_or_else(|| "episodic".to_string())
    )
}

fn scoped_layer_index_ref(scope: &str, layer: &str) -> String {
    format!(
        "{}{}/{}/recent",
        BRAIN_LLM_NOTE_INDEX_REF_PREFIX,
        clean_ref_segment(scope),
        normalize_memory_layer(layer).unwrap_or_else(|| "episodic".to_string())
    )
}

fn scoped_fact_ref(scope: &str, layer: &str, fact_key: &str) -> String {
    format!(
        "{}{}/{}/{}",
        BRAIN_LLM_FACT_REF_PREFIX,
        clean_ref_segment(scope),
        normalize_memory_layer(layer).unwrap_or_else(|| "episodic".to_string()),
        clean_ref_segment(fact_key)
    )
}

fn recent_brain_notes(
    node: &MonsterNode,
    scope: &str,
    layer: Option<&str>,
    limit: usize,
    include_expired: bool,
) -> Value {
    let layers = layer
        .map(|layer| vec![layer.to_string()])
        .unwrap_or_else(|| BRAIN_MEMORY_LAYERS.iter().map(|layer| (*layer).to_string()).collect());
    let mut seen = HashSet::new();
    let mut notes = Vec::new();
    for layer in layers {
        let index_ref = scoped_layer_index_ref(scope, &layer);
        for hash in brain_note_index_hashes(node.store(), &index_ref) {
            if !seen.insert(hash) {
                continue;
            }
            let Some(bytes) = node.store().load(&hash) else {
                continue;
            };
            let explanation = explain_brain_blob(hash, &bytes);
            if !include_expired
                && explanation
                    .get("temporal_status")
                    .and_then(|status| status.get("status"))
                    .and_then(Value::as_str)
                    .map(|status| status != "active")
                    .unwrap_or(false)
            {
                continue;
            }
            notes.push(explanation);
            if notes.len() >= limit {
                return Value::Array(notes);
            }
        }
    }
    Value::Array(notes)
}

fn brain_note_index_hashes(store: &Store, index_ref: &str) -> Vec<Hash> {
    let Some(index_hash) = store.lookup_ref(index_ref) else {
        return Vec::new();
    };
    let Some(bytes) = store.load(&index_hash) else {
        return Vec::new();
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.strip_prefix("hash="))
        .filter_map(Hash::from_hex)
        .collect()
}

fn ref_hash(store: &Store, name: &str) -> Option<Hash> {
    store.lookup_ref(name)
}

fn ref_summary(name: &str, hash: Option<Hash>) -> Value {
    json!({
        "ref": name,
        "hash": hash.map(|h| h.as_hex())
    })
}

fn optional_hash_arg(args: &Value, keys: &[&str]) -> Result<Option<Hash>, String> {
    for key in keys {
        let Some(value) = args.get(*key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(raw) = value.as_str() else {
            return Err(format!("{key} must be a 40-char hash string"));
        };
        if raw.trim().is_empty() {
            continue;
        }
        return parse_hash_text(raw).map(Some);
    }
    Ok(None)
}

fn required_hash_arg(args: &Value, keys: &[&str]) -> Result<Hash, String> {
    optional_hash_arg(args, keys)?.ok_or_else(|| {
        let joined = keys.join("/");
        format!("missing required hash argument: {joined}")
    })
}

fn parse_hash_text(raw: &str) -> Result<Hash, String> {
    let hex = raw.trim().trim_start_matches('#').to_ascii_lowercase();
    Hash::from_hex(&hex).ok_or_else(|| format!("invalid 40-char hash: {raw}"))
}

fn program_hash_batch(args: &Value) -> Result<Vec<Hash>, String> {
    let mut hashes = Vec::new();
    if let Some(values) = args.get("program_hashes").or_else(|| args.get("hashes")) {
        let Some(array) = values.as_array() else {
            return Err("program_hashes must be an array of hash strings".to_string());
        };
        for value in array.iter().take(BRAIN_MAX_PROGRAM_BATCH) {
            let Some(raw) = value.as_str() else {
                return Err("program_hashes entries must be strings".to_string());
            };
            hashes.push(parse_hash_text(raw)?);
        }
    }
    if hashes.is_empty() {
        if let Some(hash) = optional_hash_arg(args, &["program_hash", "hash"])? {
            hashes.push(hash);
        }
    }
    Ok(hashes)
}

fn load_brain_program(node: &MonsterNode, hash: Hash) -> Result<Program, String> {
    let bytes = node
        .store()
        .load(&hash)
        .ok_or_else(|| format!("program not found in brain store: {}", hash.as_hex()))?;
    Program::from_bytes(&bytes)
        .map_err(|e| format!("brain object is not a valid KASM program {}: {e}", hash.as_hex()))
}

fn program_fingerprint_hex(program: &Program) -> Result<String, String> {
    program
        .semantic_fingerprint()
        .map(|fingerprint| hex_bytes(&fingerprint))
        .map_err(|e| format!("semantic fingerprint failed: {e}"))
}

fn brain_note_text(args: &Value) -> Option<String> {
    clean_bounded_string(
        args.get("text")
            .or_else(|| args.get("observation"))
            .or_else(|| args.get("memory"))
            .or_else(|| args.get("content"))
            .or_else(|| args.get("note")),
        BRAIN_MAX_NOTE_CHARS,
    )
}

fn store_brain_note(node: &MonsterNode, args: &Value, text: &str) -> Result<Value, String> {
    let scope = brain_scope(args);
    let kind = clean_bounded_line(args.get("kind"), 80).unwrap_or_else(|| "observation".to_string());
    let source = clean_bounded_line(args.get("source"), 120).unwrap_or_else(|| "llm".to_string());
    let memory_layer = brain_memory_layer(args, &kind, &source);
    let confidence = args
        .get("confidence")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 1.0));
    let observation_hash = clean_bounded_line(args.get("observation_hash"), 80);
    let evidence_hash = clean_bounded_line(
        args.get("evidence_hash")
            .or_else(|| args.get("evidenceHash")),
        120,
    );
    let proof_hash = clean_bounded_line(args.get("proof_hash").or_else(|| args.get("proofHash")), 120);
    let kasm_contract_hash = clean_bounded_line(
        args.get("kasm_contract_hash")
            .or_else(|| args.get("kasmContractHash")),
        120,
    );
    let fact_key = clean_bounded_line(
        args.get("fact_key")
            .or_else(|| args.get("factKey"))
            .or_else(|| args.get("entity_key"))
            .or_else(|| args.get("entityKey")),
        120,
    );
    let valid_from_ms = args.get("valid_from_ms").and_then(Value::as_u64);
    let valid_until_ms = args.get("valid_until_ms").and_then(Value::as_u64);
    let importance = args
        .get("importance")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 1.0));
    let trust = clean_bounded_line(args.get("trust"), 80);
    let retention = clean_bounded_line(args.get("retention"), 80);
    let fact_ref = fact_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .map(|key| scoped_fact_ref(&scope, &memory_layer, key));
    let previous_fact_hash = fact_ref
        .as_deref()
        .and_then(|note_ref| node.store().lookup_ref(note_ref));
    let supersedes = clean_bounded_line(args.get("supersedes"), 80)
        .or_else(|| previous_fact_hash.map(|hash| hash.as_hex()));
    let (verification_status, trust_score) = brain_note_verification_status(
        &source,
        confidence,
        trust.as_deref(),
        evidence_hash.as_deref(),
        proof_hash.as_deref(),
        observation_hash.as_deref(),
        kasm_contract_hash.as_deref(),
    );
    let now = now_ms();
    let text_hash = Hash::for_blob(text.as_bytes()).as_hex();
    let mut note = String::new();
    note.push_str("forge-brain-llm-note-v1\n");
    note.push_str(&format!("created_ms={now}\n"));
    note.push_str(&format!("scope={scope}\n"));
    note.push_str(&format!("kind={}\n", sanitize_meta_value(&kind)));
    note.push_str(&format!("memory_layer={memory_layer}\n"));
    note.push_str(&format!("source={}\n", sanitize_meta_value(&source)));
    note.push_str(&format!("text_hash={text_hash}\n"));
    note.push_str(&format!("verification_status={verification_status}\n"));
    note.push_str(&format!("trust_score={trust_score:.3}\n"));
    if let Some(confidence) = confidence {
        note.push_str(&format!("confidence={confidence:.3}\n"));
    }
    if let Some(observation_hash) = &observation_hash {
        note.push_str(&format!("observation_hash={observation_hash}\n"));
    }
    if let Some(evidence_hash) = &evidence_hash {
        note.push_str(&format!("evidence_hash={evidence_hash}\n"));
    }
    if let Some(proof_hash) = &proof_hash {
        note.push_str(&format!("proof_hash={proof_hash}\n"));
    }
    if let Some(kasm_contract_hash) = &kasm_contract_hash {
        note.push_str(&format!("kasm_contract_hash={kasm_contract_hash}\n"));
    }
    if let Some(fact_key) = &fact_key {
        note.push_str(&format!("fact_key={fact_key}\n"));
    }
    if let Some(valid_from_ms) = valid_from_ms {
        note.push_str(&format!("valid_from_ms={valid_from_ms}\n"));
    }
    if let Some(valid_until_ms) = valid_until_ms {
        note.push_str(&format!("valid_until_ms={valid_until_ms}\n"));
    }
    if let Some(supersedes) = &supersedes {
        note.push_str(&format!("supersedes={supersedes}\n"));
    }
    if let Some(importance) = importance {
        note.push_str(&format!("importance={importance:.3}\n"));
    }
    if let Some(trust) = &trust {
        note.push_str(&format!("trust={trust}\n"));
    }
    if let Some(retention) = &retention {
        note.push_str(&format!("retention={retention}\n"));
    }
    note.push('\n');
    note.push_str(text);
    let hash = node
        .store()
        .store(note.as_bytes())
        .map_err(|e| format!("store brain note: {e}"))?;
    let scoped_ref = format!("refs/brain/llm/{scope}/latest");
    let scoped_layer_ref = scoped_layer_note_ref(&scope, &memory_layer);
    let by_hash_ref = format!("refs/brain/llm/by_hash/{}", hash.as_hex());
    let (index_ref, index_hash) = update_brain_note_index(node, &scope, &memory_layer, hash)?;
    node.store()
        .write_ref(BRAIN_LLM_NOTE_LATEST_REF, &hash, "brain llm note")
        .map_err(|e| format!("write brain latest note ref: {e}"))?;
    node.store()
        .write_ref(&scoped_ref, &hash, "brain llm scoped note")
        .map_err(|e| format!("write brain scoped note ref: {e}"))?;
    node.store()
        .write_ref(&scoped_layer_ref, &hash, "brain llm scoped layer note")
        .map_err(|e| format!("write brain scoped layer note ref: {e}"))?;
    node.store()
        .write_ref(&by_hash_ref, &hash, "brain llm note by hash")
        .map_err(|e| format!("write brain note hash ref: {e}"))?;
    if let Some(fact_ref) = &fact_ref {
        node.store()
            .write_ref(fact_ref, &hash, "brain llm fact latest")
            .map_err(|e| format!("write brain fact latest ref: {e}"))?;
    }
    Ok(json!({
        "action": if verification_status == "anchored" { "stored_anchored_note" } else { "stored_unverified_note" },
        "hash": hash.as_hex(),
        "refs": {
            "latest": BRAIN_LLM_NOTE_LATEST_REF,
            "scope": scoped_ref,
            "layer": scoped_layer_ref,
            "index": index_ref,
            "index_hash": index_hash.as_hex(),
            "fact": fact_ref,
            "by_hash": by_hash_ref
        },
        "scope": scope,
        "kind": kind,
        "memory_layer": memory_layer,
        "verification_status": verification_status,
        "trust_score": trust_score,
        "source": source,
        "text_hash": text_hash,
        "preview": preview_text(text, BRAIN_NOTE_PREVIEW_CHARS)
    }))
}

fn brain_note_verification_status(
    source: &str,
    confidence: Option<f64>,
    trust: Option<&str>,
    evidence_hash: Option<&str>,
    proof_hash: Option<&str>,
    observation_hash: Option<&str>,
    kasm_contract_hash: Option<&str>,
) -> (&'static str, f64) {
    let has_anchor = [evidence_hash, proof_hash, observation_hash, kasm_contract_hash]
        .iter()
        .any(|value| value.map(|raw| !raw.trim().is_empty()).unwrap_or(false));
    let trust_lower = trust.unwrap_or("").to_ascii_lowercase();
    let source_lower = source.to_ascii_lowercase();
    let mut score = confidence.unwrap_or(0.35).clamp(0.0, 1.0);
    if has_anchor {
        score = score.max(0.72);
    }
    if trust_lower.contains("tool_output")
        || trust_lower.contains("verified")
        || source_lower.contains("data_sync")
        || source_lower.contains("tool")
    {
        score = score.max(0.78);
    }
    if !has_anchor && (source_lower == "llm" || source_lower.contains("assistant")) {
        score = score.min(0.49);
    }
    if has_anchor || score >= 0.70 {
        ("anchored", score)
    } else {
        ("unverified", score)
    }
}

fn update_brain_note_index(
    node: &MonsterNode,
    scope: &str,
    layer: &str,
    hash: Hash,
) -> Result<(String, Hash), String> {
    let index_ref = scoped_layer_index_ref(scope, layer);
    let mut hashes = brain_note_index_hashes(node.store(), &index_ref);
    hashes.retain(|existing| *existing != hash);
    hashes.insert(0, hash);
    hashes.truncate(BRAIN_NOTE_INDEX_LIMIT);

    let mut out = String::new();
    out.push_str("forge-brain-note-index-v1\n");
    out.push_str(&format!("scope={}\n", clean_ref_segment(scope)));
    out.push_str(&format!(
        "memory_layer={}\n",
        normalize_memory_layer(layer).unwrap_or_else(|| "episodic".to_string())
    ));
    out.push_str(&format!("count={}\n\n", hashes.len()));
    for hash in &hashes {
        out.push_str("hash=");
        out.push_str(&hash.as_hex());
        out.push('\n');
    }
    let index_hash = node
        .store()
        .store(out.as_bytes())
        .map_err(|e| format!("store brain note index: {e}"))?;
    node.store()
        .write_ref(&index_ref, &index_hash, "brain llm note recent index")
        .map_err(|e| format!("write brain note index ref: {e}"))?;
    Ok((index_ref, index_hash))
}

fn explain_brain_blob(hash: Hash, bytes: &[u8]) -> Value {
    if let Ok(program) = Program::from_bytes(bytes) {
        return json!({
            "type": "kasm_program",
            "hash": hash.as_hex(),
            "bytes": bytes.len(),
            "target": format!("{:?}", program.target()),
            "inputs": program.inputs(),
            "outputs": program.outputs(),
            "fuel": program.fuel(),
            "nodes": program.nodes().len(),
            "semantic_fingerprint": program_fingerprint_hex(&program).ok()
        });
    }

    let Ok(text) = std::str::from_utf8(bytes) else {
        return json!({
            "type": "binary_blob",
            "hash": hash.as_hex(),
            "bytes": bytes.len()
        });
    };

    if text.starts_with("forge-brain-llm-note-v1\n") {
        let metadata = parse_brain_metadata(text);
        let body = text_after_blank_line(text);
        let temporal_status = brain_note_temporal_status(&metadata, now_ms());
        return json!({
            "type": "llm_note",
            "hash": hash.as_hex(),
            "bytes": bytes.len(),
            "metadata": metadata,
            "temporal_status": temporal_status,
            "preview": preview_text(body, BRAIN_NOTE_PREVIEW_CHARS)
        });
    }
    if text.starts_with("forge-brain-v1\n") {
        return json!({
            "type": "brain_memory_trace",
            "hash": hash.as_hex(),
            "bytes": bytes.len(),
            "metadata": parse_brain_metadata(text)
        });
    }
    if text.starts_with("forge-brain-state-v1\n") {
        return json!({
            "type": "brain_state",
            "hash": hash.as_hex(),
            "bytes": bytes.len(),
            "metadata": parse_brain_metadata(text)
        });
    }

    json!({
        "type": "text_blob",
        "hash": hash.as_hex(),
        "bytes": bytes.len(),
        "preview": preview_text(text, BRAIN_NOTE_PREVIEW_CHARS)
    })
}

fn parse_brain_metadata(text: &str) -> Value {
    let mut metadata = Map::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        metadata.insert(key.trim().to_string(), json!(value.trim()));
    }
    Value::Object(metadata)
}

fn brain_note_temporal_status(metadata: &Value, now: u64) -> Value {
    let valid_from = metadata
        .get("valid_from_ms")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let valid_until = metadata
        .get("valid_until_ms")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok());
    let status = if valid_until.map(|until| until < now).unwrap_or(false) {
        "expired"
    } else if valid_from.map(|from| from > now).unwrap_or(false) {
        "pending"
    } else {
        "active"
    };
    json!({
        "status": status,
        "now_ms": now,
        "valid_from_ms": valid_from,
        "valid_until_ms": valid_until
    })
}

fn text_after_blank_line(text: &str) -> &str {
    text.split_once("\n\n")
        .map(|(_, body)| body)
        .unwrap_or("")
}

fn clean_bounded_string(value: Option<&Value>, max_chars: usize) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.chars().take(max_chars).collect::<String>())
}

fn clean_bounded_line(value: Option<&Value>, max_chars: usize) -> Option<String> {
    clean_bounded_string(value, max_chars).map(|value| sanitize_meta_value(&value))
}

fn clean_ref_segment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars().take(80) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_') {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn sanitize_meta_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
        .collect::<String>()
        .trim()
        .chars()
        .take(240)
        .collect()
}

fn preview_text(text: &str, max_chars: usize) -> Value {
    let mut preview = String::new();
    let mut seen = 0usize;
    let mut truncated = false;
    for ch in text.chars() {
        if seen >= max_chars {
            truncated = true;
            break;
        }
        if ch.is_control() && !matches!(ch, '\n' | '\t') {
            preview.push(' ');
        } else {
            preview.push(ch);
        }
        seen += 1;
    }
    json!({
        "text": preview,
        "chars_in_preview": seen,
        "truncated": truncated
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn clean_optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.chars().take(512).collect::<String>())
}

fn sanitize_geonode_tag(value: &str, body: &str) -> String {
    let mut tag = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            tag.push(ch.to_ascii_lowercase());
        } else if !tag.ends_with('_') {
            tag.push('_');
        }
    }
    let tag = tag.trim_matches('_');
    let tag = if tag.is_empty() { "unnamed_geonode" } else { tag };
    let body = body
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    let prefix = if body.is_empty() { "mars" } else { body.as_str() };
    if tag.starts_with(&format!("{prefix}_")) {
        tag.to_string()
    } else {
        format!("{prefix}_{tag}")
    }
}

fn bounded_limit(value: Option<&Value>, default: usize, max: usize) -> usize {
    value
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(default)
        .clamp(1, max)
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("value is object")
}

fn normalize_reasoning_effort(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" | "aucun" => Ok("none"),
        "minimal" | "min" => Ok("minimal"),
        "low" | "faible" => Ok("low"),
        "medium" | "moyen" => Ok("medium"),
        "high" | "eleve" | "elevÃ©" => Ok("high"),
        "xhigh" | "extra" | "very_high" | "tres_eleve" | "tres eleve" | "tres approfondi" => {
            Ok("xhigh")
        }
        other => Err(format!("unsupported reasoning effort '{other}'")),
    }
}

fn validate_model_ref(model: &str) -> Result<(), String> {
    if model.is_empty()
        || model.len() > 120
        || !model
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '/' | ':'))
    {
        return Err("model_ref contains unsupported characters".to_string());
    }
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| match (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
            (Some(drive), Some(path)) => {
                let mut home = PathBuf::from(drive);
                home.push(path);
                Some(home)
            }
            _ => None,
        })
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

fn env_value_present(key: &str) -> bool {
    std::env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn file_present(path: &Path) -> bool {
    fs::metadata(path).map(|meta| meta.len() > 16).unwrap_or(false)
}

fn gemini_env_path() -> Result<PathBuf, String> {
    let home = home_dir().ok_or_else(|| "Could not resolve user home directory.".to_string())?;
    Ok(home.join(".gemini").join(".env"))
}

fn dotenv_quote_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn dotenv_unquote_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_string()
    }
}

fn dotenv_read_value(path: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            let value = dotenv_unquote_value(value);
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn dotenv_write_value(path: &Path, key: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.chars().any(|ch| ch == '\n' || ch == '\r' || ch.is_whitespace())
        || value.len() > 512
    {
        return Err("API key looks invalid.".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create provider config dir: {e}"))?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let replacement = format!("{key}={}", dotenv_quote_value(value));
    let mut found = false;
    let mut lines = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim_start();
        let is_target = !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .map(|(name, _)| name.trim() == key)
                .unwrap_or(false);
        if is_target {
            lines.push(replacement.clone());
            found = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        lines.push(replacement);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(path, text).map_err(|e| format!("write provider API key: {e}"))
}

fn dotenv_remove_value(path: &Path, key: &str) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let existing = fs::read_to_string(path).map_err(|e| format!("read provider config: {e}"))?;
    let mut lines = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim_start();
        let is_target = !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .map(|(name, _)| name.trim() == key)
                .unwrap_or(false);
        if !is_target {
            lines.push(line.to_string());
        }
    }
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    fs::write(path, text).map_err(|e| format!("clear provider API key: {e}"))
}

fn quick_file_hash_path(path: &Path) -> Result<u64, String> {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut file = fs::File::open(path).map_err(|e| format!("open file '{}': {e}", path.display()))?;
    let mut hash = FNV_OFFSET;
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read file '{}': {e}", path.display()))?;
        if n == 0 {
            break;
        }
        for &byte in &buf[..n] {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    Ok(hash)
}

fn quick_hash_bytes(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn system_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_ms() -> u64 {
    system_time_ms(SystemTime::now())
}

#[cfg(test)]
mod brain_tool_tests {
    use super::*;
    use scan::kasm::{Node, Target, Ty};

    fn add_self_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            3,
            vec![Node::input(0), Node::add(0, 0), Node::output(1, Ty::I64)],
        )
        .unwrap()
    }

    fn shl_one_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            4,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::shl(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn brain_tool_note_roundtrip() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-tools", "brain-note"));
        let commit = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_commit",
            &json!({
                "scope": "trading",
                "kind": "observation",
                "memory_layer": "semantic",
                "text": "LLM bridge remembers only bounded note previews and CAS hashes."
            }),
            None,
        )
        .unwrap();
        let note_hash = commit["note"]["hash"].as_str().unwrap();
        assert_eq!(commit["note"]["action"], "stored_unverified_note");

        let recall = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_recall",
            &json!({ "scope": "trading" }),
            None,
        )
        .unwrap();
        assert_eq!(recall["latest_note"]["metadata"]["scope"], "trading");

        let explain = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_explain",
            &json!({ "hash": note_hash }),
            None,
        )
        .unwrap();
        assert_eq!(explain["explanation"]["type"], "llm_note");
        assert_eq!(explain["explanation"]["metadata"]["memory_layer"], "semantic");
        assert_eq!(explain["explanation"]["temporal_status"]["status"], "active");

        let layer_recall = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_recall",
            &json!({ "scope": "trading", "memory_layer": "semantic", "limit": 4 }),
            None,
        )
        .unwrap();
        assert_eq!(layer_recall["latest_note"]["metadata"]["memory_layer"], "semantic");
        assert_eq!(layer_recall["recent_notes"].as_array().unwrap().len(), 1);

        let sessions = call_internal_tool(
            tmp.as_ref(),
            "forge_list_sessions",
            &json!({ "scope": "trading" }),
            None,
        )
        .unwrap();
        assert_eq!(sessions["brain_context"]["scope"], "trading");
        assert_eq!(
            sessions["brain_context"]["scoped_note_hash"].as_str().unwrap(),
            note_hash
        );
        assert_eq!(
            sessions["brain_context"]["layer_refs"]["semantic"]["hash"]
                .as_str()
                .unwrap(),
            note_hash
        );
    }

    #[test]
    fn brain_tool_compare_publishes_attractor() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-tools", "brain-compare"));
        let store = Store::open(tmp.as_ref()).unwrap();
        let short_hash = store.store(add_self_program().bytes()).unwrap();
        let long_hash = store.store(shl_one_program().bytes()).unwrap();
        drop(store);

        let compared = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_compare",
            &json!({
                "left_hash": short_hash.as_hex(),
                "right_hash": long_hash.as_hex()
            }),
            None,
        )
        .unwrap();
        assert_eq!(compared["equivalent_by_semantic_fingerprint"], true);
        assert_eq!(
            compared["attractor_hash"].as_str().unwrap(),
            short_hash.as_hex()
        );

        let recall = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_recall",
            &json!({ "program_hash": long_hash.as_hex() }),
            None,
        )
        .unwrap();
        assert_eq!(
            recall["program"]["resolved_hash"].as_str().unwrap(),
            short_hash.as_hex()
        );
    }

    #[test]
    fn brain_auto_commit_is_scoped_and_deduped() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-tools", "brain-auto"));
        let first = call_internal_tool(
            tmp.as_ref(),
            "forge_list_sessions",
            &json!({
                "scope": "basique",
                "brain_autocommit_force": true
            }),
            None,
        )
        .unwrap();
        let first_auto = &first["brain_context"]["auto_commit"];
        assert_eq!(first_auto["status"], "committed");
        assert_eq!(first_auto["importance"].as_f64().unwrap(), 1.0);
        let note_hash = first_auto["note_hash"].as_str().unwrap();
        let explain = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_explain",
            &json!({ "hash": note_hash }),
            None,
        )
        .unwrap();
        assert_eq!(explain["explanation"]["metadata"]["memory_layer"], "episodic");

        let second = call_internal_tool(
            tmp.as_ref(),
            "forge_list_sessions",
            &json!({
                "scope": "basique",
                "brain_autocommit_force": true
            }),
            None,
        )
        .unwrap();
        let second_auto = &second["brain_context"]["auto_commit"];
        assert_eq!(second_auto["status"], "skipped_duplicate");
        assert_eq!(second_auto["note_hash"].as_str().unwrap(), note_hash);
    }

    #[test]
    fn brain_auto_commit_redacts_secret_like_arguments() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-tools", "brain-redact"));
        let call = call_internal_tool(
            tmp.as_ref(),
            "forge_list_sessions",
            &json!({
                "scope": "basique",
                "brain_autocommit_force": true,
                "api_key": "sk-test-super-secret-value-that-must-not-enter-memory"
            }),
            None,
        )
        .unwrap();
        let note_hash = call["brain_context"]["auto_commit"]["note_hash"]
            .as_str()
            .unwrap();
        let explain = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_explain",
            &json!({ "hash": note_hash }),
            None,
        )
        .unwrap();
        let text = serde_json::to_string(&explain).unwrap();
        assert!(!text.contains("sk-test-super-secret"));
        assert!(text.contains("<redacted>"));
    }

    #[test]
    fn brain_notes_keep_history_and_fact_supersession() {
        let tmp = scan::TmpDir::new(scan::fresh_tmp_path("forge-agent-tools", "brain-history"));
        let first = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_commit",
            &json!({
                "scope": "agence_immo",
                "memory_layer": "semantic",
                "kind": "fact",
                "fact_key": "zone-75011-price",
                "evidence_hash": "evidence-a",
                "text": "Prix moyen observe: 9900 eur/m2."
            }),
            None,
        )
        .unwrap();
        assert_eq!(first["note"]["action"], "stored_anchored_note");
        let first_hash = first["note"]["hash"].as_str().unwrap().to_string();

        let second = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_commit",
            &json!({
                "scope": "agence_immo",
                "memory_layer": "semantic",
                "kind": "fact",
                "fact_key": "zone-75011-price",
                "evidence_hash": "evidence-b",
                "text": "Prix moyen observe: 10100 eur/m2."
            }),
            None,
        )
        .unwrap();
        assert_eq!(second["note"]["action"], "stored_anchored_note");

        let recall = call_internal_tool(
            tmp.as_ref(),
            "forge_brain_recall",
            &json!({
                "scope": "agence_immo",
                "memory_layer": "semantic",
                "limit": 4
            }),
            None,
        )
        .unwrap();
        let recent = recall["recent_notes"].as_array().unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0]["metadata"]["supersedes"], first_hash);
        assert_eq!(recent[0]["metadata"]["verification_status"], "anchored");
    }
}

