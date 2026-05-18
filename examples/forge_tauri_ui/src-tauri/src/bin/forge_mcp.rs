//! Forge MCP compute server.
//!
//! This binary is intentionally separate from Tauri. The UI can become a log
//! viewer, while agents call Forge as a compute tool over MCP/stdin-stdout.
#![allow(dead_code)]

#[path = "../trading_alpha.rs"]
#[allow(dead_code)]
mod trading_alpha;
#[path = "../trading_core.rs"]
#[allow(dead_code)]
mod trading_core;
#[path = "../kasm_indicators.rs"]
#[allow(dead_code)]
mod kasm_indicators;
#[path = "../forge_agent_tools.rs"]
mod forge_agent_tools;
#[path = "../forge_intent.rs"]
mod forge_intent;
#[allow(dead_code)]
#[path = "../forge_agent_runtime.rs"]
mod forge_agent_runtime;
#[path = "../forge_fbc_host.rs"]
mod forge_fbc_host;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use scan::fbc::{
    execute_tool_cell_batch_groups, parse_app_section_registry_v0, parse_tool_cell_registry_v0,
    tool_cell_output_artifact_json, ForgeVmConfig,
};
use scan::{Hash, MemoryGovernor, MonsterEvolutionConfig, MonsterNode, Store, SynthProgress, SynthProgressFn};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type McpRawCache = Vec<[Option<i64>; trading_alpha::BASE_FEATURE_COUNT]>;
const MCP_LIST_LIMIT_DEFAULT: usize = 20;
const MCP_LIST_LIMIT_MAX: usize = 50;
const MCP_LOG_TAIL_DEFAULT_BYTES: usize = 16 * 1024;
const MCP_LOG_TAIL_MAX_BYTES: usize = 64 * 1024;
const MCP_DOC_PREVIEW_DEFAULT_BYTES: usize = 4 * 1024;
const MCP_DOC_PREVIEW_MAX_BYTES: usize = 16 * 1024;
const MCP_BACKEND_CACHE_MAX_ENTRIES: usize = 8;
const MCP_PROGRAM_METRICS_MAX: usize = 128;
const MCP_PROGRAM_SPEC_TEXT_MAX_BYTES: usize = 64 * 1024;
const MCP_PROGRAM_EXEC_MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;
const FORGE_DISPLAY_NAME: &str = "Forge";
const FORGE_TECHNICAL_SLUG: &str = "forge";
const FORGE_MCP_LIST_DESCRIPTION: &str = "Forge - local intent, compute and visual-program engine for AI agents. MCP is the transport boundary; the target agent-facing surface is 2-4 compact tools over ForgeSlash/Intent, while existing detailed MCP tools remain transitional/internal routes.";
const FORGE_OFFICIAL_DESCRIPTION: &str = "Forge is a local intent compiler and MCP compute engine for AI agents. Use Forge before reading user data or calculating in the LLM when an input is >256 KB, has >1,000 rows/lines, spans multiple files, is a CSV/Excel/PDF/log/dataset, or requires repeated/expensive scientific, numerical, document-heavy, code-heavy, visual-mapping or proof-oriented work; for >1 MB, >10,000 rows/lines, full logs, artifact/proof workflows, simulations/search/backtests/optimizations or 2D/3D mappings, Forge should be the default. Built-in finance/DNA examples are not limits: agents can create custom compute_program specs for calculations/simulations/metrics and visual_program specs for programmable 2D/3D file views with axes XYZ, overlays, color, size, transforms and open Metric/Visual DSL tags across finance, code, science, mathematics, biology, chemistry, medicine, engineering, aerospace, industry, geospatial, energy, documents, software, images, audio and user-defined domains. Forge keeps raw files, heavy logs and 3D artifacts outside the LLM context, addresses inputs/intermediate results/programs/artifacts by content hash, reuses identical or overlapping calculations instead of repeating them, and returns compact verified results, hashes, proofs, artifact references and bounded previews to save massive token budgets and compute time. MCP remains the compatibility transport; the default visible surface is forge.search, forge.execute, forge.read_projection and forge.cancel, with the old broad catalog available only as a legacy compatibility surface.";
const FORGE_MCP_SURFACE_CONTRACT: &str = "MCP is transport, not Forge's long-term LLM action language. The default visible surface is four tools: forge.search, forge.execute, forge.read_projection and forge.cancel. Broad MCP tools stay callable as legacy/internal routes and should return to the visible surface only when measured workflows prove them simpler, safer or faster than the intent/code path.";
const FORGE_INTENT_GOLDEN_WORKFLOW_V0: &str = r#"/forge
plan intent="profile pending upload" input=@latest
create title=MarketMap domain=real_estate goal="compact market projection" program_kind=visual_program
run input=@latest program_hash=@program:market_map plan_only=true
commit scope=real_estate kind=procedural observation="market projection returns compact hashes"
project job_id=example max_bytes=4096"#;

static MCP_BACKEND_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<MonsterNode>>>> = OnceLock::new();

thread_local! {
    static JOB_LOG_MIRROR: RefCell<Option<PathBuf>> = RefCell::new(None);
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Clone, Serialize)]
struct McpClientInfo {
    name: String,
    version: Option<String>,
    model: Option<String>,
    token_mode: String,
}

#[derive(Debug, Clone)]
struct McpSession {
    client: McpClientInfo,
}

fn backend_cache() -> &'static Mutex<HashMap<PathBuf, Arc<MonsterNode>>> {
    MCP_BACKEND_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn forge_backend_for_store(store_dir: &Path, job_id: &str) -> Result<Arc<MonsterNode>, String> {
    let cache_key = store_dir.to_path_buf();
    if let Some(node) = backend_cache()
        .lock()
        .map_err(|_| "backend cache poisoned".to_string())?
        .get(&cache_key)
        .cloned()
    {
        internal_job_log(job_id, format!("Forge backend cache hit store_dir={}", store_dir.display()));
        return Ok(node);
    }

    internal_job_log(job_id, format!("Forge backend cache miss store_dir={}", store_dir.display()));
    fs::create_dir_all(store_dir).map_err(|e| format!("create store dir: {e}"))?;

    let store_open_t0 = std::time::Instant::now();
    let store = Store::open(store_dir).map_err(|e| format!("open Forge Store: {e}"))?;
    internal_job_log(
        job_id,
        format!("Forge Store open in {:.2}s", store_open_t0.elapsed().as_secs_f64()),
    );

    let node_t0 = std::time::Instant::now();
    let node = Arc::new(MonsterNode::new(
        store,
        MemoryGovernor::new(1024 * 1024 * 1024),
    ));
    internal_job_log(
        job_id,
        format!("MonsterNode ready in {:.2}s", node_t0.elapsed().as_secs_f64()),
    );

    let atlas_path = store_dir.join("forge.atlas");
    internal_job_log(job_id, format!("atlas_path={}", atlas_path.display()));
    let atlas_t0 = std::time::Instant::now();
    let atlas = Arc::new(scan::atlas::Atlas::open(&atlas_path).map_err(|e| format!("open Atlas: {e}"))?);
    internal_job_log(
        job_id,
        format!("Atlas open in {:.2}s", atlas_t0.elapsed().as_secs_f64()),
    );
    let attach_t0 = std::time::Instant::now();
    node.attach_atlas(Arc::clone(&atlas));
    internal_job_log(
        job_id,
        format!("Forge backend ready (atlas attached in {:.2}s)", attach_t0.elapsed().as_secs_f64()),
    );

    let mut cache = backend_cache()
        .lock()
        .map_err(|_| "backend cache poisoned".to_string())?;
    if cache.len() >= MCP_BACKEND_CACHE_MAX_ENTRIES {
        cache.clear();
    }
    let entry = cache.entry(cache_key).or_insert_with(|| Arc::clone(&node));
    Ok(Arc::clone(entry))
}

impl Default for McpSession {
    fn default() -> Self {
        Self {
            client: McpClientInfo {
                name: "Unknown MCP agent".to_string(),
                version: None,
                model: std::env::var("FORGE_MCP_MODEL").ok().filter(|v| !v.trim().is_empty()),
                token_mode: "estimated".to_string(),
            },
        }
    }
}

impl McpClientInfo {
    fn from_initialize(params: &Value) -> Self {
        let info = params.get("clientInfo").unwrap_or(&Value::Null);
        let name = info
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| params.get("client_name").and_then(Value::as_str))
            .unwrap_or("Unknown MCP agent")
            .to_string();
        let version = info
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string);
        let model = std::env::var("FORGE_MCP_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| info.get("model").and_then(Value::as_str).map(str::to_string))
            .or_else(|| params.get("model").and_then(Value::as_str).map(str::to_string));
        Self {
            name,
            version,
            model,
            token_mode: "estimated".to_string(),
        }
    }
}

fn friendly_agent_name(client: &McpClientInfo) -> &str {
    let name = client.name.trim();
    if name.is_empty() {
        "The agent"
    } else {
        name
    }
}

fn format_alpha_number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if (rounded.fract()).abs() < 0.000_001 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.3}")
    }
}

fn format_alpha_param(value: Option<f64>, fallback: &str) -> String {
    value
        .map(format_alpha_number)
        .unwrap_or_else(|| fallback.to_string())
}

fn alpha_strategy_intro_line(args: &AlphaStrategyArgs, client: &McpClientInfo) -> String {
    let stop = format_alpha_param(args.sl_display_points.or(args.sl_points), "auto");
    let target = format_alpha_param(args.tp_display_points.or(args.tp_points), "auto");
    let spread = format_alpha_param(args.spread_display_points.or(args.spread_points), "auto");
    let daily_target = format_alpha_param(
        args.target_display_points_per_day.or(args.target_pnl_per_day),
        "auto",
    );
    let train_split = args
        .train_split
        .map(|v| format!("{:.0}%", v.clamp(0.0, 1.0) * 100.0))
        .unwrap_or_else(|| "auto".to_string());
    let horizon = args
        .max_horizon_bars
        .map(|v| v.to_string())
        .unwrap_or_else(|| "auto".to_string());
    let engine = args.engine.as_deref().unwrap_or("auto");
    format!(
        "{}: I'm going to run the Forge market backtest on this session. Objective: discover a reusable long/short signal from the uploaded OHLCV data. Metrics: VWAP, RSI, ATR, ADX, stochastic, train/holdout PnL and target-hit rate. Parameters: stop {}, target {}, spread {}, daily target {}, train split {}, horizon {} bars, engine {}.",
        friendly_agent_name(client),
        stop,
        target,
        spread,
        daily_target,
        train_split,
        horizon,
        engine
    )
}

fn agent_context_accounting(client: &McpClientInfo, csv_bytes: usize, log_bytes: usize) -> Value {
    agent_context_accounting_with_artifacts(client, csv_bytes, log_bytes, 0, Value::Null)
}

fn agent_context_accounting_with_artifacts(
    client: &McpClientInfo,
    csv_bytes: usize,
    log_bytes: usize,
    artifact_bytes: usize,
    compute_avoided: Value,
) -> Value {
    let avoided_bytes = csv_bytes
        .saturating_add(log_bytes)
        .saturating_add(artifact_bytes);
    let estimated_tokens_low = (avoided_bytes + 3) / 4;
    let estimated_tokens_typical = (avoided_bytes + 2) / 3;
    let estimated_tokens_high = (avoided_bytes + 1) / 2;
    json!({
        "metric": "llm_context_avoided",
        "token_mode": client.token_mode,
        "tokenizer": client.model.as_deref().unwrap_or("model_unknown"),
        "csv_bytes": csv_bytes,
        "log_bytes": log_bytes,
        "artifact_bytes": artifact_bytes,
        "raw_input_bytes_not_sent": csv_bytes,
        "log_bytes_not_sent": log_bytes,
        "artifact_bytes_not_sent": artifact_bytes,
        "avoided_bytes": avoided_bytes,
        "estimated_tokens": estimated_tokens_typical,
        "estimated_tokens_low": estimated_tokens_low,
        "estimated_tokens_high": estimated_tokens_high,
        "tokens_saved": {
            "mode": client.token_mode,
            "truth_status": "estimated_from_real_bytes",
            "agent": client.name,
            "tokenizer": client.model.as_deref().unwrap_or("model_unknown"),
            "exact": null,
            "low": estimated_tokens_low,
            "typical": estimated_tokens_typical,
            "high": estimated_tokens_high,
            "bytes_not_sent": avoided_bytes,
            "raw_input_bytes_not_sent": csv_bytes,
            "log_bytes_not_sent": log_bytes,
            "artifact_bytes_not_sent": artifact_bytes,
            "basis": "Real bytes not returned to the LLM divided by conservative tokenizer ratios: 4/3/2 bytes per token."
        },
        "compute_avoided": compute_avoided,
        "truth_contract": {
            "bytes_are_exact": true,
            "token_counts_are_estimated": true,
            "exact_token_counts_require_agent_tokenizer": true,
            "compute_counters_are_real_when_present": true,
            "raw_input_returned": false
        },
        "estimation_basis": "Exact bytes are known. Tokens are estimated because each LLM tokenizer is different; numeric CSV can tokenize less efficiently than prose, so Forge reports a range.",
        "exact_tokens": null,
        "note": "Forge saves these tokens by keeping large files/logs on disk and returning content-addressed references, hashes, bounded previews and result manifests instead of full source content."
    })
}

fn artifact_bytes_from_execution_result(result: &Value) -> usize {
    ["metrics_artifact", "proof_artifact", "visual_mapping_artifact"]
        .iter()
        .filter_map(|key| result.get(*key))
        .filter_map(|artifact| artifact.get("bytes").and_then(Value::as_u64))
        .fold(0usize, |sum, bytes| sum.saturating_add(bytes as usize))
}

fn program_compute_avoided(execution_result: &Value) -> Value {
    let metric_count = execution_result
        .get("metric_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_hits = execution_result
        .get("cache_hit_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    json!({
        "mode": "real_executor_counters",
        "program_kind": "custom_compute_program_run",
        "metric_count": metric_count,
        "computed_count": execution_result.get("computed_count").cloned().unwrap_or(Value::Null),
        "unresolved_count": execution_result.get("unresolved_count").cloned().unwrap_or(Value::Null),
        "failed_count": execution_result.get("failed_count").cloned().unwrap_or(Value::Null),
        "cache_hit_count": cache_hits,
        "cache_miss_count": metric_count.saturating_sub(cache_hits),
        "elapsed_ms": execution_result.get("elapsed_ms").cloned().unwrap_or(Value::Null),
        "operations_unit": "metric_invocations",
        "note": "These are real Forge executor counters. Forge does not invent an exact LLM-token equivalent for computation; token savings are reported separately from real bytes not returned."
    })
}

fn alpha_compute_avoided(
    engine: &str,
    bars: usize,
    decision_rows: usize,
    candidates_evaluated: usize,
    combinations_tried: usize,
    atlas_score_hits: usize,
    atlas_full_pair_hits: usize,
    atlas_opcode_hits: usize,
    gpu_jobs_dispatched: usize,
    gpu_jobs_skipped: usize,
) -> Value {
    json!({
        "mode": "real_alpha_counters",
        "engine": engine,
        "bars": bars,
        "decision_rows": decision_rows,
        "candidates_evaluated": candidates_evaluated,
        "combinations_tried": combinations_tried,
        "cache_hits": {
            "score": atlas_score_hits,
            "full_pair": atlas_full_pair_hits,
            "opcode": atlas_opcode_hits,
            "total": atlas_score_hits
                .saturating_add(atlas_full_pair_hits)
                .saturating_add(atlas_opcode_hits)
        },
        "gpu_jobs_dispatched": gpu_jobs_dispatched,
        "gpu_jobs_skipped_by_cache": gpu_jobs_skipped,
        "operations_unit": "strategy_candidates_and_pair_evaluations",
        "note": "These are real Forge counters from the strategy run. They show how much search/dispatch Forge handled locally and how much known work was skipped before reaching the LLM."
    })
}

fn internal_job_log(job_id: &str, line: impl AsRef<str>) {
    let _ = writeln!(io::stderr(), "[forge-internal:{job_id}] {}", line.as_ref());
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuleCmp {
    Gte,
    Lte,
}

#[derive(Debug, Clone, Serialize)]
struct Rule {
    feature_idx: usize,
    feature_name: String,
    threshold: i64,
    cmp: RuleCmp,
}

#[derive(Debug, Clone)]
struct ScoredRule {
    rule: Rule,
    eval: trading_alpha::StrategyEval,
}

#[derive(Debug, Clone, Serialize)]
struct EvalSummary {
    days: usize,
    trades: usize,
    long_trades: usize,
    short_trades: usize,
    target_hit_pct: f64,
    pnl_points: f64,
    profit_factor: f64,
    sharpe: f64,
    max_drawdown_points: f64,
    win_trade_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
struct StrategyJob {
    job_id: String,
    title: String,
    status: String,
    file_path: String,
    file_hash: u64,
    bars: usize,
    train_end_bar: usize,
    strategy_hash: String,
    long_rule: Rule,
    short_rule: Rule,
    train: EvalSummary,
    holdout: EvalSummary,
    log_path: String,
}

struct ForgeDetector {
    feature_idx: usize,
    feature_name: String,
    program_hash: Hash,
    outcome: scan::MonsterEvolutionOutcome,
    train_eval: trading_alpha::StrategyEval,
}

#[derive(Debug, Clone, Deserialize)]
struct AlphaStrategyArgs {
    csv_path: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    sl_points: Option<f64>,
    #[serde(default)]
    tp_points: Option<f64>,
    #[serde(default)]
    spread_points: Option<f64>,
    #[serde(default)]
    target_pnl_per_day: Option<f64>,
    #[serde(default)]
    sl_display_points: Option<f64>,
    #[serde(default)]
    tp_display_points: Option<f64>,
    #[serde(default)]
    spread_display_points: Option<f64>,
    #[serde(default)]
    target_display_points_per_day: Option<f64>,
    #[serde(default)]
    point_size: Option<f64>,
    #[serde(default)]
    max_horizon_bars: Option<usize>,
    #[serde(default)]
    train_split: Option<f64>,
    #[serde(default)]
    top_rules_per_side: Option<usize>,
    #[serde(default)]
    engine: Option<String>,
    #[serde(default)]
    max_nodes: Option<usize>,
    #[serde(default)]
    generations: Option<usize>,
    #[serde(default)]
    beam_width: Option<usize>,
    #[serde(default)]
    feature_limit: Option<usize>,
    #[serde(default)]
    store_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RunIntentArgs {
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    capability: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    inputs: Vec<ProgramInputRef>,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgramMetricSpec {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    op: Option<String>,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    dtype: Option<String>,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    constraints: Value,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    cache: Option<String>,
    #[serde(default)]
    proof: Option<String>,
    #[serde(default, rename = "if")]
    condition: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    algorithm: Option<String>,
    #[serde(default)]
    weight: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgramDefineArgs {
    title: String,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    intent: Option<String>,
    goal: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    program_kind: Option<String>,
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    metrics: Vec<ProgramMetricSpec>,
    #[serde(default)]
    views: Vec<Value>,
    #[serde(default)]
    spec_text: Option<String>,
    #[serde(default)]
    source_schema: Value,
    #[serde(default)]
    constraints: Value,
    #[serde(default)]
    output_contract: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgramInputRef {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgramExecuteArgs {
    #[serde(default)]
    program_hash: Option<String>,
    #[serde(default)]
    program_id: Option<String>,
    #[serde(default)]
    program: Option<String>,
    #[serde(default)]
    program_title: Option<String>,
    #[serde(default)]
    program_query: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    inputs: Vec<ProgramInputRef>,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    dry_run: Option<bool>,
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    capability: Option<String>,
    #[serde(default)]
    parent_session_id: Option<String>,
}

struct ResolvedProgramInput {
    role: String,
    path: PathBuf,
    bytes: Vec<u8>,
    content_hash: String,
}

fn main() {
    if let Err(err) = serve_mcp() {
        let _ = writeln!(io::stderr(), "forge_mcp fatal: {err}");
    }
}

fn serve_mcp() -> Result<(), String> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut session = McpSession::default();
    while let Some(message) = read_mcp_message(&mut stdin)? {
        let request: JsonRpcRequest =
            serde_json::from_slice(&message.body).map_err(|e| format!("decode json-rpc: {e}"))?;
        if request.id.is_none() {
            continue;
        }
        let response = handle_request(request, &mut session);
        write_mcp_message(&mut stdout, &response, message.framing)?;
    }
    Ok(())
}

fn handle_request(request: JsonRpcRequest, session: &mut McpSession) -> Value {
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "initialize" => {
            session.client = McpClientInfo::from_initialize(&request.params);
            let protocol = request
                .params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05");
            ok_response(
                id,
                json!({
                    "protocolVersion": protocol,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": FORGE_DISPLAY_NAME,
                        "version": "0.1.0",
                        "displayName": FORGE_DISPLAY_NAME,
                        "canonicalName": FORGE_DISPLAY_NAME,
                        "technicalSlug": FORGE_TECHNICAL_SLUG,
                        "agentDisplayRule": "When listing connected MCP servers or explaining this server, display it as Forge with a capital F. The lowercase slug 'forge' is only the technical namespace/config key.",
                        "description": FORGE_MCP_LIST_DESCRIPTION,
                        "longDescription": FORGE_OFFICIAL_DESCRIPTION,
                        "surfaceContract": {
                            "status": "transitional_broad_catalog",
                            "contract": FORGE_MCP_SURFACE_CONTRACT,
                            "targetVisibleTools": ["forge.search", "forge.execute", "forge.read_projection", "forge.cancel"],
                            "mcpRole": "transport_and_compatibility_boundary"
                        }
                    }
                }),
            )
        }
        "tools/list" => ok_response(id, json!({ "tools": tools_list() })),
        "tools/call" => match handle_tool_call(&request.params, session) {
            Ok(result) => ok_response(id, result),
            Err(err) => ok_response(
                id,
                mcp_error_response(err),
            ),
        },
        _ => error_response(id, -32601, format!("unknown method: {}", request.method)),
    }
}

fn mcp_tool(name: &str, command: Option<&str>, description: &str, properties: Value, required: &[&str]) -> Value {
    let mut schema = json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    let mut tool = json!({ "name": name, "description": description, "inputSchema": schema });
    tool["annotations"] = mcp_tool_annotations(name, command);
    if let Some(execution) = mcp_tool_execution(name) {
        tool["execution"] = execution;
    }
    if let Some(command) = command {
        tool["displayName"] = json!(command);
        tool["publicCommand"] = json!(command);
    }
    tool
}

fn mcp_tool_annotations(name: &str, command: Option<&str>) -> Value {
    let read_only = matches!(
        name,
        "about"
            | "capabilities"
            | "program_compile_validate_route"
            | "jobs"
            | "sessions"
            | "documents"
            | "mapping"
            | "mapping_metrics"
            | "mapping_analysis"
            | "atlas"
            | "brain_recall"
            | "brain_explain"
            | "read"
            | "logs"
            | "programs"
            | "program"
            | "pending"
            | "doc"
            | "preview"
            | "doc_sessions"
            | "forge.search"
            | "forge.execute"
            | "forge.read_projection"
            | "forge_intent_search"
            | "forge_intent_execute"
    );
    let destructive = matches!(name, "cancel" | "forge.cancel");
    let idempotent = read_only || matches!(name, "cancel" | "forge.cancel" | "update_session");
    let open_world = matches!(name, "profile");
    json!({
        "title": command.unwrap_or(name),
        "readOnlyHint": read_only,
        "destructiveHint": destructive,
        "idempotentHint": idempotent,
        "openWorldHint": open_world
    })
}

fn mcp_tool_execution(name: &str) -> Option<Value> {
    matches!(name, "run" | "visual_program" | "forge.execute")
        .then(|| json!({ "taskSupport": "optional" }))
}

fn mcp_opt_in_tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    mcp_tool(name, None, description, properties, required)
}

fn tools_list() -> Vec<Value> {
    if compact_mcp_surface_enabled() {
        return compact_tools_list();
    }
    let mut tools = vec![
        mcp_tool("about", None, "Forge â€” local compute and visual-program engine for AI agents. FIRST CALL for large files, expensive/repetitive calculations, scientific/data/code/document analysis, programmable 2D/3D views, custom compute_program/visual_program specs, hashes, proofs or compact artifacts. Use Forge before Read/shell above 256 KB or 1,000 rows to avoid spending massive LLM tokens reading/calculating. Fast path: run { intent, inputs, plan_only:true } or run {} for one pending upload.", json!({}), &[]),
        mcp_tool("fbc_runtime", Some("/fbc_"), "/fbc_ - Experimental Forge Native Bytecode runtime snapshot for the whole app. Compiles SECTION_OWNERSHIP sections and sensitive native commands into FBC/KASM2 v0 ToolCells, runs the verifier/interpreter, writes compact proof artifacts, and returns only hashes/projections; no raw filesystem/network/secrets are exposed.", json!({
            "backend": { "type": "string", "description": "Optional backend selector: auto, fbc_interpreter, kasm_native, gpu, microvm_fallback." },
            "write_artifacts": { "type": "boolean", "description": "Default true. Writes compact FBC projection/artifact JSON under the Forge store." }
        }), &[]),
        mcp_tool("capabilities", Some("/metric"), "/metric - Forge capability GPS. Find examples plus universal creation routes for compute_program and visual_program specs: open Metric/Visual DSL, domain metrics, 2D/3D axes, overlays, artifacts and local execution. Built-in finance/DNA templates are examples, not limits. Use when the domain or program shape is unclear; otherwise call /program_ with plan_only=true directly.", json!({
            "query": { "type": "string", "description": "Optional capability/operator/domain/intent search, e.g. visual_program, 3D mapping, finance, kmer, code, volume anomalies, geospatial, audio." },
            "domain": { "type": "string", "description": "Optional compact domain filter: finance, code, documents, biology, chemistry, medicine, math, engineering, aerospace, simulation, timeseries, security, energy, geospatial, manufacturing, audio, images or any custom domain." },
            "capability": { "type": "string", "description": "Optional exact capability/template name such as universal_compute_program, universal_visual_program_2d_3d, csv_timeseries, kmer_sequence, source_code_metrics." },
            "detail": { "type": "string", "description": "compact (default) or detailed. Detailed returns matching operators and examples." },
            "detailed": { "type": "boolean", "description": "Set true to include detailed matching operators." }
        }), &[]),
        mcp_tool("create", Some("/create_"), "/create_ - Create a reusable Forge program for any domain. Check atlas first when reuse is possible. Use compute_program for local calculations/simulations/metrics, or visual_program for programmable 2D/3D views over session files. Specs use compact Metric/Visual DSL tags with open-ended domain metrics; Forge compiles the metric graph, validates routes/dependencies/math contracts, stores the program and every metric tag in My Atlas by content hash and never needs raw file content in the LLM.", json!({
            "title": { "type": "string", "maxLength": 24, "description": "Very short instrument/lens title â€” 24 characters max. Use a concise label like 'VWAP detune', 'RSI long', 'K-mer scan'. Longer titles are rejected." },
            "domain": { "type": "string", "description": "Free-form domain: finance, biology, chemistry, code, math, engineering, aerospace, medicine, geospatial, audio, images, networks, manufacturing, custom, etc." },
            "intent": { "type": "string", "description": "Natural-language reason for this program, e.g. invent a metric, model a 3D map, detect anomalies, simulate a system or measure k-mer hash quality." },
            "goal": { "type": "string", "description": "What the program should measure, discover or optimize." },
            "program_kind": { "type": "string", "description": "Optional: compute_program (default) or visual_program for programs that define 2D/3D file views." },
            "kind": { "type": "string", "description": "Alias for program_kind." },
            "template": { "type": "string", "description": "Optional existing template family, e.g. csv_timeseries, kmer_sequence, source_code_metrics." },
            "metrics": { "type": "array", "description": "Open Metric DSL tags. Each item can include tag/name, op, inputs, params, unit, goal, description, formula, algorithm, weight; agents may define domain-specific metrics instead of choosing from a closed catalog. For custom/invented metrics, formula and algorithm should describe the exact math shown in live compute cards. IMPORTANT: 'tag' is hard-capped at 16 characters (e.g. 'rsi_14', 'vwap', 'ema_delta'); 'name' at 18 characters (e.g. 'RSI 14', 'VWAP'). Longer values are truncated/rejected â€” keep node labels minimalist." },
            "views": { "type": "array", "description": "Visual program views. Each item can define type=2d or type=3d, axes x/y/z, color, size, overlays, labels, transforms and local viewer params." },
            "spec_text": { "type": "string", "description": "Optional Forge Metric/Visual DSL v1 spec containing <metric .../> and, for visual_program, <view id=\"...\" type=\"3d\" x=\"time_index\" y=\"momentum_24\" z=\"volatility_48\" color=\"forward_return_6\" /> balises." },
            "source_schema": { "type": "object", "description": "Optional schema/columns expected by the program. No source content." },
            "constraints": { "type": "object", "description": "Optional compute constraints: max windows, allowed ops, precision, cache policy." },
            "output_contract": { "type": "object", "description": "Optional expected compact outputs: tables, scores, proofs, artifact types." }
        }), &["title", "goal"]),
        mcp_tool("program_compile_validate_route", Some("/metric"), "/metric - Compile/validate/route a Forge program before storage or after reading one from Atlas. Checks metric contracts, formulas, algorithms, inputs/outputs, dtype/unit/domain/params, objective coverage, dependency map, unit dimensions, scientific validation, formula-to-executor binding and linter results. Use this to improve metric tags, choose reusable Atlas tags/programs, and repair vague or non-routable programs before /create_ or /program_.", json!({
            "program_hash": { "type": "string", "description": "Existing My Atlas program hash to recompile/validate." },
            "title": { "type": "string", "maxLength": 24, "description": "Draft title when compiling before storage. 24 characters max." },
            "domain": { "type": "string" },
            "intent": { "type": "string" },
            "goal": { "type": "string" },
            "program_kind": { "type": "string" },
            "metrics": { "type": "array" },
            "views": { "type": "array" },
            "spec_text": { "type": "string" },
            "source_schema": { "type": "object" },
            "constraints": { "type": "object" },
            "output_contract": { "type": "object" }
        }), &[]),
        mcp_tool("geonode", Some("/geo"), "/geo - Create or update a reusable Atlas GeoNode/MiniGeoNode for a named spatial coordinate anchor on any planet, moon, asteroid, solar-system body, star system or galactic object. Use when a user or assistant mentions a real place/object that is not already in My Atlas. If coordinates come from model knowledge, mark coordinate_source='llm_estimate' and include confidence. Surface lat/lon anchors can be injected into Planet visual_program views with tool=planet_sphere; astronomical ra/dec anchors are saved for future space/galaxy renderers.", json!({
            "name": { "type": "string" },
            "tag": { "type": "string" },
            "body": { "type": "string" },
            "coordinate_system": { "type": "string" },
            "lat": { "type": "number", "minimum": -90, "maximum": 90 },
            "lon": { "type": "number", "minimum": -360, "maximum": 360 },
            "ra": { "type": "number", "minimum": 0, "maximum": 360 },
            "dec": { "type": "number", "minimum": -90, "maximum": 90 },
            "distance": { "type": "number" },
            "distance_unit": { "type": "string" },
            "node_kind": { "type": "string", "description": "geo_node or mini_geo_node." },
            "parent_geonode": { "type": "string" },
            "aliases": { "type": "array", "items": { "type": "string" } },
            "coordinate_source": { "type": "string" },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "notes": { "type": "string" }
        }), &["name"]),
        mcp_tool("run", Some("/program_"), "/program_ - Run Forge compute or visual programs locally. Main fast path for pending uploads, large files, reusable compute_program/visual_program specs, CPU/GPU jobs, 2D/3D mappings, simulations, searches and proof/artifact workflows. After the first run, the same program_hash + input hashes + params returns an instant My Atlas hit. Use run {} for the only pending upload, run { pending:true }, run { job_id:\"...\" }, or run { intent, inputs, plan_only:true }. Do not read raw files or compute manually in the LLM.", json!({
            "intent": { "type": "string", "description": "Natural-language compute/visual intent, e.g. create a 3D metric map, find anomalies in this CSV, create DNA k-mer hash metrics, analyze code complexity or build a domain-specific simulator." },
            "plan_only": { "type": "boolean", "description": "If true, return a compact compute plan, cache/proof policy and next call without launching or sending raw data to the LLM." },
            "pending": { "type": "boolean", "description": "If true, claim the newest pending UI upload. If exactly one pending job exists, run {} does the same." },
            "job_id": { "type": "string", "description": "Pending Forge job id created by the UI upload dropbox." },
            "program_hash": { "type": "string", "description": "Reusable program hash returned by create or read { kind:\"programs\" }." },
            "program": { "type": "string", "description": "Program library selector: hash, title, or query for an existing Forge program/template." },
            "program_title": { "type": "string", "description": "Exact or fuzzy title of a program in the Forge library." },
            "program_query": { "type": "string", "description": "Search query for an existing program in the Forge library." },
            "capability": { "type": "string", "description": "Optional capability/template, e.g. universal_compute_program, universal_visual_program_2d_3d, alpha, csv_timeseries, kmer_sequence." },
            "inputs": { "type": "array", "description": "Input refs with path or job_id and optional role." },
            "params": { "type": "object", "description": "Runtime parameter overrides for programs/capabilities." },
            "title": { "type": "string", "description": "Optional human-readable title chosen by the agent." },
            "sl_points": { "type": "number" },
            "tp_points": { "type": "number" },
            "sl_display_points": { "type": "number" },
            "tp_display_points": { "type": "number" },
            "spread_display_points": { "type": "number" },
            "target_display_points_per_day": { "type": "number" },
            "point_size": { "type": "number" },
            "max_horizon_bars": { "type": "integer" },
            "train_split": { "type": "number" },
            "top_rules_per_side": { "type": "integer" },
            "engine": { "type": "string", "description": "auto (default), forge, forge_strict, or threshold." },
            "max_nodes": { "type": "integer" },
            "generations": { "type": "integer" },
            "beam_width": { "type": "integer" },
            "feature_limit": { "type": "integer" },
            "store_dir": { "type": "string" }
        }), &[]),
        mcp_tool("jobs", None, "List Forge sessions and pending/running/completed jobs. Use to find a job_id before run/logs/read. Returns compact summaries so the agent does not spend tokens reading Forge files from disk.", json!({ "limit": { "type": "integer", "description": "Maximum jobs to list. Default 20." } }), &[]),
        mcp_tool("sessions", None, "List/search Forge session history as compact manifests, source refs, statuses and artifact availability. No raw files.", json!({ "limit": { "type": "integer" }, "query": { "type": "string" }, "status": { "type": "string" } }), &[]),
        mcp_tool("documents", None, "List saved document/source refs from Forge sessions. Returns metadata and references only.", json!({ "limit": { "type": "integer" }, "query": { "type": "string" }, "type": { "type": "string" } }), &[]),
        mcp_tool("mapping", None, "Interpret programmable Forge 2D/3D visual mappings by compact refs, legends, axes, metrics and selection hints. No raw point clouds or source rows.", json!({ "job_id": { "type": "string" }, "mode": { "type": "string" }, "vertex_index": { "type": "integer" } }), &[]),
        mcp_tool("mapping_metrics", Some("/metric"), "/metric - Inspect the active file locally and return an extensible metric/recipe schema for 2D/3D visual programs: source columns, derived metrics, axes/color/size candidates and accepted agent-defined metrics. No source rows.", json!({ "job_id": { "type": "string" } }), &[]),
        mcp_tool("mapping_model", None, "Create or modify a programmable Forge 3D map from an agent recipe: axes XYZ, metrics, color, size, overlays, transform and objective. Forge reads the source locally and returns only artifact refs and compact diagnostics.", json!({ "job_id": { "type": "string" }, "recipe": { "type": "object" }, "mode": { "type": "string" }, "objective": { "type": "string" }, "max_points": { "type": "integer" }, "voxel_resolution": { "type": "integer" } }), &[]),
        mcp_tool("visual_program", Some("/visualprogram_"), "/visualprogram_ - Run visual_program views locally on the active session file: 2D/3D axes, overlays, labels, color/size, transforms and open Metric/Visual DSL definitions. Returns compact artifact refs, hashes and diagnostics only.", json!({ "job_id": { "type": "string" }, "metrics": { "type": "array" }, "views": { "type": "array" }, "program_hash": { "type": "string" }, "program_title": { "type": "string" }, "program_goal": { "type": "string" }, "max_points": { "type": "integer" }, "voxel_resolution": { "type": "integer" } }), &["views"]),
        mcp_tool("mapping_analysis", None, "Analyze a Forge 3D visual_program locally with PCA, voxel density, clusters, outliers, trajectory and geometry diagnostics. Compact statistics only; no raw points or rows.", json!({ "job_id": { "type": "string" }, "mode": { "type": "string" }, "voxel_resolution": { "type": "integer" }, "max_hotspots": { "type": "integer" }, "max_clusters": { "type": "integer" } }), &[]),
        mcp_tool("profile", None, "Read/update redacted Forge profile/provider settings, model choices, reasoning effort and local auth actions. Secrets are write-only.", json!({ "action": { "type": "string" }, "provider": { "type": "string" }, "model_ref": { "type": "string" }, "reasoning_effort": { "type": "string" }, "settings": { "type": "object" }, "gemini_api_key": { "type": "string" } }), &[]),
        mcp_tool("atlas", None, "Compact My Atlas overview: reusable programs, metric tags and completed run refs. Use before creating or running duplicate work.", json!({ "max_entries": { "type": "integer" }, "query": { "type": "string", "description": "Search reusable Atlas programs, metric tags or completed runs." }, "kind": { "type": "string", "description": "Optional filter: program, metric_tag, or run." } }), &[]),
        mcp_tool("brain_recall", None, "Recall Forge Brain state, latest memory refs, scoped LLM notes and optional KASM program summaries. Returns hashes/refs/previews only.", json!({ "scope": { "type": "string", "description": "Optional memory scope/section such as basic, google_suite, banger, trading or real_estate." }, "section": { "type": "string" }, "program_hash": { "type": "string", "description": "Optional KASM program hash to resolve through brain substitutions." }, "hash": { "type": "string" } }), &[]),
        mcp_tool("brain_commit", None, "Commit a bounded LLM observation note and/or a verified KASM program into Forge Brain. Program commits run brain tightening and semantic attractor publishing.", json!({ "scope": { "type": "string" }, "section": { "type": "string" }, "kind": { "type": "string" }, "source": { "type": "string" }, "confidence": { "type": "number", "minimum": 0, "maximum": 1 }, "text": { "type": "string" }, "observation": { "type": "string" }, "program_hash": { "type": "string" }, "samples": { "type": "integer" } }), &[]),
        mcp_tool("brain_compare", None, "Compare two KASM programs by Forge semantic fingerprint and publish a verified attractor when they collapse to the same behavior.", json!({ "left_hash": { "type": "string" }, "right_hash": { "type": "string" }, "a": { "type": "string" }, "b": { "type": "string" }, "samples": { "type": "integer" } }), &["left_hash", "right_hash"]),
        mcp_tool("brain_sleep", None, "Run a bounded semantic sleep pass over explicit program hashes: tighten, verify, and converge equivalent programs to shorter attractors.", json!({ "program_hashes": { "type": "array", "items": { "type": "string" } }, "program_hash": { "type": "string" }, "hash": { "type": "string" }, "samples": { "type": "integer" } }), &[]),
        mcp_tool("brain_explain", None, "Explain a Forge Brain hash or refs/brain/* ref as compact metadata: memory trace, state, LLM note or KASM program summary.", json!({ "hash": { "type": "string" }, "program_hash": { "type": "string" }, "memory_hash": { "type": "string" }, "ref": { "type": "string" }, "kind": { "type": "string", "description": "Use state to explain refs/brain/state." } }), &[]),
        mcp_tool("update_session", None, "Update safe session metadata: title, pinned/protected flags, archive/status, tags or note.", json!({ "job_id": { "type": "string" }, "title": { "type": "string" }, "status": { "type": "string" }, "pinned": { "type": "boolean" }, "protected": { "type": "boolean" }, "archived": { "type": "boolean" }, "tags": { "type": "array", "items": { "type": "string" } }, "note": { "type": "string" } }), &["job_id"]),
        mcp_tool("read", None, "Read compact Forge results, hashes, proofs, artifact refs, bounded previews, reusable programs and 3D mappings. Heavy CSV/source/log/artifact content is not returned by default.", json!({ "job_id": { "type": "string", "description": "Compute job id." }, "program_hash": { "type": "string", "description": "Optional program hash to read a reusable Forge program." }, "kind": { "type": "string", "description": "Optional: job, program, artifacts, docs, preview." }, "max_bytes": { "type": "integer", "description": "Only used for kind=preview; capped." } }), &[]),
        mcp_tool("logs", None, "Stream live Forge compute progress by cursor. Use while a job runs or appears stuck. Returns bounded log chunks and next_cursor; do not open .log files directly.", json!({ "job_id": { "type": "string" }, "cursor": { "type": "integer", "description": "Byte offset returned by the previous call. Default 0." }, "max_bytes": { "type": "integer", "description": "Maximum bytes to read. Default 65536, capped at 262144." } }), &["job_id"]),
        mcp_tool("cancel", None, "Cancel a Forge job safely by job_id. Use when the user asks to stop, abort, retry, or change parameters.", json!({ "job_id": { "type": "string" }, "reason": { "type": "string" } }), &["job_id"]),
    ];
    if intent_mcp_surface_enabled() {
        tools.push(mcp_opt_in_tool(
            "forge_intent_search",
            "Opt-in Forge intent facade search. Returns compact action signatures and examples over the current internal routes without exposing full MCP schemas.",
            json!({
                "query": { "type": "string", "description": "Capability, domain or action intent to search." },
                "limit": { "type": "integer", "description": "Maximum compact candidates. Default 4, capped at 8." }
            }),
            &[],
        ));
        tools.push(mcp_opt_in_tool(
            "forge_intent_execute",
            "Opt-in ForgeSlash v0 facade executor. Parses, compiles, policy-checks and returns a ForgeProjection without raw data; execution stays gated until parity.",
            json!({
                "program": { "type": "string", "description": "ForgeSlash v0 program, for example /forge plan intent=\"inspect latest upload\" input=@latest" },
                "max_bytes": { "type": "integer", "description": "Projection budget. Default 4096, capped at 16384." },
                "execute_safe": { "type": "boolean", "description": "If true, execute only read-only and plan_only steps; side-effect routes are skipped with proofs." },
                "mode": { "type": "string", "description": "Optional: plan (default), execute_safe, or execute_approved." },
                "approve_side_effects": { "type": "boolean", "description": "Required for mode=execute_approved when the intent contains create, non-plan run or brain_commit." },
                "approved_intent_hash": { "type": "string", "description": "Intent hash observed in a previous projection; required for side-effect approval." },
                "approved_policy_hash": { "type": "string", "description": "Policy hash observed in a previous projection; required for side-effect approval." },
                "allow_run_side_effects": { "type": "boolean", "description": "Extra gate for non-plan run steps; defaults false even when other side effects are approved." }
            }),
            &["program"],
        ));
    }
    tools
}

fn compact_tools_list() -> Vec<Value> {
    vec![
        mcp_tool(
            "forge.search",
            None,
            "Compact Forge intent search. Use to discover the next ForgeSlash action, capability family or projection route without exposing the transitional MCP catalog.",
            json!({
                "query": { "type": "string", "description": "Intent, domain, capability or task shape to search." },
                "limit": { "type": "integer", "description": "Maximum compact candidates. Default 4, capped at 8." }
            }),
            &[],
        ),
        mcp_tool(
            "forge.execute",
            None,
            "Compact Forge executor. Accepts a ForgeSlash v0 program, validates/routes it, and returns a bounded projection. Raw data stays on disk.",
            json!({
                "program": { "type": "string", "description": "ForgeSlash v0 program, for example /forge plan intent=\"inspect latest upload\" input=@latest" },
                "max_bytes": { "type": "integer", "description": "Projection budget. Default 4096, capped at 16384." },
                "execute_safe": { "type": "boolean", "description": "If true, execute only read-only and plan_only steps; side-effect routes are skipped with proofs." },
                "mode": { "type": "string", "description": "Optional: plan (default), execute_safe, or execute_approved." },
                "approve_side_effects": { "type": "boolean", "description": "Required for mode=execute_approved when the intent contains create, non-plan run or brain_commit." },
                "approved_intent_hash": { "type": "string", "description": "Intent hash observed in a previous projection; required for side-effect approval." },
                "approved_policy_hash": { "type": "string", "description": "Policy hash observed in a previous projection; required for side-effect approval." },
                "allow_run_side_effects": { "type": "boolean", "description": "Extra gate for non-plan run steps; defaults false even when other side effects are approved." }
            }),
            &["program"],
        ),
        mcp_tool(
            "forge.read_projection",
            None,
            "Read compact Forge projections, job summaries, artifact refs, hashes and bounded previews. Heavy source/log/artifact content is not returned by default.",
            json!({
                "job_id": { "type": "string", "description": "Optional Forge job id." },
                "program_hash": { "type": "string", "description": "Optional reusable program hash." },
                "kind": { "type": "string", "description": "Optional: job, program, artifacts, docs, preview." },
                "max_bytes": { "type": "integer", "description": "Only used for bounded previews; capped." }
            }),
            &[],
        ),
        mcp_tool(
            "forge.cancel",
            None,
            "Cancel a Forge job safely by job_id.",
            json!({
                "job_id": { "type": "string" },
                "reason": { "type": "string" }
            }),
            &["job_id"],
        ),
    ]
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn compact_mcp_surface_enabled() -> bool {
    if let Ok(value) = env::var("FORGE_MCP_SURFACE") {
        let value = value.trim().to_ascii_lowercase();
        if matches!(value.as_str(), "broad" | "legacy" | "full") {
            return false;
        }
        if matches!(value.as_str(), "compact" | "frontier" | "intent") {
            return true;
        }
    }
    !(env_flag("FORGE_MCP_LEGACY_SURFACE") || env_flag("FORGE_MCP_BROAD_SURFACE"))
}

fn intent_mcp_surface_enabled() -> bool {
    env_flag("FORGE_INTENT_MCP_SURFACE")
}

fn visible_tool_names() -> Vec<String> {
    visible_tool_names_from(tools_list())
}

fn visible_tool_names_from(tools: Vec<Value>) -> Vec<String> {
    tools
        .into_iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()
}

const MCP_TOOL_ALIASES: &[(&str, &[&str])] = &[
    ("about", &["about", "forge_about"]),
    ("fbc_runtime", &["fbc_runtime", "forge_fbc_runtime", "/fbc_", "fbc_", "forge.native_bytecode"]),
    ("create", &["create", "/create_", "create_", "define", "forge_program_define", "forge_program_create"]),
    ("indicator", &["/indicator", "indicator", "/indicator_", "indicator_", "/alert_", "alert_"]),
    ("metric", &["/metric", "metric"]),
    ("program_compile_validate_route", &["program_compile_validate_route", "compile_validate_route", "compile", "forge_program_compile_validate_route"]),
    ("forge_intent_search", &["forge_intent_search", "forge.search"]),
    ("capabilities", &["capabilities", "ops", "operators", "forge_ops", "forge_program_ops"]),
    ("programs", &["programs", "forge_programs_list"]),
    ("program", &["program", "forge_program_read"]),
    ("execute", &["execute", "forge_program_execute", "forge_program_run"]),
    ("alpha", &["alpha", "forge_alpha_strategy_from_csv"]),
    ("jobs", &["jobs", "forge_jobs_list"]),
    ("pending", &["pending", "forge_pending_jobs_list"]),
    ("read", &["read", "forge_job_read", "forge.read_projection", "forge_intent_read_projection"]),
    ("mapping", &["mapping", "visual_mapping", "forge_interpret_visual_mapping"]),
    ("mapping_metrics", &["mapping_metrics", "metric_catalog_3d", "forge_3d_metric_catalog"]),
    ("mapping_model", &["mapping_model", "model_mapping", "forge_model_3d_mapping", "forge_3d_model_view"]),
    ("visual_program", &["visual_program", "/visualprogram", "visualprogram", "/visualprogram_", "visualprogram_", "run_visual_program", "forge_run_visual_program", "visual_program_run"]),
    ("mapping_analysis", &["mapping_analysis", "analyze_mapping", "forge_analyze_3d_mapping"]),
    ("profile", &["profile", "settings", "forge_profile_settings"]),
    ("sessions", &["sessions", "history", "forge_list_sessions"]),
    ("documents", &["documents", "docs", "forge_list_documents", "forge_docs_list"]),
    ("atlas", &["atlas", "forge_atlas_overview"]),
    ("brain_recall", &["brain_recall", "forge_brain_recall", "recall_memory"]),
    ("brain_commit", &["brain_commit", "forge_brain_commit", "commit_memory"]),
    ("brain_compare", &["brain_compare", "forge_brain_compare", "compare_memory"]),
    ("brain_sleep", &["brain_sleep", "forge_brain_sleep", "memory_sleep"]),
    ("brain_explain", &["brain_explain", "forge_brain_explain", "explain_memory"]),
    ("geonode", &["geonode", "/geo", "geo", "/geo_", "geo_", "/minigeo", "minigeo", "/minigeo_", "minigeo_", "upsert_geonode", "forge_upsert_geonode"]),
    ("update_session", &["update_session", "forge_update_session"]),
    ("forge_intent_execute", &["forge_intent_execute", "forge.execute"]),
    ("run", &["run", "/program_", "program_", "/strategy_", "strategy_", "claim", "forge_job_run_pending", "forge_job_claim"]),
    ("logs", &["logs", "forge_job_log_tail"]),
    ("artifacts", &["artifacts", "forge_job_artifacts"]),
    ("inject", &["inject", "forge_job_inject_result"]),
    ("rename", &["rename", "forge_job_update_title"]),
    ("cancel", &["cancel", "forge_job_cancel", "forge.cancel"]),
    ("legacy_docs", &["legacy_docs"]),
    ("doc", &["doc", "forge_doc_read"]),
    ("preview", &["preview", "forge_doc_preview"]),
    ("doc_sessions", &["doc_sessions", "forge_doc_sessions"]),
];

const MCP_INTERNAL_TOOL_ROUTES: &[(&str, &str)] = &[
    ("mapping", "forge_interpret_visual_mapping"),
    ("mapping_metrics", "forge_3d_metric_catalog"),
    ("mapping_model", "forge_model_3d_mapping"),
    ("visual_program", "forge_run_visual_program"),
    ("mapping_analysis", "forge_analyze_3d_mapping"),
    ("profile", "forge_profile_settings"),
    ("sessions", "forge_list_sessions"),
    ("documents", "forge_list_documents"),
    ("atlas", "forge_atlas_overview"),
    ("brain_recall", "forge_brain_recall"),
    ("brain_commit", "forge_brain_commit"),
    ("brain_compare", "forge_brain_compare"),
    ("brain_sleep", "forge_brain_sleep"),
    ("brain_explain", "forge_brain_explain"),
    ("geonode", "forge_upsert_geonode"),
    ("update_session", "forge_update_session"),
];

fn canonical_mcp_tool_name(name: &str) -> Option<&'static str> {
    MCP_TOOL_ALIASES
        .iter()
        .find_map(|(canonical, aliases)| aliases.contains(&name).then_some(*canonical))
}

fn internal_mcp_tool_route(canonical: &str) -> Option<&'static str> {
    MCP_INTERNAL_TOOL_ROUTES
        .iter()
        .find_map(|(name, route)| (*name == canonical).then_some(*route))
}

#[cfg(test)]
mod mcp_surface_tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    fn mcp_surface_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn with_mcp_surface_env<T>(key: &'static str, value: &str, f: impl FnOnce() -> T) -> T {
        let _guard = mcp_surface_env_lock();
        let _surface = EnvVarGuard::unset("FORGE_MCP_SURFACE");
        let _legacy = EnvVarGuard::unset("FORGE_MCP_LEGACY_SURFACE");
        let _broad = EnvVarGuard::unset("FORGE_MCP_BROAD_SURFACE");
        let _requested = EnvVarGuard::set(key, value);
        f()
    }

    fn with_default_mcp_surface_env<T>(f: impl FnOnce() -> T) -> T {
        let _guard = mcp_surface_env_lock();
        let _surface = EnvVarGuard::unset("FORGE_MCP_SURFACE");
        let _legacy = EnvVarGuard::unset("FORGE_MCP_LEGACY_SURFACE");
        let _broad = EnvVarGuard::unset("FORGE_MCP_BROAD_SURFACE");
        f()
    }

    #[test]
    fn compact_facade_is_default_visible_surface_and_aliases_still_route() {
        with_default_mcp_surface_env(|| {
            assert_eq!(canonical_mcp_tool_name("forge.search"), Some("forge_intent_search"));
            assert_eq!(canonical_mcp_tool_name("forge.execute"), Some("forge_intent_execute"));
            assert_eq!(canonical_mcp_tool_name("forge.read_projection"), Some("read"));
            assert_eq!(canonical_mcp_tool_name("forge.cancel"), Some("cancel"));

            let visible_names = visible_tool_names();
            assert_eq!(
                visible_names,
                vec![
                    "forge.search".to_string(),
                    "forge.execute".to_string(),
                    "forge.read_projection".to_string(),
                    "forge.cancel".to_string(),
                ]
            );
            assert!(!visible_names.iter().any(|name| name == "forge_intent_search"));
            assert!(!visible_names.iter().any(|name| name == "forge_intent_execute"));
        });
    }

    #[test]
    fn compact_surface_exposes_exact_frontier_tools() {
        with_default_mcp_surface_env(|| {
            let visible_names = visible_tool_names_from(compact_tools_list());
            assert_eq!(
                visible_names,
                vec![
                    "forge.search".to_string(),
                    "forge.execute".to_string(),
                    "forge.read_projection".to_string(),
                    "forge.cancel".to_string(),
                ]
            );
            assert_eq!(visible_names.len(), 4);
        });
    }

    #[test]
    fn compact_cutover_readiness_is_current_default() {
        with_default_mcp_surface_env(|| {
            let readiness = compact_cutover_readiness();
            assert_eq!(readiness["kind"].as_str(), Some("forge_compact_cutover_readiness_v0"));
            assert_eq!(readiness["compact_surface_exact"].as_bool(), Some(true));
            assert_eq!(readiness["intent_routes_live"].as_bool(), Some(true));
            assert_eq!(
                readiness["approved_side_effect_gate_live"].as_bool(),
                Some(true)
            );
            assert_eq!(readiness["exact_intent_cache_live"].as_bool(), Some(true));
            assert_eq!(readiness["projection_replay_live"].as_bool(), Some(true));
            assert_eq!(readiness["broad_catalog_hidden"].as_bool(), Some(true));
            assert_eq!(readiness["status"].as_str(), Some("ready_as_current_default"));
        });
    }

    #[test]
    fn broad_catalog_is_visible_only_through_legacy_escape_hatches() {
        with_mcp_surface_env("FORGE_MCP_SURFACE", "broad", || {
            let visible = visible_tool_names();
            assert!(visible.len() > 4);
            assert!(visible.iter().any(|name| name == "create"));
            assert!(visible.iter().any(|name| name == "run"));
        });
        with_mcp_surface_env("FORGE_MCP_LEGACY_SURFACE", "1", || {
            let visible = visible_tool_names();
            assert!(visible.len() > 4);
            assert!(visible.iter().any(|name| name == "create"));
            assert!(visible.iter().any(|name| name == "run"));
        });
    }

    #[test]
    fn visible_tools_carry_mcp_safety_annotations() {
        for tool in tools_list() {
            let name = tool["name"].as_str().unwrap_or("");
            let annotations = tool.get("annotations").expect("annotations");
            assert!(annotations["readOnlyHint"].is_boolean(), "{name} readOnlyHint");
            assert!(
                annotations["destructiveHint"].is_boolean(),
                "{name} destructiveHint"
            );
            assert!(
                annotations["idempotentHint"].is_boolean(),
                "{name} idempotentHint"
            );
            assert!(
                annotations["openWorldHint"].is_boolean(),
                "{name} openWorldHint"
            );
        }
    }

    #[test]
    fn compact_surface_annotations_are_decision_ready() {
        let tools = compact_tools_list();
        let by_name: HashMap<_, _> = tools
            .iter()
            .map(|tool| (tool["name"].as_str().unwrap(), tool))
            .collect();

        assert_eq!(
            by_name["forge.search"]["annotations"]["readOnlyHint"],
            true
        );
        assert_eq!(
            by_name["forge.execute"]["annotations"]["readOnlyHint"],
            true
        );
        assert_eq!(
            by_name["forge.read_projection"]["annotations"]["readOnlyHint"],
            true
        );
        assert_eq!(
            by_name["forge.cancel"]["annotations"]["destructiveHint"],
            true
        );
        assert_eq!(
            by_name["forge.cancel"]["annotations"]["idempotentHint"],
            true
        );
        assert_eq!(
            by_name["forge.execute"]["execution"]["taskSupport"],
            "optional"
        );
    }

    #[test]
    fn policy_visible_tool_list_is_derived_from_tools_list() {
        with_default_mcp_surface_env(|| {
            let policy = forge_tool_selection_policy();
            let policy_tools: Vec<String> = policy["visible_tools"]
                .as_array()
                .expect("visible_tools array")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();

            assert_eq!(policy_tools, visible_tool_names());
            assert_eq!(
                policy["current_visible_tool_budget"].as_u64(),
                Some(policy_tools.len() as u64)
            );
        });
    }

    #[test]
    fn intent_execute_facade_returns_projection_without_running_side_effects() {
        let result = forge_intent_execute_projection(forge_intent::FORGE_SLASH_V0_EXAMPLE, 4096)
            .expect("valid ForgeSlash example");
        assert_eq!(result["ok"].as_bool(), Some(true));
        assert_eq!(result["mode"].as_str(), Some("planned_no_side_effects"));
        assert_eq!(result["raw_data_returned"].as_bool(), Some(false));
        assert!(result["forge_projection"]["trace_hash"].as_str().is_some());
        assert_eq!(
            result["forge_projection"]["raw_data_returned"].as_bool(),
            Some(false)
        );
    }

    #[test]
    fn intent_execute_projection_plans_without_claiming_pending_jobs() {
        let result = forge_intent_execute_projection(
            r#"/forge plan intent="profile market data" input=@latest"#,
            4096,
        )
        .expect("valid projection");
        assert_eq!(result["mode"].as_str(), Some("planned_no_side_effects"));
        assert_eq!(result["raw_data_returned"].as_bool(), Some(false));
        let steps = result["compiled_route_plan"]["steps"]
            .as_array()
            .expect("compiled route steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["route"].as_str(), Some("run"));
        assert_eq!(steps[0]["side_effect"].as_bool(), Some(false));
        assert_eq!(steps[0]["arguments"]["plan_only"].as_bool(), Some(true));
    }

    #[test]
    fn intent_execute_safe_runs_plan_steps_without_claiming_pending_jobs() {
        let result = forge_intent_execute_safe_v0(
            r#"/forge plan intent="profile market data" input=@latest"#,
            4096,
        )
        .expect("valid safe execution");
        assert_eq!(result["mode"].as_str(), Some("execute_safe"));
        assert_eq!(result["raw_data_returned"].as_bool(), Some(false));
        let steps = result["executed_steps"].as_array().expect("executed steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["status"].as_str(), Some("executed_safe"));
        assert_eq!(steps[0]["result_summary"]["plan_only"].as_bool(), Some(true));
        assert!(steps[0]["result_hash"].as_str().is_some());
        assert_eq!(
            result["execution_report"]["executed_step_count"].as_u64(),
            Some(1)
        );
        assert!(result["execution_report"]["execution_hash"].as_str().is_some());
    }

    #[test]
    fn intent_execute_safe_report_hash_is_stable() {
        let source = r#"/forge plan intent="profile market data" input=@latest"#;
        let first = forge_intent_execute_safe_v0(source, 4096).expect("first safe execution");
        let second = forge_intent_execute_safe_v0(source, 4096).expect("second safe execution");
        assert_eq!(
            first["execution_report"]["execution_hash"],
            second["execution_report"]["execution_hash"]
        );
        assert_eq!(
            first["execution_report"]["executed_steps_hash"],
            second["execution_report"]["executed_steps_hash"]
        );
    }

    #[test]
    fn create_route_uses_shared_runtime_rich_program_path() {
        let _guard = mcp_surface_env_lock();
        let store = std::env::temp_dir().join(format!(
            "forge-mcp-create-shared-runtime-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let store_text = store.display().to_string();
        let _store = EnvVarGuard::set("FORGE_STORE_DIR", &store_text);
        let _programs = EnvVarGuard::unset("FORGE_PROGRAMS_DIR");

        let result = define_program(
            ProgramDefineArgs {
                title: "Shared runtime create".to_string(),
                domain: Some("finance".to_string()),
                intent: Some("route one bounded metric".to_string()),
                goal: "prove create is runtime-owned".to_string(),
                kind: None,
                program_kind: Some("compute_program".to_string()),
                template: None,
                metrics: vec![metric_spec(
                    "price_mean",
                    "rolling_mean",
                    &["close"],
                    json!({ "window": 20 }),
                )],
                views: Vec::new(),
                spec_text: None,
                source_schema: json!({}),
                constraints: json!({}),
                output_contract: json!({}),
            },
            &test_client(),
        )
        .expect("runtime-backed create");

        assert_eq!(result["atlas"]["saved_to_my_atlas"].as_bool(), Some(true));
        assert_eq!(
            result["program"]["canonical"]["source"].as_str(),
            Some("forge_agent_direct_rich_create_v0")
        );
        assert_eq!(
            result["program"]["created_by_agent"]["runtime"].as_str(),
            Some(forge_agent_runtime::FORGE_AGENT_RUNTIME_V0)
        );
    }

    #[test]
    fn execute_program_uses_shared_runtime_rich_runner() {
        let _guard = mcp_surface_env_lock();
        let store = std::env::temp_dir().join(format!(
            "forge-mcp-run-shared-runtime-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let store_text = store.display().to_string();
        let _store = EnvVarGuard::set("FORGE_STORE_DIR", &store_text);
        let csv_path = store.join("input.csv");
        fs::create_dir_all(&store).expect("store dir");
        fs::write(&csv_path, "close,volume\n10,100\n12,120\n14,130\n")
            .expect("csv fixture");

        let created = define_program(
            ProgramDefineArgs {
                title: "Shared runtime execute".to_string(),
                domain: Some("finance".to_string()),
                intent: Some("run one bounded metric".to_string()),
                goal: "compute a compact rolling mean".to_string(),
                kind: None,
                program_kind: Some("compute_program".to_string()),
                template: None,
                metrics: vec![metric_spec(
                    "close_mean",
                    "rolling_mean",
                    &["close"],
                    json!({ "window": 2 }),
                )],
                views: Vec::new(),
                spec_text: None,
                source_schema: json!({}),
                constraints: json!({}),
                output_contract: json!({}),
            },
            &test_client(),
        )
        .expect("program created");

        let result = execute_program(
            ProgramExecuteArgs {
                program_hash: created["program"]["program_hash"].as_str().map(str::to_string),
                program_id: None,
                program: None,
                program_title: None,
                program_query: None,
                title: None,
                inputs: vec![ProgramInputRef {
                    role: Some("data".to_string()),
                    path: Some(csv_path.display().to_string()),
                    job_id: None,
                }],
                params: json!({}),
                dry_run: Some(false),
                intent: Some("run one bounded metric".to_string()),
                capability: None,
                parent_session_id: None,
            },
            &test_client(),
        )
        .expect("runtime-backed execute");

        assert_eq!(
            result["kind"].as_str(),
            Some("forge_agent_direct_rich_run_v0")
        );
        assert_eq!(
            result["job"]["execution"]["stage"].as_str(),
            Some("metric_toolbox_executed")
        );
        assert_eq!(
            result["job"]["program"]["program_hash"],
            created["program"]["program_hash"]
        );
    }

    #[test]
    fn intent_facade_response_omits_broad_policy_envelope() {
        let session = McpSession {
            client: McpClientInfo {
                name: "test".to_string(),
                version: None,
                model: None,
                token_mode: "unknown".to_string(),
            },
        };
        let mut params = json!({
            "arguments": {
                "program": "/forge plan intent=\"profile market data\" input=@latest",
                "execute_safe": true
            }
        });
        params
            .as_object_mut()
            .expect("params object")
            .insert("name".to_string(), json!("forge_intent_execute"));
        let response = handle_tool_call(&params, &session)
        .expect("intent facade response");
        let text = response["content"][0]["text"].as_str().expect("text payload");
        assert!(!text.contains("\"agent_instructions\""));
        assert!(!text.contains("\"tool_selection_policy\""));
        let payload: Value = serde_json::from_str(text).expect("json payload");
        assert_eq!(payload["surface"].as_str(), Some("forge_intent_compact_v0"));
        assert_eq!(payload["data"]["mode"].as_str(), Some("execute_safe"));
        assert!(payload["data"]["execution_report"]["execution_hash"].as_str().is_some());
    }

    #[test]
    fn persisted_intent_projection_round_trips_by_execution_hash() {
        let mut projection = forge_intent_execute_safe_v0(
            r#"/forge plan intent="profile market data" input=@latest"#,
            1024,
        )
        .expect("safe projection");
        let store = std::env::temp_dir().join(format!(
            "forge-intent-projection-test-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let persisted = forge_agent_runtime::persist_direct_projection(&store, &mut projection)
            .expect("persist projection");
        let execution_hash = persisted["execution_hash"]
            .as_str()
            .expect("execution hash")
            .to_string();
        let read = forge_agent_runtime::direct_read_projection(
            &store,
            &json!({ "execution_hash": execution_hash }),
        )
        .expect("read projection");
        assert_eq!(read["found"].as_bool(), Some(true));
        assert_eq!(
            read["execution_report"]["execution_hash"],
            persisted["execution_hash"]
        );
        assert_eq!(read["raw_data_returned"].as_bool(), Some(false));
        assert_eq!(
            read["kind"].as_str(),
            Some("forge_agent_direct_projection_read_v0")
        );
        assert_eq!(read["executed_steps"][0]["result_summary"]["plan_only"].as_bool(), Some(true));
    }

    #[test]
    fn persisted_intent_projection_is_indexed_listed_and_searchable() {
        let mut projection = forge_intent_execute_safe_v0(
            r#"/forge plan intent="profile market data" input=@latest"#,
            1024,
        )
        .expect("safe projection");
        let store = std::env::temp_dir().join(format!(
            "forge-intent-index-test-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let persisted = forge_agent_runtime::persist_direct_projection(&store, &mut projection)
            .expect("persist projection");
        let execution_hash = persisted["execution_hash"].as_str().expect("execution hash");
        let listed = forge_agent_runtime::direct_read_projection(&store, &json!({ "limit": 4 }))
            .expect("list projections");
        assert_eq!(listed["kind"].as_str(), Some("forge_agent_direct_projection_list_v0"));
        assert_eq!(listed["entries"][0]["execution_hash"].as_str(), Some(execution_hash));

        let search = forge_intent_search_with_store("profile market projection", 8, Some(&store));
        let results = search["results"].as_array().expect("search results");
        assert!(
            results.iter().any(|item| {
                item.get("id")
                    .and_then(Value::as_str)
                    .map(|id| id.starts_with("projection:"))
                    .unwrap_or(false)
            }),
            "projection index should be visible to forge.search"
        );
    }

    #[test]
    fn forge_search_returns_executable_next_call_and_never_dead_ends() {
        let search = forge_intent_search_with_store("memory commit semantic note", 4, None);
        assert_eq!(search["kind"].as_str(), Some("forge_search_result_v1"));
        assert_eq!(search["surface"].as_str(), Some("forge.search_default"));
        assert!(search["result_count"].as_u64().unwrap_or(0) > 0);
        assert_eq!(search["raw_data_returned"].as_bool(), Some(false));
        assert!(search["next_call"]["tool"].as_str().is_some());
        assert!(
            search["route_plan"]["steps"]
                .as_array()
                .map(|steps| !steps.is_empty())
                .unwrap_or(false)
        );

        let fallback = forge_intent_search_with_store("zzzzzzzz-no-route", 4, None);
        assert!(fallback["result_count"].as_u64().unwrap_or(0) > 0);
        assert_eq!(fallback["next_call"]["tool"].as_str(), Some("forge.search"));
    }

    #[test]
    fn exact_intent_cache_hits_only_when_mode_and_budget_match() {
        let mut projection = forge_intent_execute_safe_v0(
            r#"/forge plan intent="profile market data" input=@latest"#,
            2048,
        )
        .expect("safe projection");
        let intent_hash = projection["intent_hash"].as_str().expect("intent hash").to_string();
        let store = std::env::temp_dir().join(format!(
            "forge-intent-cache-test-{}-{}",
            std::process::id(),
            now_ms()
        ));
        forge_agent_runtime::persist_direct_projection(&store, &mut projection)
            .expect("persist projection");

        let hit = forge_agent_runtime::lookup_cached_direct_projection(&store, Some(&intent_hash), "execute_safe", 1024)
            .expect("cache lookup")
            .expect("cache hit");
        assert_eq!(hit["cache_hit"].as_bool(), Some(true));
        assert_eq!(
            hit["cache_lookup"]["cache_reason"].as_str().or_else(|| hit["cache_reason"].as_str()),
            Some("exact_intent_mode_and_budget")
        );

        let wrong_mode = forge_agent_runtime::lookup_cached_direct_projection(
            &store,
            Some(&intent_hash),
            "planned_no_side_effects",
            1024,
        )
        .expect("wrong mode cache lookup");
        assert!(wrong_mode.is_none());

        let too_small_budget =
            forge_agent_runtime::lookup_cached_direct_projection(&store, Some(&intent_hash), "execute_safe", 4096)
                .expect("budget cache lookup");
        assert!(too_small_budget.is_none());
    }

    #[test]
    fn execute_approved_requires_matching_hash_gate_for_side_effects() {
        let source = r#"/forge run input=@latest intent="claim real work""#;
        let client = test_client();
        let result = forge_intent_execute_approved_v0(
            source,
            1024,
            false,
            None,
            None,
            false,
            &client,
        )
        .expect("approval-required projection");

        assert_eq!(result["mode"].as_str(), Some("approval_required"));
        assert_eq!(result["approval_gate"]["ok"].as_bool(), Some(false));
        assert_eq!(result["approval_gate"]["approval_required"].as_bool(), Some(true));
        assert_eq!(
            result["approval_gate"]["reason"].as_str(),
            Some("side_effects_require_approve_side_effects_and_matching_intent_policy_hashes")
        );
        assert_eq!(result["executed_steps"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn execute_approved_runs_only_after_intent_and_policy_hash_match() {
        let source = r#"/forge run input=@latest intent="claim real work""#;
        let planned = forge_intent_execute_projection(source, 1024).expect("planned projection");
        let intent_hash = planned["intent_hash"].as_str().expect("intent hash");
        let policy_hash = planned["policy_report"]["policy_hash"]
            .as_str()
            .expect("policy hash");
        let client = test_client();
        let result = forge_intent_execute_approved_v0(
            source,
            1024,
            true,
            Some(intent_hash),
            Some(policy_hash),
            false,
            &client,
        )
        .expect("approved projection");

        assert_eq!(result["mode"].as_str(), Some("execute_approved"));
        assert_eq!(result["approval_gate"]["ok"].as_bool(), Some(true));
        assert_eq!(result["execution_report"]["mode"].as_str(), Some("execute_approved"));
        assert_eq!(
            result["execution_report"]["side_effects_allowed"].as_bool(),
            Some(true)
        );
        let steps = result["executed_steps"].as_array().expect("executed steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0]["status"].as_str(),
            Some("skipped_unapproved_run_side_effect")
        );
        assert_eq!(result["execution_report"]["skipped_step_count"].as_u64(), Some(1));
    }

    #[test]
    fn golden_intent_workflow_is_hash_stable_and_compact() {
        let result = forge_intent_golden_workflow_smoke_v0();
        assert_eq!(result["ok"].as_bool(), Some(true));
        assert_eq!(
            result["stable_hashes_across_two_projections"].as_bool(),
            Some(true)
        );
        assert_eq!(result["projection"]["raw_data_returned"].as_bool(), Some(false));
        assert_eq!(result["target_visible_tool_count"].as_u64(), Some(4));
    }
}

#[cfg(test)]
fn test_client() -> McpClientInfo {
    McpClientInfo {
        name: "test".to_string(),
        version: None,
        model: None,
        token_mode: "unknown".to_string(),
    }
}

fn handle_tool_call(params: &Value, session: &McpSession) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call missing name".to_string())?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    if matches!(name, "forge.read_projection" | "forge_intent_read_projection") {
        let result = forge_read_projection_compact_v0(&args)?;
        return mcp_intent_tool_response(result);
    }
    let canonical = canonical_mcp_tool_name(name).ok_or_else(|| format!("unknown Forge tool: {name}"))?;
    if let Some(route) = internal_mcp_tool_route(canonical) {
        return mcp_internal_tool_response(route, &args);
    }
    match canonical {
        "about" => mcp_tool_response(forge_about()),
        "fbc_runtime" => {
            let result = forge_fbc_runtime_snapshot_mcp(&args)?;
            mcp_tool_response(result)
        }
        "create" => {
            let args: ProgramDefineArgs =
                serde_json::from_value(args).map_err(|e| format!("bad arguments: {e}"))?;
            let result = define_program(args, &session.client)?;
            mcp_tool_response(result)
        }
        "indicator" => {
            if args.get("title").is_some() && args.get("goal").is_some() {
                let args: ProgramDefineArgs =
                    serde_json::from_value(args).map_err(|e| format!("bad arguments: {e}"))?;
                let result = define_program(args, &session.client)?;
                mcp_tool_response(result)
            } else {
                let result = list_metric_ops(&args);
                mcp_tool_response(result)
            }
        }
        "metric" => {
            let compile_like = ["program_hash", "title", "goal", "metrics", "views", "spec_text", "source_schema", "constraints", "output_contract"]
                .iter()
                .any(|key| args.get(*key).is_some());
            let result = if compile_like {
                program_compile_validate_route(&args)?
            } else {
                list_metric_ops(&args)
            };
            mcp_tool_response(result)
        }
        "program_compile_validate_route" => {
            let result = program_compile_validate_route(&args)?;
            mcp_tool_response(result)
        }
        "forge_intent_search" => {
            let query = args.get("query").and_then(Value::as_str).unwrap_or("");
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(4)
                .clamp(1, 8) as usize;
            let result = forge_intent_search(query, limit);
            mcp_intent_tool_response(result)
        }
        "forge_intent_execute" => {
            let source = args
                .get("program")
                .or_else(|| args.get("intent_program"))
                .or_else(|| args.get("source"))
                .and_then(Value::as_str)
                .ok_or_else(|| "forge_intent_execute requires a ForgeSlash v0 program string".to_string())?;
            let max_bytes = args
                .get("max_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(4096)
                .clamp(512, 16 * 1024) as usize;
            let execute_safe = args
                .get("execute_safe")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || args
                    .get("mode")
                    .and_then(Value::as_str)
                    .map(|mode| mode.eq_ignore_ascii_case("execute_safe"))
                    .unwrap_or(false);
            let execute_approved = args
                .get("approve_side_effects")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || args
                    .get("mode")
                    .and_then(Value::as_str)
                    .map(|mode| mode.eq_ignore_ascii_case("execute_approved"))
                    .unwrap_or(false);
            let cache_enabled = !args
                .get("cache")
                .and_then(Value::as_bool)
                .map(|enabled| !enabled)
                .unwrap_or(false)
                && !args
                    .get("no_cache")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            let result = if execute_approved {
                forge_intent_execute_approved_persisted_v0(
                    source,
                    max_bytes,
                    args.get("approve_side_effects").and_then(Value::as_bool).unwrap_or(false),
                    args.get("approved_intent_hash").and_then(Value::as_str),
                    args.get("approved_policy_hash").and_then(Value::as_str),
                    args.get("allow_run_side_effects").and_then(Value::as_bool).unwrap_or(false),
                    &session.client,
                )?
            } else if execute_safe {
                forge_intent_execute_safe_persisted_v0(source, max_bytes, cache_enabled)?
            } else {
                forge_intent_execute_projection_persisted_v0(source, max_bytes, cache_enabled)?
            };
            mcp_intent_tool_response(result)
        }
        "capabilities" => {
            let result = list_metric_ops(&args);
            mcp_tool_response(result)
        }
        "programs" => {
            let result = list_programs(&args)?;
            mcp_tool_response(result)
        }
        "program" => {
            let program_hash = args
                .get("program_hash")
                .or_else(|| args.get("program_id"))
                .and_then(Value::as_str)
                .ok_or_else(|| "program requires program_hash".to_string())?;
            let result = read_program(program_hash)?;
            mcp_tool_response(result)
        }
        "execute" => {
            let args: ProgramExecuteArgs =
                serde_json::from_value(args).map_err(|e| format!("bad arguments: {e}"))?;
            let result = execute_program(args, &session.client)?;
            mcp_tool_response(result)
        }
        "alpha" => {
            let args: AlphaStrategyArgs =
                serde_json::from_value(args).map_err(|e| format!("bad arguments: {e}"))?;
            let job = run_alpha_strategy(args, &session.client)?;
            mcp_tool_response(serde_json::to_value(job).map_err(|e| format!("encode strategy result: {e}"))?)
        }
        "jobs" => {
            let limit = bounded_limit(args.get("limit"), MCP_LIST_LIMIT_DEFAULT, MCP_LIST_LIMIT_MAX);
            let jobs = list_jobs(limit)?;
            let pending_count = jobs
                .iter()
                .filter(|job| job.get("status").and_then(Value::as_str) == Some("pending"))
                .count();
            mcp_tool_response(json!({
                "jobs": jobs,
                "limit": limit,
                "pending_count": pending_count,
                "safe_next_call": if pending_count == 1 { "run {}" } else if pending_count > 1 { "run { job_id:\"...\" }" } else { "run { intent:\"...\", inputs:[...], plan_only:true }" },
                "do_not_read_source": true,
                "token_safety": token_safety()
            }))
        }
        "pending" => {
            let limit = bounded_limit(args.get("limit"), MCP_LIST_LIMIT_DEFAULT, MCP_LIST_LIMIT_MAX);
            let jobs = list_pending_jobs(limit)?;
            let pending_count = jobs.len();
            mcp_tool_response(json!({
                "jobs": jobs,
                "limit": limit,
                "pending_count": pending_count,
                "safe_next_call": if pending_count == 1 { "run {}" } else if pending_count > 1 { "run { job_id:\"...\" }" } else { "ask the user to drop a file into Forge, or call run { intent, inputs, plan_only:true }" },
                "do_not_read_source": true,
                "token_safety": token_safety()
            }))
        }
        "read" => {
            let result = read_dispatch(&args)?;
            mcp_tool_response(result)
        }
        "run" => {
            let result = run_dispatch(args, &session.client)?;
            mcp_tool_response(result)
        }
        "logs" => {
            let result = tail_job_log(&args)?;
            mcp_tool_response(result)
        }
        "artifacts" => {
            let job_id = job_id_arg(&args, "artifacts")?;
            let result = job_artifacts(job_id)?;
            mcp_tool_response(result)
        }
        "inject" => {
            let result = inject_job_result(&args, &session.client)?;
            mcp_tool_response(result)
        }
        "rename" => {
            let result = update_job_title(&args)?;
            mcp_tool_response(result)
        }
        "cancel" => {
            let result = request_job_cancel(&args, &session.client)?;
            mcp_tool_response(result)
        }
        "legacy_docs" => {
            let result = list_documents(&args)?;
            mcp_tool_response(result)
        }
        "doc" => {
            let job_id = job_id_arg(&args, "doc")?;
            let result = document_summary(job_id)?;
            mcp_tool_response(result)
        }
        "preview" => {
            let result = document_preview(&args)?;
            mcp_tool_response(result)
        }
        "doc_sessions" => {
            let result = document_sessions(&args)?;
            mcp_tool_response(result)
        }
        _ => Err(format!("unrouted Forge tool: {canonical}")),
    }
}

fn mcp_internal_tool_response(tool: &str, args: &Value) -> Result<Value, String> {
    let result = call_internal_tool_value(tool, args)?;
    if matches!(
        tool,
        "forge_analyze_3d_mapping"
            | "forge_3d_metric_catalog"
            | "forge_model_3d_mapping"
            | "forge_run_visual_program"
    ) {
        return mcp_compact_tool_response(result);
    }
    mcp_tool_response(result)
}

fn call_internal_tool_value(tool: &str, args: &Value) -> Result<Value, String> {
    let store_path = forge_agent_tools::resolve_store_path()?;
    let active_job = args
        .get("job_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(active_job_id_from_env);
    forge_agent_tools::call_internal_tool(&store_path, tool, args, active_job.as_deref())
}

fn call_state_kernel_read_value(route: &str, args: &Value) -> Result<Value, String> {
    let store_path = forge_agent_tools::resolve_store_path()?;
    let normalized = forge_agent_runtime::direct_state_kernel_route_args(route, args);
    forge_agent_runtime::direct_state_kernel_read(&store_path, &normalized)
}

fn call_state_kernel_apply_value(route: &str, args: &Value) -> Result<Value, String> {
    let store_path = forge_agent_tools::resolve_store_path()?;
    let normalized = forge_agent_runtime::direct_state_kernel_route_args(route, args);
    forge_agent_runtime::direct_state_kernel_apply(&store_path, &normalized)
}

fn forge_intent_execute_projection(source: &str, max_bytes: usize) -> Result<Value, String> {
    forge_agent_runtime::direct_plan_projection(source, max_bytes)
}

fn forge_intent_execute_projection_persisted_v0(source: &str, max_bytes: usize, cache_enabled: bool) -> Result<Value, String> {
    let mut projection = forge_intent_execute_projection(source, max_bytes)?;
    let store_path = forge_store_dir()?;
    if cache_enabled {
        if let Some(cached) = forge_agent_runtime::lookup_cached_direct_projection(
            &store_path,
            projection.get("intent_hash").and_then(Value::as_str),
            "planned_no_side_effects",
            max_bytes,
        )? {
            return Ok(cached);
        }
    }
    forge_agent_runtime::persist_direct_projection(&store_path, &mut projection)?;
    Ok(projection)
}

fn forge_intent_execute_safe_v0(source: &str, max_bytes: usize) -> Result<Value, String> {
    forge_agent_runtime::direct_safe_execution_with(source, max_bytes, |idx, step, budget| {
        execute_compiled_intent_step_safe_v0(idx, step, budget)
    })
}

fn forge_intent_execute_safe_persisted_v0(source: &str, max_bytes: usize, cache_enabled: bool) -> Result<Value, String> {
    if cache_enabled {
        let planned = forge_intent_execute_projection(source, max_bytes)?;
        let store_path = forge_store_dir()?;
        if let Some(cached) = forge_agent_runtime::lookup_cached_direct_projection(
            &store_path,
            planned.get("intent_hash").and_then(Value::as_str),
            "execute_safe",
            max_bytes,
        )? {
            return Ok(cached);
        }
    }
    let mut projection = forge_intent_execute_safe_v0(source, max_bytes)?;
    let store_path = forge_store_dir()?;
    forge_agent_runtime::persist_direct_projection(&store_path, &mut projection)?;
    Ok(projection)
}

fn forge_intent_execute_approved_persisted_v0(
    source: &str,
    max_bytes: usize,
    approve_side_effects: bool,
    approved_intent_hash: Option<&str>,
    approved_policy_hash: Option<&str>,
    allow_run_side_effects: bool,
    client: &McpClientInfo,
) -> Result<Value, String> {
    let mut projection = forge_intent_execute_approved_v0(
        source,
        max_bytes,
        approve_side_effects,
        approved_intent_hash,
        approved_policy_hash,
        allow_run_side_effects,
        client,
    )?;
    let store_path = forge_store_dir()?;
    forge_agent_runtime::persist_direct_projection(&store_path, &mut projection)?;
    Ok(projection)
}

fn forge_intent_execute_approved_v0(
    source: &str,
    max_bytes: usize,
    approve_side_effects: bool,
    approved_intent_hash: Option<&str>,
    approved_policy_hash: Option<&str>,
    allow_run_side_effects: bool,
    client: &McpClientInfo,
) -> Result<Value, String> {
    forge_agent_runtime::direct_approved_execution_with(
        source,
        max_bytes,
        approve_side_effects,
        approved_intent_hash,
        approved_policy_hash,
        allow_run_side_effects,
        |idx, step, budget| {
            execute_compiled_intent_step_approved_v0(
                idx,
                step,
                budget,
                allow_run_side_effects,
                client,
            )
        },
    )
}

fn forge_read_projection_compact_v0(args: &Value) -> Result<Value, String> {
    let store_path = forge_store_dir()?;
    if projection_request_is_fbc(args) {
        return read_fbc_app_projection_from_store(&store_path, args);
    }
    if args.get("job_id").and_then(Value::as_str).is_some() {
        return read_forge_job_projection_via_fbc(&store_path, args);
    }
    forge_agent_runtime::direct_read_projection(&store_path, args)
}

fn projection_request_is_fbc(args: &Value) -> bool {
    ["kind", "ref", "projection_ref", "scope"]
        .iter()
        .filter_map(|key| args.get(*key).and_then(Value::as_str))
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower == "fbc"
                || lower == "fbc_app"
                || lower == "app_fbc"
                || lower.contains("fbc/app")
                || lower.contains("forge_fbc")
        })
}

fn read_fbc_app_projection_from_store(store_path: &Path, args: &Value) -> Result<Value, String> {
    let manifest_path = store_path
        .join("fbc")
        .join("app")
        .join("app_fbc_registry_batch.json");
    if !manifest_path.exists() {
        return Ok(json!({
            "kind": "forge_fbc_app_projection_read_v0",
            "found": false,
            "raw_data_returned": false,
            "reason": "FBC app projection has not been materialized yet; call fbc_runtime or forge_fbc_runtime_snapshot first",
            "manifest_path": manifest_path.display().to_string()
        }));
    }
    let manifest = read_json_value(&manifest_path)?;
    let records = manifest
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let limit = bounded_limit(args.get("limit"), 16, 64);
    let records = records.into_iter().take(limit).collect::<Vec<_>>();
    Ok(json!({
        "kind": "forge_fbc_app_projection_read_v0",
        "found": true,
        "raw_data_returned": false,
        "manifest_path": manifest_path.display().to_string(),
        "graph_hash": manifest.get("graphHash").cloned().unwrap_or(Value::Null),
        "ledger_root_hash": manifest.get("ledgerRootHash").cloned().unwrap_or(Value::Null),
        "tool_count": manifest.get("toolCount").cloned().unwrap_or(Value::Null),
        "ok_count": manifest.get("okCount").cloned().unwrap_or(Value::Null),
        "denied_count": manifest.get("deniedCount").cloned().unwrap_or(Value::Null),
        "record_count": records.len(),
        "records": records
    }))
}

fn read_forge_job_projection_via_fbc(store_path: &Path, args: &Value) -> Result<Value, String> {
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "job projection requires job_id".to_string())?;
    let limit = bounded_limit(args.get("limit"), 8, 32) as u16;
    let fbc = forge_fbc_host::execute_job_read_projection(Some(store_path), job_id, limit)?;
    let fbc_query = fbc.output;
    let jobs = list_jobs(limit as usize)?;
    let selected = if job_id == "latest" {
        jobs.first().cloned()
    } else {
        jobs.into_iter()
            .find(|job| job.get("job_id").or_else(|| job.get("jobId")).and_then(Value::as_str) == Some(job_id))
    };
    Ok(json!({
        "kind": "forge_job_projection_read_via_fbc_v0",
        "found": selected.is_some(),
        "raw_data_returned": false,
        "fbc_query": fbc_query,
        "fbc_proof": fbc.proof,
        "fbc_ledger_hash": fbc.ledger_hash,
        "job": selected.unwrap_or(Value::Null),
        "store_path": store_path.display().to_string()
    }))
}

fn execute_compiled_intent_step_safe_v0(index: usize, step: &Value, result_budget_bytes: usize) -> Value {
    let route = step.get("route").and_then(Value::as_str).unwrap_or("");
    let command_hash = step.get("command_hash").and_then(Value::as_str).unwrap_or("");
    let args = step.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let side_effect = step.get("side_effect").and_then(Value::as_bool).unwrap_or(true);
    if side_effect {
        return json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "skipped_side_effect",
            "reason": "execute_safe runs only read-only and plan_only intent steps"
        });
    }
    let result = match route {
        "run" => {
            if args.get("plan_only").and_then(Value::as_bool).unwrap_or(false) {
                Ok(plan_run(args))
            } else {
                Ok(json!({
                    "status": "skipped_non_plan_run",
                    "reason": "run is executable in execute_safe only when plan_only=true"
                }))
            }
        }
        "read" => read_dispatch(&args),
        "brain_recall" => call_state_kernel_read_value("brain_recall", &args),
        "brain_explain" => call_state_kernel_read_value("brain_explain", &args),
        other => Ok(json!({
            "status": "skipped_unsupported_safe_route",
            "reason": "route is not in the execute_safe allowlist",
            "route": other
        })),
    };
    match result {
        Ok(value) => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "executed_safe",
            value,
            result_budget_bytes,
        ),
        Err(err) => json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "error",
            "raw_data_returned": false,
            "error": err
        }),
    }
}

fn execute_compiled_intent_step_approved_v0(
    index: usize,
    step: &Value,
    result_budget_bytes: usize,
    allow_run_side_effects: bool,
    client: &McpClientInfo,
) -> Value {
    let route = step.get("route").and_then(Value::as_str).unwrap_or("");
    let command_hash = step.get("command_hash").and_then(Value::as_str).unwrap_or("");
    let args = step.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let side_effect = step.get("side_effect").and_then(Value::as_bool).unwrap_or(true);
    if !side_effect {
        return execute_compiled_intent_step_safe_v0(index, step, result_budget_bytes);
    }
    let result = match route {
        "create" => serde_json::from_value::<ProgramDefineArgs>(args.clone())
            .map_err(|err| format!("bad create intent arguments: {err}"))
            .and_then(|args| define_program(args, client)),
        "brain_commit" => call_state_kernel_apply_value("brain_commit", &args),
        "run" => {
            if allow_run_side_effects {
                run_dispatch(args, client)
            } else {
                return json!({
                    "index": index,
                    "route": route,
                    "command_hash": command_hash,
                    "status": "skipped_unapproved_run_side_effect",
                    "raw_data_returned": false,
                    "reason": "non-plan run requires allow_run_side_effects=true in addition to matching approval hashes"
                });
            }
        }
        other => Ok(json!({
            "status": "skipped_unsupported_side_effect_route",
            "reason": "route is not executable by execute_approved",
            "route": other
        })),
    };
    match result {
        Ok(value) => forge_agent_runtime::compact_step_result(
            index,
            route,
            command_hash,
            "executed_side_effect",
            value,
            result_budget_bytes,
        ),
        Err(err) => json!({
            "index": index,
            "route": route,
            "command_hash": command_hash,
            "status": "error",
            "raw_data_returned": false,
            "error": err
        }),
    }
}

fn forge_intent_golden_workflow_smoke_v0() -> Value {
    let search = forge_intent_search("pending upload visual program brain commit projection", 4);
    let first = forge_intent_execute_projection(FORGE_INTENT_GOLDEN_WORKFLOW_V0, 4096);
    let second = forge_intent_execute_projection(FORGE_INTENT_GOLDEN_WORKFLOW_V0, 4096);
    match (first, second) {
        (Ok(first), Ok(second)) => {
            let stable_hashes = first["intent_hash"] == second["intent_hash"]
                && first["trace_card"]["trace_hash"] == second["trace_card"]["trace_hash"]
                && first["forge_projection"]["trace_hash"] == second["forge_projection"]["trace_hash"];
            json!({
                "ok": stable_hashes
                    && first["ok"].as_bool().unwrap_or(false)
                    && first["raw_data_returned"].as_bool() == Some(false),
                "status": "step_17_golden_smoke_no_side_effects",
                "search_result_count": search["result_count"],
                "stable_hashes_across_two_projections": stable_hashes,
                "default_visible_tool_count": visible_tool_names().len(),
                "target_visible_tool_count": 4,
                "public_routing_nodes_after_cutover": ["forge.search", "forge.execute", "forge.read_projection", "forge.cancel"],
                "intent_hash": first["intent_hash"],
                "trace_hash": first["trace_card"]["trace_hash"],
                "route_count": first["trace_card"]["route_count"],
                "side_effect_count": first["trace_card"]["side_effect_count"],
                "distillation_target": first["distillation_analysis"]["target"],
                "promotion_status": first["promotion_manifest"]["status"],
                "projection": first["forge_projection"],
                "search": search,
                "source": FORGE_INTENT_GOLDEN_WORKFLOW_V0
            })
        }
        (Err(err), _) | (_, Err(err)) => json!({
            "ok": false,
            "status": "step_17_golden_smoke_failed",
            "error": err,
            "source": FORGE_INTENT_GOLDEN_WORKFLOW_V0
        }),
    }
}

fn forge_about() -> Value {
    let intent_search_smoke = forge_intent_search("visual program mapping", 3);
    let forge_slash_smoke = match forge_intent_execute_projection(forge_intent::FORGE_SLASH_V0_EXAMPLE, 4096) {
        Ok(result) => result,
        Err(err) => json!({
            "ok": false,
            "error": err,
            "example": forge_intent::FORGE_SLASH_V0_EXAMPLE
        }),
    };
    json!({
        "name": FORGE_DISPLAY_NAME,
        "display_name": FORGE_DISPLAY_NAME,
        "canonical_name": FORGE_DISPLAY_NAME,
        "technical_slug": FORGE_TECHNICAL_SLUG,
        "agent_display_rule": "Always display this MCP server as Forge with a capital F. The lowercase slug 'forge' is only the technical namespace/config key.",
        "short_description": "Forge is a local compute engine for AI agents.",
        "official_description": FORGE_OFFICIAL_DESCRIPTION,
        "surface_contract": {
            "contract": FORGE_MCP_SURFACE_CONTRACT,
            "status": "step_1_frozen_goal",
            "mcp_role": "transport_and_compatibility_boundary",
            "default_visible_tool_target": ["forge.search", "forge.execute", "forge.read_projection", "forge.cancel"],
            "compact_surface_default": "tools/list exposes only forge.search, forge.execute, forge.read_projection and forge.cancel by default.",
            "legacy_surface_escape_hatch": "FORGE_MCP_SURFACE=broad or FORGE_MCP_LEGACY_SURFACE=1 restores the transitional broad MCP catalog for compatibility debugging.",
            "tool_annotations": "Every visible tool declares MCP safety annotations: readOnlyHint, destructiveHint, idempotentHint and openWorldHint.",
            "opt_in_intent_surface_env": "FORGE_INTENT_MCP_SURFACE=1 exposes forge_intent_search and forge_intent_execute for parity testing without changing the default manifest.",
            "safe_execution_mode": "forge.execute mode=execute_safe executes only read-only and plan_only lowered routes; side-effect steps are skipped with per-step proofs.",
            "safe_execution_report": "execute_safe returns forge_intent_execution_report_v0 with result_hash per executed step, executed_steps_hash and execution_hash for replay/cache/router evidence.",
            "approved_execution_gate": "forge.execute mode=execute_approved requires approve_side_effects=true plus matching approved_intent_hash and approved_policy_hash; non-plan run also requires allow_run_side_effects=true.",
            "transitional_current_catalog": "Existing detailed MCP tools remain callable while the intent compiler proves parity.",
            "direct_mcp_exception_rule": "Keep a direct MCP tool visible only when measured workflows prove it simpler, safer or faster than the intent/code path.",
            "cutover_readiness": compact_cutover_readiness()
        },
        "forge_slash_v0": {
            "status": "step_2_source_contract",
            "grammar": forge_intent::FORGE_SLASH_V0_GRAMMAR,
            "verbs": ["recall", "plan", "create", "run", "project", "commit", "explain"],
            "authority": "Intent programs carry refs, hashes and bounded parameters only; raw filesystem paths are rejected before execution.",
            "parser_smoke": forge_slash_smoke
        },
        "forge_intent_search_v0": {
            "status": "step_5_internal_index",
            "surface": "forge.search_default",
            "contract": "Return compact intent signatures, replay candidates and executable next_call plans over current tools/capabilities/slash aliases; never return full schemas by default.",
            "smoke": intent_search_smoke
        },
        "forge_intent_golden_workflow_v0": forge_intent_golden_workflow_smoke_v0(),
        "agent_doctrine": {
            "default_reflex": "If user data is >256 KB, >1,000 rows/lines, multi-file, CSV/Excel/PDF/log/dataset, repetitive, scientific/numerical/document-heavy, or needs proof/artifacts, use Forge before reading or calculating inside the LLM. If it is >1 MB, >10,000 rows/lines, a full log, simulation/search/backtest/optimization or artifact/proof workflow, Forge is the default path.",
            "read_vs_forge_thresholds": {
                "read_is_ok_when": [
                    "the user asks for a tiny targeted inspection under 64 KB",
                    "the task is editing or reviewing a few source files in the current repo, not analyzing a large dataset",
                    "the user explicitly asks to inspect raw text and token cost is negligible"
                ],
                "use_forge_when": [
                    "file is >256 KB or likely >1,000 rows/lines",
                    "there are multiple files or repeated calculations",
                    "the source is CSV/Excel/PDF/logs/documents/datasets/telemetry/sequences",
                    "the agent would write loops, grep whole logs, parse rows, compute metrics, simulate, search, optimize or benchmark"
                ],
                "forge_required_default_when": [
                    "file is >1 MB or likely >10,000 rows/lines",
                    "the task involves full logs, many artifacts, proof/hash verification, 3D mappings, simulations, backtests, optimization or large codebase metrics",
                    "reading the raw content would consume significant context or fail for a normal user"
                ]
            },
            "fast_path": {
                "clear_intent": "run { intent:\"...\", inputs:[{ path:\"...\", role:\"data\" }], plan_only:true }",
                "pending_ui_upload": "jobs {} then run { job_id:\"...\" }",
                "after_compute": "logs { job_id, cursor } then read { job_id, kind:\"artifacts\" }",
                "note": "capabilities is a GPS for uncertainty, not a required first step when the user intent is already clear"
            },
            "use_when": [
                "input is large, repetitive, expensive, scientific, numerical, document-heavy or needs proof",
                "the user wants calculations on CSV, Excel, code, logs, documents, sequences, telemetry, scientific data or other datasets",
                "an agent needs to create its own calculation program, metric set, indicator, detector, scoring function or analysis recipe",
                "the result should be content-addressed, reproducible, hash-verifiable or reusable across sessions",
                "reading the raw file or full log would waste tokens or fail for a normal user"
            ],
            "do_not": [
                "do not read raw CSV/source/log/artifact files directly when Forge can summarize, preview, calculate or return references",
                "do not inject large source contents into the LLM context",
                "do not recompute work manually when Forge can reuse content-addressed results by hash"
            ],
            "preferred_flow": [
                "run with plan_only=true first when the intent is clear and the input/work crosses the Forge thresholds",
                "jobs then run { job_id } for pending UI uploads",
                "capabilities only when the agent needs help choosing a domain/template/operator",
                "create when the agent needs a custom reusable program",
                "run to execute the pending upload/program/capability/intent",
                "logs to monitor by cursor",
                "read to retrieve sanitized summaries, previews, artifacts and proof refs"
            ]
        },
        "what_forge_is_not": [
            "Forge is not only a trading tool.",
            "Forge is not a file reader that dumps large CSVs, logs or source files into the LLM context."
        ],
        "core_capabilities": [
            "run heavy local CPU/GPU computations for AI agents",
            "keep large documents and datasets outside the LLM context",
            "address inputs, programs, intermediate results and artifacts by hash",
            "reuse identical or overlapping calculations instead of repeating them",
            "return compact verified outputs: job ids, hashes, proofs, artifact references and bounded previews",
            "support existing templates and custom agent-defined compute programs created with metric tags"
        ],
        "domains": [
            "finance/trading/markets: indicators, strategies, backtests, volume anomalies, volatility regimes, risk",
            "code/software/security: codebase metrics, AST/pattern analysis, benchmarks, logs, dependency and duplication analysis",
            "science/math/engineering: optimization, simulation, statistics, signals, matrices, geometry, CFD, materials",
            "biology/medicine: k-mers, DNA/RNA/protein sequences, biomarkers, physiology, medical datasets, anomaly detection",
            "chemistry/pharma: molecular metrics, similarity, screening, reactions, conformers, spectra, toxicity",
            "documents/business/data: CSV/Excel/PDF/log profiling, QA, audits, scoring, comparison, extraction",
            "industry/aerospace/energy: telemetry, sensors, predictive maintenance, trajectories, embedded systems, process optimization"
        ],
        "recommended_next_call": "run { intent: \"describe the calculation you want\", inputs: [{ path: \"...\", role: \"data\" }], plan_only: true }",
        "copy_ready_fast_paths": [
            "run { intent: \"find anomalies and summarize metrics\", inputs: [{ path: \"...\", role: \"data\" }], plan_only: true }",
            "jobs {}",
            "run { job_id: \"...\" }",
            "read { job_id: \"...\", kind: \"artifacts\" }"
        ],
        "next_actions": default_next_actions(),
        "token_safety": token_safety()
    })
}

fn run_dispatch(args: Value, client: &McpClientInfo) -> Result<Value, String> {
    let args = inject_active_session_defaults(args);
    if let Some((should_run, pending_value)) = resolve_pending_run_args(&args)? {
        if should_run {
            return run_pending_job(pending_value, client);
        }
        return Ok(pending_value);
    }
    if args
        .get("plan_only")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(plan_run(args));
    }
    if args.get("program_hash").or_else(|| args.get("program_id")).is_some() {
        let args: ProgramExecuteArgs =
            serde_json::from_value(args).map_err(|e| format!("bad run program arguments: {e}"))?;
        return execute_program(args, client);
    }
    if run_has_program_library_selector(&args) {
        let mut args = args;
        let program_hash = resolve_program_library_selector(&args)?;
        if let Value::Object(ref mut obj) = args {
            obj.insert("program_hash".to_string(), json!(program_hash));
        }
        let args: ProgramExecuteArgs =
            serde_json::from_value(args).map_err(|e| format!("bad run library program arguments: {e}"))?;
        return execute_program(args, client);
    }
    if args.get("csv_path").is_some()
        || args
            .get("capability")
            .and_then(Value::as_str)
            .map(|v| v.eq_ignore_ascii_case("alpha"))
            .unwrap_or(false)
    {
        let args: AlphaStrategyArgs =
            serde_json::from_value(args).map_err(|e| format!("bad run alpha arguments: {e}"))?;
        let job = run_alpha_strategy(args, client)?;
        return serde_json::to_value(job).map_err(|e| format!("encode strategy result: {e}"));
    }
    if args.get("job_id").is_some() {
        return run_pending_job(args, client);
    }
    if args.get("intent").is_some() {
        let args: RunIntentArgs =
            serde_json::from_value(args).map_err(|e| format!("bad run intent arguments: {e}"))?;
        return run_intent(args, client);
    }
    Ok(run_needs_routing_response(&args))
}

fn active_job_id_from_env() -> Option<String> {
    let job_id = std::env::var("FORGE_ACTIVE_JOB_ID").ok()?;
    let job_id = job_id.trim().to_string();
    if job_id.is_empty() || validate_job_id(&job_id).is_err() || find_job_manifest_path(&job_id).is_err() {
        return None;
    }
    Some(job_id)
}

fn value_has_non_empty_inputs(value: &Value) -> bool {
    value
        .get("inputs")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

fn value_has_program_target(value: &Value) -> bool {
    value.get("program_hash")
        .or_else(|| value.get("program_id"))
        .or_else(|| value.get("program"))
        .or_else(|| value.get("program_title"))
        .or_else(|| value.get("program_query"))
        .is_some()
}

fn inject_active_session_defaults(mut args: Value) -> Value {
    let Some(active_job_id) = active_job_id_from_env() else {
        return args;
    };
    let has_program_target = value_has_program_target(&args);
    let has_non_empty_inputs = value_has_non_empty_inputs(&args);
    let should_default_job_id = args
        .as_object()
        .map(|obj| obj.is_empty())
        .unwrap_or(false)
        || (args.get("job_id").is_none() && args.get("pending").is_some());
    let Some(obj) = args.as_object_mut() else {
        return args;
    };
    if has_program_target && !has_non_empty_inputs {
        obj.insert(
            "inputs".to_string(),
            json!([{ "job_id": active_job_id.clone(), "role": "active_session" }]),
        );
        obj.entry("parent_session_id".to_string())
            .or_insert_with(|| json!(active_job_id));
        return args;
    }
    if should_default_job_id {
        obj.entry("job_id".to_string())
            .or_insert_with(|| json!(active_job_id));
    }
    args
}

fn resolve_pending_run_args(args: &Value) -> Result<Option<(bool, Value)>, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "run arguments must be an object".to_string())?;
    let requested_pending = obj.is_empty()
        || args.get("pending").and_then(Value::as_bool).unwrap_or(false)
        || args
            .get("job_id")
            .and_then(Value::as_str)
            .map(|v| {
                let id = v.trim().to_lowercase();
                matches!(id.as_str(), "pending" | "latest" | "latest_pending" | "newest")
            })
            .unwrap_or(false);
    if !requested_pending {
        return Ok(None);
    }

    let pending = list_pending_jobs(2)?;
    match pending.len() {
        0 => Ok(Some((false, json!({
            "state": "no_pending_job",
            "ran": false,
            "why_not": "No pending Forge UI upload is available.",
            "safe_next_call": "run { intent:\"...\", inputs:[{ path:\"...\", role:\"data\" }], plan_only:true }",
            "suggested_retry": "Ask the user to drop files into Forge, then call run {} again.",
            "do_not_read_source": true,
            "token_safety": token_safety()
        })))),
        1 => {
            let job_id = pending[0]
                .get("job_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "pending job is missing job_id".to_string())?;
            let mut launch = args.clone();
            let launch_obj = launch
                .as_object_mut()
                .ok_or_else(|| "run arguments must be an object".to_string())?;
            launch_obj.insert("job_id".to_string(), json!(job_id));
            launch_obj.remove("pending");
            Ok(Some((true, launch)))
        }
        _ => Ok(Some((false, json!({
            "state": "multiple_pending_jobs",
            "ran": false,
            "why_not": "More than one pending Forge upload exists; choose the intended job_id.",
            "pending_jobs": pending,
            "safe_next_call": "run { job_id:\"...\" }",
            "suggested_retry": "Pick one job_id from pending_jobs, then call run { job_id:\"...\" }.",
            "do_not_read_source": true,
            "token_safety": token_safety()
        })))),
    }
}

fn run_needs_routing_response(args: &Value) -> Value {
    json!({
        "state": "run_needs_target",
        "ran": false,
        "why_not": "run needs a pending job, job_id, csv_path, program/program_hash, or natural-language intent.",
        "received": summarize_run_inputs(args),
        "safe_next_call": "run { intent:\"...\", inputs:[{ path:\"...\", role:\"data\" }], plan_only:true }",
        "suggested_retry": [
            "run {} if the user has exactly one pending Forge upload",
            "run { pending:true } to claim the newest pending upload",
            "run { job_id:\"...\" } to claim a specific pending upload",
            "run { intent:\"...\", inputs:[...], plan_only:true } to plan safely before execution"
        ],
        "do_not_read_source": true,
        "do_not": [
            "do not inspect Forge source code to guess parameters",
            "do not shell-read raw CSV/log/source files for this workflow"
        ],
        "token_safety": token_safety()
    })
}

fn read_dispatch(args: &Value) -> Result<Value, String> {
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    if let Some(program_hash) = args
        .get("program_hash")
        .or_else(|| args.get("program_id"))
        .and_then(Value::as_str)
    {
        return read_program(program_hash);
    }
    if kind == "programs" {
        return list_programs(args);
    }
    if kind == "docs" || kind == "documents" {
        return call_state_kernel_read_value("documents", args);
    }
    if kind == "sessions" || kind == "history" {
        return call_state_kernel_read_value("sessions", args);
    }
    if kind == "skill_candidates" || kind == "skills" {
        return call_state_kernel_read_value("skill_candidates", args);
    }
    if kind == "verified_program_candidates" || kind == "program_candidates" || kind == "verified_programs" {
        return call_state_kernel_read_value("verified_program_candidates", args);
    }
    if kind == "mapping" || kind == "visual_mapping" || kind == "visualization_3d" {
        let store_path = forge_agent_tools::resolve_store_path()?;
        return forge_agent_tools::call_internal_tool(
            &store_path,
            "forge_interpret_visual_mapping",
            args,
            None,
        );
    }
    if kind == "mapping_metrics" || kind == "metric_catalog_3d" || kind == "3d_metric_catalog" {
        let store_path = forge_agent_tools::resolve_store_path()?;
        return forge_agent_tools::call_internal_tool(
            &store_path,
            "forge_3d_metric_catalog",
            args,
            None,
        );
    }
    if kind == "mapping_model" || kind == "model_mapping" || kind == "3d_model" {
        let store_path = forge_agent_tools::resolve_store_path()?;
        return forge_agent_tools::call_internal_tool(
            &store_path,
            "forge_model_3d_mapping",
            args,
            None,
        );
    }
    if kind == "mapping_analysis" || kind == "analyze_mapping" || kind == "visual_mapping_analysis" {
        let store_path = forge_agent_tools::resolve_store_path()?;
        return forge_agent_tools::call_internal_tool(
            &store_path,
            "forge_analyze_3d_mapping",
            args,
            None,
        );
    }
    if kind == "profile" || kind == "settings" {
        return call_state_kernel_read_value("profile", args);
    }
    if kind == "atlas" {
        return call_state_kernel_read_value("atlas", args);
    }
    let job_id = args
        .get("job_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "read requires job_id or program_hash".to_string())?;
    match kind.as_str() {
        "artifacts" | "artifact" | "proof" => job_artifacts(job_id),
        "doc" | "document" => document_summary(job_id),
        "preview" => document_preview(args),
        "sessions" => document_sessions(args),
        _ => read_job_summary(job_id),
    }
}

fn run_has_program_library_selector(args: &Value) -> bool {
    ["program", "program_title", "program_query"]
        .iter()
        .any(|key| args.get(*key).and_then(Value::as_str).map(|v| !v.trim().is_empty()).unwrap_or(false))
}

fn resolve_program_library_selector(args: &Value) -> Result<String, String> {
    let selector = args
        .get("program")
        .or_else(|| args.get("program_title"))
        .or_else(|| args.get("program_query"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "program library selector is empty".to_string())?;
    if selector.len() == 16 && selector.chars().all(|c| c.is_ascii_hexdigit()) {
        validate_content_hash(selector, "program")?;
        return Ok(selector.to_string());
    }

    let query = selector.to_lowercase();
    let dir = programs_dir()?;
    let mut candidates = Vec::<(SystemTime, String, String)>::new();
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
                let value = match read_json_value(&entry.path()) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let summary = summarize_program_value(value);
                let hash = summary
                    .get("program_hash")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if hash.is_empty() {
                    continue;
                }
                let haystack = [
                    summary.get("title").and_then(Value::as_str).unwrap_or(""),
                    summary.get("domain").and_then(Value::as_str).unwrap_or(""),
                    summary.get("goal").and_then(Value::as_str).unwrap_or(""),
                    summary.get("template").and_then(Value::as_str).unwrap_or(""),
                    hash.as_str(),
                ]
                .join(" ")
                .to_lowercase();
                if haystack.contains(&query) {
                    candidates.push((modified, hash, haystack));
                }
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("read programs dir '{}': {err}", dir.display())),
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates
        .into_iter()
        .map(|(_, hash, _)| hash)
        .next()
        .ok_or_else(|| {
            format!(
                "no Forge program found for '{selector}'. Use read {{ kind:\"programs\", query:\"{selector}\" }} or create a new program first."
            )
        })
}

fn plan_run(args: Value) -> Value {
    let intent = args
        .get("intent")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let capability = args
        .get("capability")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let inferred = infer_capability(
        if intent.is_empty() { None } else { Some(intent.as_str()) },
        if capability.is_empty() { None } else { Some(capability.as_str()) },
    );
    let input_summary = summarize_run_inputs(&args);
    let proposed_metrics = proposed_metrics_for_capability(&inferred);
    let required_files = required_files_for_capability(&inferred, &input_summary);
    let input_policy = input_policy_for_capability(&inferred, &intent, &input_summary);
    let cache_probe = plan_cache_probe(&args, &proposed_metrics, &input_summary);
    let estimated_compute_cost = estimate_compute_cost(&inferred, proposed_metrics.len(), &input_summary);
    let suggested_program = suggested_program_for_intent(&inferred, &intent, &capability);
    let selected_template = selected_template_profile(&inferred, &intent, &capability);
    let recommended_tool = if inferred == "custom_program" {
        "create"
    } else {
        "run"
    };
    let recommended_next_call = capability_launch_command(&inferred, false);
    let ready_to_launch_call = capability_launch_command(&inferred, true);
    json!({
        "plan_only": true,
        "intent": intent,
        "requested_capability": capability,
        "inferred_capability": inferred,
        "recommended_tool": recommended_tool,
        "recommended_next_call": recommended_next_call,
        "ready_to_launch_call": ready_to_launch_call,
        "selected_template": selected_template,
        "input_summary": input_summary,
        "input_policy": input_policy,
        "required_files": required_files,
        "proposed_metrics": proposed_metrics,
        "program_planner": {
            "planner_version": "metric_dsl_planner_v1",
            "status": "suggested_program_ready",
            "suggested_program": suggested_program,
            "recommended_create_call": "create { ...program_planner.suggested_program... }",
            "agent_rule": "If the user confirms the plan, pass suggested_program fields to create. If the user changes the goal, call run plan_only again with the refined intent."
        },
        "compute_contract": {
            "raw_input_not_returned": true,
            "source_content_included": false,
            "logs_are_cursor_based": true,
            "content_addressed": true,
            "identical_work_reused_by_hash": true,
            "proof_artifacts_expected": true
        },
        "estimated_work": {
            "mode": "planning_only",
            "compute_cost": estimated_compute_cost,
            "cache_probe": cache_probe,
            "token_savings": "large files and logs stay on disk; Forge returns compact ids, hashes, proofs, artifacts and bounded previews"
        },
        "next_actions": [
            "If this plan matches the user intent, call the recommended run/create command.",
            "Use logs { job_id, cursor } while a job runs.",
            "Use read { job_id, kind:\"artifacts\" } when completed."
        ]
    })
}

fn proposed_metrics_for_capability(capability: &str) -> Vec<Value> {
    let specs = match capability {
        "csv_timeseries" => vec![
            metric_spec("csv_profile", "csv_profile", &["csv"], json!({})),
            metric_spec("volume_zscore", "zscore", &["volume"], json!({ "window": 48, "threshold": 3.0 })),
            metric_spec("close_rolling_mean", "rolling_mean", &["close"], json!({ "window": 20 })),
            metric_spec("close_rolling_std", "rolling_std", &["close"], json!({ "window": 20 })),
            metric_spec("price_volume_correlation_delta", "correlation_delta", &["close", "volume"], json!({ "window": 96 })),
        ],
        "kmer_sequence" => vec![
            metric_spec("sequence_bytes", "bytes", &["file"], json!({})),
            metric_spec("gc_content", "gc_content", &["sequence"], json!({})),
            metric_spec("kmer_count", "kmer_count", &["sequence"], json!({ "k": 7 })),
            metric_spec("kmer_collision_rate", "kmer_collision_rate", &["sequence"], json!({ "k": 7 })),
        ],
        "source_code_metrics" => vec![
            metric_spec("source_line_count", "line_count", &["file"], json!({})),
            metric_spec("source_char_count", "char_count", &["file"], json!({})),
            metric_spec("source_byte_entropy", "byte_entropy", &["file"], json!({})),
            metric_spec("source_byte_histogram", "byte_histogram", &["file"], json!({})),
        ],
        "security_crypto" => vec![
            metric_spec("synthetic_avalanche_score", "synthetic_hash_avalanche", &["synthetic"], json!({ "samples": 4096, "bytes": 32, "hash_bits": 64 })),
            metric_spec("synthetic_collision_rate", "synthetic_hash_collision_rate", &["synthetic"], json!({ "samples": 65536, "bytes": 24, "hash_bits": 32 })),
            metric_spec("synthetic_bit_bias", "synthetic_hash_bit_bias", &["synthetic"], json!({ "samples": 32768, "bytes": 32, "hash_bits": 64 })),
            metric_spec("optional_input_entropy", "byte_entropy", &["file"], json!({ "skip_if_no_input": true })),
            metric_spec("optional_input_histogram", "byte_histogram", &["file"], json!({ "skip_if_no_input": true })),
        ],
        "alpha" => Vec::new(),
        _ => Vec::new(),
    };
    specs
        .into_iter()
        .filter_map(|spec| serde_json::to_value(spec).ok())
        .collect()
}

fn suggested_program_for_intent(capability: &str, intent: &str, requested_capability: &str) -> Value {
    let safe_intent = if intent.trim().is_empty() {
        format!("Run a {capability} Forge compute program")
    } else {
        intent.trim().to_string()
    };
    match capability {
        "alpha" => json!({
            "title": "Alpha strategy search plan",
            "domain": "finance",
            "intent": safe_intent,
            "goal": "Build a content-addressed trading strategy search DAG from market data, candidate labels, scoring and validation.",
            "template": "alpha_strategy_search",
            "metrics": [
                metric_node("ohlcv_profile", "input", "finance", "csv_profile", &["market_data"], "ohlcv_profile", "table", json!({})),
                metric_node("trade_label_grid", "simulate", "finance", "trade_label_grid", &["ohlcv_profile"], "labels", "timeseries", json!({ "sl_max_points": 9, "tp_min_points": 2, "daily_target_points": 7 })),
                metric_node("feature_scan", "transform", "finance", "indicator_feature_scan", &["ohlcv_profile"], "features", "table", json!({ "families": ["vwap", "stochastic", "atr", "adx", "rsi", "ema"] })),
                metric_node("strategy_candidates", "optimize", "finance", "strategy_search", &["features", "labels"], "candidate_strategies", "table", json!({ "objective": "risk_adjusted_daily_points" })),
                metric_node("holdout_validation", "validate", "finance", "walk_forward_holdout", &["candidate_strategies"], "validated_strategies", "table", json!({ "proof": "deterministic" })),
                metric_node("strategy_export", "export", "finance", "artifact_export", &["validated_strategies"], "strategy_artifacts", "artifact", json!({}))
            ],
            "constraints": {
                "raw_content_policy": "hash inputs; do not return raw market rows",
                "validation_required": true
            },
            "output_contract": {
                "returns": ["strategy summary", "metrics.json", "proof.json", "artifact refs", "hashes"]
            }
        }),
        "csv_timeseries" => json!({
            "title": "Timeseries anomaly and regime planner",
            "domain": "timeseries",
            "intent": safe_intent,
            "goal": "Profile a tabular/timeseries file, derive anomaly/regime signals, score and select the most relevant events.",
            "template": "csv_timeseries",
            "metrics": [
                metric_node("csv_profile", "input", "timeseries", "csv_profile", &["csv"], "profile", "table", json!({})),
                metric_node("primary_zscore", "transform", "timeseries", "zscore", &["volume"], "primary_zscore", "timeseries", json!({ "window": 48, "threshold": 3.0 })),
                metric_node("rolling_baseline", "aggregate", "timeseries", "rolling_mean", &["close"], "baseline", "timeseries", json!({ "window": 20 })),
                metric_node("rolling_volatility", "aggregate", "timeseries", "rolling_std", &["close"], "volatility", "timeseries", json!({ "window": 20 })),
                metric_node("regime_shift", "compare", "timeseries", "correlation_delta", &["close", "volume"], "regime_shift", "f64", json!({ "window": 96 })),
                metric_node("event_score", "score", "timeseries", "weighted_score", &["primary_zscore", "volatility", "regime_shift"], "event_score", "timeseries", json!({ "weights": [0.45, 0.25, 0.30] })),
                metric_node("top_events", "select", "timeseries", "top_k", &["event_score"], "top_events", "table", json!({ "k": 50 }))
            ],
            "constraints": {
                "raw_content_policy": "hash input file; return compact event refs and artifacts"
            },
            "output_contract": {
                "returns": ["top events", "scores", "proof.json", "metrics.json"]
            }
        }),
        "kmer_sequence" => json!({
            "title": "Sequence k-mer and entropy planner",
            "domain": "biology",
            "intent": safe_intent,
            "goal": "Measure sequence composition, k-mer diversity, collision behavior and compact proof artifacts.",
            "template": "kmer_sequence",
            "metrics": [
                metric_node("sequence_profile", "input", "biology", "bytes", &["sequence"], "sequence_profile", "bytes", json!({})),
                metric_node("gc_content", "transform", "biology", "gc_content", &["sequence"], "gc_ratio", "f64", json!({})),
                metric_node("kmer_distribution", "aggregate", "biology", "kmer_count", &["sequence"], "kmer_distribution", "distribution", json!({ "k": 7 })),
                metric_node("kmer_hash_quality", "compare", "biology", "kmer_collision_rate", &["sequence"], "kmer_hash_quality", "distribution", json!({ "k": 7, "buckets": 65536 })),
                metric_node("sequence_score", "score", "biology", "weighted_score", &["gc_ratio", "kmer_distribution", "kmer_hash_quality"], "sequence_score", "f64", json!({}))
            ],
            "constraints": {
                "raw_content_policy": "hash sequence; return compact distributions and proof"
            },
            "output_contract": {
                "returns": ["sequence metrics", "collision stats", "proof.json"]
            }
        }),
        "source_code_metrics" => json!({
            "title": "Source code metrics planner",
            "domain": "code",
            "intent": safe_intent,
            "goal": "Measure source size, entropy and structural signals without injecting full source into the LLM.",
            "template": "source_code_metrics",
            "metrics": [
                metric_node("source_lines", "input", "code", "line_count", &["source"], "source_lines", "i64", json!({})),
                metric_node("source_chars", "aggregate", "code", "char_count", &["source"], "source_chars", "i64", json!({})),
                metric_node("source_entropy", "transform", "code", "byte_entropy", &["source"], "source_entropy", "f64", json!({})),
                metric_node("byte_shape", "aggregate", "code", "byte_histogram", &["source"], "byte_shape", "distribution", json!({})),
                metric_node("complexity_score", "score", "code", "weighted_score", &["source_lines", "source_entropy", "byte_shape"], "complexity_score", "f64", json!({}))
            ],
            "constraints": {
                "raw_content_policy": "never return full source by default"
            },
            "output_contract": {
                "returns": ["code metrics", "hashes", "proof.json"]
            }
        }),
        "security_crypto" => json!({
            "title": "Hash quality and defensive crypto planner",
            "domain": "security",
            "intent": safe_intent,
            "goal": "Evaluate hash/distribution behavior in authorized file mode or synthetic/no-input lab mode.",
            "template": "security_crypto",
            "metrics": [
                metric_node("avalanche", "simulate", "security", "synthetic_hash_avalanche", &["synthetic"], "avalanche_score", "distribution", json!({ "samples": 4096, "bytes": 32, "hash_bits": 64 })),
                metric_node("collision_rate", "simulate", "security", "synthetic_hash_collision_rate", &["synthetic"], "collision_rate", "distribution", json!({ "samples": 65536, "bytes": 24, "hash_bits": 32 })),
                metric_node("bit_bias", "compare", "security", "synthetic_hash_bit_bias", &["synthetic"], "bit_bias", "distribution", json!({ "samples": 32768, "bytes": 32, "hash_bits": 64 })),
                metric_node("optional_entropy", "transform", "security", "byte_entropy", &["file"], "optional_entropy", "f64", json!({ "skip_if_no_input": true })),
                metric_node("hash_quality_score", "score", "security", "weighted_score", &["avalanche_score", "collision_rate", "bit_bias", "optional_entropy"], "hash_quality_score", "f64", json!({}))
            ],
            "constraints": {
                "authorized_or_synthetic_only": true,
                "raw_content_policy": "do not return secrets, credentials, full ciphertexts or binaries"
            },
            "output_contract": {
                "returns": ["quality metrics", "proof.json", "artifact hashes"]
            }
        }),
        _ => json!({
            "title": "Custom Forge Metric DSL program",
            "domain": if requested_capability.trim().is_empty() { "custom" } else { requested_capability.trim() },
            "intent": safe_intent,
            "goal": "Convert the user's objective into a Metric DSL DAG: inputs, transforms, comparisons, scores, selection, validation and proof.",
            "template": "custom_metric_dsl",
            "metrics": [
                metric_node("source_profile", "input", "custom", "profile_input", &["input"], "source_profile", "custom:source_profile", json!({})),
                metric_node("feature_extract", "transform", "custom", "custom_feature_extract", &["source_profile"], "features", "custom:features", json!({})),
                metric_node("objective_score", "score", "custom", "custom_score", &["features"], "objective_score", "f64", json!({})),
                metric_node("selected_results", "select", "custom", "top_k", &["objective_score"], "selected_results", "table", json!({ "k": 20 })),
                metric_node("proof_bundle", "prove", "custom", "proof_bundle", &["selected_results"], "proof_bundle", "artifact", json!({}))
            ],
            "constraints": {
                "unknown_ops_allowed": true,
                "raw_content_policy": "hash inputs; return compact results and proof refs"
            },
            "output_contract": {
                "returns": ["selected results", "metrics.json", "proof.json"]
            }
        }),
    }
}

fn metric_node(
    id: &str,
    kind: &str,
    domain: &str,
    op: &str,
    inputs: &[&str],
    output: &str,
    dtype: &str,
    params: Value,
) -> Value {
    json!({
        "id": id,
        "tag": id,
        "kind": kind,
        "domain": domain,
        "op": op,
        "inputs": inputs,
        "output": output,
        "dtype": dtype,
        "params": params,
        "cache": "content",
        "proof": "hash"
    })
}

fn metric_math_formula(metric: &Value) -> String {
    for key in ["formula", "equation", "expression", "math"] {
        if let Some(value) = metric.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
            return value.to_string();
        }
    }
    if let Some(params) = metric.get("params") {
        for key in ["formula", "equation", "expression", "math"] {
            if let Some(value) = params.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
                return value.to_string();
            }
        }
    }
    let op = metric_op(metric).unwrap_or_else(|| "custom_metric".to_string());
    let op_lc = op.to_lowercase().replace('-', "_");
    let inputs = metric_inputs(metric);
    let first = inputs.get(0).map(String::as_str).unwrap_or("x");
    let second = inputs.get(1).map(String::as_str).unwrap_or("y");
    let window = metric
        .pointer("/params/window")
        .and_then(Value::as_u64)
        .or_else(|| metric.pointer("/params/period").and_then(Value::as_u64));
    match op_lc.as_str() {
        "vwap" | "volume_weighted_average_price" => {
            "VWAP_t = Î£(typical_price_i Ã— volume_i) / Î£(volume_i)".to_string()
        }
        "ema" | "exponential_moving_average" => {
            let n = window.unwrap_or(0);
            if n > 0 {
                format!("EMA_t = Î±Â·{first}_t + (1-Î±)Â·EMA_(t-1), Î±=2/({n}+1)")
            } else {
                format!("EMA_t = Î±Â·{first}_t + (1-Î±)Â·EMA_(t-1)")
            }
        }
        "sma" | "rolling_mean" | "moving_average" => {
            let n = window.map(|v| v.to_string()).unwrap_or_else(|| "window".to_string());
            format!("mean_{n}({first}) = Î£({first}) / {n}")
        }
        "rolling_std" | "std" | "standard_deviation" | "volatility" => {
            let n = window.map(|v| v.to_string()).unwrap_or_else(|| "window".to_string());
            format!("Ïƒ_{n}({first}) = sqrt(Î£({first}-Î¼)^2 / {n})")
        }
        "zscore" | "z_score" => {
            let n = window.map(|v| v.to_string()).unwrap_or_else(|| "window".to_string());
            format!("z_t = ({first}_t - Î¼_{n}({first})) / Ïƒ_{n}({first})")
        }
        "rsi" | "rsi14" => "RSI = 100 - 100 / (1 + avg_gain / avg_loss)".to_string(),
        "atr" => {
            "ATR = EMA(max(high-low, |high-prevClose|, |low-prevClose|))".to_string()
        }
        "adx" => "ADX = EMA(|+DI - -DI| / (+DI + -DI) Ã— 100)".to_string(),
        "stochastic" | "stoch" => {
            "Stoch = (close - lowestLow_n) / (highestHigh_n - lowestLow_n)".to_string()
        }
        "bollinger" | "bollinger_bands" => {
            let n = window.map(|v| v.to_string()).unwrap_or_else(|| "n".to_string());
            format!("middle=mean_{n}({first}); upper/lower=middle Â± kÂ·Ïƒ_{n}({first})")
        }
        "macd" => "MACD = EMA_fast(close) - EMA_slow(close); signal = EMA(MACD)".to_string(),
        "heikin_ashi" | "haikin_ashi" => {
            "HA_close=(open+high+low+close)/4; HA_open=(prev_HA_open+prev_HA_close)/2".to_string()
        }
        "correlation" | "correlation_delta" => {
            format!("corr({first},{second}) = cov({first},{second}) / (Ïƒ_{first}Â·Ïƒ_{second})")
        }
        "weighted_score" => {
            let list = if inputs.is_empty() { "metrics".to_string() } else { inputs.join(", ") };
            format!("score = Î£(weight_i Ã— normalized(metric_i)) over [{list}]")
        }
        "top_k" => format!("select top-k rows by {first}"),
        _ => {
            let args = if inputs.is_empty() {
                "inputs".to_string()
            } else {
                inputs.join(", ")
            };
            let params = metric
                .get("params")
                .and_then(Value::as_object)
                .filter(|obj| !obj.is_empty())
                .and_then(|obj| serde_json::to_string(obj).ok())
                .unwrap_or_default();
            if params.is_empty() {
                format!("{op}({args})")
            } else {
                format!("{op}({args}; params={params})")
            }
        }
    }
}

fn metric_math_algorithm(metric: &Value) -> String {
    for key in ["algorithm", "method", "procedure"] {
        if let Some(value) = metric.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
            return value.to_string();
        }
    }
    if let Some(params) = metric.get("params") {
        for key in ["algorithm", "method", "procedure"] {
            if let Some(value) = params.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
                return value.to_string();
            }
        }
    }
    let op = metric_op(metric).unwrap_or_else(|| "custom_metric".to_string());
    let inputs = metric_inputs(metric);
    if inputs.is_empty() {
        format!("evaluate metric op '{op}' over each resolved input artifact")
    } else {
        format!("resolve [{}], compute '{op}', persist compact output '{}'", inputs.join(", "), metric_output(metric))
    }
}

fn metric_math_contract(metrics: &[Value]) -> Value {
    let mut consumers: HashMap<String, Vec<String>> = HashMap::new();
    for metric in metrics {
        let tag = metric_tag(metric);
        for input in metric_inputs(metric) {
            consumers.entry(input).or_default().push(tag.clone());
        }
    }
    json!(metrics
        .iter()
        .enumerate()
        .map(|(idx, metric)| {
            let tag = metric_tag(metric);
            let output = metric_output(metric);
            json!({
                "index": idx + 1,
                "tag": tag,
                "name": metric.get("name").cloned().unwrap_or(Value::Null),
                "op": metric_op(metric).unwrap_or_else(|| "custom_metric".to_string()),
                "inputs": metric_inputs(metric),
                "output": output,
                "formula": metric_math_formula(metric),
                "algorithm": metric_math_algorithm(metric),
                "feeds": consumers.get(&output).cloned().unwrap_or_default(),
                "source": if metric.get("formula").is_some()
                    || metric.get("algorithm").is_some()
                    || metric.pointer("/params/formula").is_some()
                    || metric.pointer("/params/algorithm").is_some() {
                    "program_declared"
                } else {
                    "op_signature"
                }
            })
        })
        .collect::<Vec<_>>())
}


fn required_files_for_capability(capability: &str, input_summary: &Value) -> Value {
    let has_path_input = input_summary
        .get("inputs")
        .and_then(Value::as_array)
        .map(|items| items.iter().any(|item| item.get("kind").and_then(Value::as_str) == Some("path")))
        .unwrap_or(false);
    let expected = match capability {
        "alpha" => vec!["OHLCV CSV with time/open/high/low/close/volume columns"],
        "csv_timeseries" => vec!["CSV or tabular timeseries file"],
        "kmer_sequence" => vec!["DNA/RNA/sequence text file or FASTA-like document"],
        "source_code_metrics" => vec!["source file, source tree manifest, or code artifact path"],
        "security_crypto" => vec!["optional file/hash/ciphertext/binary/log to analyze, or no file for synthetic lab experiments"],
        _ => vec!["at least one input path or pending Forge job"],
    };
    json!({
        "expected": expected,
        "provided": has_path_input,
        "missing": if has_path_input { Vec::<String>::new() } else { vec!["input path".to_string()] },
        "raw_content_required_in_llm": false
    })
}

fn input_policy_for_capability(capability: &str, intent: &str, input_summary: &Value) -> Value {
    let has_input = input_summary
        .get("inputs")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    let free_mode_next_call = free_mode_next_call(capability, intent);
    json!({
        "mode": if has_input { "file_provided" } else { "choose_file_or_free" },
        "provided": has_input,
        "requires_user_choice": !has_input,
        "real_data_result_requires_input": true,
        "can_plan_without_input": true,
        "can_create_program_without_input": true,
        "can_run_without_input": capability == "security_crypto",
        "input_mode_options": [
            {
                "mode": "file",
                "prompt": "Ask the user to upload/provide a file, dataset, folder, artifact or pending Forge job if they want results grounded in their own data.",
                "next_call": capability_launch_command(capability, false),
                "raw_content_policy": "Forge hashes and reads the file locally; the LLM receives compact ids, previews, hashes, proofs and artifacts, not raw content."
            },
            {
                "mode": "free",
                "prompt": "If the user has no file yet, let the agent create or run a synthetic/no-input Forge program: generated samples, parameter sweeps, simulations, toy datasets, benchmark vectors or mathematical search spaces.",
                "next_call": free_mode_next_call,
                "raw_content_policy": "No user file is required; Forge stores the synthetic program and artifacts by content hash."
            }
        ],
        "ask_user_prompt": "Do you have a file/dataset/artifact for Forge to analyze, or should Forge start in free mode with a synthetic/no-input program?",
        "agent_rule": "Do not assume a file is mandatory. If no input is provided, ask the user to choose file mode or free mode, then call create/run accordingly."
    })
}

fn free_mode_next_call(capability: &str, intent: &str) -> &'static str {
    match capability {
        "security_crypto" => "run { intent: \"measure hash avalanche, collision rate and bit bias in synthetic lab mode\", capability: \"security_crypto\" }",
        "alpha" => "create { title: \"Synthetic market strategy lab\", domain: \"finance\", intent: \"Explore strategy metrics on generated market scenarios before real data\", goal: \"Generate toy OHLCV regimes and score candidate strategy metrics\", metrics: [...] }",
        "csv_timeseries" => "create { title: \"Synthetic timeseries anomaly lab\", domain: \"timeseries\", intent: \"Explore anomaly metrics on generated time series before real data\", goal: \"Generate synthetic signals and score anomaly detectors\", metrics: [...] }",
        "kmer_sequence" => "create { title: \"Synthetic sequence hash lab\", domain: \"biology\", intent: \"Explore sequence/k-mer metrics on generated sequences before real data\", goal: \"Generate synthetic sequences and score k-mer/hash behavior\", metrics: [...] }",
        "source_code_metrics" => "create { title: \"Synthetic code metrics lab\", domain: \"code\", intent: \"Explore source metrics on generated code-like artifacts before a real codebase\", goal: \"Generate toy source structures and score metrics\", metrics: [...] }",
        _ if !intent.is_empty() => "create { title: \"Free-mode compute program\", domain: \"custom\", intent: \"describe the requested free-mode experiment\", goal: \"define synthetic inputs, metrics and outputs\", metrics: [...] }",
        _ => "create { title: \"Free-mode compute program\", domain: \"custom\", intent: \"describe the requested free-mode experiment\", goal: \"define synthetic inputs, metrics and outputs\", metrics: [...] }",
    }
}

fn plan_cache_probe(args: &Value, proposed_metrics: &[Value], input_summary: &Value) -> Value {
    let input_bytes = total_input_bytes(input_summary);
    let can_probe = !proposed_metrics.is_empty() && input_bytes > 0;
    let jobs_dir = jobs_dir().ok();
    let cache_dir = jobs_dir.as_ref().map(|dir| metric_cache_dir(dir));
    json!({
        "mode": if can_probe { "runtime_metric_hash_probe_available" } else { "hash_probe_deferred_until_run" },
        "cache_dir": cache_dir.map(|dir| dir.display().to_string()),
        "input_bytes_known": input_bytes,
        "metric_count": proposed_metrics.len(),
        "program_selector_present": args.get("program_hash").or_else(|| args.get("program")).or_else(|| args.get("program_title")).or_else(|| args.get("program_query")).is_some(),
        "policy": "run hashes program + normalized metric tags + input content hashes; matching metric artifacts are reused and not recomputed"
    })
}

fn estimate_compute_cost(capability: &str, metric_count: usize, input_summary: &Value) -> Value {
    let input_bytes = total_input_bytes(input_summary);
    let class = match capability {
        "alpha" => "heavy_cpu_gpu_search",
        "csv_timeseries" => "medium_tabular_scan",
        "kmer_sequence" => "medium_sequence_scan",
        "source_code_metrics" => "light_to_medium_text_scan",
        "security_crypto" => if input_bytes > 0 { "medium_defensive_crypto_file_scan" } else { "medium_synthetic_crypto_lab" },
        _ => "unknown_until_program_created",
    };
    json!({
        "class": class,
        "metric_count": metric_count,
        "input_bytes_known": input_bytes,
        "expected_dispatch": if capability == "alpha" { "Forge strategy engine can use CPU/GPU routing; toolbox metrics use the unified metric executor." } else { "unified metric executor with content-addressed cache reuse" },
        "llm_context_cost": "bounded: only this plan, ids, hashes, logs cursors and artifact refs are returned"
    })
}

fn total_input_bytes(input_summary: &Value) -> u64 {
    input_summary
        .get("inputs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("bytes").and_then(Value::as_u64))
                .sum()
        })
        .unwrap_or(0)
}

fn capability_launch_command(capability: &str, execute: bool) -> &'static str {
    match (capability, execute) {
        ("universal_compute_program", _) => "create { title: \"Universal compute program\", program_kind: \"compute_program\", domain: \"custom\", intent: \"...\", goal: \"describe what to measure\", metrics: [...] }",
        ("universal_visual_program_2d_3d", _) => "create { title: \"Universal visual program\", program_kind: \"visual_program\", domain: \"custom\", goal: \"surface useful structure in this file\", views: [{ type: \"3d\", axes: { x: \"...\", y: \"...\", z: \"...\" }, color: \"...\" }] }",
        ("alpha", true) => "run { intent: \"find a robust strategy from this OHLCV CSV\", inputs: [{ path: \"...\", role: \"market_data\" }] }",
        ("csv_timeseries", true) => "run { intent: \"find volume anomalies and timeseries regimes\", inputs: [{ path: \"...\", role: \"csv\" }] }",
        ("kmer_sequence", true) => "run { intent: \"measure DNA k-mer hashing and sequence entropy\", inputs: [{ path: \"...\", role: \"sequence\" }] }",
        ("source_code_metrics", true) => "run { intent: \"profile source code metrics and entropy\", inputs: [{ path: \"...\", role: \"source\" }] }",
        ("security_crypto", true) => "run { intent: \"measure hash avalanche, collision rate and bit bias in synthetic lab mode\", capability: \"security_crypto\" }",
        ("alpha", false) => "run { intent: \"find a robust strategy from this OHLCV CSV\", inputs: [{ path: \"...\", role: \"market_data\" }], plan_only: true }",
        ("csv_timeseries", false) => "run { intent: \"find volume anomalies and timeseries regimes\", inputs: [{ path: \"...\", role: \"csv\" }], plan_only: true }",
        ("kmer_sequence", false) => "run { intent: \"measure DNA k-mer hashing and sequence entropy\", inputs: [{ path: \"...\", role: \"sequence\" }], plan_only: true }",
        ("source_code_metrics", false) => "run { intent: \"profile source code metrics and entropy\", inputs: [{ path: \"...\", role: \"source\" }], plan_only: true }",
        ("security_crypto", false) => "run { intent: \"measure hash avalanche, collision rate and bit bias\", capability: \"security_crypto\", plan_only: true }",
        _ => "create { title: \"Custom compute program\", domain: \"...\", intent: \"...\", goal: \"describe what to measure\", metrics: [...] }",
    }
}

fn selected_template_profile(capability: &str, intent: &str, requested_capability: &str) -> Value {
    let query = if !intent.trim().is_empty() {
        intent
    } else if !requested_capability.trim().is_empty() {
        requested_capability
    } else {
        capability
    };
    let templates = matching_capability_templates(Some(query), None, Some(capability), true);
    templates
        .first()
        .cloned()
        .unwrap_or_else(|| json!({
            "template": "custom_metric_dsl",
            "capability": capability,
            "domain": if requested_capability.trim().is_empty() { "custom" } else { requested_capability.trim() },
            "status": "scaffold",
            "execution_mode": "create_then_run",
            "reason": "No exact builtin template matched; Forge will generate a reusable Metric DSL program from the user's intent."
        }))
}

fn run_intent(args: RunIntentArgs, client: &McpClientInfo) -> Result<Value, String> {
    let inferred = infer_capability(args.intent.as_deref(), args.capability.as_deref());
    match inferred.as_str() {
        "alpha" => {
            let Some(input_path) = first_input_path(&args.inputs) else {
                return Ok(json!({
                    "status": "needs_user_choice",
                    "reason": "No OHLCV input was provided. Forge can either analyze a user market file or create a free-mode synthetic market lab program.",
                    "plan": plan_run(json!({
                        "intent": args.intent,
                        "capability": args.capability,
                        "inputs": args.inputs,
                        "params": args.params,
                        "plan_only": true
                    }))
                }));
            };
            let alpha_args = AlphaStrategyArgs {
                csv_path: input_path,
                title: args.title.or_else(|| Some("Forge intent alpha strategy".to_string())),
                sl_points: None,
                tp_points: None,
                spread_points: None,
                target_pnl_per_day: None,
                sl_display_points: None,
                tp_display_points: None,
                spread_display_points: None,
                target_display_points_per_day: None,
                point_size: None,
                max_horizon_bars: None,
                train_split: None,
                top_rules_per_side: None,
                engine: None,
                max_nodes: None,
                generations: None,
                beam_width: None,
                feature_limit: None,
                store_dir: None,
            };
            let job = run_alpha_strategy(alpha_args, client)?;
            serde_json::to_value(job).map_err(|e| format!("encode strategy result: {e}"))
        }
        "csv_timeseries" | "kmer_sequence" | "source_code_metrics" => {
            if first_input_path(&args.inputs).is_none() {
                return Ok(json!({
                    "status": "needs_user_choice",
                    "reason": "No input was provided. Forge can either analyze a user file or create a free-mode synthetic/no-input program.",
                    "plan": plan_run(json!({
                        "intent": args.intent,
                        "capability": args.capability,
                        "inputs": args.inputs,
                        "params": args.params,
                        "plan_only": true
                    }))
                }));
            }
            let program = define_intent_program(&args, &inferred, client)?;
            let program_hash = program
                .pointer("/program/program_hash")
                .and_then(Value::as_str)
                .ok_or_else(|| "intent program creation did not return program_hash".to_string())?
                .to_string();
            let exec_args = ProgramExecuteArgs {
                program_hash: Some(program_hash),
                program_id: None,
                program: None,
                program_title: None,
                program_query: None,
                title: args.title,
                inputs: args.inputs,
                params: args.params,
                dry_run: None,
                intent: args.intent,
                capability: args.capability,
                parent_session_id: active_job_id_from_env(),
            };
            execute_program(exec_args, client)
        }
        "security_crypto" => {
            let program = define_intent_program(&args, &inferred, client)?;
            let program_hash = program
                .pointer("/program/program_hash")
                .and_then(Value::as_str)
                .ok_or_else(|| "intent program creation did not return program_hash".to_string())?
                .to_string();
            let exec_args = ProgramExecuteArgs {
                program_hash: Some(program_hash),
                program_id: None,
                program: None,
                program_title: None,
                program_query: None,
                title: args.title,
                inputs: args.inputs,
                params: args.params,
                dry_run: None,
                intent: args.intent,
                capability: args.capability,
                parent_session_id: active_job_id_from_env(),
            };
            execute_program(exec_args, client)
        }
        _ => Ok(json!({
            "status": "planned",
            "reason": "This natural-language intent needs a custom program definition before execution.",
            "recommended_tool": "create",
            "recommended_next_call": "create { title: \"Custom compute program\", goal: \"describe what to measure\", metrics: [...] }",
            "plan": plan_run(json!({
                "intent": args.intent,
                "capability": args.capability,
                "inputs": args.inputs,
                "params": args.params,
                "plan_only": true
            }))
        })),
    }
}

fn define_intent_program(args: &RunIntentArgs, inferred: &str, client: &McpClientInfo) -> Result<Value, String> {
    let title = args
        .title
        .clone()
        .unwrap_or_else(|| format!("Forge {inferred} intent program"));
    let goal = args
        .intent
        .clone()
        .unwrap_or_else(|| format!("Execute {inferred} metrics with Forge"));
    let metrics = match inferred {
        "csv_timeseries" => vec![
            metric_spec("csv_profile", "csv_profile", &["csv"], json!({})),
            metric_spec("volume_zscore", "zscore", &["volume"], json!({ "window": 48, "threshold": 3.0 })),
            metric_spec("close_rolling_mean", "rolling_mean", &["close"], json!({ "window": 20 })),
            metric_spec("close_rolling_std", "rolling_std", &["close"], json!({ "window": 20 })),
            metric_spec("price_volume_correlation_delta", "correlation_delta", &["close", "volume"], json!({ "window": 96 })),
        ],
        "kmer_sequence" => vec![
            metric_spec("sequence_bytes", "bytes", &["file"], json!({})),
            metric_spec("gc_content", "gc_content", &["sequence"], json!({})),
            metric_spec("kmer_count", "kmer_count", &["sequence"], json!({ "k": 7 })),
            metric_spec("kmer_collision_rate", "kmer_collision_rate", &["sequence"], json!({ "k": 7 })),
        ],
        "source_code_metrics" => vec![
            metric_spec("source_line_count", "line_count", &["file"], json!({})),
            metric_spec("source_char_count", "char_count", &["file"], json!({})),
            metric_spec("source_byte_entropy", "byte_entropy", &["file"], json!({})),
            metric_spec("source_byte_histogram", "byte_histogram", &["file"], json!({})),
        ],
        "security_crypto" => vec![
            metric_spec("synthetic_avalanche_score", "synthetic_hash_avalanche", &["synthetic"], json!({ "samples": 4096, "bytes": 32, "hash_bits": 64 })),
            metric_spec("synthetic_collision_rate", "synthetic_hash_collision_rate", &["synthetic"], json!({ "samples": 65536, "bytes": 24, "hash_bits": 32 })),
            metric_spec("synthetic_bit_bias", "synthetic_hash_bit_bias", &["synthetic"], json!({ "samples": 32768, "bytes": 32, "hash_bits": 64 })),
            metric_spec("optional_input_entropy", "byte_entropy", &["file"], json!({ "skip_if_no_input": true })),
            metric_spec("optional_input_histogram", "byte_histogram", &["file"], json!({ "skip_if_no_input": true })),
        ],
        _ => Vec::new(),
    };
    if metrics.is_empty() {
        return Err(format!("no builtin intent program for capability {inferred}"));
    }
    define_program(
        ProgramDefineArgs {
            title,
            domain: Some(inferred.to_string()),
            intent: args.intent.clone(),
            goal,
            kind: None,
            program_kind: None,
            template: Some(inferred.to_string()),
            metrics,
            views: Vec::new(),
            spec_text: None,
            source_schema: Value::Null,
            constraints: json!({
                "intent_defined": true,
                "source_content_policy": "hash inputs; do not return raw content"
            }),
            output_contract: json!({
                "returns": ["metrics.json", "proof.json", "job manifest", "hashes", "bounded logs"]
            }),
        },
        client,
    )
}

fn metric_spec(name: &str, op: &str, inputs: &[&str], params: Value) -> ProgramMetricSpec {
    ProgramMetricSpec {
        id: Some(name.to_string()),
        tag: name.to_string(),
        name: Some(name.to_string()),
        kind: Some("transform".to_string()),
        domain: None,
        op: Some(op.to_string()),
        inputs: inputs.iter().map(|v| v.to_string()).collect(),
        output: Some(name.to_string()),
        dtype: None,
        params,
        constraints: Value::Null,
        unit: None,
        cache: Some("content".to_string()),
        proof: Some("hash".to_string()),
        condition: None,
        goal: None,
        description: None,
        formula: None,
        algorithm: None,
        weight: None,
    }
}

fn infer_capability(intent: Option<&str>, capability: Option<&str>) -> String {
    let text = format!(
        "{} {}",
        capability.unwrap_or_default(),
        intent.unwrap_or_default()
    )
    .to_lowercase();
    if let Some(explicit) = capability.map(str::trim).filter(|v| !v.is_empty()) {
        if let Some(template) = matching_capability_templates(None, None, Some(explicit), true).first() {
            if let Some(capability) = template.get("capability").and_then(Value::as_str) {
                return capability.to_string();
            }
        }
    }
    if let Some(template) = matching_capability_templates(Some(&text), None, None, true).first() {
        if let Some(capability) = template.get("capability").and_then(Value::as_str) {
            if capability != "custom_program" || text.contains("custom") {
                return capability.to_string();
            }
        }
    }
    if text.contains("alpha")
        || text.contains("strategy")
        || text.contains("backtest")
        || text.contains("trading")
        || text.contains("ohlc")
        || text.contains("market")
    {
        "alpha".to_string()
    } else if text.contains("dna")
        || text.contains("rna")
        || text.contains("genome")
        || text.contains("sequence")
        || text.contains("kmer")
        || text.contains("k-mer")
    {
        "kmer_sequence".to_string()
    } else if text.contains("code")
        || text.contains("source")
        || text.contains("ast")
        || text.contains("complexity")
        || text.contains("software")
    {
        "source_code_metrics".to_string()
    } else if text.contains("crypto")
        || text.contains("cryptograph")
        || text.contains("hash")
        || text.contains("cipher")
        || text.contains("collision")
        || text.contains("avalanche")
        || text.contains("bit bias")
        || text.contains("rng")
        || text.contains("prng")
        || text.contains("security")
    {
        "security_crypto".to_string()
    } else if text.contains("csv")
        || text.contains("timeseries")
        || text.contains("time series")
        || text.contains("volume")
        || text.contains("anomal")
        || text.contains("sensor")
        || text.contains("telemetry")
        || text.contains("finance")
        || text.contains("document")
        || text.contains("energy")
    {
        "csv_timeseries".to_string()
    } else if text.contains("simulation")
        || text.contains("physics")
        || text.contains("chem")
        || text.contains("molecule")
        || text.contains("medical")
        || text.contains("engineering")
        || text.contains("aerospace")
    {
        "custom_program".to_string()
    } else {
        capability
            .filter(|v| !v.trim().is_empty())
            .map(|v| v.trim().to_lowercase())
            .unwrap_or_else(|| "custom_program".to_string())
    }
}

fn first_input_path(inputs: &[ProgramInputRef]) -> Option<String> {
    inputs
        .iter()
        .find_map(|input| input.path.as_deref())
        .map(|v| v.to_string())
}

fn summarize_run_inputs(args: &Value) -> Value {
    let mut refs = Vec::new();
    if let Some(csv_path) = args.get("csv_path").and_then(Value::as_str) {
        refs.push(summarize_input_ref("csv_path", csv_path));
    }
    if let Some(inputs) = args.get("inputs").and_then(Value::as_array) {
        for input in inputs.iter().take(8) {
            if let Some(path) = input.get("path").and_then(Value::as_str) {
                let role = input.get("role").and_then(Value::as_str).unwrap_or("input");
                refs.push(summarize_input_ref(role, path));
            } else if let Some(job_id) = input.get("job_id").and_then(Value::as_str) {
                refs.push(json!({
                    "role": input.get("role").and_then(Value::as_str).unwrap_or("job"),
                    "kind": "job",
                    "job_id": job_id,
                    "content_included": false
                }));
            }
        }
    }
    json!({
        "inputs": refs,
        "truncated": args.get("inputs").and_then(Value::as_array).map(|v| v.len() > 8).unwrap_or(false)
    })
}

fn summarize_input_ref(role: &str, path_text: &str) -> Value {
    match resolve_path(path_text).and_then(|path| {
        fs::metadata(&path)
            .map(|meta| (path, meta.len()))
            .map_err(|e| format!("metadata: {e}"))
    }) {
        Ok((path, bytes)) => json!({
            "role": role,
            "kind": "path",
            "path": path.display().to_string(),
            "bytes": bytes,
            "content_included": false,
            "token_savings_estimate": estimate_tokens_from_bytes(bytes as usize)
        }),
        Err(err) => json!({
            "role": role,
            "kind": "path",
            "path": path_text,
            "content_included": false,
            "metadata_error": err
        }),
    }
}

fn estimate_tokens_from_bytes(bytes: usize) -> Value {
    json!({
        "mode": "rough_bytes_to_tokens_range",
        "low": bytes / 4,
        "typical": bytes / 3,
        "high": bytes / 2,
        "note": "Exact count depends on model tokenizer and file structure; numeric CSV/log data can tokenize worse than prose."
    })
}

fn list_metric_ops(args: &Value) -> Value {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let domain = args
        .get("domain")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let capability = args
        .get("capability")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let detailed = args
        .get("detailed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || args
            .get("detail")
            .and_then(Value::as_str)
            .map(|v| v.eq_ignore_ascii_case("detailed") || v.eq_ignore_ascii_case("full"))
            .unwrap_or(false);
    let ops = metric_operator_catalog();
    let filtered = if let Some(q) = &query {
        ops.into_iter()
            .filter(|op| serde_json::to_string(op).unwrap_or_default().to_lowercase().contains(q))
            .collect::<Vec<_>>()
    } else if let Some(domain) = &domain {
        ops.into_iter()
            .filter(|op| serde_json::to_string(op).unwrap_or_default().to_lowercase().contains(domain))
            .collect::<Vec<_>>()
    } else if let Some(capability) = &capability {
        let inferred = infer_capability(Some(capability), Some(capability));
        let needle = match inferred.as_str() {
            "csv_timeseries" => "tabular".to_string(),
            "kmer_sequence" => "biology".to_string(),
            "source_code_metrics" => "code".to_string(),
            "alpha" => "finance".to_string(),
            "security_crypto" => "security".to_string(),
            _ => capability.to_string(),
        };
        ops.into_iter()
            .filter(|op| serde_json::to_string(op).unwrap_or_default().to_lowercase().contains(&needle))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let query_text = query
        .as_deref()
        .or(domain.as_deref())
        .or(capability.as_deref())
        .unwrap_or("");
    let inferred = infer_capability(Some(query_text), None);
    let requested_capability = capability.as_deref().unwrap_or("");
    let planner_intent = if query_text.trim().is_empty() {
        "Choose or create a Forge compute program"
    } else {
        query_text
    };
    let suggested_program = suggested_program_for_intent(&inferred, planner_intent, requested_capability);
    let template_matches = matching_capability_templates(query.as_deref(), domain.as_deref(), capability.as_deref(), detailed);
    let recommended_tool = if inferred == "custom_program" { "create" } else { "run" };
    let recommended_next_call = capability_launch_command(&inferred, false);
    json!({
        "mode": if detailed || query.is_some() || domain.is_some() || capability.is_some() { "guided_detail" } else { "compact_gps" },
        "domains": forge_domain_cards(),
        "template_registry": {
            "registry_version": "forge_template_registry_v1",
            "visible_tool_count": 8,
            "total_templates": forge_capability_templates().len(),
            "templates_returned": template_matches.len(),
            "templates": template_matches,
            "policy": "Templates are internal capabilities, not visible MCP tools. Agents route through capabilities -> run plan_only -> create/run."
        },
        "metric_dsl": metric_dsl_contract(),
        "operators": if detailed || query.is_some() || domain.is_some() || capability.is_some() { filtered } else { Vec::<Value>::new() },
        "recommended_tool": recommended_tool,
        "recommended_next_call": recommended_next_call,
        "inferred_capability": inferred,
        "program_planner": {
            "planner_version": "metric_dsl_planner_v1",
            "status": "suggested_program_ready",
            "suggested_program": suggested_program,
            "recommended_create_call": "create { ...program_planner.suggested_program... }",
            "agent_rule": "Use this suggested_program when the user wants a reusable/custom compute program. Otherwise call run with the inferred capability or run plan_only for a narrower plan."
        },
        "use_when": "Use Forge when input is large, repetitive, expensive, scientific, numerical, document-heavy, or needs content-addressed proof.",
        "do_not": "Do not read raw CSV/source/log files directly when Forge can calculate, summarize, preview or return artifacts.",
        "examples": forge_official_examples(),
        "extension_policy": {
            "unknown_ops_allowed": true,
            "unknown_ops_behavior": "define stores them and execute records them as custom_unresolved until an executor plugin/template is added",
            "universality_rule": "domains, params and metric tags are open-ended; hashes make even custom metrics stable and reusable"
        },
        "next_actions": default_next_actions()
    })
}

fn metric_operator_catalog() -> Vec<Value> {
    vec![
        metric_op_info("bytes", "universal/documents/code/science", "File byte length and content hash metadata.", &["file"], &[]),
        metric_op_info("line_count", "documents/code/csv", "Count text lines without returning content.", &["file"], &[]),
        metric_op_info("char_count", "documents/code", "Count UTF-8 characters when input is text.", &["file"], &[]),
        metric_op_info("byte_entropy", "universal/security/biology/code", "Shannon entropy over raw bytes.", &["file"], &[]),
        metric_op_info("byte_histogram", "universal/security/code", "Compact 256-bin byte histogram with hash-addressed artifact output.", &["file"], &[]),
        metric_op_info("csv_profile", "documents/finance/timeseries/science/engineering/energy", "Infer CSV headers, row count and numeric column stats.", &["csv"], &[]),
        metric_op_info("zscore", "finance/timeseries/engineering/medicine/energy", "Compute z-score statistics for a numeric column.", &["column"], &["window", "threshold"]),
        metric_op_info("rolling_mean", "finance/timeseries/simulation/engineering", "Compute last/min/max rolling mean for a numeric column.", &["column"], &["window"]),
        metric_op_info("rolling_std", "finance/timeseries/simulation/engineering", "Compute last/min/max rolling standard deviation for a numeric column.", &["column"], &["window"]),
        metric_op_info("correlation", "math/finance/timeseries/science/medicine", "Pearson correlation between two numeric columns.", &["column_a", "column_b"], &[]),
        metric_op_info("correlation_delta", "finance/timeseries/energy/aerospace", "Difference between full-window and tail-window Pearson correlation.", &["column_a", "column_b"], &["window"]),
        metric_op_info("gc_content", "biology/medicine", "GC content ratio for DNA/RNA-like sequence text.", &["sequence"], &[]),
        metric_op_info("kmer_count", "biology/medicine/hash", "Unique and total k-mer counts for sequence text.", &["sequence"], &["k"]),
        metric_op_info("kmer_collision_rate", "biology/hash/security", "Collision rate after hashing k-mers with Forge's fast content hash.", &["sequence"], &["k", "buckets"]),
        metric_op_info("entropy", "universal/biology/documents/security/timeseries", "Shannon entropy over text symbols or a named sequence column.", &["text_or_sequence"], &["window"]),
        metric_op_info("synthetic_hash_avalanche", "security/crypto/hash/simulation", "Flip one bit in deterministic synthetic samples and measure changed output bits.", &["synthetic"], &["samples", "bytes", "hash_bits"]),
        metric_op_info("synthetic_hash_collision_rate", "security/crypto/hash/simulation", "Estimate collision rate for a toy/fingerprint hash over deterministic synthetic samples.", &["synthetic"], &["samples", "bytes", "hash_bits"]),
        metric_op_info("synthetic_hash_bit_bias", "security/crypto/hash/simulation", "Measure per-bit output bias over deterministic synthetic hash samples.", &["synthetic"], &["samples", "bytes", "hash_bits"]),
    ]
}

fn forge_capability_templates() -> Vec<Value> {
    vec![
        capability_template(
            "universal_compute_program",
            "universal_compute_program",
            "any",
            "Create reusable compute_program specs for any domain with open Metric DSL tags, local execution, hashes, proofs and compact results. Finance and DNA are examples, not limits.",
            &["universal", "compute program", "custom domain", "metric dsl", "simulation", "optimizer", "detector", "classifier"],
            "metric_visual_dsl_scaffold",
            "create_then_run",
            &["any_session_file_or_objective"],
            &["metrics.json", "proof.json", "hashes", "bounded logs"],
        ),
        capability_template(
            "universal_visual_program_2d_3d",
            "universal_visual_program_2d_3d",
            "any",
            "Create visual_program specs that turn session files into programmable 2D/3D views with axes XYZ, overlays, color, size, labels, transforms and compact artifacts.",
            &["visual program", "2d", "3d", "xyz", "mapping", "visual mapping", "point cloud", "metric recipe"],
            "metric_visual_dsl_scaffold",
            "create_then_run",
            &["session_file", "metric_recipe"],
            &["views_2d", "views_3d", "visual_mapping", "artifacts_2d", "artifacts_3d", "hashes"],
        ),
        capability_template(
            "alpha_strategy_search",
            "alpha",
            "finance",
            "Search and validate market strategy candidates from OHLCV data without injecting rows into the LLM.",
            &["alpha", "strategy", "backtest", "trading", "ohlcv", "market", "indicator"],
            "builtin_alpha_engine",
            "runnable",
            &["market_data"],
            &["strategy summary", "holdout metrics", "proof.json", "artifact refs"],
        ),
        capability_template(
            "timeseries_anomaly_regime",
            "csv_timeseries",
            "timeseries",
            "Profile tabular time series, detect anomalies/regime shifts and return compact event artifacts.",
            &["csv", "timeseries", "time series", "volume", "anomaly", "sensor", "telemetry", "energy"],
            "unified_metric_executor",
            "runnable",
            &["csv"],
            &["top events", "scores", "metrics.json", "proof.json"],
        ),
        capability_template(
            "large_csv_profile",
            "csv_timeseries",
            "documents",
            "Profile large CSV/Excel-like tables with bounded previews and hash-addressed summaries.",
            &["large csv", "document", "excel", "table", "profile", "columns", "dataset"],
            "unified_metric_executor",
            "runnable",
            &["csv"],
            &["row/column stats", "numeric summaries", "proof.json"],
        ),
        capability_template(
            "sequence_kmer_hash",
            "kmer_sequence",
            "biology",
            "Measure sequence composition, k-mer counts, entropy and hash collision behavior.",
            &["dna", "rna", "genome", "sequence", "kmer", "k-mer", "motif", "protein"],
            "unified_metric_executor",
            "runnable",
            &["sequence"],
            &["sequence metrics", "k-mer distribution", "collision stats", "proof.json"],
        ),
        capability_template(
            "source_code_metrics",
            "source_code_metrics",
            "code",
            "Measure source size, entropy and structural proxies without reading full source into context.",
            &["code", "source", "software", "ast", "complexity", "repository", "logs"],
            "unified_metric_executor",
            "runnable",
            &["source"],
            &["code metrics", "hashes", "proof.json"],
        ),
        capability_template(
            "hash_quality_lab",
            "security_crypto",
            "security",
            "Run defensive hash/distribution experiments on authorized inputs or synthetic no-input samples.",
            &["crypto", "cryptography", "hash", "cipher", "collision", "avalanche", "bit bias", "security"],
            "unified_metric_executor",
            "runnable_or_synthetic",
            &["optional_authorized_file"],
            &["avalanche", "collision rate", "bit bias", "proof.json"],
        ),
        capability_template(
            "document_comparison",
            "custom_program",
            "documents",
            "Create a reusable program for comparing large documents, logs, reports or datasets by metrics.",
            &["compare", "diff", "audit", "report", "pdf", "logs", "documents", "quality"],
            "metric_dsl_scaffold",
            "create_then_run",
            &["documents"],
            &["compact deltas", "scores", "artifact refs", "proof.json"],
        ),
        capability_template(
            "chemistry_molecular_metrics",
            "custom_program",
            "chemistry",
            "Create a molecule/reaction/screening metric program from domain-specific tags.",
            &["chemistry", "molecule", "reaction", "smiles", "compound", "screening", "spectra"],
            "metric_dsl_scaffold",
            "create_then_run",
            &["molecule_or_reaction_data"],
            &["molecular scores", "screening table", "proof.json"],
        ),
        capability_template(
            "medical_signal_anomaly",
            "csv_timeseries",
            "medicine",
            "Analyze medical/physiology signals or cohort tables as bounded time-series metrics.",
            &["medical", "medicine", "biomarker", "physiology", "cohort", "signal", "patient"],
            "unified_metric_executor",
            "runnable",
            &["medical_timeseries_or_table"],
            &["signal anomalies", "stats", "proof.json"],
        ),
        capability_template(
            "engineering_sensor_anomaly",
            "csv_timeseries",
            "engineering",
            "Analyze industrial sensors, tolerances and process anomalies from large telemetry tables.",
            &["engineering", "sensor", "industrial", "maintenance", "process", "tolerance"],
            "unified_metric_executor",
            "runnable",
            &["sensor_timeseries"],
            &["anomalies", "regime shifts", "proof.json"],
        ),
        capability_template(
            "aerospace_telemetry",
            "csv_timeseries",
            "aerospace",
            "Analyze spacecraft/flight telemetry streams with compact anomaly and regime artifacts.",
            &["aerospace", "space", "trajectory", "telemetry", "flight", "mission"],
            "unified_metric_executor",
            "runnable",
            &["telemetry"],
            &["telemetry events", "risk signals", "proof.json"],
        ),
        capability_template(
            "simulation_parameter_sweep",
            "custom_program",
            "simulation",
            "Create a parameter-sweep or Monte Carlo compute program with metrics, selection and proof.",
            &["simulation", "montecarlo", "monte carlo", "sweep", "parameter", "physics", "cfd"],
            "metric_dsl_scaffold",
            "create_then_run",
            &["model_or_params"],
            &["sweep results", "scores", "proof.json"],
        ),
        capability_template(
            "math_optimization",
            "custom_program",
            "math",
            "Create an optimization/statistics/matrix/graph metric program from a natural-language objective.",
            &["math", "optimization", "statistics", "matrix", "graph", "symbolic", "numeric"],
            "metric_dsl_scaffold",
            "create_then_run",
            &["data_or_objective"],
            &["objective scores", "selected candidates", "proof.json"],
        ),
        capability_template(
            "energy_grid_timeseries",
            "csv_timeseries",
            "energy",
            "Analyze energy markets, grid telemetry or production series using timeseries metrics.",
            &["energy", "grid", "production", "power", "gas", "oil", "forecast"],
            "unified_metric_executor",
            "runnable",
            &["energy_timeseries"],
            &["events", "volatility/regime stats", "proof.json"],
        ),
    ]
}

fn capability_template(
    template: &str,
    capability: &str,
    domain: &str,
    description: &str,
    aliases: &[&str],
    execution_mode: &str,
    status: &str,
    inputs: &[&str],
    outputs: &[&str],
) -> Value {
    json!({
        "template": template,
        "capability": capability,
        "domain": domain,
        "description": description,
        "aliases": aliases,
        "execution_mode": execution_mode,
        "status": status,
        "inputs": inputs,
        "outputs": outputs,
        "next_call": capability_launch_command(capability, false)
    })
}

fn matching_capability_templates(
    query: Option<&str>,
    domain: Option<&str>,
    capability: Option<&str>,
    detailed: bool,
) -> Vec<Value> {
    let q = query.map(|v| v.trim().to_lowercase()).filter(|v| !v.is_empty());
    let d = domain.map(|v| v.trim().to_lowercase()).filter(|v| !v.is_empty());
    let c = capability.map(|v| v.trim().to_lowercase()).filter(|v| !v.is_empty());
    let mut scored = forge_capability_templates()
        .into_iter()
        .filter_map(|template| {
            let text = serde_json::to_string(&template).unwrap_or_default().to_lowercase();
            let mut score = 0usize;
            if let Some(domain) = &d {
                if template.get("domain").and_then(Value::as_str).map(|v| v.eq_ignore_ascii_case(domain)).unwrap_or(false) {
                    score += 100;
                } else if text.contains(domain) {
                    score += 25;
                } else {
                    return None;
                }
            }
            if let Some(capability) = &c {
                if template.get("capability").and_then(Value::as_str).map(|v| v.eq_ignore_ascii_case(capability)).unwrap_or(false) {
                    score += 100;
                } else if text.contains(capability) {
                    score += 25;
                }
            }
            if let Some(query) = &q {
                if text.contains(query) {
                    score += 80;
                } else {
                    for part in query.split_whitespace().filter(|part| part.len() > 2) {
                        if text.contains(part) {
                            score += 10;
                        }
                    }
                }
            }
            if q.is_none() && d.is_none() && c.is_none() {
                score += 1;
            }
            (score > 0).then_some((score, template))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let limit = if detailed || q.is_some() || d.is_some() || c.is_some() { 8 } else { 5 };
    scored.into_iter().take(limit).map(|(_, template)| template).collect()
}

fn forge_domain_cards() -> Value {
    json!([
        { "domain": "finance", "capabilities": ["csv_timeseries", "alpha", "backtest", "risk"], "when_to_use": "OHLCV CSVs, indicators, anomalies, market regimes, strategy search, risk metrics." },
        { "domain": "code", "capabilities": ["source_code_metrics", "ast_query", "bench", "duplication"], "when_to_use": "large codebases, source metrics, benchmark plans, dependency or pattern analysis." },
        { "domain": "documents", "capabilities": ["large_csv_analysis", "document_metrics", "comparison"], "when_to_use": "large CSV/Excel/PDF/log documents that should not enter the LLM context." },
        { "domain": "biology", "capabilities": ["kmer_sequence", "sequence_entropy", "motif_scan"], "when_to_use": "DNA/RNA/protein-like sequences, k-mer hashes, motifs, sequence statistics." },
        { "domain": "chemistry", "capabilities": ["molecular_metrics", "reaction_balance", "screening"], "when_to_use": "molecules, reactions, spectra, compound scoring and custom metric programs." },
        { "domain": "medicine", "capabilities": ["timeseries", "biomarker_metrics", "signal_anomaly"], "when_to_use": "medical signals, biomarkers, cohort tables, anomaly detection and compact proof outputs." },
        { "domain": "math", "capabilities": ["statistics", "optimization", "matrix", "graph"], "when_to_use": "numeric optimization, statistics, matrices, graphs, symbolic/numerical recipes." },
        { "domain": "engineering", "capabilities": ["sensor_timeseries", "process_optimization", "simulation"], "when_to_use": "industrial data, sensors, tolerances, maintenance, process and reliability calculations." },
        { "domain": "aerospace", "capabilities": ["telemetry", "trajectory", "risk_simulation"], "when_to_use": "telemetry, trajectories, mission logs, space/flight system analysis." },
        { "domain": "simulation", "capabilities": ["montecarlo", "parameter_search", "n_body", "cfd_plan"], "when_to_use": "large repeated model runs, sweeps, stochastic experiments and proofable artifacts." },
        { "domain": "timeseries", "capabilities": ["csv_timeseries", "regime_detection", "anomaly_detection"], "when_to_use": "sensor, market, energy, medical or industrial sequences over time." },
        { "domain": "security", "capabilities": ["security_crypto", "hash_metrics", "entropy", "binary_signature"], "when_to_use": "defensive/lab crypto, hash quality metrics, entropy, signatures, owned binaries/logs, synthetic avalanche/collision/bit-bias experiments." },
        { "domain": "energy", "capabilities": ["sensor_timeseries", "forecast_metrics", "risk"], "when_to_use": "energy markets, grid telemetry, production, volatility and operational anomalies." }
        ,
        { "domain": "any", "capabilities": ["universal_compute_program", "universal_visual_program_2d_3d", "visual_program", "compute_program"], "when_to_use": "Any user-defined domain where the agent should create metrics, calculations, simulations or programmable 2D/3D file views instead of using a closed catalog." }
    ])
}

fn forge_official_examples() -> Value {
    json!([
        { "intent": "create a custom technical indicator from OHLCV metrics", "next_call": "run { intent: \"create a technical indicator from this OHLCV CSV\", inputs: [{ path: \"...\", role: \"market_data\" }], plan_only: true }" },
        { "intent": "detect volume anomalies in a large CSV", "next_call": "run { intent: \"find volume anomalies in this CSV\", inputs: [{ path: \"...\", role: \"csv\" }], plan_only: true }" },
        { "intent": "measure DNA k-mer collision rates and entropy", "next_call": "run { intent: \"measure DNA k-mer hashing and sequence entropy\", inputs: [{ path: \"...\", role: \"sequence\" }], plan_only: true }" },
        { "intent": "profile source code metrics without reading the source into context", "next_call": "run { intent: \"profile source code metrics and entropy\", inputs: [{ path: \"...\", role: \"source\" }], plan_only: true }" },
        { "intent": "define a chemistry, medical, aerospace, engineering or math workflow", "next_call": "create { title: \"Domain compute program\", domain: \"science\", intent: \"describe the analysis\", goal: \"describe what to measure\", metrics: [...] }" },
        { "intent": "compare large business documents or logs", "next_call": "create { title: \"Document comparison program\", domain: \"documents\", intent: \"compare large files without injecting content\", goal: \"return compact deltas, hashes and proof artifacts\", metrics: [...] }" },
        { "intent": "find motifs in industrial sensor signals", "next_call": "run { intent: \"find repeated motifs and anomalies in this sensor timeseries\", inputs: [{ path: \"...\", role: \"telemetry\" }], plan_only: true }" },
        { "intent": "run a defensive crypto/hash lab without a file", "next_call": "run { intent: \"measure hash avalanche, collision rate and bit bias\", capability: \"security_crypto\", plan_only: true }" },
        { "intent": "analyze a user-owned ciphertext/binary/hash dataset", "next_call": "run { intent: \"detect weak cryptographic patterns in this authorized file\", capability: \"security_crypto\", inputs: [{ path: \"...\", role: \"authorized_crypto_data\" }], plan_only: true }" }
        ,
        { "intent": "create a programmable 2D/3D view for any uploaded file", "next_call": "create { title: \"Universal visual program\", program_kind: \"visual_program\", domain: \"custom\", goal: \"surface useful structure in this file\", views: [{ type: \"3d\", axes: { x: \"...\", y: \"...\", z: \"...\" }, color: \"...\", size: \"...\" }] }" },
        { "intent": "run visual views without sending raw rows to the LLM", "next_call": "visual_program { job_id: \"...\", views: [{ type: \"3d\", axes: { x: \"time_index\", y: \"metric_a\", z: \"metric_b\" }, color: \"metric_c\" }] }" }
    ])
}

fn metric_op_info(name: &str, domain: &str, description: &str, inputs: &[&str], params: &[&str]) -> Value {
    json!({
        "op": name,
        "domain": domain,
        "description": description,
        "inputs": inputs,
        "params": params,
        "status": "builtin"
    })
}

fn builtin_metric_op_names() -> Vec<String> {
    metric_operator_catalog()
        .into_iter()
        .filter_map(|op| op.get("op").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn program_execution_readiness(metrics: &[Value], program_hash: Option<&str>) -> Value {
    let builtin_ops = builtin_metric_op_names();
    let mut builtin = Vec::new();
    let mut custom = Vec::new();
    let mut missing = Vec::new();
    for metric in metrics {
        let tag = metric
            .get("tag")
            .and_then(Value::as_str)
            .or_else(|| metric.get("id").and_then(Value::as_str))
            .unwrap_or("metric");
        let op = metric.get("op").and_then(Value::as_str).unwrap_or("");
        let row = json!({
            "tag": tag,
            "op": op,
            "kind": metric.get("kind").cloned().unwrap_or(Value::Null),
            "output": metric.get("output").cloned().unwrap_or(Value::Null)
        });
        if op.trim().is_empty() {
            missing.push(row);
        } else if builtin_ops.iter().any(|known| known.eq_ignore_ascii_case(op)) {
            builtin.push(row);
        } else {
            custom.push(row);
        }
    }
    let custom_count = custom.len();
    let missing_count = missing.len();
    let builtin_count = builtin.len();
    let can_execute_now = missing_count == 0;
    let execution_mode = if missing_count > 0 {
        "invalid_missing_ops"
    } else if custom_count == 0 {
        "all_builtin_metric_toolbox"
    } else if builtin_count == 0 {
        "declarative_custom_extensions"
    } else {
        "mixed_builtin_and_custom_unresolved"
    };
    let next_call = program_hash
        .map(|hash| format!("run {{ program_hash:\"{hash}\", inputs:[{{ path:\"...\", role:\"data\" }}] }}"))
        .unwrap_or_else(|| "run { program_hash:\"...\", inputs:[{ path:\"...\", role:\"data\" }] }".to_string());
    json!({
        "readiness_version": "program_execution_readiness_v1",
        "metric_count": metrics.len(),
        "builtin_count": builtin_count,
        "custom_unresolved_count": custom_count,
        "missing_op_count": missing_count,
        "can_execute_now": can_execute_now,
        "execution_mode": execution_mode,
        "builtin_metrics": builtin,
        "custom_unresolved_metrics": custom,
        "missing_op_metrics": missing,
        "next_call": next_call,
        "policy": "Builtin metrics execute through the unified metric executor. Custom ops are preserved by content hash and reported as custom_unresolved until a matching executor/template/kernel exists.",
        "agent_rule": if custom_count > 0 {
            "Run is still allowed: builtin metrics compute now and custom ops remain explicit unresolved extension points. Ask the user before treating unresolved custom metrics as final results."
        } else {
            "Run can execute the full program with the current builtin metric toolbox."
        }
    })
}

fn metric_declares_text(metric: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        metric.get(*key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).is_some()
            || metric
                .get("params")
                .and_then(|params| params.get(*key))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .is_some()
    })
}

fn metric_stage_rank(kind: &str) -> u8 {
    match kind {
        "input" => 0,
        "transform" | "aggregate" | "compare" => 1,
        "score" | "select" => 2,
        "simulate" | "optimize" => 3,
        "validate" | "prove" => 4,
        "export" => 5,
        _ => 1,
    }
}

fn metric_domain_text(metric: &Value, fallback_domain: Option<&str>) -> String {
    metric
        .get("domain")
        .and_then(Value::as_str)
        .or(fallback_domain)
        .unwrap_or("custom")
        .trim()
        .to_lowercase()
}

fn metric_unit_text(metric: &Value) -> String {
    metric
        .get("unit")
        .and_then(Value::as_str)
        .or_else(|| metric.pointer("/params/unit").and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

fn unit_dimension(unit: &str) -> &'static str {
    let unit = unit.trim().to_lowercase();
    if unit.is_empty() || matches!(unit.as_str(), "1" | "ratio" | "pct" | "%" | "percent" | "score" | "probability") {
        "dimensionless"
    } else if ["usd", "$", "eur", "price", "points", "pips", "quote"].iter().any(|v| unit.contains(v)) {
        "price"
    } else if ["bar", "bars", "s", "sec", "min", "hour", "day", "week", "month", "year", "time"].iter().any(|v| unit.contains(v)) {
        "time"
    } else if ["volume", "shares", "contracts", "count", "ticks"].iter().any(|v| unit.contains(v)) {
        "count"
    } else if ["c", "kelvin", "temperature"].iter().any(|v| unit.contains(v)) {
        "temperature"
    } else if ["m/s", "km/h", "velocity", "speed"].iter().any(|v| unit.contains(v)) {
        "velocity"
    } else if ["m", "meter", "km", "distance", "length"].iter().any(|v| unit.contains(v)) {
        "length"
    } else if ["kg", "mass"].iter().any(|v| unit.contains(v)) {
        "mass"
    } else if ["j", "joule", "watt", "energy", "power"].iter().any(|v| unit.contains(v)) {
        "energy"
    } else {
        "custom"
    }
}

fn op_allows_mixed_dimensions(op: &str) -> bool {
    let op = op.trim().to_lowercase();
    [
        "correlation",
        "correlation_delta",
        "ratio",
        "zscore",
        "normalize",
        "standardize",
        "score",
        "classifier",
        "regression",
        "model",
        "select",
        "rank",
        "visual_map",
        "mapping",
    ]
    .iter()
    .any(|allowed| op.contains(allowed))
}

fn metric_contract_validator(metrics: &[Value], domain: Option<&str>, builtin_set: &HashSet<String>) -> Value {
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for (idx, metric) in metrics.iter().enumerate() {
        let tag = metric_tag(metric);
        let op = metric_op(metric).unwrap_or_default();
        let op_lc = op.to_lowercase();
        let dtype = metric.get("dtype").and_then(Value::as_str).unwrap_or("");
        let unit = metric_unit_text(metric);
        let inputs = metric_inputs(metric);
        let has_formula = metric_declares_text(metric, &["formula", "equation", "expression", "math"]);
        let has_algorithm = metric_declares_text(metric, &["algorithm", "method", "procedure"]);
        let mut row_errors = Vec::<String>::new();
        let mut row_warnings = Vec::<String>::new();
        if op.trim().is_empty() {
            row_errors.push("missing op".to_string());
        }
        if metric_output(metric).trim().is_empty() {
            row_errors.push("missing output".to_string());
        }
        if dtype.trim().is_empty() {
            row_warnings.push("dtype not declared".to_string());
        }
        if unit.trim().is_empty() {
            row_warnings.push("unit not declared".to_string());
        }
        if metric_domain_text(metric, domain).is_empty() {
            row_warnings.push("domain not declared".to_string());
        }
        if !builtin_set.contains(&op_lc) && !has_formula {
            row_errors.push("custom metric needs formula".to_string());
        }
        if !builtin_set.contains(&op_lc) && !has_algorithm {
            row_errors.push("custom metric needs algorithm".to_string());
        }
        if idx > 0 && inputs.is_empty() {
            row_warnings.push("metric has no declared inputs".to_string());
        }
        for error in &row_errors {
            errors.push(format!("{tag}: {error}"));
        }
        for warning in &row_warnings {
            warnings.push(format!("{tag}: {warning}"));
        }
        rows.push(json!({
            "tag": tag,
            "op": op,
            "inputs": inputs,
            "output": metric_output(metric),
            "dtype": if dtype.is_empty() { Value::Null } else { json!(dtype) },
            "unit": if unit.is_empty() { Value::Null } else { json!(unit) },
            "dimension": unit_dimension(&unit),
            "domain": metric_domain_text(metric, domain),
            "formula_declared": has_formula,
            "algorithm_declared": has_algorithm,
            "params_declared": metric.get("params").and_then(Value::as_object).map(|v| !v.is_empty()).unwrap_or(false),
            "errors": row_errors,
            "warnings": row_warnings
        }));
    }
    json!({
        "validator": "metric_contract_validator_v1",
        "status": if errors.is_empty() { "ok" } else { "needs_repair" },
        "metrics": rows,
        "errors": errors,
        "warnings": warnings,
        "required_fields": ["formula", "algorithm", "inputs", "output", "dtype", "unit", "domain", "params"]
    })
}

fn objective_coverage_checker(metrics: &[Value], goal: &str) -> Value {
    let goal_lc = goal.to_lowercase();
    let goal_terms = goal_lc
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|term| term.len() >= 4)
        .collect::<HashSet<_>>();
    let mut rows = Vec::new();
    let mut weak = Vec::new();
    let mut total = 0f64;
    for metric in metrics {
        let tag = metric_tag(metric);
        let haystack = [
            tag.as_str(),
            metric.get("name").and_then(Value::as_str).unwrap_or(""),
            metric.get("goal").and_then(Value::as_str).unwrap_or(""),
            metric.get("description").and_then(Value::as_str).unwrap_or(""),
            metric.get("op").and_then(Value::as_str).unwrap_or(""),
            metric.get("formula").and_then(Value::as_str).unwrap_or(""),
            metric.get("algorithm").and_then(Value::as_str).unwrap_or(""),
        ]
        .join(" ")
        .to_lowercase();
        let matched = goal_terms
            .iter()
            .filter(|term| haystack.contains(**term))
            .count();
        let kind_bonus = match metric.get("kind").and_then(Value::as_str).unwrap_or("") {
            "score" | "select" | "simulate" | "optimize" | "validate" | "prove" => 0.25,
            _ => 0.0,
        };
        let score = ((matched as f64 / goal_terms.len().max(1) as f64) + kind_bonus).min(1.0);
        if score < 0.12 {
            weak.push(tag.clone());
        }
        total += score;
        rows.push(json!({
            "tag": tag,
            "coverage_score": score,
            "matched_goal_terms": matched,
            "helps_objective": score >= 0.12
        }));
    }
    let mean = if metrics.is_empty() { 0.0 } else { total / metrics.len() as f64 };
    json!({
        "checker": "objective_coverage_checker_v1",
        "goal": goal,
        "mean_coverage_score": mean,
        "status": if metrics.is_empty() {
            "no_metrics"
        } else if mean < 0.18 {
            "weak_objective_coverage"
        } else {
            "ok"
        },
        "weak_metrics": weak,
        "metrics": rows,
        "note": "Heuristic text/role coverage: this flags vague metrics; final scientific relevance still comes from validation/holdout evidence."
    })
}

fn metric_dependency_map(metrics: &[Value]) -> Value {
    let outputs = metrics
        .iter()
        .map(|metric| (metric_output(metric), metric_tag(metric)))
        .collect::<HashMap<_, _>>();
    let mut edges = Vec::new();
    let mut nodes = Vec::new();
    for metric in metrics {
        let tag = metric_tag(metric);
        let output = metric_output(metric);
        nodes.push(json!({
            "tag": tag,
            "output": output,
            "op": metric_op(metric).unwrap_or_default(),
            "kind": metric.get("kind").cloned().unwrap_or(Value::Null),
            "unit": metric.get("unit").cloned().unwrap_or(Value::Null),
            "dtype": metric.get("dtype").cloned().unwrap_or(Value::Null)
        }));
        for input in metric_inputs(metric) {
            edges.push(json!({
                "from": outputs.get(&input).cloned().unwrap_or_else(|| input.clone()),
                "from_output": input,
                "to": tag,
                "relationship": "feeds"
            }));
        }
    }
    json!({
        "map": "metric_dependency_map_v1",
        "nodes": nodes,
        "edges": edges,
        "display_hint": "Use this to show exactly how metrics feed each other, e.g. VWAP -> Bollinger -> score."
    })
}

fn unit_dimension_checker(metrics: &[Value]) -> Value {
    let outputs = metrics
        .iter()
        .map(|metric| (metric_output(metric), metric_unit_text(metric)))
        .collect::<HashMap<_, _>>();
    let mut rows = Vec::new();
    let mut warnings = Vec::new();
    for metric in metrics {
        let tag = metric_tag(metric);
        let op = metric_op(metric).unwrap_or_default();
        let input_units = metric_inputs(metric)
            .into_iter()
            .filter_map(|input| outputs.get(&input).cloned().map(|unit| (input, unit)))
            .collect::<Vec<_>>();
        let dims = input_units
            .iter()
            .map(|(_, unit)| unit_dimension(unit))
            .collect::<HashSet<_>>();
        let mixed = dims.len() > 1 && !op_allows_mixed_dimensions(&op);
        if mixed {
            warnings.push(format!("{tag}: mixes dimensions {:?} through op '{op}'", dims));
        }
        rows.push(json!({
            "tag": tag,
            "op": op,
            "output_unit": metric_unit_text(metric),
            "output_dimension": unit_dimension(&metric_unit_text(metric)),
            "input_units": input_units,
            "mixed_dimensions": mixed,
            "status": if mixed { "warning" } else { "ok" }
        }));
    }
    json!({
        "checker": "unit_dimension_checker_v1",
        "status": if warnings.is_empty() { "ok" } else { "warnings" },
        "warnings": warnings,
        "metrics": rows
    })
}

fn scientific_validation_block(metrics: &[Value], domain: Option<&str>) -> Value {
    let domain = domain.unwrap_or("custom").to_lowercase();
    let has_validation_metric = metrics.iter().any(|metric| {
        matches!(
            metric.get("kind").and_then(Value::as_str).unwrap_or(""),
            "validate" | "prove"
        ) || metric_tag(metric).contains("holdout")
            || metric_tag(metric).contains("walk_forward")
            || metric_tag(metric).contains("bootstrap")
            || metric_tag(metric).contains("permutation")
    });
    let recommended = if domain.contains("finance") || domain.contains("market") || domain.contains("timeseries") {
        vec!["walk-forward split", "holdout PnL/accuracy", "bootstrap confidence interval", "permutation test against shuffled labels", "transaction-cost stress"]
    } else if domain.contains("medicine") || domain.contains("biology") {
        vec!["train/test split", "cross-validation", "bootstrap confidence interval", "permutation test", "effect-size and false-discovery control"]
    } else if domain.contains("physics") || domain.contains("engineering") || domain.contains("simulation") {
        vec!["baseline comparison", "sensitivity analysis", "Monte Carlo uncertainty", "unit consistency check", "residual/error bound"]
    } else {
        vec!["train/test or holdout split", "bootstrap confidence interval", "permutation/random baseline", "sensitivity analysis", "reproducible proof artifact"]
    };
    json!({
        "block": "scientific_validation_v1",
        "domain": domain,
        "has_validation_metric": has_validation_metric,
        "status": if has_validation_metric { "declared" } else { "recommended_missing" },
        "recommended_methods": recommended,
        "rule": "Ambitious discovery programs should include validation metrics before treating outputs as scientific/decision evidence."
    })
}

fn formula_to_executor_binding(routes: &[Value]) -> Value {
    let rows = routes
        .iter()
        .map(|route| {
            let formula = route.get("formula_declared").and_then(Value::as_bool).unwrap_or(false);
            let algorithm = route.get("algorithm_declared").and_then(Value::as_bool).unwrap_or(false);
            let bound = route.get("executor_bound").and_then(Value::as_bool).unwrap_or(false);
            let route_name = route.get("route").and_then(Value::as_str).unwrap_or("");
            json!({
                "tag": route.get("tag").cloned().unwrap_or(Value::Null),
                "op": route.get("op").cloned().unwrap_or(Value::Null),
                "route": route_name,
                "formula_declared": formula,
                "algorithm_declared": algorithm,
                "executor_bound": bound,
                "status": if bound {
                    "executable"
                } else if formula || algorithm {
                    "formula_declared_but_no_executor_yet"
                } else {
                    "unresolved_missing_math_contract"
                }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "binding": "formula_to_executor_binding_v1",
        "rows": rows
    })
}

fn program_explain_plan(metrics: &[Value], routes: &[Value], terminal_outputs: &[String]) -> Value {
    let steps = metrics
        .iter()
        .enumerate()
        .map(|(idx, metric)| {
            let route = routes.get(idx).unwrap_or(&Value::Null);
            let tag = metric_tag(metric);
            let formula = metric_math_formula(metric);
            let inputs = metric_inputs(metric);
            json!({
                "step": idx + 1,
                "tag": tag,
                "calculation": formula,
                "uses": inputs,
                "produces": metric_output(metric),
                "executor": route.get("route").cloned().unwrap_or(Value::Null),
                "display_line": format!(
                    "{}. {} = {}; uses [{}]",
                    idx + 1,
                    metric_output(metric),
                    formula,
                    inputs.join(", ")
                )
            })
        })
        .collect::<Vec<_>>();
    json!({
        "plan": "program_explain_plan_v1",
        "calculation_chain": steps,
        "terminal_outputs": terminal_outputs,
        "display_contract": {
            "live_card_should_show": "formula/display_line for each active metric plus redundancy badges when Atlas/cache hits occur",
            "chat_should_show": "only concise program creation/build events, not raw executor internals"
        }
    })
}

fn universal_program_linter(metrics: &[Value], routes: &[Value], warnings: &[String], errors: &[String], goal: &str) -> Value {
    let mut lint = Vec::new();
    if goal.split_whitespace().count() < 5 {
        lint.push(json!({ "severity": "warning", "rule": "objective_too_short", "message": "Program objective is too vague." }));
    }
    if metrics.is_empty() {
        lint.push(json!({ "severity": "error", "rule": "no_metrics", "message": "Program has no metric tags." }));
    }
    let unresolved = routes
        .iter()
        .filter(|route| !route.get("executor_bound").and_then(Value::as_bool).unwrap_or(false))
        .count();
    if unresolved > 0 {
        lint.push(json!({ "severity": "warning", "rule": "unresolved_executors", "message": format!("{unresolved} metric(s) are not bound to an executable engine yet.") }));
    }
    if metrics.len() > 2 && !metrics.iter().any(|metric| matches!(metric.get("kind").and_then(Value::as_str).unwrap_or(""), "score" | "select" | "validate" | "prove")) {
        lint.push(json!({ "severity": "warning", "rule": "no_decision_or_validation_metric", "message": "Program has several metrics but no explicit score/select/validate/prove tag." }));
    }
    for error in errors {
        lint.push(json!({ "severity": "error", "rule": "compiler_error", "message": error }));
    }
    for warning in warnings.iter().take(24) {
        lint.push(json!({ "severity": "warning", "rule": "compiler_warning", "message": warning }));
    }
    let error_count = lint
        .iter()
        .filter(|row| row.get("severity").and_then(Value::as_str) == Some("error"))
        .count();
    json!({
        "linter": "universal_program_linter_v1",
        "status": if error_count == 0 { "pass_with_warnings_allowed" } else { "needs_repair" },
        "error_count": error_count,
        "items": lint
    })
}

fn compile_program_metric_routes(
    metrics: &[Value],
    program_kind: &str,
    domain: Option<&str>,
    goal: &str,
    metric_graph: &Value,
) -> Value {
    let builtin_ops = builtin_metric_op_names();
    let builtin_set = builtin_ops
        .iter()
        .map(|op| op.to_lowercase())
        .collect::<HashSet<_>>();
    let outputs = metrics
        .iter()
        .map(|metric| metric_output(metric))
        .collect::<HashSet<_>>();
    let mut consumers: HashMap<String, Vec<String>> = HashMap::new();
    let mut terminal_outputs = outputs.clone();
    for metric in metrics {
        let tag = metric_tag(metric);
        for input in metric_inputs(metric) {
            if outputs.contains(&input) {
                terminal_outputs.remove(&input);
                consumers.entry(input).or_default().push(tag.clone());
            }
        }
    }

    let mut routes = Vec::new();
    let mut warnings = Vec::<String>::new();
    let mut errors = Vec::<String>::new();
    let mut builtin_count = 0usize;
    let mut custom_count = 0usize;
    let mut formula_declared_count = 0usize;
    let mut algorithm_declared_count = 0usize;

    for (idx, metric) in metrics.iter().enumerate() {
        let tag = metric_tag(metric);
        let op = metric_op(metric).unwrap_or_default();
        let op_lc = op.to_lowercase();
        let kind = metric
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("transform")
            .to_string();
        let inputs = metric_inputs(metric);
        let external_inputs = inputs
            .iter()
            .filter(|input| !outputs.contains(*input))
            .cloned()
            .collect::<Vec<_>>();
        let internal_inputs = inputs
            .iter()
            .filter(|input| outputs.contains(*input))
            .cloned()
            .collect::<Vec<_>>();
        let has_formula = metric_declares_text(metric, &["formula", "equation", "expression", "math"]);
        let has_algorithm = metric_declares_text(metric, &["algorithm", "method", "procedure"]);
        if has_formula {
            formula_declared_count += 1;
        }
        if has_algorithm {
            algorithm_declared_count += 1;
        }

        let route = if op.trim().is_empty() {
            errors.push(format!("{tag}: missing op, cannot route this metric"));
            "invalid_missing_op"
        } else if builtin_set.contains(&op_lc) {
            builtin_count += 1;
            "builtin_rust_metric_executor"
        } else if op_lc.contains("cuda") || op_lc.contains("gpu") {
            custom_count += 1;
            warnings.push(format!("{tag}: CUDA/GPU executor requested by op '{op}' but no concrete kernel binding exists yet"));
            "cuda_kernel_pending_binding"
        } else if op_lc.contains("python") || op_lc.contains("sidecar") {
            custom_count += 1;
            warnings.push(format!("{tag}: Python/sidecar executor requested by op '{op}' but no sidecar binding exists yet"));
            "python_sidecar_pending_binding"
        } else if program_kind == "visual_program" {
            custom_count += 1;
            "visual_program_materializer"
        } else if has_formula || has_algorithm {
            custom_count += 1;
            warnings.push(format!("{tag}: formula/algorithm declared but no executor is bound to op '{op}' yet"));
            "declarative_custom_metric_pending_executor"
        } else {
            custom_count += 1;
            warnings.push(format!("{tag}: op '{op}' is custom and lacks formula/algorithm detail"));
            "custom_unresolved_missing_math_contract"
        };

        if kind != "input" && inputs.is_empty() {
            warnings.push(format!("{tag}: non-input metric has no inputs"));
        }
        if !has_formula && !builtin_set.contains(&op_lc) {
            warnings.push(format!("{tag}: custom metric has no explicit formula"));
        }
        if !has_algorithm && !builtin_set.contains(&op_lc) {
            warnings.push(format!("{tag}: custom metric has no explicit algorithm"));
        }
        if matches!(kind.as_str(), "compare" | "score" | "optimize" | "validate") && inputs.len() < 2 {
            warnings.push(format!("{tag}: kind '{kind}' usually needs multiple inputs to cross metrics"));
        }

        routes.push(json!({
            "index": idx + 1,
            "tag": tag,
            "kind": kind,
            "stage_rank": metric_stage_rank(metric.get("kind").and_then(Value::as_str).unwrap_or("transform")),
            "op": if op.is_empty() { Value::Null } else { json!(op) },
            "route": route,
            "executor_bound": route == "builtin_rust_metric_executor" || route == "visual_program_materializer",
            "formula_declared": has_formula,
            "algorithm_declared": has_algorithm,
            "inputs": inputs,
            "internal_inputs": internal_inputs,
            "external_inputs": external_inputs,
            "output": metric_output(metric),
            "dtype": metric.get("dtype").cloned().unwrap_or(Value::Null),
            "unit": metric.get("unit").cloned().unwrap_or(Value::Null),
            "feeds": consumers.get(&metric_output(metric)).cloned().unwrap_or_default()
        }));
    }

    let has_validation = metrics.iter().any(|metric| {
        matches!(
            metric.get("kind").and_then(Value::as_str).unwrap_or(""),
            "validate" | "prove" | "export"
        )
    });
    if metrics.len() >= 4 && !has_validation {
        warnings.push("program has several metrics but no validate/prove/export metric to close the objective".to_string());
    }
    if terminal_outputs.is_empty() && !metrics.is_empty() {
        warnings.push("no terminal output detected; every metric output feeds another node".to_string());
    }

    let terminal_outputs_vec = terminal_outputs.into_iter().collect::<Vec<_>>();
    let metric_contract_validation = metric_contract_validator(metrics, domain, &builtin_set);
    let objective_coverage = objective_coverage_checker(metrics, goal);
    let dependency_map = metric_dependency_map(metrics);
    let unit_dimension_check = unit_dimension_checker(metrics);
    let scientific_validation = scientific_validation_block(metrics, domain);
    let formula_executor_binding = formula_to_executor_binding(&routes);
    let explain_plan = program_explain_plan(metrics, &routes, &terminal_outputs_vec);
    let universal_linter = universal_program_linter(metrics, &routes, &warnings, &errors, goal);

    let status = if !errors.is_empty() {
        "compile_failed"
    } else if custom_count > 0 {
        "compiled_with_custom_routes"
    } else {
        "compiled_runnable"
    };
    json!({
        "compiler_version": "forge_program_compiler_v1",
        "status": status,
        "program_kind": program_kind,
        "domain": domain,
        "goal": goal,
        "metric_count": metrics.len(),
        "builtin_routed_count": builtin_count,
        "custom_route_count": custom_count,
        "formula_declared_count": formula_declared_count,
        "algorithm_declared_count": algorithm_declared_count,
        "graph": metric_graph,
        "routes": routes,
        "metric_contract_validator": metric_contract_validation,
        "objective_coverage_checker": objective_coverage,
        "unit_dimension_checker": unit_dimension_check,
        "metric_dependency_map": dependency_map,
        "scientific_validation_block": scientific_validation,
        "formula_to_executor_binding": formula_executor_binding,
        "program_explain_plan": explain_plan,
        "universal_program_linter": universal_linter,
        "terminal_outputs": terminal_outputs_vec,
        "warnings": warnings,
        "errors": errors,
        "policy": "program_compile_validate_route: compile metric graph, validate contracts/units/dependencies/objective coverage, route each metric to an executor, produce explain/display contracts, and refuse only structural errors while reporting repairable warnings."
    })
}

fn define_program(args: ProgramDefineArgs, client: &McpClientInfo) -> Result<Value, String> {
    let store_path = forge_store_dir()?;
    let mut value = serde_json::to_value(args).map_err(|e| format!("encode create args: {e}"))?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "created_by_agent".to_string(),
            serde_json::to_value(client).map_err(|e| format!("encode client info: {e}"))?,
        );
    }
    forge_agent_runtime::direct_create_rich_program(&store_path, &value, &client.name)
}

fn list_programs(args: &Value) -> Result<Value, String> {
    let limit = bounded_limit(args.get("limit"), MCP_LIST_LIMIT_DEFAULT, MCP_LIST_LIMIT_MAX);
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let domain_filter = args
        .get("domain")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let dir = programs_dir()?;
    let mut entries = Vec::new();
    match fs::read_dir(&dir) {
        Ok(read_dir) => {
            entries.extend(
                read_dir
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
                    .filter_map(|e| {
                        let modified = e.metadata().ok()?.modified().ok()?;
                        Some((modified, e.path()))
                    }),
            );
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("read programs dir '{}': {err}", dir.display())),
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));

    let mut programs = Vec::new();
    for (_, path) in entries {
        let value = read_json_value(&path)?;
        let summary = summarize_program_value(value);
        let haystack = [
            summary.get("title").and_then(Value::as_str).unwrap_or(""),
            summary.get("domain").and_then(Value::as_str).unwrap_or(""),
            summary.get("goal").and_then(Value::as_str).unwrap_or(""),
            summary.get("program_kind").and_then(Value::as_str).unwrap_or(""),
            summary.get("program_hash").and_then(Value::as_str).unwrap_or(""),
        ]
        .join(" ")
        .to_lowercase();
        if let Some(q) = &query {
            if !haystack.contains(q) {
                continue;
            }
        }
        if let Some(domain) = &domain_filter {
            if summary
                .get("domain")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase()
                != *domain
            {
                continue;
            }
        }
        programs.push(summary);
        if programs.len() >= limit {
            break;
        }
    }
    Ok(json!({
        "programs": programs,
        "limit": limit,
        "query": query,
        "domain": domain_filter,
        "content_policy": {
            "source_content_included": false,
            "program_specs_are_hash_addressed": true
        }
    }))
}

fn read_program(program_hash: &str) -> Result<Value, String> {
    validate_content_hash(program_hash, "program_hash")?;
    let path = program_manifest_path(program_hash)?;
    let value = read_json_value(&path)?;
    Ok(json!({
        "program": sanitize_program_value(value),
        "program_path": path.display().to_string(),
        "content_policy": {
            "source_content_included": false,
            "spec_text_full_content_included": false
        }
    }))
}

fn compile_validate_route_definition(args: ProgramDefineArgs) -> Result<Value, String> {
    let title = bounded_clean_program_title(&args.title)?;
    let goal = bounded_clean_text(&args.goal, "goal", 4 * 1024)?;
    let domain = args
        .domain
        .as_deref()
        .map(|v| bounded_clean_text(v, "domain", 120))
        .transpose()?;
    let template = args
        .template
        .as_deref()
        .map(|v| bounded_clean_text(v, "template", 120))
        .transpose()?;
    let explicit_program_kind = args
        .program_kind
        .as_deref()
        .or(args.kind.as_deref())
        .map(|v| normalize_program_kind(v))
        .transpose()?;
    let spec_text = args
        .spec_text
        .as_deref()
        .map(|v| bounded_clean_text(v, "spec_text", MCP_PROGRAM_SPEC_TEXT_MAX_BYTES))
        .transpose()?;
    let visual_views = normalize_visual_program_views(args.views, spec_text.as_deref())?;
    let program_kind = infer_program_kind(
        explicit_program_kind.as_deref(),
        template.as_deref(),
        spec_text.as_deref(),
        &visual_views,
    );
    let mut compile_errors = Vec::<String>::new();
    let mut metrics = args.metrics;
    if let Some(text) = &spec_text {
        match extract_metric_tags(text) {
            Ok(tags) => metrics.extend(tags),
            Err(err) => compile_errors.push(format!("spec_text metric tags: {err}")),
        }
    }
    if metrics.is_empty() && !(program_kind == "visual_program" && !visual_views.is_empty()) {
        compile_errors.push("compile requires at least one metric tag in metrics[] or spec_text".to_string());
    }
    if metrics.len() > MCP_PROGRAM_METRICS_MAX {
        return Err(format!("too many metric tags: {} > {}", metrics.len(), MCP_PROGRAM_METRICS_MAX));
    }
    let mut normalized_metrics = Vec::new();
    for (idx, metric) in metrics.into_iter().enumerate() {
        match normalize_program_metric(metric, idx, domain.as_deref()) {
            Ok(metric) => normalized_metrics.push(metric),
            Err(err) => compile_errors.push(format!("metric {}: {err}", idx + 1)),
        }
    }
    let metric_graph = match validate_metric_dsl_graph(&normalized_metrics) {
        Ok(graph) => graph,
        Err(err) => {
            compile_errors.push(err.clone());
            json!({
                "node_count": normalized_metrics.len(),
                "edge_count": 0,
                "is_dag": false,
                "errors": [err]
            })
        }
    };
    let mut program_compiler = compile_program_metric_routes(
        &normalized_metrics,
        &program_kind,
        domain.as_deref(),
        &goal,
        &metric_graph,
    );
    if !compile_errors.is_empty() {
        if let Some(obj) = program_compiler.as_object_mut() {
            obj.insert("status".to_string(), json!("needs_repair"));
            obj.insert("repair_required".to_string(), json!(true));
            obj.insert("can_run".to_string(), json!(false));
            let entry = obj.entry("errors".to_string()).or_insert_with(|| json!([]));
            if let Some(errors) = entry.as_array_mut() {
                for err in &compile_errors {
                    errors.push(json!(err));
                }
            }
        }
    }
    let canonical = json!({
        "title": title,
        "domain": domain,
        "goal": goal,
        "program_kind": program_kind,
        "template": template,
        "metrics": normalized_metrics,
        "visual_program": if program_kind == "visual_program" {
            visual_program_contract(&visual_views, &normalized_metrics)
        } else {
            Value::Null
        },
        "metric_contract": metric_math_contract(&normalized_metrics),
        "metric_graph": metric_graph,
        "program_compiler": program_compiler,
        "source_schema": args.source_schema,
        "constraints": args.constraints,
        "output_contract": args.output_contract
    });
    let hash = format!(
        "{:016x}",
        quick_file_hash(&serde_json::to_vec(&canonical).map_err(|e| format!("encode compile basis: {e}"))?)
    );
    Ok(json!({
        "tool": "program_compile_validate_route",
        "stored": false,
        "would_program_hash": hash,
        "canonical": canonical,
        "execution_readiness": program_execution_readiness(&normalized_metrics, Some(&hash)),
        "next_step": "If status is runnable or warnings are acceptable, call create to store this program in My Atlas, then run. If needs_repair, fix the metric tags/formulas/units/routes first."
    }))
}

fn program_compile_validate_route(args: &Value) -> Result<Value, String> {
    if let Some(program_hash) = args
        .get("program_hash")
        .or_else(|| args.get("program_id"))
        .and_then(Value::as_str)
    {
        validate_content_hash(program_hash, "program_hash")?;
        let program = read_json_value(&program_manifest_path(program_hash)?)?;
        let canonical = program.get("canonical").cloned().unwrap_or(Value::Null);
        let metrics = canonical
            .get("metrics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let goal = canonical.get("goal").and_then(Value::as_str).unwrap_or("");
        let program_kind = program_kind_from_manifest(&program);
        let domain = canonical.get("domain").and_then(Value::as_str);
        let metric_graph = validate_metric_dsl_graph(&metrics).unwrap_or_else(|err| json!({
            "node_count": metrics.len(),
            "edge_count": 0,
            "is_dag": false,
            "errors": [err]
        }));
        let compiler = compile_program_metric_routes(&metrics, &program_kind, domain, goal, &metric_graph);
        return Ok(json!({
            "tool": "program_compile_validate_route",
            "stored": true,
            "program_hash": program_hash,
            "program": summarize_program_value(program),
            "compiler": compiler,
            "execution_readiness": program_execution_readiness(&metrics, Some(program_hash)),
            "next_step": "Use this compiler report to choose/reuse Atlas tags, repair unresolved metrics, or run when route status is acceptable."
        }));
    }
    let define_args: ProgramDefineArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("bad compile arguments: {e}"))?;
    compile_validate_route_definition(define_args)
}

fn execute_program(args: ProgramExecuteArgs, client: &McpClientInfo) -> Result<Value, String> {
    let store_path = forge_store_dir()?;
    let mut value = serde_json::to_value(args).map_err(|e| format!("encode run args: {e}"))?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "created_by_agent".to_string(),
            serde_json::to_value(client).map_err(|e| format!("encode client info: {e}"))?,
        );
    }
    forge_agent_runtime::direct_run_rich_program(&store_path, &value, &client.name)
}

fn attach_program_run_to_parent_session(parent_job_id: &str, run_ref: Value) -> Result<(), String> {
    validate_job_id(parent_job_id)?;
    let mut parent = read_job_value(parent_job_id)?;
    let Some(obj) = parent.as_object_mut() else {
        return Ok(());
    };
    let entry = obj
        .entry("session_program_runs".to_string())
        .or_insert_with(|| json!([]));
    if !entry.is_array() {
        *entry = json!([]);
    }
    let runs = entry
        .as_array_mut()
        .ok_or_else(|| "session_program_runs is not an array".to_string())?;
    let run_id = run_ref.get("job_id").and_then(Value::as_str).unwrap_or("");
    if !run_id.is_empty()
        && runs
            .iter()
            .any(|item| item.get("job_id").and_then(Value::as_str) == Some(run_id))
    {
        return Ok(());
    }
    runs.push(run_ref);
    if runs.len() > 50 {
        let drop_count = runs.len() - 50;
        runs.drain(0..drop_count);
    }
    obj.insert("updated_ms".to_string(), json!(now_ms()));
    persist_existing_job_value(parent_job_id, parent)
}

fn execute_metric_toolbox(
    program: &Value,
    inputs: &[ResolvedProgramInput],
    jobs_dir: &Path,
    job_id: &str,
    run_hash: &str,
    log_path: &Path,
    log: &mut Vec<String>,
) -> Result<Value, String> {
    let metrics = program
        .pointer("/canonical/metrics")
        .and_then(Value::as_array)
        .ok_or_else(|| "program canonical metrics missing".to_string())?;
    push_job_log(
        log_path,
        log,
        format!("Running the metric toolbox across {} metric tag{}.", metrics.len(), if metrics.len() == 1 { "" } else { "s" }),
    )?;

    let started = std::time::Instant::now();
    let mut results = Vec::new();
    let mut executed = 0usize;
    let mut unresolved = 0usize;
    let mut failed = 0usize;
    let mut cache_hits = 0usize;
    let math_contract = metric_math_contract(metrics);
    let math_rows = math_contract.as_array().cloned().unwrap_or_default();
    for (idx, metric) in metrics.iter().enumerate() {
        let tag = metric_tag(metric);
        let math_row = math_rows.get(idx);
        let formula = math_row
            .and_then(|row| row.get("formula"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| "")
            .trim();
        let algorithm = math_row
            .and_then(|row| row.get("algorithm"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| "")
            .trim();
        let feeds = math_row
            .and_then(|row| row.get("feeds"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        push_job_log(
            log_path,
            log,
            format!(
                "{} = {} | uses [{}] -> {}",
                tag,
                if formula.is_empty() { "formula not declared; using exact op signature" } else { formula },
                metric_inputs(metric).join(", "),
                metric_output(metric)
            ),
        )?;
        if !algorithm.is_empty() {
            push_job_log(
                log_path,
                log,
                format!("{} algorithm: {}", tag, algorithm),
            )?;
        }
        if !feeds.is_empty() {
            push_job_log(
                log_path,
                log,
                format!("{} flows into [{}]", metric_output(metric), feeds),
            )?;
        }
        let metric_started = std::time::Instant::now();
        let metric_hash = metric_invocation_hash(metric, inputs);
        let result = if let Some(cached) = read_metric_result_cache(jobs_dir, &metric_hash)? {
            cache_hits += 1;
            let status = cached.get("status").and_then(Value::as_str).unwrap_or("computed");
            match status {
                "computed" => executed += 1,
                "custom_unresolved" => unresolved += 1,
                "failed" => failed += 1,
                _ => {}
            }
            annotate_metric_result(cached, true, metric_started.elapsed().as_secs_f64() * 1000.0)
        } else {
            let computed = match compute_builtin_metric(metric, inputs) {
            Ok(Some(value)) => {
                executed += 1;
                json!({
                    "tag": tag,
                    "op": metric_op(metric),
                    "status": "computed",
                    "elapsed_ms": metric_started.elapsed().as_secs_f64() * 1000.0,
                    "value": value,
                    "metric_hash": metric_hash.clone()
                })
            }
            Ok(None) => {
                unresolved += 1;
                json!({
                    "tag": tag,
                    "op": metric_op(metric),
                    "status": "custom_unresolved",
                    "elapsed_ms": metric_started.elapsed().as_secs_f64() * 1000.0,
                    "reason": "No builtin executor for this op yet. The metric remains content-addressed and can be mapped to a future plugin/kernel/template.",
                    "metric_hash": metric_hash.clone()
                })
            }
            Err(err) => {
                failed += 1;
                json!({
                    "tag": tag,
                    "op": metric_op(metric),
                    "status": "failed",
                    "elapsed_ms": metric_started.elapsed().as_secs_f64() * 1000.0,
                    "error": err,
                    "metric_hash": metric_hash.clone()
                })
            }
            };
            if computed.get("status").and_then(Value::as_str) != Some("failed") {
                persist_metric_result_cache(jobs_dir, &metric_hash, &computed)?;
            }
            annotate_metric_result(computed, false, metric_started.elapsed().as_secs_f64() * 1000.0)
        };
        push_job_log(
            log_path,
            log,
            format!(
                "{} -> {} via {} ({})",
                tag,
                metric_output(metric),
                metric_op(metric).unwrap_or_else(|| "custom".to_string()),
                if result.get("cache_hit").and_then(Value::as_bool).unwrap_or(false) {
                    "reused identical mapped calculation"
                } else {
                    result.get("status").and_then(Value::as_str).unwrap_or("computed")
                }
            ),
        )?;
        results.push(result);
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let metrics_path = jobs_dir.join(format!("{job_id}.metrics.json"));
    let proof_path = jobs_dir.join(format!("{job_id}.proof.json"));
    let visual_mapping_path = jobs_dir.join(format!("{job_id}.visual_mapping.json"));
    let metrics_doc = json!({
        "job_id": job_id,
        "run_hash": run_hash,
        "program_hash": program.get("program_hash").cloned().unwrap_or(Value::Null),
        "elapsed_ms": elapsed_ms,
        "metric_results": results,
        "source_content_included": false
    });
    persist_json_pretty(&metrics_path, &metrics_doc)?;
    let metrics_hash = quick_file_hash_path(&metrics_path)?;

    let proof_doc = json!({
        "job_id": job_id,
        "proof_kind": "forge_metric_toolbox_v1",
        "run_hash": run_hash,
        "program_hash": program.get("program_hash").cloned().unwrap_or(Value::Null),
        "inputs": inputs.iter().map(|input| json!({
            "role": input.role,
            "path": input.path.display().to_string(),
            "bytes": input.bytes.len(),
            "content_hash": input.content_hash,
            "content_included": false
        })).collect::<Vec<_>>(),
        "metrics_artifact_hash": format!("{metrics_hash:016x}"),
        "elapsed_ms": elapsed_ms,
        "source_content_included": false,
        "determinism_note": "Metric results are derived from program_hash + input content hashes + normalized metric invocations."
    });
    persist_json_pretty(&proof_path, &proof_doc)?;
    let proof_hash = quick_file_hash_path(&proof_path)?;

    let visual_views = metrics
        .iter()
        .enumerate()
        .map(|(idx, metric)| {
            let tag = metric_tag(metric);
            json!({
                "id": format!("metric_{idx}_{tag}"),
                "title": tag,
                "type": "metric_result_node",
                "metric_tag": metric_tag(metric),
                "metric_op": metric_op(metric).unwrap_or_else(|| "custom".to_string()),
                "coordinates": {
                    "x": { "source": "metric_index", "value": idx as u64 },
                    "y": { "source": "metric_elapsed_ms_or_value", "fallback": "result.elapsed_ms" },
                    "z": { "source": "metric_hash_bucket", "fallback": "metric_hash" }
                },
                "encoding": {
                    "color": { "source": "status", "values": ["computed", "custom_unresolved", "failed"] },
                    "size": { "source": "cache_hit", "values": ["miss", "hit"] }
                },
                "select_returns": ["metric_tag", "metric_op", "status", "metric_hash", "cache_hit", "elapsed_ms"]
            })
        })
        .collect::<Vec<_>>();
    let visual_mapping_doc = json!({
        "version": "forge.visual_mapping.v1",
        "kind": "program_result_visual_mapping",
        "job_id": job_id,
        "run_hash": run_hash,
        "program_hash": program.get("program_hash").cloned().unwrap_or(Value::Null),
        "created_ms": now_ms(),
        "source_artifacts": {
            "metrics_path": metrics_path.display().to_string(),
            "metrics_hash_algorithm": "forge_fnv1a64",
            "metrics_hash": format!("{metrics_hash:016x}"),
            "proof_path": proof_path.display().to_string(),
            "proof_hash_algorithm": "forge_fnv1a64",
            "proof_hash": format!("{proof_hash:016x}")
        },
        "views": [{
            "id": "metric_result_space",
            "title": "Metric result space",
            "type": "point_cloud_contract",
            "recommended": true,
            "purpose": "Render each computed metric as a selectable node tied to its result hash, status, elapsed time and proof artifact.",
            "nodes": visual_views,
            "axes": [
                { "axis": "x", "meaning": "metric order in the normalized program DAG" },
                { "axis": "y", "meaning": "elapsed/value-derived height chosen by the viewer" },
                { "axis": "z", "meaning": "stable hash bucket for visual separation" }
            ],
            "selection_contract": {
                "select_returns": ["job_id", "program_hash", "run_hash", "metric_tag", "metric_hash", "metrics_path", "proof_path"],
                "raw_input_returned": false
            }
        }],
        "content_policy": {
            "raw_input_included": false,
            "metrics_content_inlined_to_llm": false,
            "download_by_reference_only": true
        },
        "agent_guidance": [
            "Use this mapping to attach a 3D/graph view to the compute result.",
            "Do not paste metrics/proof/point-cloud contents into the LLM context.",
            "Use mapping_analysis when the user asks what the 3D geometry means; it returns compact PCA/cluster/outlier diagnostics without raw points.",
            "Use read { kind:'artifacts', job_id } to retrieve file references and hashes."
        ]
    });
    persist_json_pretty(&visual_mapping_path, &visual_mapping_doc)?;
    let visual_mapping_hash = quick_file_hash_path(&visual_mapping_path)?;

    push_job_log(
        log_path,
        log,
        format!(
            "executor_status=completed computed={} unresolved={} failed={} elapsed={elapsed_ms:.3}ms",
            executed, unresolved, failed
        ),
    )?;

    Ok(json!({
        "stage": "metric_toolbox_executed",
        "builtin_executor": true,
        "elapsed_ms": elapsed_ms,
        "metric_count": metrics.len(),
        "computed_count": executed,
        "unresolved_count": unresolved,
        "failed_count": failed,
        "cache_hit_count": cache_hits,
        "compute_avoided": {
            "mode": "real_executor_counters",
            "program_kind": "custom_compute_program_run",
            "metric_count": metrics.len(),
            "computed_count": executed,
            "unresolved_count": unresolved,
            "failed_count": failed,
            "cache_hit_count": cache_hits,
            "cache_miss_count": metrics.len().saturating_sub(cache_hits),
            "elapsed_ms": elapsed_ms,
            "operations_unit": "metric_invocations"
        },
        "metrics_artifact": file_artifact_value("metrics", metrics_path)?,
        "proof_artifact": file_artifact_value("proof", proof_path)?,
        "visual_mapping_artifact": file_artifact_value("visual_mapping", visual_mapping_path)?,
        "metrics_artifact_hash": format!("{metrics_hash:016x}"),
        "proof_artifact_hash": format!("{proof_hash:016x}"),
        "visual_mapping_artifact_hash": format!("{visual_mapping_hash:016x}"),
        "visual_mapping": {
            "available": true,
            "version": "forge.visual_mapping.v1",
            "kind": "program_result_visual_mapping",
            "path": jobs_dir.join(format!("{job_id}.visual_mapping.json")).display().to_string(),
            "hash_algorithm": "forge_fnv1a64",
            "hash": format!("{visual_mapping_hash:016x}"),
            "view_count": 1,
            "node_count": metrics.len() as u64,
            "download_by_reference_only": true
        }
    }))
}

fn annotate_metric_result(mut value: Value, cache_hit: bool, elapsed_ms: f64) -> Value {
    if let Value::Object(ref mut obj) = value {
        obj.insert("cache_hit".to_string(), json!(cache_hit));
        obj.insert("dispatch_elapsed_ms".to_string(), json!(elapsed_ms));
        obj.insert(
            "execution_contract".to_string(),
            json!("unified_metric_path_v1"),
        );
    }
    value
}

fn metric_cache_dir(jobs_dir: &Path) -> PathBuf {
    jobs_dir
        .parent()
        .map(|parent| parent.join("metric-cache"))
        .unwrap_or_else(|| jobs_dir.join("metric-cache"))
}

fn metric_cache_path(jobs_dir: &Path, metric_hash: &str) -> PathBuf {
    metric_cache_dir(jobs_dir).join(format!("{metric_hash}.json"))
}

fn read_metric_result_cache(jobs_dir: &Path, metric_hash: &str) -> Result<Option<Value>, String> {
    validate_content_hash(metric_hash, "metric_hash")?;
    let path = metric_cache_path(jobs_dir, metric_hash);
    match read_json_value(&path) {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.contains("os error 2") || err.contains("introuvable") || err.contains("not found") => Ok(None),
        Err(err) => Err(err),
    }
}

fn persist_metric_result_cache(jobs_dir: &Path, metric_hash: &str, value: &Value) -> Result<(), String> {
    validate_content_hash(metric_hash, "metric_hash")?;
    let dir = metric_cache_dir(jobs_dir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("create metric cache dir '{}': {e}", dir.display()))?;
    let mut cached = value.clone();
    if let Value::Object(ref mut obj) = cached {
        obj.insert("cache_key".to_string(), json!(metric_hash));
        obj.insert("content_addressed".to_string(), json!(true));
        obj.insert("execution_contract".to_string(), json!("unified_metric_path_v1"));
    }
    persist_json_pretty(&metric_cache_path(jobs_dir, metric_hash), &cached)
}

fn compute_builtin_metric(metric: &Value, inputs: &[ResolvedProgramInput]) -> Result<Option<Value>, String> {
    let op = metric_op(metric)
        .map(|v| v.to_lowercase())
        .unwrap_or_default();
    if inputs.is_empty()
        && metric_param(metric, "skip_if_no_input")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        return Ok(Some(json!({
            "skipped": true,
            "reason": "metric is optional and no input file was provided",
            "synthetic_mode": true
        })));
    }
    match op.as_str() {
        "bytes" => Ok(Some(json!({
            "total_bytes": inputs.iter().map(|input| input.bytes.len()).sum::<usize>(),
            "inputs": inputs.iter().map(|input| json!({
                "role": input.role,
                "bytes": input.bytes.len(),
                "content_hash": input.content_hash
            })).collect::<Vec<_>>()
        }))),
        "line_count" => {
            let input = first_input(inputs)?;
            let text = String::from_utf8_lossy(&input.bytes);
            Ok(Some(json!({
                "lines": text.lines().count(),
                "input_hash": input.content_hash
            })))
        }
        "char_count" => {
            let input = first_input(inputs)?;
            let text = String::from_utf8_lossy(&input.bytes);
            Ok(Some(json!({
                "chars": text.chars().count(),
                "input_hash": input.content_hash
            })))
        }
        "byte_entropy" => {
            let input = first_input(inputs)?;
            Ok(Some(json!({
                "entropy_bits_per_byte": byte_entropy(&input.bytes),
                "input_hash": input.content_hash
            })))
        }
        "byte_histogram" => {
            let input = first_input(inputs)?;
            let mut hist = [0u64; 256];
            for &byte in &input.bytes {
                hist[byte as usize] += 1;
            }
            Ok(Some(json!({
                "histogram": hist.to_vec(),
                "input_hash": input.content_hash
            })))
        }
        "csv_profile" => {
            let input = first_input(inputs)?;
            let text = String::from_utf8_lossy(&input.bytes);
            Ok(Some(csv_profile(&text)))
        }
        "zscore" => {
            let values = metric_numeric_column(metric, inputs)?;
            let threshold = metric_param_f64(metric, "threshold").unwrap_or(3.0);
            let stats = numeric_stats(&values)?;
            let std = stats
                .get("std")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let mean = stats
                .get("mean")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let zscores = if std > 0.0 {
                values.iter().map(|v| (v - mean) / std).collect::<Vec<_>>()
            } else {
                vec![0.0; values.len()]
            };
            let max_abs = zscores.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
            let anomalies = zscores.iter().filter(|v| v.abs() >= threshold).count();
            Ok(Some(json!({
                "stats": stats,
                "threshold": threshold,
                "max_abs_zscore": max_abs,
                "anomaly_count": anomalies,
                "last_zscore": zscores.last().copied().unwrap_or(0.0)
            })))
        }
        "rolling_mean" => {
            let values = metric_numeric_column(metric, inputs)?;
            let window = metric_param_usize(metric, "window").unwrap_or(20).max(1);
            Ok(Some(rolling_summary(&values, window, true)?))
        }
        "rolling_std" => {
            let values = metric_numeric_column(metric, inputs)?;
            let window = metric_param_usize(metric, "window").unwrap_or(20).max(2);
            Ok(Some(rolling_summary(&values, window, false)?))
        }
        "correlation" => {
            let (a, b) = metric_two_numeric_columns(metric, inputs)?;
            Ok(Some(json!({ "pearson": pearson(&a, &b)? })))
        }
        "correlation_delta" => {
            let (a, b) = metric_two_numeric_columns(metric, inputs)?;
            let window = metric_param_usize(metric, "window").unwrap_or(64).max(2);
            let full = pearson(&a, &b)?;
            let start = a.len().saturating_sub(window).min(b.len().saturating_sub(window));
            let tail = pearson(&a[start..], &b[start..])?;
            Ok(Some(json!({
                "pearson_full": full,
                "pearson_tail": tail,
                "delta": tail - full,
                "tail_window": window
            })))
        }
        "entropy" => {
            let text = metric_text_input(metric, inputs)?;
            Ok(Some(json!({
                "entropy_bits_per_symbol": text_entropy(&text),
                "symbols": text.chars().count()
            })))
        }
        "gc_content" => {
            let sequence = metric_sequence_input(metric, inputs)?;
            let gc = sequence.bytes().filter(|b| matches!(b, b'G' | b'C')).count();
            let acgt = sequence.bytes().filter(|b| matches!(b, b'A' | b'C' | b'G' | b'T')).count();
            Ok(Some(json!({
                "gc_ratio": if acgt == 0 { 0.0 } else { gc as f64 / acgt as f64 },
                "bases_acgt": acgt,
                "ignored_symbols": sequence.len().saturating_sub(acgt)
            })))
        }
        "kmer_count" => {
            let sequence = metric_sequence_input(metric, inputs)?;
            let k = metric_param_usize(metric, "k").unwrap_or(7).max(1);
            Ok(Some(kmer_summary(&sequence, k, None)))
        }
        "kmer_collision_rate" => {
            let sequence = metric_sequence_input(metric, inputs)?;
            let k = metric_param_usize(metric, "k").unwrap_or(7).max(1);
            let buckets = metric_param_usize(metric, "buckets").or(Some(1 << 16));
            Ok(Some(kmer_summary(&sequence, k, buckets)))
        }
        "synthetic_hash_avalanche" => Ok(Some(synthetic_hash_avalanche(metric))),
        "synthetic_hash_collision_rate" => Ok(Some(synthetic_hash_collision_rate(metric))),
        "synthetic_hash_bit_bias" => Ok(Some(synthetic_hash_bit_bias(metric))),
        "" => Err("metric op is missing".to_string()),
        _ => Ok(None),
    }
}

fn first_input(inputs: &[ResolvedProgramInput]) -> Result<&ResolvedProgramInput, String> {
    inputs.first().ok_or_else(|| "metric execution requires at least one path input".to_string())
}

fn synthetic_hash_avalanche(metric: &Value) -> Value {
    let samples = metric_param_usize(metric, "samples").unwrap_or(4096).clamp(1, 200_000);
    let bytes_len = metric_param_usize(metric, "bytes").unwrap_or(32).clamp(1, 4096);
    let hash_bits = metric_param_usize(metric, "hash_bits").unwrap_or(64).clamp(1, 64);
    let mut total_ratio = 0.0;
    let mut min_ratio = 1.0;
    let mut max_ratio = 0.0;
    for i in 0..samples {
        let mut bytes = synthetic_sample_bytes(i as u64, bytes_len);
        let base = truncate_hash_bits(quick_file_hash(&bytes), hash_bits);
        let bit = i % (bytes_len * 8);
        bytes[bit / 8] ^= 1u8 << (bit % 8);
        let flipped = truncate_hash_bits(quick_file_hash(&bytes), hash_bits);
        let changed = (base ^ flipped).count_ones() as f64;
        let ratio = changed / hash_bits as f64;
        total_ratio += ratio;
        min_ratio = f64::min(min_ratio, ratio);
        max_ratio = f64::max(max_ratio, ratio);
    }
    json!({
        "experiment": "synthetic_hash_avalanche",
        "hash": "forge_fnv1a64_truncated_lab_metric",
        "samples": samples,
        "sample_bytes": bytes_len,
        "hash_bits": hash_bits,
        "mean_changed_bit_ratio": total_ratio / samples as f64,
        "ideal_ratio": 0.5,
        "min_changed_bit_ratio": min_ratio,
        "max_changed_bit_ratio": max_ratio,
        "synthetic_input": true,
        "security_note": "Defensive/lab metric for hash behavior exploration; not a credential cracking tool."
    })
}

fn synthetic_hash_collision_rate(metric: &Value) -> Value {
    let samples = metric_param_usize(metric, "samples").unwrap_or(65_536).clamp(1, 1_000_000);
    let bytes_len = metric_param_usize(metric, "bytes").unwrap_or(24).clamp(1, 4096);
    let hash_bits = metric_param_usize(metric, "hash_bits").unwrap_or(32).clamp(1, 64);
    let mut seen: HashMap<u64, usize> = HashMap::new();
    let mut collisions = 0usize;
    for i in 0..samples {
        let bytes = synthetic_sample_bytes(i as u64, bytes_len);
        let h = truncate_hash_bits(quick_file_hash(&bytes), hash_bits);
        if seen.insert(h, i).is_some() {
            collisions += 1;
        }
    }
    json!({
        "experiment": "synthetic_hash_collision_rate",
        "hash": "forge_fnv1a64_truncated_lab_metric",
        "samples": samples,
        "sample_bytes": bytes_len,
        "hash_bits": hash_bits,
        "unique_hashes": seen.len(),
        "collisions": collisions,
        "collision_rate": collisions as f64 / samples as f64,
        "synthetic_input": true,
        "security_note": "Use to compare toy/fingerprint behavior and birthday-bound intuition on authorized/lab data."
    })
}

fn synthetic_hash_bit_bias(metric: &Value) -> Value {
    let samples = metric_param_usize(metric, "samples").unwrap_or(32_768).clamp(1, 1_000_000);
    let bytes_len = metric_param_usize(metric, "bytes").unwrap_or(32).clamp(1, 4096);
    let hash_bits = metric_param_usize(metric, "hash_bits").unwrap_or(64).clamp(1, 64);
    let mut ones = vec![0usize; hash_bits];
    for i in 0..samples {
        let bytes = synthetic_sample_bytes(i as u64, bytes_len);
        let h = truncate_hash_bits(quick_file_hash(&bytes), hash_bits);
        for bit in 0..hash_bits {
            if ((h >> bit) & 1) == 1 {
                ones[bit] += 1;
            }
        }
    }
    let ratios = ones
        .iter()
        .map(|count| *count as f64 / samples as f64)
        .collect::<Vec<_>>();
    let max_abs_bias = ratios
        .iter()
        .map(|ratio| (ratio - 0.5).abs())
        .fold(0.0_f64, f64::max);
    json!({
        "experiment": "synthetic_hash_bit_bias",
        "hash": "forge_fnv1a64_truncated_lab_metric",
        "samples": samples,
        "sample_bytes": bytes_len,
        "hash_bits": hash_bits,
        "max_abs_bias_from_half": max_abs_bias,
        "bit_one_ratios": ratios,
        "synthetic_input": true,
        "security_note": "Defensive/lab distribution metric; no raw secret material is generated or returned."
    })
}

fn synthetic_sample_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(0xd1b5_4a32_d192_ed03);
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn truncate_hash_bits(hash: u64, bits: usize) -> u64 {
    if bits >= 64 {
        hash
    } else {
        hash & ((1u64 << bits) - 1)
    }
}

fn metric_tag(metric: &Value) -> String {
    metric
        .get("tag")
        .and_then(Value::as_str)
        .unwrap_or("metric")
        .to_string()
}

fn metric_op(metric: &Value) -> Option<String> {
    metric.get("op").and_then(Value::as_str).map(str::to_string)
}

fn metric_inputs(metric: &Value) -> Vec<String> {
    metric
        .get("inputs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn metric_output(metric: &Value) -> String {
    metric
        .get("output")
        .and_then(Value::as_str)
        .or_else(|| metric.get("tag").and_then(Value::as_str))
        .or_else(|| metric.get("id").and_then(Value::as_str))
        .unwrap_or("metric_output")
        .to_string()
}

fn metric_param(metric: &Value, name: &str) -> Option<Value> {
    metric
        .get("params")
        .and_then(|params| params.get(name))
        .cloned()
}

fn metric_param_usize(metric: &Value, name: &str) -> Option<usize> {
    metric_param(metric, name).and_then(|value| {
        value
            .as_u64()
            .map(|v| v as usize)
            .or_else(|| value.as_str().and_then(|v| v.parse::<usize>().ok()))
    })
}

fn metric_param_f64(metric: &Value, name: &str) -> Option<f64> {
    metric_param(metric, name).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|v| v.parse::<f64>().ok()))
    })
}

fn metric_invocation_hash(metric: &Value, inputs: &[ResolvedProgramInput]) -> String {
    let basis = json!({
        "metric": metric,
        "inputs": inputs.iter().map(|input| json!({
            "role": input.role,
            "content_hash": input.content_hash
        })).collect::<Vec<_>>()
    });
    let bytes = serde_json::to_vec(&basis).unwrap_or_default();
    format!("{:016x}", quick_file_hash(&bytes))
}

fn metric_text_input(metric: &Value, inputs: &[ResolvedProgramInput]) -> Result<String, String> {
    let input_names = metric_inputs(metric);
    if let Some(column) = input_names.first() {
        if let Ok(values) = csv_text_column(&String::from_utf8_lossy(&first_input(inputs)?.bytes), column) {
            return Ok(values.join("\n"));
        }
    }
    Ok(String::from_utf8_lossy(&first_input(inputs)?.bytes).to_string())
}

fn metric_sequence_input(metric: &Value, inputs: &[ResolvedProgramInput]) -> Result<String, String> {
    let raw = metric_text_input(metric, inputs)?;
    Ok(raw
        .chars()
        .filter_map(|ch| {
            let up = ch.to_ascii_uppercase();
            matches!(up, 'A' | 'C' | 'G' | 'T' | 'N').then_some(up)
        })
        .collect())
}

fn metric_numeric_column(metric: &Value, inputs: &[ResolvedProgramInput]) -> Result<Vec<f64>, String> {
    let column = metric_inputs(metric)
        .first()
        .cloned()
        .ok_or_else(|| format!("metric '{}' requires one column input", metric_tag(metric)))?;
    csv_numeric_column(&String::from_utf8_lossy(&first_input(inputs)?.bytes), &column)
}

fn metric_two_numeric_columns(metric: &Value, inputs: &[ResolvedProgramInput]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let input_names = metric_inputs(metric);
    if input_names.len() < 2 {
        return Err(format!("metric '{}' requires two column inputs", metric_tag(metric)));
    }
    let text = String::from_utf8_lossy(&first_input(inputs)?.bytes);
    let a = csv_numeric_column(&text, &input_names[0])?;
    let b = csv_numeric_column(&text, &input_names[1])?;
    let n = a.len().min(b.len());
    if n < 2 {
        return Err("correlation requires at least two paired numeric rows".to_string());
    }
    Ok((a[..n].to_vec(), b[..n].to_vec()))
}

fn csv_profile(text: &str) -> Value {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or("");
    let delimiter = detect_delimiter(header);
    let headers = split_delimited_line(header, delimiter);
    let mut rows = 0usize;
    let mut numeric_counts = vec![0usize; headers.len()];
    let mut missing_counts = vec![0usize; headers.len()];
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        rows += 1;
        let cells = split_delimited_line(line, delimiter);
        for i in 0..headers.len() {
            let cell = cells.get(i).map(String::as_str).unwrap_or("").trim();
            if cell.is_empty() {
                missing_counts[i] += 1;
            } else if cell.parse::<f64>().is_ok() {
                numeric_counts[i] += 1;
            }
        }
    }
    json!({
        "rows": rows,
        "columns": headers.iter().enumerate().map(|(i, name)| json!({
            "name": name,
            "numeric_count": numeric_counts.get(i).copied().unwrap_or(0),
            "missing_count": missing_counts.get(i).copied().unwrap_or(0),
            "numeric_ratio": if rows == 0 { 0.0 } else { numeric_counts.get(i).copied().unwrap_or(0) as f64 / rows as f64 }
        })).collect::<Vec<_>>(),
        "delimiter": delimiter.to_string()
    })
}

fn csv_numeric_column(text: &str, column: &str) -> Result<Vec<f64>, String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or_else(|| "CSV is empty".to_string())?;
    let delimiter = detect_delimiter(header);
    let headers = split_delimited_line(header, delimiter);
    let target = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(column))
        .ok_or_else(|| format!("CSV column '{column}' not found"))?;
    let mut values = Vec::new();
    for line in lines {
        let cells = split_delimited_line(line, delimiter);
        if let Some(cell) = cells.get(target) {
            if let Ok(value) = cell.trim().parse::<f64>() {
                if value.is_finite() {
                    values.push(value);
                }
            }
        }
    }
    if values.is_empty() {
        return Err(format!("CSV column '{column}' has no numeric values"));
    }
    Ok(values)
}

fn csv_text_column(text: &str, column: &str) -> Result<Vec<String>, String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or_else(|| "CSV is empty".to_string())?;
    let delimiter = detect_delimiter(header);
    let headers = split_delimited_line(header, delimiter);
    let target = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(column))
        .ok_or_else(|| format!("CSV column '{column}' not found"))?;
    Ok(lines
        .filter_map(|line| split_delimited_line(line, delimiter).get(target).cloned())
        .collect())
}

fn detect_delimiter(header: &str) -> char {
    [',', ';', '\t', '|']
        .into_iter()
        .max_by_key(|delimiter| header.matches(*delimiter).count())
        .unwrap_or(',')
}

fn split_delimited_line(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|cell| cell.trim().trim_matches('"').to_string())
        .collect()
}

fn numeric_stats(values: &[f64]) -> Result<Value, String> {
    if values.is_empty() {
        return Err("numeric stats require at least one value".to_string());
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let min = values.iter().fold(f64::INFINITY, |a, b| a.min(*b));
    let max = values.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
    Ok(json!({
        "count": values.len(),
        "mean": mean,
        "std": var.sqrt(),
        "min": min,
        "max": max,
        "last": values.last().copied().unwrap_or(0.0)
    }))
}

fn rolling_summary(values: &[f64], window: usize, mean_mode: bool) -> Result<Value, String> {
    if values.len() < window {
        return Err(format!("rolling window {window} > value count {}", values.len()));
    }
    let mut out = Vec::new();
    for slice in values.windows(window) {
        let mean = slice.iter().sum::<f64>() / window as f64;
        let value = if mean_mode {
            mean
        } else {
            (slice.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / window as f64).sqrt()
        };
        out.push(value);
    }
    Ok(json!({
        "window": window,
        "points": out.len(),
        "last": out.last().copied().unwrap_or(0.0),
        "min": out.iter().fold(f64::INFINITY, |a, b| a.min(*b)),
        "max": out.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b))
    }))
}

fn pearson(a: &[f64], b: &[f64]) -> Result<f64, String> {
    let n = a.len().min(b.len());
    if n < 2 {
        return Err("pearson correlation requires at least two paired values".to_string());
    }
    let (a, b) = (&a[..n], &b[..n]);
    let mean_a = a.iter().sum::<f64>() / n as f64;
    let mean_b = b.iter().sum::<f64>() / n as f64;
    let mut num = 0.0;
    let mut den_a = 0.0;
    let mut den_b = 0.0;
    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        num += da * db;
        den_a += da * da;
        den_b += db * db;
    }
    let den = (den_a * den_b).sqrt();
    Ok(if den == 0.0 { 0.0 } else { num / den })
}

fn byte_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut hist = [0usize; 256];
    for &byte in bytes {
        hist[byte as usize] += 1;
    }
    let n = bytes.len() as f64;
    hist.iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f64 / n;
            -p * p.log2()
        })
        .sum()
}

fn text_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let mut hist: HashMap<char, usize> = HashMap::new();
    for ch in text.chars() {
        *hist.entry(ch).or_insert(0) += 1;
    }
    let n = text.chars().count() as f64;
    hist.values()
        .map(|&count| {
            let p = count as f64 / n;
            -p * p.log2()
        })
        .sum()
}

fn kmer_summary(sequence: &str, k: usize, buckets: Option<usize>) -> Value {
    if k == 0 || sequence.len() < k {
        return json!({ "k": k, "total_kmers": 0, "unique_kmers": 0 });
    }
    let bytes = sequence.as_bytes();
    let mut counts: HashMap<&[u8], usize> = HashMap::new();
    let mut bucket_counts: HashMap<usize, usize> = HashMap::new();
    for window in bytes.windows(k) {
        *counts.entry(window).or_insert(0) += 1;
        if let Some(bucket_count) = buckets {
            let hash = quick_file_hash(window) as usize;
            *bucket_counts.entry(hash % bucket_count.max(1)).or_insert(0) += 1;
        }
    }
    let total = bytes.len() - k + 1;
    let collisions = bucket_counts
        .values()
        .map(|count| count.saturating_sub(1))
        .sum::<usize>();
    json!({
        "k": k,
        "total_kmers": total,
        "unique_kmers": counts.len(),
        "repeat_ratio": if total == 0 { 0.0 } else { 1.0 - counts.len() as f64 / total as f64 },
        "hash_buckets": buckets,
        "hash_collision_count": collisions,
        "hash_collision_rate": if total == 0 { 0.0 } else { collisions as f64 / total as f64 }
    })
}

fn normalize_program_metric(
    mut metric: ProgramMetricSpec,
    idx: usize,
    default_domain: Option<&str>,
) -> Result<Value, String> {
    let fallback = metric
        .id
        .as_deref()
        .or_else(|| metric.name.as_deref())
        .or_else(|| metric.description.as_deref())
        .unwrap_or("metric");
    let tag = if metric.tag.trim().is_empty() {
        sanitize_metric_tag(fallback, idx)
    } else {
        sanitize_metric_tag(&metric.tag, idx)
    };
    let id = metric
        .id
        .as_deref()
        .map(|v| sanitize_metric_tag(v, idx))
        .unwrap_or_else(|| tag.clone());
    let kind = metric
        .kind
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric kind", 80))
        .transpose()?
        .unwrap_or_else(|| "transform".to_string())
        .to_lowercase();
    validate_metric_kind(&kind, &tag)?;
    let domain = metric
        .domain
        .take()
        .or_else(|| default_domain.map(str::to_string))
        .map(|v| bounded_clean_text(&v, "metric domain", 120))
        .transpose()?;
    let op = metric
        .op
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric op", 120))
        .transpose()?;
    let inputs = metric
        .inputs
        .iter()
        .map(|v| bounded_clean_text(v, "metric input", 160))
        .collect::<Result<Vec<_>, _>>()?;
    let output = metric
        .output
        .as_deref()
        .map(|v| sanitize_metric_ref(v, "metric output", idx))
        .transpose()?
        .unwrap_or_else(|| tag.clone());
    // Cap human-readable metric name so it stays minimalist in the
    // atlas UI cubes alongside the sanitised tag.
    let name = metric
        .name
        .as_deref()
        .map(bounded_clean_metric_name)
        .transpose()?;
    let dtype = metric
        .dtype
        .as_deref()
        .map(|v| normalize_open_enum(v, "metric dtype", metric_dtype_values(), &tag))
        .transpose()?;
    let description = metric
        .description
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric description", 2 * 1024))
        .transpose()?;
    let goal = metric
        .goal
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric goal", 2 * 1024))
        .transpose()?;
    let unit = metric
        .unit
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric unit", 80))
        .transpose()?;
    let formula = metric
        .formula
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric formula", 2 * 1024))
        .transpose()?;
    let algorithm = metric
        .algorithm
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric algorithm", 2 * 1024))
        .transpose()?;
    let cache = metric
        .cache
        .as_deref()
        .map(|v| normalize_open_enum(v, "metric cache", &["content", "session", "none"], &tag))
        .transpose()?
        .unwrap_or_else(|| "content".to_string());
    let proof = metric
        .proof
        .as_deref()
        .map(|v| normalize_open_enum(v, "metric proof", &["hash", "replay", "statistical", "deterministic", "audit", "none"], &tag))
        .transpose()?
        .unwrap_or_else(|| "hash".to_string());
    let condition = metric
        .condition
        .as_deref()
        .map(|v| bounded_clean_text(v, "metric if", 512))
        .transpose()?;
    if !metric.params.is_object() && !metric.params.is_null() {
        return Err(format!("metric '{tag}' params must be an object"));
    }
    if !metric.constraints.is_object() && !metric.constraints.is_null() {
        return Err(format!("metric '{tag}' constraints must be an object"));
    }
    Ok(json!({
        "id": id,
        "tag": tag,
        "name": name,
        "kind": kind,
        "domain": domain,
        "op": op,
        "inputs": inputs,
        "output": output,
        "dtype": dtype,
        "params": if metric.params.is_null() { json!({}) } else { metric.params },
        "constraints": if metric.constraints.is_null() { json!({}) } else { metric.constraints },
        "unit": unit,
        "cache": cache,
        "proof": proof,
        "if": condition,
        "goal": goal,
        "description": description,
        "formula": formula,
        "algorithm": algorithm,
        "weight": metric.weight
    }))
}

fn metric_dsl_contract() -> Value {
    json!({
        "name": "Forge Metric DSL",
        "version": "1.0",
        "shape": "content-addressed DAG of metric nodes",
        "required_semantics": "Each metric node produces one named output; inputs can reference source fields/files or outputs from previous metric nodes.",
        "math_display_contract": "Live compute cards must display only formulas/algorithms declared by metric tags or exact op signatures derived from those tags; never infer formulas from chat/log text.",
        "compiler_contract": "create automatically emits program_compiler: dependency DAG, route table, terminal outputs, warnings/errors and executor bindings for every metric.",
        "kinds": metric_kind_values(),
        "dtypes": metric_dtype_values(),
        "cache": ["content", "session", "none"],
        "proof": ["hash", "replay", "statistical", "deterministic", "audit", "none"],
        "open_extension_rule": "domain/op/params are open-ended; dtype/cache/proof accept known values or custom:<name> for future executors.",
        "pipeline_pattern": "input -> transform/aggregate/compare -> score/select/optimize -> validate/prove/export"
    })
}

fn visual_program_contract(views: &[Value], metrics: &[Value]) -> Value {
    let view_2d_count = views
        .iter()
        .filter(|view| view.get("type").and_then(Value::as_str) == Some("2d"))
        .count();
    let view_3d_count = views
        .iter()
        .filter(|view| view.get("type").and_then(Value::as_str) == Some("3d"))
        .count();
    json!({
        "name": "Forge Visual Program DSL",
        "version": "1.0",
        "shape": "one content-addressed program can define unlimited metric nodes and multiple 2D/3D views over the same file/session",
        "metric_count": metrics.len(),
        "view_count": views.len(),
        "view_2d_count": view_2d_count,
        "view_3d_count": view_3d_count,
        "views": views,
        "required_3d_axes": ["x", "y", "z"],
        "optional_encodings": ["color", "size", "overlays", "filters", "params"],
        "execution_policy": "Forge materializes visual artifacts locally from metric/view recipes; raw rows, arrays and point clouds are returned by reference only.",
        "viewer_relationship": "2D and 3D cards are alternate views of the same visual_program, not separate sessions or unrelated programs."
    })
}

fn normalize_program_kind(value: &str) -> Result<String, String> {
    let clean = bounded_clean_text(value, "program_kind", 80)?
        .to_lowercase()
        .replace('-', "_");
    match clean.as_str() {
        "visual" | "visual_program" | "forge_visual_program" | "visual_mapping" | "viewer_program" | "mapping_program" => {
            Ok("visual_program".to_string())
        }
        "compute" | "compute_program" | "forge_compute_program" | "metric_program" | "custom_compute_program" => {
            Ok("compute_program".to_string())
        }
        _ => Err(format!(
            "invalid program_kind '{clean}'. Expected compute_program or visual_program"
        )),
    }
}

fn infer_program_kind(
    explicit: Option<&str>,
    template: Option<&str>,
    spec_text: Option<&str>,
    views: &[Value],
) -> String {
    if let Some(kind) = explicit {
        return kind.to_string();
    }
    let template_hint = template
        .map(|value| value.to_lowercase())
        .unwrap_or_default();
    let spec_has_view = spec_text
        .map(|value| value.to_lowercase().contains("<view"))
        .unwrap_or(false);
    if !views.is_empty()
        || spec_has_view
        || template_hint.contains("visual")
        || template_hint.contains("mapping")
        || template_hint.contains("viewer")
    {
        "visual_program".to_string()
    } else {
        "compute_program".to_string()
    }
}

fn program_kind_from_manifest(program: &Value) -> String {
    program
        .pointer("/canonical/program_kind")
        .and_then(Value::as_str)
        .or_else(|| program.get("program_kind").and_then(Value::as_str))
        .or_else(|| program.get("kind").and_then(Value::as_str))
        .map(|value| {
            if value.eq_ignore_ascii_case("forge_visual_program")
                || value.eq_ignore_ascii_case("visual_program")
            {
                "visual_program".to_string()
            } else {
                "compute_program".to_string()
            }
        })
        .unwrap_or_else(|| "compute_program".to_string())
}

fn normalize_visual_program_views(
    explicit_views: Vec<Value>,
    spec_text: Option<&str>,
) -> Result<Vec<Value>, String> {
    let mut views = Vec::new();
    for view in explicit_views {
        let idx = views.len();
        views.push(normalize_visual_program_view(view, idx)?);
    }
    if let Some(text) = spec_text {
        for view in extract_visual_view_tags(text)? {
            let idx = views.len();
            views.push(normalize_visual_program_view(view, idx)?);
        }
    }
    if views.len() > 64 {
        return Err(format!("too many <view> tags: {} > 64", views.len()));
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (idx, view) in views.iter_mut().enumerate() {
        let base = view
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("view_{}", idx + 1));
        let count = seen.entry(base.clone()).or_insert(0);
        if *count > 0 {
            if let Value::Object(obj) = view {
                obj.insert("id".to_string(), json!(format!("{base}_{}", *count + 1)));
            }
        }
        *count += 1;
    }
    Ok(views)
}

fn normalize_visual_program_view(view: Value, idx: usize) -> Result<Value, String> {
    let Value::Object(mut obj) = view else {
        return Err(format!("visual view {} must be an object", idx + 1));
    };
    let raw_type = obj
        .get("type")
        .or_else(|| obj.get("view_type"))
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("3d");
    let view_type = normalize_visual_view_type(raw_type)?;
    let title = obj
        .get("title")
        .or_else(|| obj.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if view_type == "3d" {
                format!("3D visual view {}", idx + 1)
            } else {
                format!("2D visual view {}", idx + 1)
            }
        });
    let id = obj
        .get("id")
        .or_else(|| obj.get("tag"))
        .or_else(|| obj.get("name"))
        .and_then(Value::as_str)
        .map(|value| sanitize_visual_view_id(value, idx))
        .unwrap_or_else(|| sanitize_visual_view_id(&title, idx));

    let mut axes = obj
        .get("axes")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (axis, aliases) in [
        ("x", ["x", "x_metric", "x_axis"]),
        ("y", ["y", "y_metric", "y_axis"]),
        ("z", ["z", "z_metric", "z_axis"]),
    ] {
        if axes.get(axis).is_none() {
            for alias in aliases {
                if let Some(value) = obj.get(alias) {
                    axes.insert(axis.to_string(), value.clone());
                    break;
                }
            }
        }
    }
    if view_type == "3d" {
        for axis in ["x", "y", "z"] {
            if axes
                .get(axis)
                .and_then(Value::as_str)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
                == false
            {
                return Err(format!(
                    "3D visual view '{id}' requires axis '{axis}' in axes or as {axis}=\"...\""
                ));
            }
        }
    } else {
        if axes.get("x").is_none() {
            axes.insert("x".to_string(), json!("time_index"));
        }
        if axes.get("y").is_none() {
            let y = obj
                .get("metric")
                .or_else(|| obj.get("y_metric"))
                .or_else(|| obj.get("series"))
                .cloned()
                .unwrap_or_else(|| json!("close"));
            axes.insert("y".to_string(), y);
        }
    }

    if let Some(overlays) = obj.get("overlays").and_then(Value::as_str).map(split_metric_list) {
        obj.insert("overlays".to_string(), json!(overlays));
    }
    obj.insert("id".to_string(), json!(id));
    obj.insert("title".to_string(), json!(title));
    obj.insert("type".to_string(), json!(view_type));
    obj.insert("axes".to_string(), Value::Object(axes));
    obj.insert(
        "content_policy".to_string(),
        json!({
            "raw_input_included": false,
            "raw_series_returned": false,
            "point_cloud_returned": false,
            "view_is_recipe_not_data": true
        }),
    );
    Ok(Value::Object(obj))
}

fn normalize_visual_view_type(value: &str) -> Result<String, String> {
    let clean = value.trim().to_lowercase().replace('-', "_");
    match clean.as_str() {
        "3d" | "pointcloud" | "point_cloud" | "mapping_3d" | "map3d" | "xyz" => {
            Ok("3d".to_string())
        }
        "2d" | "chart" | "plot" | "series" | "mapping_2d" | "map2d" => Ok("2d".to_string()),
        _ => Err(format!("invalid visual view type '{clean}'. Expected 2d or 3d")),
    }
}

fn sanitize_visual_view_id(value: &str, idx: usize) -> String {
    let clean = sanitize_metric_tag(value, idx);
    if clean.is_empty() {
        format!("view_{}", idx + 1)
    } else {
        clean
    }
}

fn split_metric_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn validate_metric_dsl_graph(metrics: &[Value]) -> Result<Value, String> {
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut outputs: HashMap<String, usize> = HashMap::new();
    for (idx, metric) in metrics.iter().enumerate() {
        for field in ["id", "tag"] {
            if let Some(name) = metric.get(field).and_then(Value::as_str) {
                if let Some(prev) = names.insert(name.to_string(), idx) {
                    if prev != idx {
                        return Err(format!(
                            "duplicate metric {field} '{name}' at metric {} and {}",
                            prev + 1,
                            idx + 1
                        ));
                    }
                }
            }
        }
        if let Some(output) = metric.get("output").and_then(Value::as_str) {
            if let Some(prev) = outputs.insert(output.to_string(), idx) {
                return Err(format!(
                    "duplicate metric output '{output}' at metric {} and {}",
                    prev + 1,
                    idx + 1
                ));
            }
        }
    }

    let mut deps = vec![Vec::<usize>::new(); metrics.len()];
    for (idx, metric) in metrics.iter().enumerate() {
        let self_refs = [
            metric.get("id").and_then(Value::as_str).unwrap_or(""),
            metric.get("tag").and_then(Value::as_str).unwrap_or(""),
            metric.get("output").and_then(Value::as_str).unwrap_or(""),
        ];
        for input in metric_inputs(metric) {
            if self_refs.iter().any(|v| !v.is_empty() && *v == input) {
                return Err(format!("metric '{}' cannot input itself via '{input}'", metric_tag(metric)));
            }
            if let Some(dep) = names.get(&input).or_else(|| outputs.get(&input)).copied() {
                deps[idx].push(dep);
            }
        }
    }
    validate_metric_graph_acyclic(&deps, metrics)?;
    Ok(json!({
        "node_count": metrics.len(),
        "edge_count": deps.iter().map(Vec::len).sum::<usize>(),
        "is_dag": true,
        "node_ids": metrics.iter().filter_map(|metric| metric.get("id").and_then(Value::as_str)).collect::<Vec<_>>(),
        "outputs": metrics.iter().filter_map(|metric| metric.get("output").and_then(Value::as_str)).collect::<Vec<_>>()
    }))
}

fn validate_metric_graph_acyclic(deps: &[Vec<usize>], metrics: &[Value]) -> Result<(), String> {
    fn visit(
        node: usize,
        deps: &[Vec<usize>],
        metrics: &[Value],
        state: &mut [u8],
        stack: &mut Vec<usize>,
    ) -> Result<(), String> {
        match state[node] {
            1 => {
                let cycle = stack
                    .iter()
                    .chain(std::iter::once(&node))
                    .map(|idx| metric_tag(&metrics[*idx]))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return Err(format!("metric graph cycle detected: {cycle}"));
            }
            2 => return Ok(()),
            _ => {}
        }
        state[node] = 1;
        stack.push(node);
        for &dep in &deps[node] {
            visit(dep, deps, metrics, state, stack)?;
        }
        stack.pop();
        state[node] = 2;
        Ok(())
    }

    let mut state = vec![0u8; deps.len()];
    let mut stack = Vec::new();
    for node in 0..deps.len() {
        visit(node, deps, metrics, &mut state, &mut stack)?;
    }
    Ok(())
}

fn validate_metric_kind(kind: &str, tag: &str) -> Result<(), String> {
    if metric_kind_values().contains(&kind) {
        Ok(())
    } else {
        Err(format!(
            "metric '{tag}' has invalid kind '{kind}'. Expected one of: {}",
            metric_kind_values().join(", ")
        ))
    }
}

fn metric_kind_values() -> &'static [&'static str] {
    &[
        "input",
        "transform",
        "aggregate",
        "compare",
        "score",
        "select",
        "simulate",
        "optimize",
        "validate",
        "prove",
        "export",
    ]
}

fn metric_dtype_values() -> &'static [&'static str] {
    &[
        "f64",
        "i64",
        "bool",
        "string",
        "bytes",
        "json",
        "vector",
        "matrix",
        "table",
        "timeseries",
        "graph",
        "molecule",
        "sequence",
        "artifact",
        "image",
        "volume",
        "mesh",
        "pointcloud",
        "trajectory",
        "model",
        "distribution",
    ]
}

fn normalize_open_enum(value: &str, field: &str, allowed: &[&str], tag: &str) -> Result<String, String> {
    let clean = bounded_clean_text(value, field, 120)?.to_lowercase();
    if allowed.contains(&clean.as_str()) || clean.starts_with("custom:") {
        Ok(clean)
    } else {
        Err(format!(
            "metric '{tag}' invalid {field} '{clean}'. Expected one of [{}] or custom:<name>",
            allowed.join(", ")
        ))
    }
}

fn sanitize_metric_ref(value: &str, field: &str, idx: usize) -> Result<String, String> {
    let clean = sanitize_metric_tag(value, idx);
    if clean.is_empty() {
        Err(format!("{field} cannot be empty"))
    } else {
        Ok(clean)
    }
}

fn extract_metric_tags(spec_text: &str) -> Result<Vec<ProgramMetricSpec>, String> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(rel) = spec_text[offset..].find("<metric") {
        let start = offset + rel;
        let after_start = start + "<metric".len();
        if spec_text[after_start..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
            offset = after_start;
            continue;
        }
        let Some(close_rel) = spec_text[after_start..].find('>') else {
            return Err("unterminated <metric ...> tag".to_string());
        };
        let header_end = after_start + close_rel;
        let header = spec_text[after_start..header_end].trim();
        let self_closing = header.ends_with('/');
        let attrs = parse_metric_attrs(header.trim_end_matches('/').trim());
        let (body, next_offset) = if self_closing {
            ("", header_end + 1)
        } else if let Some(end_rel) = spec_text[header_end + 1..].find("</metric>") {
            let body_start = header_end + 1;
            let body_end = body_start + end_rel;
            (spec_text[body_start..body_end].trim(), body_end + "</metric>".len())
        } else {
            ("", header_end + 1)
        };
        let mut params = serde_json::Map::new();
        let mut constraints = Value::Null;
        for (key, value) in attrs.iter() {
            match key.as_str() {
                "id" | "tag" | "name" | "kind" | "domain" | "op" | "input" | "inputs" | "output" | "dtype" | "unit" | "cache" | "proof" | "if" | "goal" | "description" | "formula" | "algorithm" | "weight" => {}
                "constraints" => {
                    constraints = parse_metric_attr_value(value);
                }
                "params" => {
                    if let Value::Object(obj) = parse_metric_attr_value(value) {
                        params.extend(obj);
                    } else {
                        params.insert(key.clone(), json!(value));
                    }
                }
                _ => {
                    params.insert(key.clone(), parse_metric_attr_value(value));
                }
            }
        }
        let inputs = attrs
            .get("inputs")
            .or_else(|| attrs.get("input"))
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.push(ProgramMetricSpec {
            id: attrs.get("id").cloned(),
            tag: attrs
                .get("tag")
                .or_else(|| attrs.get("name"))
                .or_else(|| attrs.get("id"))
                .cloned()
                .unwrap_or_default(),
            name: attrs.get("name").cloned(),
            kind: attrs.get("kind").cloned(),
            domain: attrs.get("domain").cloned(),
            op: attrs.get("op").cloned(),
            inputs,
            output: attrs.get("output").cloned(),
            dtype: attrs.get("dtype").cloned(),
            params: Value::Object(params),
            constraints,
            unit: attrs.get("unit").cloned(),
            cache: attrs.get("cache").cloned(),
            proof: attrs.get("proof").cloned(),
            condition: attrs.get("if").cloned(),
            goal: attrs.get("goal").cloned(),
            description: attrs
                .get("description")
                .cloned()
                .or_else(|| (!body.is_empty()).then(|| body.to_string())),
            formula: attrs.get("formula").cloned(),
            algorithm: attrs.get("algorithm").cloned(),
            weight: attrs.get("weight").and_then(|v| v.parse::<f64>().ok()),
        });
        if out.len() > MCP_PROGRAM_METRICS_MAX {
            return Err(format!("too many <metric> tags; max {}", MCP_PROGRAM_METRICS_MAX));
        }
        offset = next_offset;
    }
    Ok(out)
}

fn extract_visual_view_tags(spec_text: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(rel) = spec_text[offset..].find("<view") {
        let start = offset + rel;
        let after_start = start + "<view".len();
        if spec_text[after_start..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
            offset = after_start;
            continue;
        }
        let Some(close_rel) = spec_text[after_start..].find('>') else {
            return Err("unterminated <view ...> tag".to_string());
        };
        let header_end = after_start + close_rel;
        let header = spec_text[after_start..header_end].trim();
        let self_closing = header.ends_with('/');
        let attrs = parse_metric_attrs(header.trim_end_matches('/').trim());
        let (body, next_offset) = if self_closing {
            ("", header_end + 1)
        } else if let Some(end_rel) = spec_text[header_end + 1..].find("</view>") {
            let body_start = header_end + 1;
            let body_end = body_start + end_rel;
            (spec_text[body_start..body_end].trim(), body_end + "</view>".len())
        } else {
            ("", header_end + 1)
        };
        let mut obj = serde_json::Map::new();
        let mut axes = serde_json::Map::new();
        for (key, value) in attrs {
            let parsed = parse_metric_attr_value(&value);
            match key.as_str() {
                "x" | "x_metric" | "x_axis" => {
                    axes.insert("x".to_string(), parsed);
                }
                "y" | "y_metric" | "y_axis" => {
                    axes.insert("y".to_string(), parsed);
                }
                "z" | "z_metric" | "z_axis" => {
                    axes.insert("z".to_string(), parsed);
                }
                "axes" => {
                    if let Value::Object(map) = parsed {
                        for (axis, axis_value) in map {
                            axes.insert(axis, axis_value);
                        }
                    } else {
                        obj.insert(key, parsed);
                    }
                }
                _ => {
                    obj.insert(key, parsed);
                }
            }
        }
        if !axes.is_empty() {
            obj.insert("axes".to_string(), Value::Object(axes));
        }
        if !body.is_empty() && !obj.contains_key("description") {
            obj.insert("description".to_string(), json!(body));
        }
        out.push(Value::Object(obj));
        if out.len() > 64 {
            return Err("too many <view> tags; max 64".to_string());
        }
        offset = next_offset;
    }
    Ok(out)
}

fn parse_metric_attrs(input: &str) -> HashMap<String, String> {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut attrs = HashMap::new();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-' | b':'))
        {
            i += 1;
        }
        if i == key_start {
            i += 1;
            continue;
        }
        let key = input[key_start..i].trim().to_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            attrs.insert(key, "true".to_string());
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            attrs.insert(key, String::new());
            break;
        }
        let value = if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            let value = input[value_start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            input[value_start..i].to_string()
        };
        attrs.insert(key, value);
    }
    attrs
}

fn parse_metric_attr_value(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Value::String(String::new());
    }
    if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
        return json;
    }
    if let Ok(num) = trimmed.parse::<i64>() {
        return json!(num);
    }
    if let Ok(num) = trimmed.parse::<f64>() {
        if num.is_finite() {
            return json!(num);
        }
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "true" => json!(true),
        "false" => json!(false),
        _ => json!(trimmed),
    }
}

fn bounded_clean_text(value: &str, field: &str, max_bytes: usize) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} cannot be empty"));
    }
    if trimmed.len() > max_bytes {
        return Err(format!("{field} too large: {} > {max_bytes} bytes", trimmed.len()));
    }
    Ok(trimmed.to_string())
}

/// Hard cap on instrument/lens display titles. LLMs receive an explicit
/// error if they exceed it â€” forces concise labels like "VWAP detune"
/// or "K-mer scan" instead of paragraphs. Counts UTF-8 chars, not bytes.
const PROGRAM_TITLE_MAX_CHARS: usize = 24;

fn bounded_clean_program_title(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("title cannot be empty".to_string());
    }
    let char_count = trimmed.chars().count();
    if char_count > PROGRAM_TITLE_MAX_CHARS {
        return Err(format!(
            "title too long: {char_count} chars > {PROGRAM_TITLE_MAX_CHARS} max. Use a short label (e.g. 'VWAP detune', 'RSI long', 'K-mer scan')."
        ));
    }
    Ok(trimmed.to_string())
}

/// Hard cap on human-readable metric/node names. Atlas cubes are small;
/// long names overflow. Forces concise labels like 'RSI 14', 'VWAP'.
const METRIC_NAME_MAX_CHARS: usize = 18;

fn bounded_clean_metric_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("metric name cannot be empty".to_string());
    }
    let char_count = trimmed.chars().count();
    if char_count > METRIC_NAME_MAX_CHARS {
        return Err(format!(
            "metric name too long: {char_count} chars > {METRIC_NAME_MAX_CHARS} max. Use a short label (e.g. 'RSI 14', 'VWAP', 'EMA delta')."
        ));
    }
    Ok(trimmed.to_string())
}

/// Hard cap on metric/node tags. Forces minimalist labels like `rsi_14`,
/// `vwap`, `ema_delta` instead of long descriptive sentences. The atlas
/// UI shows these as small cubes â€” long tags blow up the layout.
const METRIC_TAG_MAX_CHARS: usize = 16;

fn sanitize_metric_tag(value: &str, idx: usize) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '_' | '-' | ' ' | '.' | '/') && !out.ends_with('_') {
            out.push('_');
        }
        if out.len() >= METRIC_TAG_MAX_CHARS {
            break;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        format!("metric_{}", idx + 1)
    } else {
        out
    }
}

fn sanitize_program_value(value: Value) -> Value {
    let Value::Object(mut obj) = value else {
        return value;
    };
    obj.remove("spec_text");
    obj.insert("source_content_included".to_string(), json!(false));
    obj.insert("spec_text_full_content_included".to_string(), json!(false));
    Value::Object(obj)
}

fn summarize_program_value(value: Value) -> Value {
    let program_hash = value.get("program_hash").cloned().unwrap_or(Value::Null);
    let canonical = value.get("canonical").cloned().unwrap_or(Value::Null);
    let metrics = canonical
        .get("metrics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let program_kind = canonical
        .get("program_kind")
        .cloned()
        .or_else(|| value.get("program_kind").cloned())
        .unwrap_or_else(|| json!("compute_program"));
    let visual_views = canonical
        .pointer("/visual_program/views")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let metric_contract = canonical
        .get("metric_contract")
        .cloned()
        .unwrap_or_else(|| metric_math_contract(&metrics));
    let program_compiler = canonical
        .get("program_compiler")
        .cloned()
        .unwrap_or_else(|| compile_program_metric_routes(
            &metrics,
            program_kind.as_str().unwrap_or("compute_program"),
            canonical.get("domain").and_then(Value::as_str),
            canonical.get("goal").and_then(Value::as_str).unwrap_or(""),
            canonical.get("metric_graph").unwrap_or(&Value::Null),
        ));
    json!({
        "program_id": value.get("program_id").cloned().unwrap_or(Value::Null),
        "program_hash": program_hash,
        "program_kind": program_kind,
        "title": canonical.get("title").cloned().unwrap_or(Value::Null),
        "domain": canonical.get("domain").cloned().unwrap_or(Value::Null),
        "intent": canonical.get("intent").cloned().unwrap_or(Value::Null),
        "goal": canonical.get("goal").cloned().unwrap_or(Value::Null),
        "template": canonical.get("template").cloned().unwrap_or(Value::Null),
        "metric_count": metrics.len(),
        "metric_contract": metric_contract,
        "program_compiler": program_compiler,
        "visual_view_count": visual_views.len(),
        "visual_view_types": visual_views
            .iter()
            .filter_map(|view| view.get("type").and_then(Value::as_str))
            .take(24)
            .collect::<Vec<_>>(),
        "execution_readiness": value.get("execution_readiness").cloned().unwrap_or_else(|| program_execution_readiness(&metrics, value.get("program_hash").and_then(Value::as_str))),
        "metric_tags": metrics
            .iter()
            .filter_map(|metric| metric.get("tag").and_then(Value::as_str))
            .take(24)
            .collect::<Vec<_>>(),
        "status": value.get("status").cloned().unwrap_or(Value::Null),
        "created_ms": value.get("created_ms").cloned().unwrap_or(Value::Null),
        "content_addressed": true,
        "source_content_included": false
    })
}

fn programs_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FORGE_PROGRAMS_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        return Ok(PathBuf::from(path).join("programs"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata)
            .join("com.forge.ui")
            .join("forge-store")
            .join("programs"));
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(".forge-store").join("programs"))
        .map_err(|e| format!("resolve programs dir: {e}"))
}

fn program_manifest_path(program_hash: &str) -> Result<PathBuf, String> {
    validate_content_hash(program_hash, "program_hash")?;
    Ok(programs_dir()?.join(format!("{program_hash}.json")))
}

fn validate_content_hash(value: &str, field: &str) -> Result<(), String> {
    if value.len() < 8
        || value.len() > 64
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(format!("invalid {field}; expected hex content hash"));
    }
    Ok(())
}

fn persist_json_pretty(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("encode json '{}': {e}", path.display()))?;
    fs::write(path, bytes).map_err(|e| format!("write json '{}': {e}", path.display()))
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("read json '{}': {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("decode json '{}': {e}", path.display()))
}

fn forge_fbc_runtime_snapshot_mcp(args: &Value) -> Result<Value, String> {
    let store_path = forge_store_dir()?;
    let ownership_path = forge_workspace_dir_mcp()
        .join("examples")
        .join("forge_tauri_ui")
        .join("ui")
        .join("SECTION_OWNERSHIP.json");
    let ownership_json = fs::read_to_string(&ownership_path)
        .map_err(|e| format!("read SECTION_OWNERSHIP '{}': {e}", ownership_path.display()))?;
    let registry = parse_app_section_registry_v0(&ownership_json)
        .map_err(|e| format!("parse app FBC registry: {e:?}"))?;
    let tool_registry_path = forge_workspace_dir_mcp()
        .join("examples")
        .join("forge_tauri_ui")
        .join("source-registry")
        .join("real-estate-tool-cells.json");
    let tool_registry_json = fs::read_to_string(&tool_registry_path)
        .map_err(|e| format!("read real-estate tool cells '{}': {e}", tool_registry_path.display()))?;
    let tool_registry = parse_tool_cell_registry_v0(&tool_registry_json)
        .map_err(|e| format!("parse real-estate tool cell registry: {e:?}"))?;
    let tool_graph_path = store_path
        .join("real-estate-harvester")
        .join("data")
        .join("living_dataflow_graph.jsonl");
    let tool_graph_jsonl = if tool_graph_path.exists() {
        fs::read(&tool_graph_path)
            .map_err(|e| format!("read real-estate living graph '{}': {e}", tool_graph_path.display()))?
    } else {
        Vec::new()
    };
    let mut config = ForgeVmConfig::default();
    config.backend = args
        .get("backend")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("auto")
        .to_string();
    let batch = execute_tool_cell_batch_groups(
        &[
            (&registry.cells, &registry.graph_jsonl),
            (&tool_registry.cells, &tool_graph_jsonl),
        ],
        &config,
    );
    let output_dir = store_path.join("fbc").join("app");
    let manifest_path = output_dir.join("app_fbc_registry_batch.json");
    if args
        .get("write_artifacts")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        fs::create_dir_all(&output_dir)
            .map_err(|e| format!("create FBC app output dir '{}': {e}", output_dir.display()))?;
        for record in &batch.records {
            let artifact = tool_cell_output_artifact_json(
                record,
                &batch.graph_hash,
                &registry.registry_hash,
                &batch.ledger_root_hash,
            );
            let path = output_dir.join(format!("{}.json", safe_fbc_artifact_name_mcp(&record.command)));
            fs::write(&path, format!("{artifact}\n"))
                .map_err(|e| format!("write FBC artifact '{}': {e}", path.display()))?;
        }
        fs::write(&manifest_path, format!("{}\n", batch.projection_json))
            .map_err(|e| format!("write FBC manifest '{}': {e}", manifest_path.display()))?;
    }
    let projection = serde_json::from_str::<Value>(&batch.projection_json)
        .unwrap_or_else(|_| json!({ "kind": "forge_fbc_projection_parse_error" }));
    Ok(json!({
        "kind": "forge_fbc_app_runtime_snapshot_v0",
        "source": "forge_mcp_fbc_runtime",
        "job_id": format!("fbc-app-{}", now_ms()),
        "registry_hash": registry.registry_hash,
        "tool_registry_hash": tool_registry.registry_hash,
        "graph_hash": batch.graph_hash,
        "section_count": registry.section_count,
        "sensitive_command_count": registry.sensitive_command_count,
        "tool_cell_registry_count": tool_registry.cells.len(),
        "cell_count": batch.tool_count,
        "ok_count": batch.ok_count,
        "denied_count": batch.denied_count,
        "ledger_root_hash": batch.ledger_root_hash,
        "manifest_path": manifest_path.display().to_string(),
        "output_dir": output_dir.display().to_string(),
        "raw_input_returned": false,
        "capability_only": true,
        "projection": projection
    }))
}

fn forge_workspace_dir_mcp() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(3)
        .map(PathBuf::from)
        .unwrap_or(manifest_dir)
}

fn safe_fbc_artifact_name_mcp(command: &str) -> String {
    let mut out = command
        .trim_matches('/')
        .trim_end_matches('_')
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect::<String>();
    if out.is_empty() {
        out = "fbc_artifact".to_string();
    }
    out
}

fn forge_store_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("FORGE_JOBS_DIR") {
        if let Some(parent) = PathBuf::from(path).parent() {
            return Ok(parent.to_path_buf());
        }
    }
    if let Some(path) = std::env::var_os("FORGE_PROGRAMS_DIR") {
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

fn mcp_tool_response(value: Value) -> Result<Value, String> {
    let workflow_guidance = workflow_guidance(&value);
    let payload = json!({
        "server_identity": forge_server_identity(),
        "data": value,
        "workflow_guidance": workflow_guidance,
        "agent_instructions": mcp_agent_instructions(),
        "tool_selection_policy": forge_tool_selection_policy(),
        "token_safety": token_safety(),
        "next_actions": default_next_actions()
    });
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("encode MCP tool response: {e}"))?;
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    }))
}

fn mcp_intent_tool_response(value: Value) -> Result<Value, String> {
    let payload = json!({
        "server_identity": {
            "display_name": FORGE_DISPLAY_NAME,
            "canonical_name": FORGE_DISPLAY_NAME,
            "technical_slug": FORGE_TECHNICAL_SLUG
        },
        "surface": "forge_intent_compact_v0",
        "raw_data_returned": false,
        "data": value,
        "token_safety": {
            "compact_response": true,
            "source_content_included": false,
            "artifact_content_included": false,
            "raw_input_not_returned": true,
            "policy": "Intent facade responses return compact projections, hashes and bounded execution evidence; broad workflow doctrine stays in about/tool policy."
        }
    });
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("encode MCP intent response: {e}"))?;
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    }))
}

fn mcp_compact_tool_response(value: Value) -> Result<Value, String> {
    let payload = json!({
        "server_identity": {
            "display_name": FORGE_DISPLAY_NAME,
            "canonical_name": FORGE_DISPLAY_NAME
        },
        "data": value,
        "token_safety": {
            "compact_response": true,
            "source_content_included": false,
            "artifact_content_included": false,
            "point_cloud_content_included": false,
            "raw_points_returned": false,
            "policy": "Forge performed the heavy 3D analysis locally and returned only compact diagnostics."
        }
    });
    let text =
        serde_json::to_string(&payload).map_err(|e| format!("encode compact MCP response: {e}"))?;
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    }))
}

fn mcp_error_response(message: String) -> Value {
    let diagnostic = error_diagnostic(&message);
    let payload = json!({
        "server_identity": forge_server_identity(),
        "error": message,
        "why_failed": diagnostic.get("why_failed").cloned().unwrap_or(Value::Null),
        "safe_next_call": diagnostic.get("safe_next_call").cloned().unwrap_or(Value::Null),
        "suggested_retry": diagnostic.get("suggested_retry").cloned().unwrap_or(Value::Null),
        "do_not_read_source": true,
        "do_not": [
            "do not inspect Forge source code to run or debug a user compute job",
            "do not shell-read raw CSV/log/source files after a Forge MCP error",
            "use jobs/read/logs/artifacts or retry the safe_next_call instead"
        ],
        "agent_instructions": mcp_agent_instructions(),
        "token_safety": token_safety(),
        "next_actions": default_next_actions()
    });
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&payload)
                .unwrap_or_else(|_| "{\"error\":\"MCP error\"}".to_string())
        }],
        "isError": true
    })
}

fn forge_server_identity() -> Value {
    json!({
        "display_name": FORGE_DISPLAY_NAME,
        "canonical_name": FORGE_DISPLAY_NAME,
        "technical_slug": FORGE_TECHNICAL_SLUG,
        "agent_display_rule": "When listing connected MCP servers or tools, call this server Forge with a capital F. Do not present the lowercase technical slug as the product name."
    })
}

fn error_diagnostic(message: &str) -> Value {
    let lower = message.to_lowercase();
    if lower.contains("not pending") {
        json!({
            "why_failed": "The selected job is no longer pending, so Forge will not claim it as a new calculation.",
            "safe_next_call": "jobs {}",
            "suggested_retry": "Use jobs {} to find a pending/running/completed session, then run { job_id:\"...\" } only for a pending job."
        })
    } else if lower.contains("csv_path") || lower.contains("csv") {
        json!({
            "why_failed": "Forge needs a CSV path or a pending upload that already contains one.",
            "safe_next_call": "run {}",
            "suggested_retry": "If a file was dropped into Forge, call run {} or pending {}; otherwise ask the user to upload/provide a file."
        })
    } else if lower.contains("program") {
        json!({
            "why_failed": "Forge could not resolve the requested reusable program.",
            "safe_next_call": "capabilities { query:\"...\" }",
            "suggested_retry": "Use capabilities to pick a template or create { ... } to define a reusable Metric DSL program."
        })
    } else if lower.contains("job") {
        json!({
            "why_failed": "Forge could not resolve the requested job id or job state.",
            "safe_next_call": "jobs {}",
            "suggested_retry": "Use jobs {} or pending {} to retrieve valid job ids, then call read/logs/run with a returned id."
        })
    } else {
        json!({
            "why_failed": "Forge rejected the MCP call before returning raw data.",
            "safe_next_call": "about {}",
            "suggested_retry": "Use about {} for the workflow doctrine, or run { intent:\"...\", inputs:[...], plan_only:true } for a safe compute plan."
        })
    }
}

fn token_safety() -> Value {
    json!({
        "raw_input_not_returned": true,
        "csv_included": false,
        "source_content_included": false,
        "full_log_included": false,
        "artifact_content_included": false,
        "use_forge_instead_of_raw_file_access": true,
        "agent_must_not_shell_read_user_inputs": true,
        "agent_must_not_debug_forge_source_for_user_jobs": true,
        "manifest_is_sanitized": true,
        "large_values_are_references": true,
        "default_log_tail_bytes": MCP_LOG_TAIL_DEFAULT_BYTES,
        "max_log_tail_bytes": MCP_LOG_TAIL_MAX_BYTES,
        "default_doc_preview_bytes": MCP_DOC_PREVIEW_DEFAULT_BYTES,
        "max_doc_preview_bytes": MCP_DOC_PREVIEW_MAX_BYTES,
        "max_list_items": MCP_LIST_LIMIT_MAX,
        "token_economy": {
            "how_it_saves_tokens": "Forge sends job ids, file hashes, artifact references, cursor-based log tails and bounded previews instead of full CSVs, logs or artifacts.",
            "tokens_saved_estimate": "response-specific estimates use byte ranges when Forge knows file sizes; otherwise assume large files/logs stay on disk and only compact references enter the LLM context",
            "content_addressed": true,
            "recompute_avoidance": "Inputs and computed artifacts are addressed by content hash, so identical work can be reused instead of recomputed.",
            "example_1_5mb_ascii_csv_estimated_tokens": {
                "source_bytes": 1529681,
                "low": 382421,
                "typical": 509894,
                "high": 764841,
                "note": "Exact tokens depend on the agent model tokenizer; numeric CSV often tokenizes less efficiently than prose."
            }
        },
        "raw_file_access_policy": "For user data and Forge jobs, agents should use jobs/read/logs/run instead of shell file reads. Source-code inspection is only appropriate when the user asks to develop/debug Forge itself.",
        "read_vs_forge_thresholds": {
            "read_ok_under": "64 KB for a tiny targeted inspection, especially normal repo code editing/review",
            "prefer_forge_at": ">256 KB, >1,000 rows/lines, multiple files, CSV/Excel/PDF/log/dataset, repeated calculations, metrics, simulations or document-heavy work",
            "forge_default_at": ">1 MB, >10,000 rows/lines, full logs, artifact/proof/hash workflows, 3D mappings, search/backtest/optimization/simulation",
            "fast_path": "run { intent, inputs, plan_only:true }"
        },
        "policy": "MCP responses return indexes, summaries, cursors and file references by default. CSVs, full logs and heavy artifacts stay on disk. The preview tool returns only a bounded source sample."
    })
}

fn mcp_agent_instructions() -> Value {
    json!({
        "display_name_rule": "Use the canonical product name Forge with a capital F. The lowercase 'forge' is only a technical MCP slug.",
        "do_not_use_shell_for_forge_inputs": true,
        "do_not_read_csv_or_source_files_directly": true,
        "do_not_inspect_forge_source_to_run_user_jobs": true,
        "automatic_triggers": [
            "user data >256 KB, >1,000 rows/lines or many files: prefer Forge before Read/shell",
            "user data >1 MB, >10,000 rows/lines, full logs or artifact/proof workflows: Forge should be the default path",
            "CSV/Excel/PDF/log/source/document/data parsing likely to consume significant context",
            "task requires loops, comparisons, simulations, optimization, backtests, search, metrics or repeated calculations",
            "domain is finance, code, documents, biology, chemistry, medicine, mathematics, engineering, aerospace, energy, industry, security or scientific data",
            "user asks for custom metrics/programs, indicators, anomaly detection, scoring, benchmarking or artifact generation",
            "result should be content-addressed, reproducible, downloadable, hash-verifiable or delivered as an artifact such as 3D .ply"
        ],
        "default_workflow": "Fast path: /program_ with plan_only=true -> /program_ -> logs by cursor -> read artifacts/proofs. Use /metric first only when the domain/template/operator is unclear.",
        "use_forge_before_manual_compute_when": [
            "input is large, repetitive, expensive, scientific, numerical, document-heavy or needs proof",
            "the agent is tempted to parse a CSV/log/source/document directly above the thresholds",
            "the user asks for custom calculations, metrics, indicators, detectors, simulations or analyses over data"
        ],
        "use_these_tools_instead": {
            "discover": "/metric",
            "create_program": "/create_",
            "launch_compute": "/program_",
            "sessions": "sessions",
            "session_status": "jobs",
            "documents": "documents",
            "3d_mapping": "mapping",
            "3d_metric_catalog": "/metric",
            "3d_modeling": "mapping_model",
            "visual_program": "/visualprogram_",
            "3d_mapping_analysis": "mapping_analysis",
            "profile_settings": "profile",
            "atlas": "atlas",
            "brain_memory": "brain_recall, brain_commit, brain_compare, brain_sleep, brain_explain",
            "summaries_previews_artifacts": "read",
            "logs": "logs",
            "cancel": "cancel"
        },
        "if_no_exact_capability_exists": "Use /program_ with plan_only=true for a first plan; use /metric only if the domain/template is unclear; then /create_ a reusable Metric DSL program instead of inventing a new visible tool.",
        "if_no_input_is_provided": "Ask the user whether they have a file/dataset/artifact/pending job or want free/synthetic/no-input mode.",
        "reason": "Forge MCP is the compute and document boundary. Direct shell/file reads can fail for normal users and can waste LLM context tokens. Forge returns content-addressed references and hashes instead."
    })
}

fn forge_tool_selection_policy() -> Value {
    let visible_tools = visible_tool_names();
    let current_visible_tool_count = visible_tools.len();
    json!({
        "status": if compact_mcp_surface_enabled() { "compact_cutover_candidate" } else { "transitional_broad_catalog" },
        "surface_contract": FORGE_MCP_SURFACE_CONTRACT,
        "compact_cutover_readiness": compact_cutover_readiness(),
        "target_visible_tool_budget": 4,
        "target_visible_tools": ["forge.search", "forge.execute", "forge.read_projection", "forge.cancel"],
        "current_visible_tool_budget": current_visible_tool_count,
        "visible_tools": visible_tools,
        "tool_annotations": "All visible tools expose MCP annotations for read-only, destructive, idempotent and open-world planning hints.",
        "via_negativa_audit": mcp_via_negativa_audit(current_visible_tool_count),
        "public_command_language": {
            "capabilities": "/metric",
            "program_compile_validate_route": "/metric",
            "create": "/create_",
            "run": "/program_",
            "mapping_metrics": "/metric",
            "visual_program": "/visualprogram_",
            "geonode": "/geo",
            "minigeonode": "/minigeo",
            "strategy": "/strategy_",
            "indicator": "/indicator",
            "alert": "/alert_",
            "transcript": "/transcript_"
        },
        "routing_rules": [
            { "intent": "before reading user data >256 KB, >1,000 rows/lines, multi-file or doing repeated calculations", "tool": "/program_", "arguments": { "plan_only": true } },
            { "intent": "pending UI upload is ready", "tool": "/program_", "arguments": { "job_id": "..." } },
            { "intent": "what is Forge / when to use it", "tool": "about" },
            { "intent": "choose a domain, capability, template, metric or next call", "tool": "/metric" },
            { "intent": "plan expensive or unfamiliar compute without side effects", "tool": "/program_", "arguments": { "plan_only": true } },
            { "intent": "define a custom reusable compute program", "tool": "/create_" },
            { "intent": "execute pending upload, program, capability or natural-language intent", "tool": "/program_" },
            { "intent": "find recent/pending/completed sessions", "tool": "sessions" },
            { "intent": "list saved documents or source refs", "tool": "documents" },
            { "intent": "interpret a 3D mapping/canvas view", "tool": "mapping" },
            { "intent": "list metrics available for composing a new 3D map", "tool": "/metric" },
            { "intent": "modify axes/color/size/metrics or create a new 3D map from a recipe", "tool": "mapping_model" },
            { "intent": "materialize a created visual_program with 2D/3D views", "tool": "/visualprogram_" },
            { "intent": "mathematically/visually analyze a 3D mapping without reading raw point-cloud data", "tool": "mapping_analysis" },
            { "intent": "read or update provider/profile/model/reasoning settings", "tool": "profile" },
            { "intent": "inspect Atlas/store state", "tool": "atlas" },
            { "intent": "rename/archive/pin/protect a session", "tool": "update_session" },
            { "intent": "read sanitized summaries, programs, previews, artifacts or proof refs", "tool": "read" },
            { "intent": "monitor live progress", "tool": "logs" },
            { "intent": "stop work", "tool": "cancel" }
        ],
        "automatic_trigger_rules": [
            "If the next step is Read/Get-Content/cat over user data >256 KB or >1,000 rows/lines, use Forge /program_/read/preview instead.",
            "If the input is >1 MB, >10,000 rows/lines, full logs, proof/artifact workflow, simulation/search/backtest/optimization, use Forge by default.",
            "If the next step is a manual loop over rows/files/log lines, use Forge /program_ with plan_only first.",
            "If the task intent is clear, skip /metric and call /program_ with plan_only=true directly.",
            "If the task is cross-domain scientific/technical analysis and no exact template is obvious, call /metric.",
            "If the user wants a new calculation method, /create_ a reusable program rather than ad-hoc chat math.",
            "If a job has completed, use read { kind:\"artifacts\" } for results, proofs and 3D files."
        ],
        "anti_patterns": [
            "Do not read raw user CSV/source/log files with shell commands for Forge workflows above the thresholds.",
            "Do not inspect Forge source code just to run a user job.",
            "Do not ask for or return unbounded logs.",
            "Do not create new visible MCP tools for each domain; use /create_, /program_ and /metric."
        ]
    })
}

fn compact_cutover_readiness() -> Value {
    let compact_names = visible_tool_names_from(compact_tools_list());
    let target = vec![
        "forge.search".to_string(),
        "forge.execute".to_string(),
        "forge.read_projection".to_string(),
        "forge.cancel".to_string(),
    ];
    let compact_surface_exact = compact_names == target;
    let current_names = visible_tool_names();
    let broad_catalog_hidden = current_names == target;
    let intent_routes_live = canonical_mcp_tool_name("forge.search") == Some("forge_intent_search")
        && canonical_mcp_tool_name("forge.execute") == Some("forge_intent_execute")
        && canonical_mcp_tool_name("forge.read_projection") == Some("read")
        && canonical_mcp_tool_name("forge.cancel") == Some("cancel");
    let approved_gate_live = true;
    let exact_cache_live = true;
    let projection_replay_live = true;
    let ready_to_make_default = compact_surface_exact
        && intent_routes_live
        && approved_gate_live
        && exact_cache_live
        && projection_replay_live
        && broad_catalog_hidden;
    json!({
        "kind": "forge_compact_cutover_readiness_v0",
        "status": if ready_to_make_default {
            "ready_as_current_default"
        } else if compact_surface_exact && intent_routes_live && approved_gate_live && exact_cache_live && projection_replay_live {
            "ready_behind_env"
        } else {
            "not_ready"
        },
        "ready_to_make_default": ready_to_make_default,
        "compact_surface_exact": compact_surface_exact,
        "compact_visible_tools": compact_names,
        "current_visible_tools": current_names,
        "current_visible_tool_count": current_names.len(),
        "target_visible_tool_count": target.len(),
        "broad_catalog_hidden": broad_catalog_hidden,
        "intent_routes_live": intent_routes_live,
        "approved_side_effect_gate_live": approved_gate_live,
        "exact_intent_cache_live": exact_cache_live,
        "projection_replay_live": projection_replay_live,
        "default_surface": "compact",
        "legacy_escape_hatches": ["FORGE_MCP_SURFACE=broad", "FORGE_MCP_LEGACY_SURFACE=1", "FORGE_MCP_BROAD_SURFACE=1"],
        "remaining_cutover_blocker": if broad_catalog_hidden {
            Value::Null
        } else {
            json!("default tools/list still exposes the transitional broad MCP catalog")
        },
        "raw_data_returned": false
    })
}

fn mcp_via_negativa_audit(current_visible_tool_count: usize) -> Value {
    let alias_count: usize = MCP_TOOL_ALIASES.iter().map(|(_, aliases)| aliases.len()).sum();
    let internal_route_count = MCP_INTERNAL_TOOL_ROUTES.len();
    json!({
        "wall": "context_size_latency_proof_quality",
        "target": "collapse broad MCP tool selection into a compact intent facade",
        "current_visible_tool_count": current_visible_tool_count,
        "target_visible_tool_count": 4,
        "visible_tools_to_remove_from_default_surface": current_visible_tool_count.saturating_sub(4),
        "canonical_alias_groups": MCP_TOOL_ALIASES.len(),
        "handled_aliases": alias_count,
        "internal_route_count": internal_route_count,
        "first_fusion_candidates": [
            "capabilities + program_compile_validate_route + mapping_metrics -> forge.search",
            "create + run + visual_program + mapping_model + mapping_analysis -> forge.execute",
            "jobs + sessions + documents + atlas + read + logs -> forge.read_projection",
            "brain_recall + brain_commit + brain_compare + brain_sleep + brain_explain -> ForgeSlash recall/commit/explain verbs"
        ],
        "facade_aliases_live_but_not_visible": {
            "forge.search": "forge_intent_search",
            "forge.execute": "forge_intent_execute",
            "forge.read_projection": "read",
            "forge.cancel": "cancel"
        },
        "do_not_add": [
            "new visible MCP tools for domains",
            "parallel UI-only intent runners",
            "forwarding-only wrappers around existing MCP calls"
        ],
        "promotion_rule": "A direct MCP tool survives the default surface only with a measured proof that it is simpler, safer or faster than the intent/code path."
    })
}

fn forge_intent_search(query: &str, limit: usize) -> Value {
    let store_path = forge_store_dir().ok();
    forge_intent_search_with_store(query, limit, store_path.as_deref())
}

fn forge_intent_search_with_store(query: &str, limit: usize, store_path: Option<&Path>) -> Value {
    let needle = query.trim().to_ascii_lowercase();
    let limit = limit.clamp(1, 8);
    let mut entries = forge_intent_search_index();
    if let Some(store_path) = store_path {
        entries.extend(forge_intent_projection_search_entries(store_path));
    }
    let mut scored: Vec<(usize, Value)> = entries
        .into_iter()
        .map(|entry| (intent_search_score(&entry, &needle), entry))
        .filter(|(score, _)| needle.is_empty() || *score > 0)
        .collect();
    if scored.is_empty() {
        scored = forge_intent_search_fallback_entries()
            .into_iter()
            .map(|entry| (0, entry))
            .collect();
    }
    scored.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left["id"].as_str().cmp(&right["id"].as_str()))
    });
    let results: Vec<Value> = scored
        .into_iter()
        .take(limit)
        .map(|(score, mut entry)| {
            let next_call = forge_intent_search_next_call(&entry, query);
            if let Value::Object(ref mut obj) = entry {
                obj.insert("score".to_string(), json!(score));
                obj.insert("next_call".to_string(), next_call);
            }
            entry
        })
        .collect();
    let recommended = results.first().cloned().unwrap_or(Value::Null);
    let next_call = recommended
        .get("next_call")
        .cloned()
        .unwrap_or_else(|| json!({
            "tool": "forge.search",
            "arguments": { "query": query, "limit": limit },
            "reason": "search returned no route; retry discovery"
        }));
    json!({
        "kind": "forge_search_result_v1",
        "query": query,
        "mode": "compact_intent_router",
        "surface": "forge.search_default",
        "full_schemas_returned": false,
        "includes_projection_index": store_path.is_some(),
        "result_count": results.len(),
        "recommended": recommended,
        "next_call": next_call,
        "route_plan": forge_intent_search_route_plan(&results, query),
        "raw_data_returned": false,
        "results": results
    })
}

fn forge_intent_search_fallback_entries() -> Vec<Value> {
    forge_intent_search_index()
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.get("id").and_then(Value::as_str),
                Some("discover" | "execute" | "projection")
            )
        })
        .collect()
}

fn forge_intent_search_next_call(entry: &Value, query: &str) -> Value {
    let target = entry
        .get("target_tool")
        .and_then(Value::as_str)
        .unwrap_or("forge.search");
    let example = entry.get("example").and_then(Value::as_str).unwrap_or("");
    let reason = entry.get("summary").and_then(Value::as_str).unwrap_or("");
    match target {
        "forge.execute" => json!({
            "tool": "forge.execute",
            "arguments": {
                "source": if example.trim_start().starts_with("/forge") {
                    example
                } else {
                    "/forge plan intent=\"route this request through Forge\" input=@latest"
                },
                "max_bytes": 4096
            },
            "reason": reason,
            "approval_required_for_side_effects": true
        }),
        "forge.read_projection" => {
            if let Some(hash) = entry
                .pointer("/projection/execution_hash")
                .and_then(Value::as_str)
                .or_else(|| entry.pointer("/projection/projection_hash").and_then(Value::as_str))
            {
                json!({
                    "tool": "forge.read_projection",
                    "arguments": { "execution_hash": hash, "max_bytes": 4096 },
                    "reason": reason
                })
            } else {
                json!({
                    "tool": "forge.read_projection",
                    "arguments": { "limit": 8 },
                    "reason": reason
                })
            }
        }
        "forge.cancel" => json!({
            "tool": "forge.cancel",
            "arguments": { "job_id": "<job_id>", "reason": query },
            "reason": reason,
            "requires_job_id": true
        }),
        other if other == "forge.search" || other == "fbc_runtime" => json!({
            "tool": "forge.search",
            "arguments": { "query": query, "limit": 4 },
            "reason": reason,
            "follow_up": if other == "fbc_runtime" {
                json!({ "legacy_tool": "fbc_runtime", "note": "callable as a legacy/internal route, not part of the visible default schema" })
            } else {
                Value::Null
            }
        }),
        _ => json!({
            "tool": "forge.search",
            "arguments": { "query": query, "limit": 4 },
            "reason": reason
        }),
    }
}

fn forge_intent_search_route_plan(results: &[Value], query: &str) -> Value {
    let next = results
        .first()
        .and_then(|entry| entry.get("next_call"))
        .cloned()
        .unwrap_or_else(|| json!({
            "tool": "forge.search",
            "arguments": { "query": query, "limit": 4 }
        }));
    json!({
        "kind": "forge_search_route_plan_v1",
        "steps": [
            {
                "step": "execute_recommended_next_call",
                "call": next
            },
            {
                "step": "read_or_replay_projection",
                "call": {
                    "tool": "forge.read_projection",
                    "arguments": { "limit": 8 }
                }
            }
        ],
        "raw_data_returned": false
    })
}

fn forge_intent_projection_search_entries(store_path: &Path) -> Vec<Value> {
    let Ok(index) = forge_agent_runtime::direct_read_projection(
        store_path,
        &json!({ "list": true, "limit": 32 }),
    ) else {
        return Vec::new();
    };
    index
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .take(32)
                .filter_map(|entry| {
                    let projection_hash = entry.get("projection_hash").and_then(Value::as_str)?;
                    Some(json!({
                        "id": format!("projection:{projection_hash}"),
                        "target_tool": "forge.read_projection",
                        "verbs": ["project", "explain"],
                        "current_routes": ["forge.read_projection"],
                        "slash_aliases": ["forge.read_projection"],
                        "tags": ["projection", "execution_hash", "trace_hash", "intent_hash", "cache", "replay"],
                        "compact_signature": "forge.read_projection execution_hash=<hash> -> compact persisted intent projection",
                        "example": format!("forge.read_projection {{ execution_hash: \"{}\" }}", entry.get("execution_hash").and_then(Value::as_str).unwrap_or(projection_hash)),
                        "summary": "Persisted intent projection with compact hashes, promotion status and bounded execution evidence.",
                        "projection": entry
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn intent_search_score(entry: &Value, needle: &str) -> usize {
    if needle.is_empty() {
        return 1;
    }
    let mut score = 0usize;
    for term in needle.split_whitespace() {
        let haystack = serde_json::to_string(entry)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if haystack.contains(term) {
            score += 1;
        }
        if entry.get("id").and_then(Value::as_str).is_some_and(|id| id.contains(term)) {
            score += 2;
        }
    }
    score
}

fn forge_intent_search_index() -> Vec<Value> {
    vec![
        json!({
            "id": "discover",
            "target_tool": "forge.search",
            "verbs": ["recall", "plan", "explain"],
            "current_routes": ["capabilities", "program_compile_validate_route", "mapping_metrics", "atlas"],
            "slash_aliases": ["/metric"],
            "tags": ["capability", "template", "metric", "visual", "domain", "atlas", "reuse"],
            "compact_signature": "forge.search query=<intent|domain|capability> -> compact candidates + examples",
            "example": "/forge plan intent=\"find useful metrics for this file\" input=@latest",
            "summary": "Discover capabilities, reusable programs, metric routes and examples without exposing full MCP schemas."
        }),
        json!({
            "id": "execute",
            "target_tool": "forge.execute",
            "verbs": ["plan", "create", "run"],
            "current_routes": ["create", "run", "visual_program", "mapping_model", "mapping_analysis"],
            "slash_aliases": ["/create_", "/program_", "/visualprogram_"],
            "tags": ["run", "compute", "visual_program", "mapping", "program", "plan_only", "artifact"],
            "compact_signature": "forge.execute program=<ForgeSlash> -> validate + route + run + projection",
            "example": "/forge run input=@latest intent=\"create a compact 3D visual program\" plan_only=true",
            "summary": "Validate and execute compute or visual intents while keeping raw files outside the LLM context."
        }),
        json!({
            "id": "projection",
            "target_tool": "forge.read_projection",
            "verbs": ["project", "explain"],
            "current_routes": ["jobs", "sessions", "documents", "read", "logs", "atlas"],
            "slash_aliases": ["read", "logs"],
            "tags": ["job", "session", "document", "preview", "proof", "artifact", "logs", "hash"],
            "compact_signature": "forge.read_projection ref=<job|program|artifact> max_bytes=<n> -> bounded refs/previews",
            "example": "/forge project job_id=latest max_bytes=4096",
            "summary": "Read compact hashes, metrics, previews and proof/artifact refs instead of raw data."
        }),
        json!({
            "id": "fbc_runtime",
            "target_tool": "fbc_runtime",
            "verbs": ["verify", "project", "snapshot"],
            "current_routes": ["fbc_runtime"],
            "slash_aliases": ["/fbc_"],
            "tags": ["fbc", "kasm2", "bytecode", "verifier", "capability", "proof", "app_runtime"],
            "compact_signature": "/fbc_ backend=auto -> app sections + sensitive commands as verified FBC proof projection",
            "example": "fbc_runtime { backend: \"auto\", write_artifacts: true }",
            "summary": "Compile the whole app ownership surface into FBC/KASM2 v0 cells and return compact proofs without raw host access."
        }),
        json!({
            "id": "brain",
            "target_tool": "forge.execute",
            "verbs": ["recall", "commit", "explain"],
            "current_routes": ["brain_recall", "brain_commit", "brain_compare", "brain_sleep", "brain_explain"],
            "slash_aliases": ["brain commands"],
            "tags": ["memory", "brain", "semantic", "episodic", "procedural", "godel", "distill"],
            "compact_signature": "forge.execute program=<recall|commit|explain> -> scoped evidence-aware memory route",
            "example": "/forge recall scope=real_estate",
            "summary": "Use the evidence-aware Forge brain without adding a parallel memory store."
        }),
        json!({
            "id": "geo",
            "target_tool": "forge.execute",
            "verbs": ["create", "commit"],
            "current_routes": ["geonode"],
            "slash_aliases": ["/geo", "/minigeo"],
            "tags": ["geo", "planet", "anchor", "coordinate", "atlas", "visual_program"],
            "compact_signature": "forge.execute program=<create geo anchor> -> Atlas GeoNode/MiniGeoNode",
            "example": "/forge create kind=geonode title=mars_anchor goal=\"anchor a named location\"",
            "summary": "Create reusable spatial anchors for visual programs and Atlas references."
        }),
        json!({
            "id": "profile",
            "target_tool": "forge.execute",
            "verbs": ["recall", "commit"],
            "current_routes": ["profile", "update_session"],
            "slash_aliases": ["profile", "settings"],
            "tags": ["profile", "provider", "model", "settings", "session", "metadata"],
            "compact_signature": "forge.execute program=<profile/session intent> -> safe settings/session update",
            "example": "/forge commit scope=profile kind=semantic observation=\"preferred model policy changed\"",
            "summary": "Route settings and safe session metadata changes through bounded intents."
        }),
        json!({
            "id": "cancel",
            "target_tool": "forge.cancel",
            "verbs": [],
            "current_routes": ["cancel"],
            "slash_aliases": ["cancel"],
            "tags": ["stop", "abort", "cancel", "job"],
            "compact_signature": "forge.cancel job_id=<id> reason=<text>",
            "example": "forge.cancel { job_id: \"...\", reason: \"user changed parameters\" }",
            "summary": "Stop running work safely without routing through a broad tool catalog."
        }),
    ]
}

fn workflow_guidance(value: &Value) -> Value {
    if value.get("plan_only").and_then(Value::as_bool).unwrap_or(false) {
        let requires_choice = value
            .pointer("/input_policy/requires_user_choice")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let prompt = value
            .pointer("/input_policy/ask_user_prompt")
            .and_then(Value::as_str)
            .unwrap_or("Ask the user whether to use a file/dataset/artifact or free/synthetic mode.");
        return json!({
            "state": "planned_no_side_effects",
            "recommended_sequence": [
                if requires_choice { "ask_user_for_input_mode" } else { "confirm_or_launch" },
                "run the ready_to_launch_call when the user approves execution",
                "logs { job_id, cursor } while running",
                "read { job_id, kind:\"artifacts\" } when completed",
                "create { ...program_planner.suggested_program... } only if the user needs a reusable custom program"
            ],
            "fast_path": "If the intent is clear, this plan_only call is the whole planning step; do not call capabilities unless the plan says the domain/template is ambiguous.",
            "ask_user_prompt": if requires_choice { prompt } else { "" },
            "do_not": [
                "do not read the raw file yourself",
                "do not inspect Forge source to guess parameters",
                "do not calculate the planned metrics in the LLM context"
            ]
        });
    }

    if value.get("mode").and_then(Value::as_str) == Some("compact_gps")
        || value.get("mode").and_then(Value::as_str) == Some("guided_detail")
    {
        return json!({
            "state": "capability_guidance",
            "recommended_sequence": [
                "If the user intent is clear, call run { intent, inputs, plan_only:true } now.",
                "If the user wants a reusable/custom program, call create with program_planner.suggested_program.",
                "If the user has already uploaded a pending file, call jobs then run { job_id }."
            ],
            "do_not": [
                "do not dump the full catalogue unless explicitly requested",
                "do not add a new visible tool for this domain"
            ]
        });
    }

    if let Some(program) = value.get("program") {
        let program_hash = program
            .get("program_hash")
            .and_then(Value::as_str)
            .unwrap_or("");
        let readiness = program.get("execution_readiness").cloned().unwrap_or(Value::Null);
        let custom_count = readiness
            .get("custom_unresolved_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let missing_count = readiness
            .get("missing_op_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        return json!({
            "state": "program_ready",
            "recommended_sequence": [
                format!("run {{ program_hash:\"{program_hash}\", inputs:[{{ path:\"...\", role:\"data\" }}] }}"),
                "logs { job_id, cursor }",
                "read { job_id, kind:\"artifacts\" }"
            ],
            "readiness_summary": {
                "custom_unresolved_count": custom_count,
                "missing_op_count": missing_count,
                "agent_rule": if missing_count > 0 {
                    "Fix missing metric ops before run."
                } else if custom_count > 0 {
                    "Run may produce completed_with_unresolved_ops; do not present custom metrics as computed until an executor exists."
                } else {
                    "All metrics are runnable by the current builtin toolbox."
                }
            },
            "do_not": [
                "do not paste raw input content into create",
                "do not manually recompute metrics already represented by this program_hash"
            ]
        });
    }

    if let Some(job) = value.get("job") {
        let status = job.get("status").and_then(Value::as_str).unwrap_or("unknown");
        let job_id = job.get("job_id").and_then(Value::as_str).unwrap_or("");
        let sequence = match status {
            "pending" => vec![format!("run {{ job_id:\"{job_id}\" }}")],
            "running" => vec![format!("logs {{ job_id:\"{job_id}\", cursor:0 }}")],
            "completed" | "completed_with_unresolved_ops" | "completed_with_metric_errors" => {
                vec![format!("read {{ job_id:\"{job_id}\", kind:\"artifacts\" }}")]
            }
            "failed" => vec![
                format!("logs {{ job_id:\"{job_id}\", cursor:0 }}"),
                format!("run {{ job_id:\"{job_id}\", engine:\"threshold\" }}"),
            ],
            _ => vec![format!("read {{ job_id:\"{job_id}\" }}")],
        };
        return json!({
            "state": format!("job_{status}"),
            "recommended_sequence": sequence,
            "do_not": [
                "do not open the job manifest/log/source files directly",
                "do not read full logs; use logs cursor"
            ]
        });
    }

    if let Some(jobs) = value.get("jobs").and_then(Value::as_array) {
        let pending = jobs
            .iter()
            .filter(|job| job.get("status").and_then(Value::as_str) == Some("pending"))
            .count();
        return json!({
            "state": "job_list",
            "jobs_returned": jobs.len(),
            "pending_jobs_returned": pending,
            "recommended_sequence": if pending > 0 {
                if pending == 1 {
                    vec!["run {} to claim the only pending job"]
                } else {
                    vec!["run { job_id:\"...\" } to claim a specific pending job"]
                }
            } else {
                vec!["run { intent:\"...\", inputs:[...], plan_only:true } or ask the user to upload a file"]
            },
            "do_not": [
                "do not enumerate forge-store with shell",
                "do not read CSV/log paths from manifests directly"
            ]
        });
    }

    if value.get("next_cursor").is_some() && value.get("text").is_some() {
        let next_cursor = value.get("next_cursor").and_then(Value::as_u64).unwrap_or(0);
        let eof = value.get("eof").and_then(Value::as_bool).unwrap_or(false);
        return json!({
            "state": "log_tail",
            "recommended_sequence": if eof {
                vec!["read { job_id, kind:\"artifacts\" } if the job is complete".to_string()]
            } else {
                vec![format!("logs {{ job_id:\"...\", cursor:{next_cursor} }}")]
            },
            "do_not": [
                "do not request full logs",
                "do not open .log files with shell"
            ]
        });
    }

    json!({
        "state": "generic",
        "recommended_sequence": [
            "run { intent:\"...\", inputs:[...], plan_only:true } when the intent is clear",
            "capabilities { query:\"...\" } only when the domain/template is unclear",
            "create only if a reusable custom program is needed"
        ],
        "do_not": [
            "do not read raw Forge input files directly",
            "do not compute large workloads inside the LLM"
        ]
    })
}

fn default_next_actions() -> Value {
    json!([
        "Fast path: use run { intent, inputs, plan_only:true } before reading heavy user data or doing expensive compute.",
        "Use capabilities { query } only when the domain/template/operator is unclear.",
        "Use create { title, goal, metrics/spec_text } when the agent needs a custom reusable program.",
        "Use run { program/program_hash, inputs } or run { job_id } to execute.",
        "Use logs { job_id, cursor } for live progress and read { job_id, kind:\"artifacts\" } for compact results/proofs."
    ])
}

fn bounded_limit(raw: Option<&Value>, default: usize, max: usize) -> usize {
    raw.and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(default)
        .clamp(1, max)
}

fn run_alpha_strategy(args: AlphaStrategyArgs, client: &McpClientInfo) -> Result<StrategyJob, String> {
    let started_ms = now_ms();
    let csv_path = resolve_path(&args.csv_path)?;
    let csv_bytes = fs::read(&csv_path)
        .map_err(|e| format!("read csv '{}': {e}", csv_path.display()))?;
    let file_hash = quick_file_hash(&csv_bytes);
    let job_title = resolve_job_title(args.title.as_deref(), &csv_path);
    let job_id = format!("alpha-{started_ms}-{file_hash:016x}");
    let jobs_dir = jobs_dir()?;
    fs::create_dir_all(&jobs_dir).map_err(|e| format!("create jobs dir: {e}"))?;
    let log_path = jobs_dir.join(format!("{job_id}.log"));
    let mut log = Vec::<String>::new();
    internal_job_log(&job_id, format!("job started title={job_title}"));
    internal_job_log(
        &job_id,
        format!(
            "mcp client={} version={} model={} token_mode={}",
            client.name,
            client.version.as_deref().unwrap_or("unknown"),
            client.model.as_deref().unwrap_or("unknown"),
            client.token_mode
        ),
    );
    internal_job_log(&job_id, format!("csv={} bytes={}", csv_path.display(), csv_bytes.len()));
    internal_job_log(
        &job_id,
        "llm_context_policy csv_sent_to_llm=false full_logs_sent_to_llm=false result_artifact_sent_by_reference=true",
    );
    push_job_log(
        &log_path,
        &mut log,
        alpha_strategy_intro_line(&args, client),
    )?;
    push_job_log(&log_path, &mut log, "Selecting a Forge program for this session...".to_string())?;
    push_job_log(
        &log_path,
        &mut log,
        "Program selected: Market backtest and alpha signal synthesis.".to_string(),
    )?;
    persist_job_value(
        &jobs_dir,
        &job_id,
        json!({
            "job_id": job_id,
            "title": job_title.clone(),
            "kind": "alpha_strategy_from_csv",
            "status": "running",
            "file_path": csv_path.display().to_string(),
            "file_hash": file_hash,
            "bars": null,
            "strategy_hash": null,
            "log_path": log_path.display().to_string(),
            "agents": [client],
            "context_accounting": agent_context_accounting(client, csv_bytes.len(), 0)
        }),
    )?;

    let point_size = args.point_size.unwrap_or(0.01).abs().max(1e-12);
    let mut cfg = trading_alpha::SynthConfig::default();
    cfg.sl_points = args
        .sl_display_points
        .map(|v| v.abs() * point_size)
        .or(args.sl_points.map(f64::abs))
        .unwrap_or(cfg.sl_points);
    cfg.tp_points = args
        .tp_display_points
        .map(|v| v.abs() * point_size)
        .or(args.tp_points.map(f64::abs))
        .unwrap_or(cfg.tp_points);
    cfg.spread_points = args
        .spread_display_points
        .map(|v| v.abs() * point_size)
        .or(args.spread_points.map(f64::abs))
        .unwrap_or(cfg.spread_points);
    cfg.target_pnl_per_day = args
        .target_display_points_per_day
        .map(|v| v.abs() * point_size)
        .or(args.target_pnl_per_day.map(f64::abs))
        .unwrap_or(cfg.target_pnl_per_day);
    cfg.max_horizon_bars = args.max_horizon_bars.unwrap_or(cfg.max_horizon_bars).max(1);
    cfg.train_split = args.train_split.unwrap_or(cfg.train_split).clamp(0.2, 0.9);
    let top_n = args.top_rules_per_side.unwrap_or(18).clamp(4, 64);

    push_job_log(&log_path, &mut log, "Reading the candles and preparing the backtest.".to_string())?;
    let bars = trading_alpha::parse_csv(&csv_bytes).map_err(|e| format!("CSV parse error: {e}"))?;
    if bars.len() < 250 {
        return Err(format!("need at least 250 OHLC bars, got {}", bars.len()));
    }
    persist_job_value(
        &jobs_dir,
        &job_id,
        json!({
            "job_id": job_id,
            "title": job_title.clone(),
            "kind": "alpha_strategy_from_csv",
            "status": "running",
            "file_path": csv_path.display().to_string(),
            "file_hash": file_hash,
            "bars": bars.len(),
            "strategy_hash": null,
            "log_path": log_path.display().to_string(),
            "agents": [client],
            "context_accounting": agent_context_accounting(
                client,
                csv_bytes.len(),
                fs::metadata(&log_path).map(|m| m.len() as usize).unwrap_or(0)
            )
        }),
    )?;
    let engine = args.engine.as_deref().unwrap_or("auto");
    if engine != "threshold" {
        let bars_len = bars.len();
        let result = run_forge_alpha_job(
            args.clone(),
            cfg,
            csv_path.clone(),
            file_hash,
            job_id.clone(),
            jobs_dir.clone(),
            log_path.clone(),
            log.clone(),
            bars.clone(),
            client,
        );
        if result.is_ok() {
            return result;
        }
        let err = result.as_ref().err().cloned().unwrap_or_else(|| "unknown Forge synthesis error".to_string());
        if matches!(engine, "forge" | "forge_strict") {
            let _ = append_job_log_only(&log_path, &format!("job failed: {err}"));
            let _ = mark_job_failed(
                &jobs_dir,
                &job_id,
                &csv_path,
                file_hash,
                Some(bars_len),
                &log_path,
                &err,
                client,
            );
            return result;
        }
        append_job_log_only(
            &log_path,
            &format!("Forge synthesis failed: {err}; falling back to threshold engine"),
        )?;
        push_job_log(
            &log_path,
            &mut log,
            "Forge synthesis did not produce a complete strategy; fallback threshold search starts now".to_string(),
        )?;
    }
    internal_job_log(&job_id, format!(
        "threshold engine config bars={} sl={:.4} tp={:.4} spread={:.4} target/day={:.4} horizon={}",
        bars.len(),
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points,
        cfg.target_pnl_per_day,
        cfg.max_horizon_bars
    ));
    push_job_log(&log_path, &mut log, format!("loaded {} OHLC bars for feature extraction", bars.len()))?;

    let train_end = trading_alpha::train_holdout_split(bars.len(), cfg);
    internal_job_log(&job_id, format!("split train=[200,{train_end}) holdout=[{train_end},{})", bars.len()));
    push_job_log(&log_path, &mut log, "calculating VWAP / anchored VWAP / RSI / ATR / ADX / Stochastic feature matrix".to_string())?;
    let raw_cache = build_mcp_raw_feature_cache(&bars, trading_alpha::MIN_HISTORY..bars.len());
    push_job_log(&log_path, &mut log, "feature matrix ready: VWAP, stochastic, volatility and session features".to_string())?;
    let decision_rows = count_decision_rows(&bars, &raw_cache, trading_alpha::MIN_HISTORY..train_end);
    internal_job_log(
        &job_id,
        format!(
            "decision rows with any feature in train: {}",
            decision_rows
        ),
    );

    push_job_log(&log_path, &mut log, "enumerating LONG/SHORT threshold rules over Alpha features".to_string())?;
    let rules = enumerate_rules(&bars, &raw_cache, trading_alpha::MIN_HISTORY..train_end, cfg);
    push_job_log(&log_path, &mut log, format!("{} candidate LONG/SHORT rules generated", rules.len()))?;
    if rules.is_empty() {
        mark_job_failed(
            &jobs_dir,
            &job_id,
            &csv_path,
            file_hash,
            Some(bars.len()),
            &log_path,
            "no alpha rules could be enumerated from this CSV",
            client,
        )?;
        return Err("no alpha rules could be enumerated from this CSV".to_string());
    }

    let mut long_rules = score_side_rules(
        &bars,
        &raw_cache,
        cfg,
        &rules,
        true,
        train_end,
        &log_path,
        &mut log,
    )?;
    let mut short_rules = score_side_rules(
        &bars,
        &raw_cache,
        cfg,
        &rules,
        false,
        train_end,
        &log_path,
        &mut log,
    )?;
    long_rules.truncate(top_n);
    short_rules.truncate(top_n);
    push_job_log(&log_path, &mut log, format!(
        "preselected LONG={} SHORT={} rule candidates",
        long_rules.len(),
        short_rules.len()
    ))?;

    push_job_log(&log_path, &mut log, "pairing LONG and SHORT rules on train split".to_string())?;
    let (long_rule, short_rule, train_eval) =
        match best_dual_rule_pair(&bars, &raw_cache, cfg, &long_rules, &short_rules, train_end) {
            Ok(best) => best,
            Err(err) => {
                mark_job_failed(
                    &jobs_dir,
                    &job_id,
                    &csv_path,
                    file_hash,
                    Some(bars.len()),
                    &log_path,
                    &err,
                    client,
                )?;
                return Err(err);
            }
        };
    push_job_log(&log_path, &mut log, "evaluating selected LONG/SHORT strategy on holdout".to_string())?;
    let holdout_eval = eval_pair(
        &bars,
        &raw_cache,
        cfg,
        train_end..bars.len(),
        &long_rule,
        &short_rule,
    );

    let strategy_text = format!(
        "LONG if {} {:?} {}; SHORT if {} {:?} {}; cfg sl={:.6} tp={:.6} spread={:.6}",
        long_rule.feature_name,
        long_rule.cmp,
        long_rule.threshold,
        short_rule.feature_name,
        short_rule.cmp,
        short_rule.threshold,
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points
    );
    let strategy_hash = format!("{:016x}", quick_file_hash(strategy_text.as_bytes()));
    internal_job_log(&job_id, format!("selected strategy_hash={strategy_hash}"));
    let pair_combinations = long_rules.len().saturating_mul(short_rules.len());
    let compute_avoided = alpha_compute_avoided(
        "threshold",
        bars.len(),
        decision_rows,
        rules.len(),
        pair_combinations,
        0,
        0,
        0,
        0,
        0,
    );
    push_job_log(&log_path, &mut log, format!(
        "train target={:.1}% pnl={:.4} trades={} | holdout target={:.1}% pnl={:.4} trades={}",
        train_eval.pct_days_target_hit(),
        train_eval.total_pnl_points,
        train_eval.total_trades,
        holdout_eval.pct_days_target_hit(),
        holdout_eval.total_pnl_points,
        holdout_eval.total_trades
    ))?;

    let job = StrategyJob {
        job_id: job_id.clone(),
        title: job_title,
        status: "completed".to_string(),
        file_path: csv_path.display().to_string(),
        file_hash,
        bars: bars.len(),
        train_end_bar: train_end,
        strategy_hash,
        long_rule,
        short_rule,
        train: summarize_eval(&train_eval),
        holdout: summarize_eval(&holdout_eval),
        log_path: log_path.display().to_string(),
    };
    let mut job_json = serde_json::to_value(&job).map_err(|e| format!("encode job: {e}"))?;
    if let Value::Object(ref mut obj) = job_json {
        obj.insert("agents".to_string(), json!([client]));
        obj.insert("compute_avoided".to_string(), compute_avoided.clone());
        obj.insert(
            "context_accounting".to_string(),
            agent_context_accounting_with_artifacts(
                client,
                csv_bytes.len(),
                fs::metadata(&log_path).map(|m| m.len() as usize).unwrap_or(0),
                0,
                compute_avoided,
            ),
        );
    }
    let job_json = serde_json::to_vec_pretty(&job_json).map_err(|e| format!("encode job: {e}"))?;
    let job_path = jobs_dir.join(format!("{job_id}.json"));
    fs::write(&job_path, job_json).map_err(|e| format!("write job: {e}"))?;
    push_job_log(&log_path, &mut log, "Backtest complete. Forge saved the selected strategy, hashes and proof references.".to_string())?;
    Ok(job)
}

fn enumerate_rules(
    bars: &[trading_alpha::Bar],
    raw_cache: &McpRawCache,
    range: std::ops::Range<usize>,
    cfg: trading_alpha::SynthConfig,
) -> Vec<Rule> {
    let mut out = Vec::new();
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));
    for fi in 0..trading_alpha::BASE_FEATURE_COUNT {
        let mut values = Vec::new();
        for i in range.start.max(trading_alpha::MIN_HISTORY)..end {
            if !trading_alpha::is_decision_hour(&bars[i]) {
                continue;
            }
            if let Some(value) = raw_cache.get(i).and_then(|row| row[fi]) {
                values.push(value);
            }
        }
        values.sort_unstable();
        values.dedup();
        if values.len() < 2 {
            continue;
        }
        for pct in [10usize, 20, 30, 40, 50, 60, 70, 80, 90] {
            let idx = (values.len() - 1) * pct / 100;
            let threshold = values[idx];
            let feature_name = trading_alpha::FEATURE_NAMES
                .get(fi)
                .copied()
                .unwrap_or("feature")
                .to_string();
            out.push(Rule {
                feature_idx: fi,
                feature_name: feature_name.clone(),
                threshold,
                cmp: RuleCmp::Gte,
            });
            out.push(Rule {
                feature_idx: fi,
                feature_name,
                threshold,
                cmp: RuleCmp::Lte,
            });
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn run_forge_alpha_job(
    args: AlphaStrategyArgs,
    mut cfg: trading_alpha::SynthConfig,
    csv_path: PathBuf,
    file_hash: u64,
    job_id: String,
    jobs_dir: PathBuf,
    log_path: PathBuf,
    mut log: Vec<String>,
    bars: Vec<trading_alpha::Bar>,
    client: &McpClientInfo,
) -> Result<StrategyJob, String> {
    let job_t0 = std::time::Instant::now();
    push_job_log(&log_path, &mut log, "running Forge dual-classifier synthesis".to_string())?;
    internal_job_log(&job_id, format!(
        "forge engine config bars={} sl={:.4} tp={:.4} spread={:.4} target/day={:.4} horizon={}",
        bars.len(),
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points,
        cfg.target_pnl_per_day,
        cfg.max_horizon_bars
    ));

    let store_dir = if let Some(raw) = args.store_dir.as_deref() {
        resolve_path(raw)?
    } else {
        jobs_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| jobs_dir.clone())
    };
    internal_job_log(&job_id, format!("store_dir={}", store_dir.display()));
    let backend_t0 = std::time::Instant::now();
    let node = forge_backend_for_store(&store_dir, &job_id)?;
    let atlas = node
        .atlas()
        .ok_or_else(|| "Forge backend missing attached Atlas".to_string())?;
    internal_job_log(
        &job_id,
        format!("Forge backend acquired in {:.2}s", backend_t0.elapsed().as_secs_f64()),
    );

    let train_end = trading_alpha::train_holdout_split(bars.len(), cfg);
    internal_job_log(&job_id, format!("split train=[200,{train_end}) holdout=[{train_end},{})", bars.len()));
    let straddle_grid_t0 = std::time::Instant::now();
    if let Some(selection) = trading_alpha::select_best_straddle_grid_config(
        &bars,
        trading_alpha::MIN_HISTORY..train_end,
        cfg,
    ) {
        cfg = selection.cfg;
        internal_job_log(
            &job_id,
            format!(
                "selected_straddle_grid combinations={} decision_rows={} sl={:.4} tp={:.4} target_hit={:.2}% pnl={:.4} avg_expiry_bars={:.2}",
                selection.combinations,
                selection.decision_rows,
                cfg.sl_points,
                cfg.tp_points,
                selection.target_hit_pct,
                selection.total_pnl_points,
                selection.avg_expiry_bars
            ),
        );
        push_job_log(
            &log_path,
            &mut log,
            format!(
                "SL/TP grid selected for simultaneous H4 LONG+SHORT: {} combinations, SL={:.4}, TP={:.4}, train target={:.1}%, avg expiry={:.2} bars",
                selection.combinations,
                cfg.sl_points,
                cfg.tp_points,
                selection.target_hit_pct,
                selection.avg_expiry_bars
            ),
        )?;
    }
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "target math: continuation/reversal labels test both sides over horizon={} bars; LONG=take-profit-before-stop, SHORT=symmetric take-profit-before-stop",
            cfg.max_horizon_bars
        ),
    )?;
    internal_job_log(
        &job_id,
        format!(
            "selected_straddle_grid_timing {}",
            fmt_elapsed_ms_ns(straddle_grid_t0.elapsed())
        ),
    );

    push_job_log(&log_path, &mut log, "calculating raw Alpha features: VWAP, anchored VWAP, RSI, ATR, ADX, stochastic".to_string())?;
    push_job_log(
        &log_path,
        &mut log,
        "feature math: typical=(high+low+close)/3; body=close-open; range=high-low; return_n=close/close[n]-1; volume_z=(volume-mean_window)/std_window".to_string(),
    )?;
    push_job_log(
        &log_path,
        &mut log,
        "indicator math: VWAP=Î£(typical*volume)/Î£(volume); RSI=100-100/(1+avg_gain/avg_loss); ATR=EMA(true_range); ADX=EMA(|+DI--DI|/(+DI+-DI)*100)".to_string(),
    )?;
    let raw_t0 = std::time::Instant::now();
    let (mut raw_feature_cache, mut raw_stats) = build_mcp_raw_feature_matrix_cache_with_atlas(
        &bars,
        trading_alpha::MIN_HISTORY..bars.len(),
        node.store(),
        &atlas,
        file_hash,
    );
    let raw_rows = raw_stats.atlas_hits + raw_stats.computed_rows;
    if raw_rows == 0 {
        internal_job_log(
            &job_id,
            "raw cache complete-row path produced 0 rows; retrying MCP partial feature materialization".to_string(),
        );
        let (fallback_cache, fallback_stats, fallback_missing_scalars) =
            build_mcp_partial_raw_feature_cache_with_atlas(
                &bars,
                trading_alpha::MIN_HISTORY..bars.len(),
                &atlas,
                file_hash,
            );
        raw_feature_cache = fallback_cache;
        raw_stats = fallback_stats;
        internal_job_log(
            &job_id,
            format!(
                "partial raw cache filled {} missing feature scalars with 0 for MCP resilience",
                fallback_missing_scalars
            ),
        );
    }
    internal_job_log(
        &job_id,
        format!(
            "raw cache ready in {:.2}s (atlas_hits={} computed={} persisted={})",
            raw_t0.elapsed().as_secs_f64(),
            raw_stats.atlas_hits,
            raw_stats.computed_rows,
            raw_stats.persisted_values
        ),
    );
    let raw_elapsed = raw_t0.elapsed();
    let raw_rows = raw_stats.atlas_hits + raw_stats.computed_rows;
    let _ = atlas.flush();
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "raw feature matrix ready in {} | rows={} | atlas={} | computed={} | persisted={} | avoided={:.1}%",
            fmt_elapsed_ms_ns(raw_elapsed),
            raw_rows,
            raw_stats.atlas_hits,
            raw_stats.computed_rows,
            raw_stats.persisted_values,
            ratio_pct(raw_stats.atlas_hits, raw_rows)
        ),
    )?;

    push_job_log(
        &log_path,
        &mut log,
        format!(
            "building simultaneous H4 LONG+SHORT labels with SL={:.4}, TP={:.4}, horizon={} bars",
            cfg.sl_points,
            cfg.tp_points,
            cfg.max_horizon_bars
        ),
    )?;
    push_job_log(
        &log_path,
        &mut log,
        "label math: y_continue=1 when next-side target is touched before stop; y_reversal=1 when opposite-side target wins first; ambiguous rows are kept as hard negatives".to_string(),
    )?;
    let label_t0 = std::time::Instant::now();
    let (label_cache, label_stats) = trading_alpha::build_binary_label_cache_with_stats(
        &bars,
        trading_alpha::MIN_HISTORY..bars.len(),
        cfg,
        &atlas,
        file_hash,
    );
    internal_job_log(
        &job_id,
        format!(
            "label cache ready in {:.2}s (atlas_hits={} computed={} persisted={})",
            label_t0.elapsed().as_secs_f64(),
            label_stats.atlas_hits,
            label_stats.computed_rows,
            label_stats.persisted_values
        ),
    );
    if label_stats.grid_evaluated_pairs > 0 {
        internal_job_log(
            &job_id,
            format!(
                "sl_tp_grid pairs={} long_tp={} short_tp={} long_sl={} short_sl={} avg_bars_long={:.2} avg_bars_short={:.2}",
                label_stats.grid_evaluated_pairs,
                label_stats.long_take_profit_hits,
                label_stats.short_take_profit_hits,
                label_stats.long_stop_loss_hits,
                label_stats.short_stop_loss_hits,
                label_stats.long_bars_held_sum as f64 / label_stats.grid_evaluated_pairs as f64,
                label_stats.short_bars_held_sum as f64 / label_stats.grid_evaluated_pairs as f64,
            ),
        );
        push_job_log(
            &log_path,
            &mut log,
            format!(
                "SL/TP grid measured: {} combinations, LONG TP hits={}, SHORT TP hits={}, avg expiry {:.2}/{:.2} bars",
                label_stats.grid_evaluated_pairs,
                label_stats.long_take_profit_hits,
                label_stats.short_take_profit_hits,
                label_stats.long_bars_held_sum as f64 / label_stats.grid_evaluated_pairs as f64,
                label_stats.short_bars_held_sum as f64 / label_stats.grid_evaluated_pairs as f64,
            ),
        )?;
    }
    let label_elapsed = label_t0.elapsed();
    let label_rows = label_stats.atlas_hits + label_stats.computed_rows;
    let _ = atlas.flush();
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "LONG/SHORT label matrix ready in {} | rows={} | atlas={} | computed={} | persisted={} | avoided={:.1}%",
            fmt_elapsed_ms_ns(label_elapsed),
            label_rows,
            label_stats.atlas_hits,
            label_stats.computed_rows,
            label_stats.persisted_values,
            ratio_pct(label_stats.atlas_hits, label_rows)
        ),
    )?;

    let feature_examples_t0 = std::time::Instant::now();
    let per_feature = trading_alpha::build_binary_feature_examples_with_caches(
        &bars,
        trading_alpha::MIN_HISTORY..train_end,
        &raw_feature_cache,
        &label_cache,
    );
    if per_feature.is_empty() {
        mark_job_failed(
            &jobs_dir,
            &job_id,
            &csv_path,
            file_hash,
            Some(bars.len()),
            &log_path,
            "no Forge binary feature examples generated",
            client,
        )?;
        return Err("no Forge binary feature examples generated".to_string());
    }
    let feature_limit = args
        .feature_limit
        .unwrap_or(per_feature.len())
        .clamp(1, per_feature.len());
    let top_n = args.top_rules_per_side.unwrap_or(6).clamp(1, 24);
    let max_nodes = args.max_nodes.unwrap_or(12).clamp(5, 24);
    let generations = args.generations.unwrap_or(3).clamp(1, 5);
    let beam_width = args.beam_width.unwrap_or(256).clamp(64, 768);
    let total_example_rows: usize = per_feature
        .iter()
        .map(|(_, long_examples, short_examples)| long_examples.len() + short_examples.len())
        .sum();
    let feature_examples_elapsed = feature_examples_t0.elapsed();
    internal_job_log(
        &job_id,
        format!(
            "binary feature example matrix ready {} features={} rows={}",
            fmt_elapsed_ms_ns(feature_examples_elapsed),
            per_feature.len(),
            total_example_rows
        ),
    );
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "binary feature example matrix ready in {} | features={} | labeled rows={}",
            fmt_elapsed_ms_ns(feature_examples_elapsed),
            per_feature.len(),
            total_example_rows
        ),
    )?;
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "synthesizing detectors over {}/{} Alpha features (LONG/SHORT classifiers)",
            feature_limit,
            per_feature.len()
        ),
    )?;
    internal_job_log(&job_id, format!("synth params top_per_side={top_n} max_nodes={max_nodes} generations={generations} beam={beam_width}"));

    let mut long_detectors = Vec::<ForgeDetector>::new();
    let mut short_detectors = Vec::<ForgeDetector>::new();
    let mut total_candidates = 0usize;
    let detector_total_t0 = std::time::Instant::now();
    for (fi, (feature_name, long_examples, short_examples)) in per_feature.iter().enumerate().take(feature_limit) {
        let feature_t0 = std::time::Instant::now();
        let long_pos = long_examples.iter().filter(|(_, label)| *label == 1).count();
        let short_pos = short_examples.iter().filter(|(_, label)| *label == 1).count();
        push_job_log(
            &log_path,
            &mut log,
            format!(
                "[feature {}/{}] {} examples={} long_pos={} short_pos={}",
                fi + 1,
                feature_limit,
                feature_name,
                long_examples.len(),
                long_pos,
                short_pos
            ),
        )?;
        push_job_log(
            &log_path,
            &mut log,
            format!(
                "[feature {}/{}] {} classifier math: normalize feature window -> threshold/boolean expression search -> minimize train loss -> validate target-hit/PnL on holdout",
                fi + 1,
                feature_limit,
                feature_name
            ),
        )?;
        if long_pos > 0 {
            if let Some(detector) = evolve_detector(
                &node,
                &log_path,
                &mut log,
                &job_id,
                feature_name,
                fi,
                long_examples,
                true,
                &bars,
                &raw_feature_cache,
                cfg,
                trading_alpha::MIN_HISTORY..train_end,
                generations,
                max_nodes,
                beam_width,
            )? {
                total_candidates = total_candidates.saturating_add(detector.outcome.candidates_evaluated);
                long_detectors.push(detector);
            }
        }
        if short_pos > 0 {
            if let Some(detector) = evolve_detector(
                &node,
                &log_path,
                &mut log,
                &job_id,
                feature_name,
                fi,
                short_examples,
                false,
                &bars,
                &raw_feature_cache,
                cfg,
                trading_alpha::MIN_HISTORY..train_end,
                generations,
                max_nodes,
                beam_width,
            )? {
                total_candidates = total_candidates.saturating_add(detector.outcome.candidates_evaluated);
                short_detectors.push(detector);
            }
        }
        push_job_log(
            &log_path,
            &mut log,
            format!(
                "[feature {}/{}] {} complete in {}",
                fi + 1,
                feature_limit,
                feature_name,
                fmt_elapsed_ms_ns(feature_t0.elapsed())
            ),
        )?;
    }
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "detector synthesis phase complete in {} | long={} | short={} | candidates={}",
            fmt_elapsed_ms_ns(detector_total_t0.elapsed()),
            long_detectors.len(),
            short_detectors.len(),
            total_candidates
        ),
    )?;

    long_detectors.sort_by(|a, b| compare_eval(&b.train_eval, &a.train_eval));
    short_detectors.sort_by(|a, b| compare_eval(&b.train_eval, &a.train_eval));
    long_detectors.truncate(top_n);
    short_detectors.truncate(top_n);
    if long_detectors.is_empty() || short_detectors.is_empty() {
        if let Some(job) = complete_one_sided_forge_job(
            &node,
            &bars,
            &raw_feature_cache,
            cfg,
            train_end,
            &csv_path,
            file_hash,
            &job_id,
            &jobs_dir,
            &log_path,
            &mut log,
            long_detectors.first().or_else(|| short_detectors.first()),
            !long_detectors.is_empty(),
            total_candidates,
            client,
        )? {
            return Ok(job);
        }
        let err = format!(
            "Forge synthesis did not produce both sides (long={} short={})",
            long_detectors.len(),
            short_detectors.len()
        );
        mark_job_failed(&jobs_dir, &job_id, &csv_path, file_hash, Some(bars.len()), &log_path, &err, client)?;
        return Err(err);
    }

    push_job_log(
        &log_path,
        &mut log,
        format!(
            "pairing top detectors: long={} short={}",
            long_detectors.len(),
            short_detectors.len()
        ),
    )?;
    let pairing_t0 = std::time::Instant::now();
    let mut best_pair: Option<(usize, usize, trading_alpha::StrategyEval)> = None;
    for (li, long) in long_detectors.iter().enumerate() {
        for (si, short) in short_detectors.iter().enumerate() {
            let eval = eval_program_pair(
                &node,
                &bars,
                &raw_feature_cache,
                cfg,
                trading_alpha::MIN_HISTORY..train_end,
                long,
                short,
            );
            let replace = best_pair
                .as_ref()
                .map(|(_, _, current)| compare_eval(&eval, current) == Ordering::Greater)
                .unwrap_or(true);
            if replace {
                best_pair = Some((li, si, eval));
            }
        }
    }
    internal_job_log(
        &job_id,
        format!(
            "pairing_timing {} combinations={}",
            fmt_elapsed_ms_ns(pairing_t0.elapsed()),
            long_detectors.len().saturating_mul(short_detectors.len())
        ),
    );
    let (best_long_idx, best_short_idx, train_eval) =
        best_pair.ok_or_else(|| "Forge pairing produced no strategy".to_string())?;
    let long = &long_detectors[best_long_idx];
    let short = &short_detectors[best_short_idx];
    let holdout_t0 = std::time::Instant::now();
    let holdout_eval = eval_program_pair(
        &node,
        &bars,
        &raw_feature_cache,
        cfg,
        train_end..bars.len(),
        long,
        short,
    );
    internal_job_log(
        &job_id,
        format!("holdout_eval_timing {}", fmt_elapsed_ms_ns(holdout_t0.elapsed())),
    );

    let strategy_text = format!(
        "FORGE LONG {} {} SHORT {} {} cfg sl={:.6} tp={:.6} spread={:.6}",
        long.feature_name,
        long.program_hash,
        short.feature_name,
        short.program_hash,
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points
    );
    let strategy_hash = format!("{:016x}", quick_file_hash(strategy_text.as_bytes()));
    internal_job_log(
        &job_id,
        format!(
            "selected Forge strategy_hash={} long_feature={} long_program={} long_nodes={} short_feature={} short_program={} short_nodes={}",
            strategy_hash,
            long.feature_name,
            long.program_hash,
            long.outcome.program.nodes().len(),
            short.feature_name,
            short.program_hash,
            short.outcome.program.nodes().len()
        ),
    );
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "selected Alpha strategy: LONG detector on {} + SHORT detector on {}",
            long.feature_name, short.feature_name
        ),
    )?;
    push_job_log(
        &log_path,
        &mut log,
        format!(
            "train target={:.1}% pnl={:.4} trades={} | holdout target={:.1}% pnl={:.4} trades={}",
            train_eval.pct_days_target_hit(),
            train_eval.total_pnl_points,
            train_eval.total_trades,
            holdout_eval.pct_days_target_hit(),
            holdout_eval.total_pnl_points,
            holdout_eval.total_trades
        ),
    )?;
    push_job_log(
        &log_path,
        &mut log,
        format!("Forge strategy job completed in {}", fmt_elapsed_ms_ns(job_t0.elapsed())),
    )?;

    let job = StrategyJob {
        job_id: job_id.clone(),
        title: resolve_job_title(args.title.as_deref(), &csv_path),
        status: "completed".to_string(),
        file_path: csv_path.display().to_string(),
        file_hash,
        bars: bars.len(),
        train_end_bar: train_end,
        strategy_hash,
        long_rule: Rule {
            feature_idx: long.feature_idx,
            feature_name: long.feature_name.clone(),
            threshold: 0,
            cmp: RuleCmp::Gte,
        },
        short_rule: Rule {
            feature_idx: short.feature_idx,
            feature_name: short.feature_name.clone(),
            threshold: 0,
            cmp: RuleCmp::Gte,
        },
        train: summarize_eval(&train_eval),
        holdout: summarize_eval(&holdout_eval),
        log_path: log_path.display().to_string(),
    };
    let mut job_json = serde_json::to_value(&job).map_err(|e| format!("encode job value: {e}"))?;
    if let Value::Object(ref mut obj) = job_json {
        let pairing_combinations = long_detectors.len().saturating_mul(short_detectors.len());
        let compute_avoided = alpha_compute_avoided(
            "forge",
            bars.len(),
            total_example_rows,
            total_candidates,
            long.outcome
                .combinations_tried
                .saturating_add(short.outcome.combinations_tried)
                .saturating_add(pairing_combinations),
            long.outcome.atlas_score_hits.saturating_add(short.outcome.atlas_score_hits),
            long.outcome
                .atlas_full_pair_hits
                .saturating_add(short.outcome.atlas_full_pair_hits),
            long.outcome
                .atlas_opcode_hits
                .saturating_add(short.outcome.atlas_opcode_hits),
            long.outcome
                .gpu_jobs_dispatched
                .saturating_add(short.outcome.gpu_jobs_dispatched),
            long.outcome
                .gpu_jobs_skipped
                .saturating_add(short.outcome.gpu_jobs_skipped),
        );
        obj.insert("engine".to_string(), json!("forge"));
        obj.insert("long_program_hash".to_string(), json!(long.program_hash.to_string()));
        obj.insert("short_program_hash".to_string(), json!(short.program_hash.to_string()));
        obj.insert("candidates_evaluated".to_string(), json!(total_candidates));
        obj.insert("agents".to_string(), json!([client]));
        obj.insert("compute_avoided".to_string(), compute_avoided.clone());
        obj.insert(
            "context_accounting".to_string(),
            agent_context_accounting_with_artifacts(
                client,
                fs::metadata(&csv_path).map(|m| m.len() as usize).unwrap_or(0),
                fs::metadata(&log_path).map(|m| m.len() as usize).unwrap_or(0),
                0,
                compute_avoided,
            ),
        );
    }
    persist_job_value(&jobs_dir, &job_id, job_json)?;
    let manifest_path = jobs_dir.join(format!("{job_id}.json"));
    internal_job_log(
        &job_id,
        format!(
            "artifacts manifest={} manifest_bytes={} log={} log_bytes={} csv_bytes={} csv_sent_to_llm=false full_logs_sent_to_llm=false",
            manifest_path.display(),
            fs::metadata(&manifest_path).map(|m| m.len()).unwrap_or(0),
            log_path.display(),
            fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0),
            fs::metadata(&csv_path).map(|m| m.len()).unwrap_or(0)
        ),
    );
    push_job_log(&log_path, &mut log, "Backtest complete. Forge saved the selected strategy, hashes and proof references.".to_string())?;
    Ok(job)
}

#[allow(clippy::too_many_arguments)]
fn complete_one_sided_forge_job(
    node: &MonsterNode,
    bars: &[trading_alpha::Bar],
    raw_feature_cache: &[Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>],
    cfg: trading_alpha::SynthConfig,
    train_end: usize,
    csv_path: &Path,
    file_hash: u64,
    job_id: &str,
    jobs_dir: &Path,
    log_path: &Path,
    log: &mut Vec<String>,
    detector: Option<&ForgeDetector>,
    long_side: bool,
    total_candidates: usize,
    client: &McpClientInfo,
) -> Result<Option<StrategyJob>, String> {
    let Some(detector) = detector else {
        return Ok(None);
    };
    let side = if long_side { "LONG" } else { "SHORT" };
    push_job_log(
        log_path,
        log,
        format!(
            "only {side} detectors were produced; completing as one-sided Forge strategy"
        ),
    )?;
    let train_eval = eval_program_side(
        node,
        bars,
        raw_feature_cache,
        cfg,
        trading_alpha::MIN_HISTORY..train_end,
        detector.feature_idx,
        &detector.program_hash,
        long_side,
    );
    let holdout_eval = eval_program_side(
        node,
        bars,
        raw_feature_cache,
        cfg,
        train_end..bars.len(),
        detector.feature_idx,
        &detector.program_hash,
        long_side,
    );
    let strategy_text = format!(
        "FORGE_ONE_SIDED {side} {} {} cfg sl={:.6} tp={:.6} spread={:.6}",
        detector.feature_name,
        detector.program_hash,
        cfg.sl_points,
        cfg.tp_points,
        cfg.spread_points
    );
    let strategy_hash = format!("{:016x}", quick_file_hash(strategy_text.as_bytes()));
    let active_rule = Rule {
        feature_idx: detector.feature_idx,
        feature_name: detector.feature_name.clone(),
        threshold: 0,
        cmp: RuleCmp::Gte,
    };
    let disabled_rule = Rule {
        feature_idx: 0,
        feature_name: "disabled".to_string(),
        threshold: i64::MAX,
        cmp: RuleCmp::Gte,
    };
    let job = StrategyJob {
        job_id: job_id.to_string(),
        title: resolve_job_title(None, csv_path),
        status: "completed".to_string(),
        file_path: csv_path.display().to_string(),
        file_hash,
        bars: bars.len(),
        train_end_bar: train_end,
        strategy_hash,
        long_rule: if long_side { active_rule.clone() } else { disabled_rule.clone() },
        short_rule: if long_side { disabled_rule } else { active_rule },
        train: summarize_eval(&train_eval),
        holdout: summarize_eval(&holdout_eval),
        log_path: log_path.display().to_string(),
    };
    let mut job_json = serde_json::to_value(&job).map_err(|e| format!("encode job value: {e}"))?;
    if let Value::Object(ref mut obj) = job_json {
        let compute_avoided = alpha_compute_avoided(
            "forge_one_sided",
            bars.len(),
            train_end.saturating_sub(trading_alpha::MIN_HISTORY),
            total_candidates,
            detector.outcome.combinations_tried,
            detector.outcome.atlas_score_hits,
            detector.outcome.atlas_full_pair_hits,
            detector.outcome.atlas_opcode_hits,
            detector.outcome.gpu_jobs_dispatched,
            detector.outcome.gpu_jobs_skipped,
        );
        obj.insert("engine".to_string(), json!("forge_one_sided"));
        obj.insert("side".to_string(), json!(side));
        obj.insert("program_hash".to_string(), json!(detector.program_hash.to_string()));
        obj.insert("candidates_evaluated".to_string(), json!(total_candidates));
        obj.insert("agents".to_string(), json!([client]));
        obj.insert("compute_avoided".to_string(), compute_avoided.clone());
        obj.insert(
            "context_accounting".to_string(),
            agent_context_accounting_with_artifacts(
                client,
                fs::metadata(csv_path).map(|m| m.len() as usize).unwrap_or(0),
                fs::metadata(log_path).map(|m| m.len() as usize).unwrap_or(0),
                0,
                compute_avoided,
            ),
        );
    }
    persist_job_value(jobs_dir, job_id, job_json)?;
    let manifest_path = jobs_dir.join(format!("{job_id}.json"));
    internal_job_log(
        job_id,
        format!(
            "artifacts manifest={} manifest_bytes={} log={} log_bytes={} csv_bytes={} csv_sent_to_llm=false full_logs_sent_to_llm=false",
            manifest_path.display(),
            fs::metadata(&manifest_path).map(|m| m.len()).unwrap_or(0),
            log_path.display(),
            fs::metadata(log_path).map(|m| m.len()).unwrap_or(0),
            fs::metadata(csv_path).map(|m| m.len()).unwrap_or(0)
        ),
    );
    internal_job_log(
        job_id,
        format!(
            "selected one-sided {side} strategy_hash={} feature={} program={} nodes={}",
            job.strategy_hash,
            detector.feature_name,
            detector.program_hash,
            detector.outcome.program.nodes().len()
        ),
    );
    push_job_log(
        log_path,
        log,
        format!("selected one-sided {side} detector on {}", detector.feature_name),
    )?;
    push_job_log(
        log_path,
        log,
        format!(
            "train target={:.1}% pnl={:.4} trades={} | holdout target={:.1}% pnl={:.4} trades={}",
            train_eval.pct_days_target_hit(),
            train_eval.total_pnl_points,
            train_eval.total_trades,
            holdout_eval.pct_days_target_hit(),
            holdout_eval.total_pnl_points,
            holdout_eval.total_trades
        ),
    )?;
    push_job_log(log_path, log, "Backtest complete. Forge saved the selected strategy, hashes and proof references.".to_string())?;
    Ok(Some(job))
}

fn build_mcp_partial_raw_feature_cache_with_atlas(
    bars: &[trading_alpha::Bar],
    range: std::ops::Range<usize>,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> (
    Vec<Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>>,
    trading_alpha::AtlasCacheStats,
    usize,
) {
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(1));
    let feature_cache = trading_alpha::FeatureCache::build(bars);
    let mut rows = vec![None; bars.len()];
    let mut stats = trading_alpha::AtlasCacheStats::default();
    let mut missing_scalars = 0usize;

    for i in start..end {
        if !trading_alpha::is_decision_hour(&bars[i]) {
            continue;
        }

        let mut row = [0i64; trading_alpha::BASE_FEATURE_COUNT];
        let mut any_feature = false;
        for (fi, slot) in row.iter_mut().enumerate() {
            let key = scan::atlas::Atlas::feature_key(file_hash, fi as u8, i as u32);
            if let Some(value) = atlas.lookup_result(&key) {
                *slot = scan::atlas::Atlas::unpack_i64(&value);
                any_feature = true;
                continue;
            }

            match trading_alpha::extract_raw_feature(bars, i, fi, &feature_cache) {
                Some(value) => {
                    *slot = value;
                    any_feature = true;
                }
                None => {
                    *slot = 0;
                    missing_scalars = missing_scalars.saturating_add(1);
                }
            }
            let packed = scan::atlas::Atlas::pack_i64(*slot);
            if atlas.record_result(&key, &packed).unwrap_or(false) {
                stats.persisted_values += 1;
            }
        }

        if any_feature {
            rows[i] = Some(row);
            stats.computed_rows += 1;
        }
    }

    (rows, stats, missing_scalars)
}

const RAW_FEATURE_MATRIX_SCHEMA_VERSION: u8 = 1;
const RAW_FEATURE_MATRIX_MAGIC: &[u8; 8] = b"FRAWMAT1";

fn build_mcp_raw_feature_matrix_cache_with_atlas(
    bars: &[trading_alpha::Bar],
    range: std::ops::Range<usize>,
    store: &Store,
    atlas: &scan::atlas::Atlas,
    file_hash: u64,
) -> (
    Vec<Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>>,
    trading_alpha::AtlasCacheStats,
) {
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(1));
    let key = scan::atlas::Atlas::alpha_raw_feature_matrix_key(
        file_hash,
        start as u32,
        end as u32,
        bars.len() as u32,
        trading_alpha::BASE_FEATURE_COUNT as u8,
        RAW_FEATURE_MATRIX_SCHEMA_VERSION,
    );

    if let Some(blob_hash) = atlas.lookup_result(&key) {
        if let Some(bytes) = store.load(&Hash::from_bytes(blob_hash)) {
            if let Some(rows) = decode_raw_feature_matrix_blob(&bytes, bars.len(), start, end) {
                let mut stats = trading_alpha::AtlasCacheStats::default();
                stats.atlas_hits = rows.iter().filter(|row| row.is_some()).count();
                return (rows, stats);
            }
        }
    }

    let feature_cache = trading_alpha::FeatureCache::build(bars);
    let mut rows = vec![None; bars.len()];
    let mut stats = trading_alpha::AtlasCacheStats::default();
    for i in start..end {
        if !trading_alpha::is_decision_hour(&bars[i]) {
            continue;
        }
        let Some(row) = trading_alpha::extract_raw_feature_vector(bars, i, &feature_cache) else {
            continue;
        };
        rows[i] = Some(row);
        stats.computed_rows += 1;
    }

    let encoded = encode_raw_feature_matrix_blob(&rows, start, end);
    if let Ok(blob_hash) = store.store(&encoded) {
        if atlas.record_result(&key, blob_hash.as_bytes()).unwrap_or(false) {
            stats.persisted_values = 1;
        }
        let _ = atlas.flush();
    }
    (rows, stats)
}

fn encode_raw_feature_matrix_blob(
    rows: &[Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>],
    start: usize,
    end: usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        32 + rows.len() * (1 + trading_alpha::BASE_FEATURE_COUNT * std::mem::size_of::<i64>()),
    );
    out.extend_from_slice(RAW_FEATURE_MATRIX_MAGIC);
    out.push(RAW_FEATURE_MATRIX_SCHEMA_VERSION);
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&(start as u32).to_le_bytes());
    out.extend_from_slice(&(end as u32).to_le_bytes());
    out.push(trading_alpha::BASE_FEATURE_COUNT as u8);
    for row in rows {
        match row {
            Some(values) => {
                out.push(1);
                for value in values {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
            None => out.push(0),
        }
    }
    out
}

fn decode_raw_feature_matrix_blob(
    bytes: &[u8],
    expected_rows: usize,
    expected_start: usize,
    expected_end: usize,
) -> Option<Vec<Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>>> {
    let header_len = RAW_FEATURE_MATRIX_MAGIC.len() + 1 + 4 + 4 + 4 + 1;
    if bytes.len() < header_len || &bytes[..8] != RAW_FEATURE_MATRIX_MAGIC {
        return None;
    }
    let version = bytes[8];
    if version != RAW_FEATURE_MATRIX_SCHEMA_VERSION {
        return None;
    }
    let rows_len = u32::from_le_bytes(bytes[9..13].try_into().ok()?) as usize;
    let start = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let end = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let feature_count = bytes[21] as usize;
    if rows_len != expected_rows
        || start != expected_start
        || end != expected_end
        || feature_count != trading_alpha::BASE_FEATURE_COUNT
    {
        return None;
    }

    let mut offset = header_len;
    let mut rows = Vec::with_capacity(rows_len);
    for _ in 0..rows_len {
        let flag = *bytes.get(offset)?;
        offset += 1;
        if flag == 0 {
            rows.push(None);
            continue;
        }
        if flag != 1 {
            return None;
        }
        let mut values = [0i64; trading_alpha::BASE_FEATURE_COUNT];
        for slot in &mut values {
            let next = offset + std::mem::size_of::<i64>();
            *slot = i64::from_le_bytes(bytes.get(offset..next)?.try_into().ok()?);
            offset = next;
        }
        rows.push(Some(values));
    }
    if offset != bytes.len() {
        return None;
    }
    Some(rows)
}

#[allow(clippy::too_many_arguments)]
fn evolve_detector(
    node: &MonsterNode,
    log_path: &Path,
    log: &mut Vec<String>,
    job_id: &str,
    feature_name: &str,
    feature_idx: usize,
    examples: &[(i64, i64)],
    long_side: bool,
    bars: &[trading_alpha::Bar],
    raw_feature_cache: &[Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>],
    cfg: trading_alpha::SynthConfig,
    eval_range: std::ops::Range<usize>,
    generations: usize,
    max_nodes: usize,
    beam_width: usize,
) -> Result<Option<ForgeDetector>, String> {
    let side = if long_side { "LONG" } else { "SHORT" };
    let path_for_progress = log_path.to_path_buf();
    let job_id_for_progress = job_id.to_string();
    let tag = format!("{feature_name} {side}");
    let progress_last_gen = Arc::new(std::sync::Mutex::new(0usize));
    let progress_last_gen_for_cb = Arc::clone(&progress_last_gen);
    let progress: SynthProgressFn = Arc::new(move |p: SynthProgress| {
        let total_jobs = p.jobs_dispatched.saturating_add(p.jobs_skipped);
        let work_items = p.total_scorings.saturating_mul(p.n_examples);
        let avoided_pct = ratio_pct(p.jobs_skipped, total_jobs);
        let ns_per_job = ns_per(p.depth_ns, p.jobs_dispatched.max(1));
        let ns_per_work_item = ns_per(p.depth_ns, work_items.max(1));
        let backend = if !p.gpu_backend.is_empty() && p.gpu_backend.starts_with("CACHE") {
            p.gpu_backend
        } else if p.jobs_dispatched == 0 && p.jobs_skipped > 0 {
            "ATLAS-CACHE"
        } else if p.gpu_used {
            p.gpu_backend
        } else if p.gpu_eligible && p.gpu_attempted {
            "CPU-fallback-after-gpu-attempt"
        } else {
            "CPU"
        };
        internal_job_log(
            &job_id_for_progress,
            format!(
                "{tag} phase={} depth={}/{} pairs={} beam={} backend={} gpu_eligible={} depth_ms={} depth_ns={} total_scorings={} n_examples={} work_items={} loss={} jobs_dispatched={} jobs_skipped={} avoided_pct={:.1} ns_per_job={} ns_per_work_item={} atlas_pair_hits={} atlas_opcode_hits={} best={}",
                p.phase,
                p.depth,
                p.max_depth,
                p.pairs,
                p.beam_size,
                backend,
                p.gpu_eligible,
                p.depth_ms,
                p.depth_ns,
                p.total_scorings,
                p.n_examples,
                work_items,
                p.best_loss,
                p.jobs_dispatched,
                p.jobs_skipped,
                avoided_pct,
                ns_per_job,
                ns_per_work_item,
                p.atlas_full_pair_hits,
                p.atlas_opcode_hits,
                p.best_expr
            ),
        );
        if p.phase == "start" {
            if let Ok(mut last_gen) = progress_last_gen_for_cb.lock() {
                let current_gen = p.depth.max(1);
                if current_gen > *last_gen {
                    *last_gen = current_gen;
                    let line = format!(
                        "[{tag}] expression search pass {}: testing boolean thresholds over {} labeled examples; current best loss={} expr={}",
                        current_gen,
                        p.n_examples,
                        p.best_loss,
                        p.best_expr
                    );
                    let _ = append_job_log_only(&path_for_progress, &line);
                }
            }
        }
    });
    internal_job_log(
        job_id,
        format!(
            "{feature_name} {side} evolve_i64_program examples={} max_nodes={} generations={} beam={}",
            examples.len(),
            max_nodes,
            generations,
            beam_width
        ),
    );
    push_job_log(
        log_path,
        log,
        format!(
            "[{feature_name} {side}] training detector on {} labeled examples",
            examples.len()
        ),
    )?;
    let t0 = std::time::Instant::now();
    let config = MonsterEvolutionConfig {
        generations,
        max_nodes,
        beam_width,
        holdout_stride: 4,
        progress: Some(progress),
        skip_prepass: true,
    };
    let outcome = match node.evolve_i64_program(examples, config) {
        Ok(outcome) => outcome,
        Err(err) => {
            push_job_log(log_path, log, format!("[{feature_name} {side}] error: {err}"))?;
            return Ok(None);
        }
    };
    let elapsed = t0.elapsed();
    let train_eval = eval_program_side(
        node,
        bars,
        raw_feature_cache,
        cfg,
        eval_range,
        feature_idx,
        &outcome.program_hash,
        long_side,
    );
    internal_job_log(
        job_id,
        format!(
            "{feature_name} {side} done {} source={} program={} nodes={} train_loss={} holdout_loss={} candidates={} combinations={} atlas_score_hits={} atlas_pair_hits={} atlas_opcode_hits={} dispatched={} skipped={} trades={} pnl={:.4} target={:.1}%",
            fmt_elapsed_ms_ns(elapsed),
            outcome.source,
            outcome.program_hash,
            outcome.program.nodes().len(),
            outcome.train_loss,
            outcome.holdout_loss,
            outcome.candidates_evaluated,
            outcome.combinations_tried,
            outcome.atlas_score_hits,
            outcome.atlas_full_pair_hits,
            outcome.atlas_opcode_hits,
            outcome.gpu_jobs_dispatched,
            outcome.gpu_jobs_skipped,
            train_eval.total_trades,
            train_eval.total_pnl_points,
            train_eval.pct_days_target_hit()
        ),
    );
    push_job_log(
        log_path,
        log,
        format!(
            "[{feature_name} {side}] detector ready: trades={} pnl={:.4} target={:.1}% | train_loss={} holdout_loss={} | expression={}",
            train_eval.total_trades,
            train_eval.total_pnl_points,
            train_eval.pct_days_target_hit(),
            outcome.train_loss,
            outcome.holdout_loss,
            outcome.source
        ),
    )?;
    let program_hash = outcome.program_hash;
    Ok(Some(ForgeDetector {
        feature_idx,
        feature_name: feature_name.to_string(),
        program_hash,
        outcome,
        train_eval,
    }))
}

fn score_side_rules(
    bars: &[trading_alpha::Bar],
    raw_cache: &McpRawCache,
    cfg: trading_alpha::SynthConfig,
    rules: &[Rule],
    long_side: bool,
    train_end: usize,
    log_path: &Path,
    log: &mut Vec<String>,
) -> Result<Vec<ScoredRule>, String> {
    let mut scored = Vec::with_capacity(rules.len());
    let side = if long_side { "LONG" } else { "SHORT" };
    push_job_log(log_path, log, format!("scoring {side} rules: {} candidates", rules.len()))?;
    for (idx, rule) in rules.iter().enumerate() {
        if idx > 0 && idx % 48 == 0 {
            push_job_log(log_path, log, format!("scoring {side} rules: {idx}/{}", rules.len()))?;
        }
        let eval = eval_side_rule(
            bars,
            raw_cache,
            cfg,
            trading_alpha::MIN_HISTORY..train_end,
            rule,
            long_side,
        );
        if eval.total_trades > 0 {
            scored.push(ScoredRule {
                rule: rule.clone(),
                eval,
            });
        }
    }
    scored.sort_by(|a, b| compare_eval(&b.eval, &a.eval));
    push_job_log(log_path, log, format!("scoring {side} rules done: {} viable", scored.len()))?;
    Ok(scored)
}

fn best_dual_rule_pair(
    bars: &[trading_alpha::Bar],
    raw_cache: &McpRawCache,
    cfg: trading_alpha::SynthConfig,
    long_rules: &[ScoredRule],
    short_rules: &[ScoredRule],
    train_end: usize,
) -> Result<(Rule, Rule, trading_alpha::StrategyEval), String> {
    let mut best: Option<(Rule, Rule, trading_alpha::StrategyEval)> = None;
    for long in long_rules {
        for short in short_rules {
            let eval = eval_pair(
                bars,
                raw_cache,
                cfg,
                trading_alpha::MIN_HISTORY..train_end,
                &long.rule,
                &short.rule,
            );
            if eval.total_trades == 0 {
                continue;
            }
            let replace = best
                .as_ref()
                .map(|(_, _, current)| compare_eval(&eval, current) == Ordering::Greater)
                .unwrap_or(true);
            if replace {
                best = Some((long.rule.clone(), short.rule.clone(), eval));
            }
        }
    }
    best.ok_or_else(|| "no viable dual LONG/SHORT rule pair found".to_string())
}

fn eval_pair(
    bars: &[trading_alpha::Bar],
    raw_cache: &McpRawCache,
    cfg: trading_alpha::SynthConfig,
    range: std::ops::Range<usize>,
    long_rule: &Rule,
    short_rule: &Rule,
) -> trading_alpha::StrategyEval {
    let mut eval = trading_alpha::StrategyEval::default();
    let mut current_day_ms: i64 = -1;
    let mut current_day_pnl = 0.0;
    let mut current_day_had_trade = false;
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));

    for i in start..end {
        let bar = bars[i];
        let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
        if day_ms != current_day_ms {
            flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
            current_day_ms = day_ms;
            current_day_pnl = 0.0;
            current_day_had_trade = false;
        }
        if !trading_alpha::is_decision_hour(&bar) {
            continue;
        }
        let Some(long_value) = raw_cache.get(i).and_then(|row| row[long_rule.feature_idx]) else {
            continue;
        };
        let Some(short_value) = raw_cache.get(i).and_then(|row| row[short_rule.feature_idx]) else {
            continue;
        };
        let long_pred = predict_rule(long_rule, long_value);
        let short_pred = predict_rule(short_rule, short_value);
        if long_pred == 0 || short_pred == 0 {
            continue;
        }
        let straddle = trading_alpha::simulate_straddle(
            bars,
            i,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars,
        );
        if straddle.long.exit_reason == trading_alpha::ExitReason::NotPossible
            || straddle.short.exit_reason == trading_alpha::ExitReason::NotPossible
        {
            continue;
        }
        record_straddle(&mut eval, &straddle);
        current_day_pnl += straddle.pnl_points;
        current_day_had_trade = true;
    }
    flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
    eval
}

fn eval_program_pair(
    node: &MonsterNode,
    bars: &[trading_alpha::Bar],
    raw_cache: &[Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>],
    cfg: trading_alpha::SynthConfig,
    range: std::ops::Range<usize>,
    long: &ForgeDetector,
    short: &ForgeDetector,
) -> trading_alpha::StrategyEval {
    let mut eval = trading_alpha::StrategyEval::default();
    let mut current_day_ms: i64 = -1;
    let mut current_day_pnl = 0.0;
    let mut current_day_had_trade = false;
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));

    for i in start..end {
        let bar = bars[i];
        let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
        if day_ms != current_day_ms {
            flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
            current_day_ms = day_ms;
            current_day_pnl = 0.0;
            current_day_had_trade = false;
        }
        if !trading_alpha::is_decision_hour(&bar) {
            continue;
        }
        let Some(row) = raw_cache.get(i).copied().flatten() else {
            continue;
        };
        let long_pred = node
            .call_one_i64(&long.program_hash, row[long.feature_idx])
            .unwrap_or(0);
        let short_pred = node
            .call_one_i64(&short.program_hash, row[short.feature_idx])
            .unwrap_or(0);
        if long_pred == 0 || short_pred == 0 {
            continue;
        }
        let straddle = trading_alpha::simulate_straddle(
            bars,
            i,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars,
        );
        if straddle.long.exit_reason == trading_alpha::ExitReason::NotPossible
            || straddle.short.exit_reason == trading_alpha::ExitReason::NotPossible
        {
            continue;
        }
        record_straddle(&mut eval, &straddle);
        current_day_pnl += straddle.pnl_points;
        current_day_had_trade = true;
    }
    flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
    eval
}

fn eval_program_side(
    node: &MonsterNode,
    bars: &[trading_alpha::Bar],
    raw_cache: &[Option<[i64; trading_alpha::BASE_FEATURE_COUNT]>],
    cfg: trading_alpha::SynthConfig,
    range: std::ops::Range<usize>,
    feature_idx: usize,
    program_hash: &Hash,
    long_side: bool,
) -> trading_alpha::StrategyEval {
    let mut eval = trading_alpha::StrategyEval::default();
    let mut current_day_ms: i64 = -1;
    let mut current_day_pnl = 0.0;
    let mut current_day_had_trade = false;
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));

    for i in start..end {
        let bar = bars[i];
        let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
        if day_ms != current_day_ms {
            flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
            current_day_ms = day_ms;
            current_day_pnl = 0.0;
            current_day_had_trade = false;
        }
        if !trading_alpha::is_decision_hour(&bar) {
            continue;
        }
        let Some(row) = raw_cache.get(i).copied().flatten() else {
            continue;
        };
        if node
            .call_one_i64(program_hash, row[feature_idx])
            .unwrap_or(0)
            == 0
        {
            continue;
        }
        let direction = if long_side {
            trading_alpha::Direction::Long
        } else {
            trading_alpha::Direction::Short
        };
        let trade = trading_alpha::simulate_trade(
            bars,
            i,
            direction,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars,
        );
        if trade.exit_reason == trading_alpha::ExitReason::NotPossible {
            continue;
        }
        record_trade(&mut eval, direction, trade.pnl_points);
        current_day_pnl += trade.pnl_points;
        current_day_had_trade = true;
    }
    flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
    eval
}

fn eval_side_rule(
    bars: &[trading_alpha::Bar],
    raw_cache: &McpRawCache,
    cfg: trading_alpha::SynthConfig,
    range: std::ops::Range<usize>,
    rule: &Rule,
    long_side: bool,
) -> trading_alpha::StrategyEval {
    let mut eval = trading_alpha::StrategyEval::default();
    let mut current_day_ms: i64 = -1;
    let mut current_day_pnl = 0.0;
    let mut current_day_had_trade = false;
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(cfg.max_horizon_bars));

    for i in start..end {
        let bar = bars[i];
        let day_ms = bar.time_ms.div_euclid(86_400_000) * 86_400_000;
        if day_ms != current_day_ms {
            flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
            current_day_ms = day_ms;
            current_day_pnl = 0.0;
            current_day_had_trade = false;
        }
        if !trading_alpha::is_decision_hour(&bar) {
            continue;
        }
        let Some(value) = raw_cache.get(i).and_then(|row| row[rule.feature_idx]) else {
            continue;
        };
        if predict_rule(rule, value) == 0 {
            continue;
        }
        let direction = if long_side {
            trading_alpha::Direction::Long
        } else {
            trading_alpha::Direction::Short
        };
        let trade = trading_alpha::simulate_trade(
            bars,
            i,
            direction,
            cfg.sl_points,
            cfg.tp_points,
            cfg.spread_points,
            cfg.max_horizon_bars,
        );
        if trade.exit_reason == trading_alpha::ExitReason::NotPossible {
            continue;
        }
        record_trade(&mut eval, direction, trade.pnl_points);
        current_day_pnl += trade.pnl_points;
        current_day_had_trade = true;
    }
    flush_day(&mut eval, current_day_ms, current_day_had_trade, current_day_pnl, cfg);
    eval
}

fn build_mcp_raw_feature_cache(
    bars: &[trading_alpha::Bar],
    range: std::ops::Range<usize>,
) -> McpRawCache {
    let feature_cache = trading_alpha::FeatureCache::build(bars);
    let mut rows = vec![[None; trading_alpha::BASE_FEATURE_COUNT]; bars.len()];
    let start = range.start.max(trading_alpha::MIN_HISTORY);
    let end = range.end.min(bars.len().saturating_sub(1));
    for i in start..end {
        for fi in 0..trading_alpha::BASE_FEATURE_COUNT {
            rows[i][fi] = trading_alpha::extract_raw_feature(bars, i, fi, &feature_cache);
        }
    }
    rows
}

fn count_decision_rows(
    bars: &[trading_alpha::Bar],
    raw_cache: &McpRawCache,
    range: std::ops::Range<usize>,
) -> usize {
    let end = range.end.min(bars.len());
    let mut count = 0;
    for i in range.start.max(trading_alpha::MIN_HISTORY)..end {
        if !trading_alpha::is_decision_hour(&bars[i]) {
            continue;
        }
        if raw_cache
            .get(i)
            .map(|row| row.iter().any(Option::is_some))
            .unwrap_or(false)
        {
            count += 1;
        }
    }
    count
}

fn flush_day(
    eval: &mut trading_alpha::StrategyEval,
    current_day_ms: i64,
    current_day_had_trade: bool,
    current_day_pnl: f64,
    cfg: trading_alpha::SynthConfig,
) {
    if current_day_ms < 0 || !current_day_had_trade {
        return;
    }
    eval.days_evaluated += 1;
    if current_day_pnl > 0.0 {
        eval.days_profitable += 1;
    }
    if current_day_pnl >= cfg.target_pnl_per_day {
        eval.days_target_hit += 1;
    }
    eval.day_pnl_distribution.push(current_day_pnl);
}

fn record_trade(
    eval: &mut trading_alpha::StrategyEval,
    direction: trading_alpha::Direction,
    pnl_points: f64,
) {
    eval.total_trades += 1;
    match direction {
        trading_alpha::Direction::Long => eval.long_trades += 1,
        trading_alpha::Direction::Short => eval.short_trades += 1,
    }
    if pnl_points > 0.0 {
        eval.winning_trades += 1;
    } else {
        eval.losing_trades += 1;
    }
    eval.total_pnl_points += pnl_points;
}

fn record_straddle(eval: &mut trading_alpha::StrategyEval, straddle: &trading_alpha::StraddleResult) {
    record_trade(eval, trading_alpha::Direction::Long, straddle.long.pnl_points);
    record_trade(eval, trading_alpha::Direction::Short, straddle.short.pnl_points);
}

fn predict_rule(rule: &Rule, value: i64) -> i64 {
    match rule.cmp {
        RuleCmp::Gte => (value >= rule.threshold) as i64,
        RuleCmp::Lte => (value <= rule.threshold) as i64,
    }
}

fn compare_eval(a: &trading_alpha::StrategyEval, b: &trading_alpha::StrategyEval) -> Ordering {
    score_eval(a)
        .partial_cmp(&score_eval(b))
        .unwrap_or(Ordering::Equal)
}

fn score_eval(eval: &trading_alpha::StrategyEval) -> f64 {
    let pf = eval.profit_factor();
    let pf = if pf.is_finite() { pf.min(10.0) } else { 10.0 };
    eval.pct_days_target_hit() * 1_000_000.0
        + eval.total_pnl_points * 10_000.0
        + pf * 1_000.0
        + eval.pct_winning_trades() * 10.0
        + eval.total_trades as f64
}

fn summarize_eval(eval: &trading_alpha::StrategyEval) -> EvalSummary {
    EvalSummary {
        days: eval.days_evaluated,
        trades: eval.total_trades,
        long_trades: eval.long_trades,
        short_trades: eval.short_trades,
        target_hit_pct: round2(eval.pct_days_target_hit()),
        pnl_points: round4(eval.total_pnl_points),
        profit_factor: round4(eval.profit_factor()),
        sharpe: round4(eval.sharpe_ratio(252.0)),
        max_drawdown_points: round4(eval.max_drawdown_points()),
        win_trade_pct: round2(eval.pct_winning_trades()),
    }
}

fn ratio_pct(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        (numerator as f64 * 100.0) / denominator as f64
    }
}

fn ns_per(total_ns: u128, units: usize) -> u128 {
    if units == 0 {
        0
    } else {
        total_ns / units as u128
    }
}

fn fmt_elapsed_ms_ns(elapsed: Duration) -> String {
    format!("{:.3} ms ({} ns)", elapsed.as_secs_f64() * 1000.0, elapsed.as_nanos())
}

fn round2(v: f64) -> f64 {
    if v.is_finite() {
        (v * 100.0).round() / 100.0
    } else {
        v
    }
}

fn round4(v: f64) -> f64 {
    if v.is_finite() {
        (v * 10_000.0).round() / 10_000.0
    } else {
        v
    }
}

fn resolve_path(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|e| format!("resolve current dir: {e}"))
}

fn resolve_job_title(title: Option<&str>, csv_path: &Path) -> String {
    title
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| {
            csv_path
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Forge compute".to_string())
}

fn jobs_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FORGE_JOBS_DIR") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        return Ok(PathBuf::from(path).join("jobs"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata)
            .join("com.forge.ui")
            .join("forge-store")
            .join("jobs"));
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(".forge-store").join("jobs"))
        .map_err(|e| format!("resolve jobs dir: {e}"))
}

fn discover_job_dirs() -> Result<Vec<PathBuf>, String> {
    let primary = jobs_dir()?;
    let mut dirs = vec![primary];
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        dirs.push(PathBuf::from(path).join("jobs"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(
            PathBuf::from(appdata)
                .join("com.forge.ui")
                .join("forge-store")
                .join("jobs"),
        );
    }
    let mut unique = Vec::new();
    for dir in dirs {
        if !unique.iter().any(|existing: &PathBuf| existing == &dir) {
            unique.push(dir);
        }
    }
    Ok(unique)
}

fn find_job_manifest_path(job_id: &str) -> Result<PathBuf, String> {
    validate_job_id(job_id)?;
    let filename = format!("{job_id}.json");
    for dir in discover_job_dirs()? {
        let path = dir.join(&filename);
        if path.exists() {
            return Ok(path);
        }
    }
    Ok(jobs_dir()?.join(filename))
}

fn list_jobs(limit: usize) -> Result<Vec<Value>, String> {
    let mut entries = Vec::new();
    for dir in discover_job_dirs()? {
        match fs::read_dir(&dir) {
            Ok(dir_entries) => {
                entries.extend(
                    dir_entries
                        .filter_map(Result::ok)
                        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
                        .filter_map(|e| {
                            let modified = e.metadata().ok()?.modified().ok()?;
                            Some((modified, e.path()))
                        }),
                );
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) => {}
            Err(err) => return Err(format!("read jobs dir '{}': {err}", dir.display())),
        }
    }
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = Vec::new();
    let mut seen_job_ids = std::collections::HashSet::new();
    for (_, path) in entries {
        let job_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        if !seen_job_ids.insert(job_id.clone()) {
            continue;
        }
        match fs::read(&path)
            .map_err(|e| format!("read job '{}': {e}", path.display()))
            .and_then(|bytes| {
                serde_json::from_slice::<Value>(&bytes)
                    .map_err(|e| format!("decode job '{}': {e}", path.display()))
            }) {
            Ok(value) => {
                let hidden = value
                    .get("history_hidden")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let kind = value.get("kind").and_then(Value::as_str).unwrap_or("");
                if hidden || matches!(kind, "custom_compute_program_run" | "visual_program_run") {
                    continue;
                }
                out.push(sanitize_job_value(value));
            }
            Err(error) => out.push(json!({
                "job_id": job_id,
                "status": "decode_error",
                "manifest_path": path.display().to_string(),
                "error": error
            })),
        }
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

fn read_job_summary(job_id: &str) -> Result<Value, String> {
    Ok(json!({
        "job": sanitize_job_value(read_job_value(job_id)?),
        "manifest_path": find_job_manifest_path(job_id)?.display().to_string(),
        "safe_next_call": "read { job_id:\"...\", kind:\"artifacts\" }",
        "do_not_read_source": true,
        "token_safety": token_safety(),
        "note": "read returns a sanitized manifest summary. Use artifacts for file references/hashes and logs for bounded cursor-based progress; do not open raw files directly."
    }))
}

fn sanitize_job_value(value: Value) -> Value {
    let Value::Object(mut obj) = value else {
        return value;
    };
    let removed = [
        "candles",
        "ohlc",
        "rows",
        "raw_rows",
        "data",
        "features",
        "feature_matrix",
        "labels",
        "predictions",
        "prediction_cache",
        "confluence_rows",
        "stage1_predictions",
        "stage2_predictions",
        "log",
        "logs",
        "full_log",
        "content",
        "source_content",
        "csv_content",
        "artifact_content",
    ];
    let mut stripped = Vec::new();
    for key in removed {
        if obj.remove(key).is_some() {
            stripped.push(key);
        }
    }
    if let Some(result_job) = obj.remove("result_job") {
        obj.insert("result_job".to_string(), sanitize_job_value(result_job));
    }
    obj.insert("manifest_sanitized".to_string(), json!(true));
    obj.insert("stripped_heavy_fields".to_string(), json!(stripped));
    Value::Object(obj)
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

fn job_id_arg<'a>(args: &'a Value, tool_name: &str) -> Result<&'a str, String> {
    args.get("job_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{tool_name} requires job_id"))
        .and_then(|job_id| {
            validate_job_id(job_id)?;
            Ok(job_id)
        })
}

fn job_manifest_path(job_id: &str) -> Result<PathBuf, String> {
    validate_job_id(job_id)?;
    Ok(jobs_dir()?.join(format!("{job_id}.json")))
}

fn read_job_value(job_id: &str) -> Result<Value, String> {
    let path = find_job_manifest_path(job_id)?;
    let bytes = fs::read(&path).map_err(|e| format!("read job '{}': {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("decode job '{}': {e}", path.display()))
}

fn persist_existing_job_value(job_id: &str, value: Value) -> Result<(), String> {
    let dir = jobs_dir()?;
    persist_job_value(&dir, job_id, value)
}

fn list_pending_jobs(limit: usize) -> Result<Vec<Value>, String> {
    let jobs = list_jobs(limit.saturating_mul(4).max(limit))?;
    let mut pending = Vec::new();
    for job in jobs {
        if job.get("status").and_then(Value::as_str) == Some("pending") {
            pending.push(job);
            if pending.len() >= limit {
                break;
            }
        }
    }
    Ok(pending)
}

fn list_documents(args: &Value) -> Result<Value, String> {
    let limit = bounded_limit(args.get("limit"), MCP_LIST_LIMIT_DEFAULT, MCP_LIST_LIMIT_MAX);
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let type_filter = args
        .get("type")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());

    let jobs = list_jobs(MCP_LIST_LIMIT_MAX.saturating_mul(4))?;
    let mut docs = Vec::new();
    for job in jobs {
        if job.get("status").and_then(Value::as_str) == Some("archived") {
            continue;
        }
        let label = document_label(&job);
        let file_type = document_file_type(&label);
        if let Some(q) = &query {
            if !label.to_lowercase().contains(q)
                && !job
                    .get("job_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(q)
            {
                continue;
            }
        }
        if let Some(kind) = &type_filter {
            if &file_type != kind {
                continue;
            }
        }

        let job_id = job.get("job_id").and_then(Value::as_str).unwrap_or("");
        let paths = source_file_paths(&job);
        let source_refs = paths
            .into_iter()
            .map(|path| file_artifact_value("source", path))
            .collect::<Result<Vec<_>, _>>()?;

        docs.push(json!({
            "job_id": job_id,
            "title": label,
            "type": file_type,
            "status": job.get("status").cloned().unwrap_or(Value::Null),
            "pinned": job.get("pinned").or_else(|| job.get("is_pinned")).cloned().unwrap_or(Value::Bool(false)),
            "created_ms": job.get("created_ms").or_else(|| job.get("createdMs")).cloned().unwrap_or(Value::Null),
            "last_modified_ms": job.get("last_modified_ms").or_else(|| job.get("lastModifiedMs")).cloned().unwrap_or(Value::Null),
            "bytes": job.get("bytes").or_else(|| job.get("file_bytes")).cloned().unwrap_or(Value::Null),
            "strategy_hash": job.get("strategy_hash").cloned().unwrap_or(Value::Null),
            "source_refs": source_refs,
            "content_included": false
        }));
        if docs.len() >= limit {
            break;
        }
    }

    Ok(json!({
        "documents": docs,
        "limit": limit,
        "query": query,
        "type": type_filter,
        "content_policy": {
            "source_content_included": false,
            "previews_only_via": "preview",
            "large_values_are_references": true
        }
    }))
}

fn document_summary(job_id: &str) -> Result<Value, String> {
    let job = sanitize_job_value(read_job_value(job_id)?);
    let artifacts = job_artifacts(job_id)?;
    Ok(json!({
        "document": {
            "job_id": job_id,
            "title": document_label(&job),
            "type": document_file_type(&document_label(&job)),
            "job": job,
            "artifacts": artifacts
        },
        "content_policy": {
            "source_content_included": false,
            "artifact_content_included": false,
            "use_preview_for_bounded_source_sample": true
        }
    }))
}

fn document_preview(args: &Value) -> Result<Value, String> {
    let job_id = job_id_arg(args, "preview")?;
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(MCP_DOC_PREVIEW_DEFAULT_BYTES as u64)
        .clamp(1, MCP_DOC_PREVIEW_MAX_BYTES as u64) as usize;
    let job = read_job_value(job_id)?;
    let source = source_file_paths(&job)
        .into_iter()
        .next()
        .ok_or_else(|| format!("document '{job_id}' has no source file reference"))?;
    let mut file = fs::File::open(&source)
        .map_err(|e| format!("open document source '{}': {e}", source.display()))?;
    let mut buf = vec![0u8; max_bytes];
    let bytes_read = file
        .read(&mut buf)
        .map_err(|e| format!("read document source '{}': {e}", source.display()))?;
    buf.truncate(bytes_read);
    let total_bytes = fs::metadata(&source)
        .map_err(|e| format!("metadata document source '{}': {e}", source.display()))?
        .len();
    Ok(json!({
        "job_id": job_id,
        "title": document_label(&job),
        "path": source.display().to_string(),
        "preview": String::from_utf8_lossy(&buf),
        "bytes_read": bytes_read,
        "max_bytes": max_bytes,
        "total_bytes": total_bytes,
        "truncated": (bytes_read as u64) < total_bytes,
        "content_policy": {
            "bounded_preview": true,
            "full_source_content_included": false,
            "max_preview_bytes": MCP_DOC_PREVIEW_MAX_BYTES
        }
    }))
}

fn document_sessions(args: &Value) -> Result<Value, String> {
    let limit = bounded_limit(args.get("limit"), MCP_LIST_LIMIT_DEFAULT, MCP_LIST_LIMIT_MAX);
    let reference_label = if let Some(job_id) = args.get("job_id").and_then(Value::as_str) {
        validate_job_id(job_id)?;
        document_label(&read_job_value(job_id)?)
    } else {
        args.get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "sessions requires job_id or title".to_string())?
            .to_string()
    };
    let needle = reference_label.to_lowercase();
    let mut sessions = Vec::new();
    for job in list_jobs(MCP_LIST_LIMIT_MAX.saturating_mul(4))? {
        if document_label(&job).to_lowercase() == needle {
            sessions.push(json!({
                "job_id": job.get("job_id").cloned().unwrap_or(Value::Null),
                "title": document_label(&job),
                "status": job.get("status").cloned().unwrap_or(Value::Null),
                "created_ms": job.get("created_ms").or_else(|| job.get("createdMs")).cloned().unwrap_or(Value::Null),
                "last_modified_ms": job.get("last_modified_ms").or_else(|| job.get("lastModifiedMs")).cloned().unwrap_or(Value::Null),
                "strategy_hash": job.get("strategy_hash").cloned().unwrap_or(Value::Null),
                "content_included": false
            }));
            if sessions.len() >= limit {
                break;
            }
        }
    }
    Ok(json!({
        "title": reference_label,
        "sessions": sessions,
        "limit": limit,
        "content_policy": {
            "manifests_are_sanitized": true,
            "source_content_included": false
        }
    }))
}

fn document_label(job: &Value) -> String {
    job.get("title")
        .and_then(Value::as_str)
        .or_else(|| job.get("original_file_name").and_then(Value::as_str))
        .or_else(|| {
            job.get("original_file_names")
                .and_then(Value::as_array)
                .and_then(|v| v.first())
                .and_then(Value::as_str)
        })
        .or_else(|| {
            job.get("file_path")
                .and_then(Value::as_str)
                .and_then(|p| Path::new(p).file_name())
                .and_then(|p| p.to_str())
        })
        .or_else(|| job.get("job_id").and_then(Value::as_str))
        .unwrap_or("Forge document")
        .to_string()
}

fn document_file_type(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower.ends_with(".csv") || lower.contains("csv") {
        "csv"
    } else if lower.ends_with(".json") || lower.contains("json") {
        "json"
    } else if lower.ends_with(".txt") || lower.contains("txt") {
        "txt"
    } else if lower.ends_with(".pdf") || lower.contains("pdf") {
        "pdf"
    } else {
        "generic"
    }
    .to_string()
}

fn run_pending_job(mut args: Value, client: &McpClientInfo) -> Result<Value, String> {
    let pending_job_id = job_id_arg(&args, "forge_job_run_pending")?.to_string();
    let mut pending = read_job_value(&pending_job_id)?;
    let status = pending.get("status").and_then(Value::as_str).unwrap_or("unknown");
    let retryable_failed = status == "failed"
        && pending_csv_path(&pending).is_some()
        && pending
            .get("kind")
            .and_then(Value::as_str)
            .map(|kind| kind.contains("pending") || kind.contains("upload"))
            .unwrap_or(true);
    if status != "pending" && !retryable_failed {
        return Err(format!(
            "pending job '{pending_job_id}' is not pending (status={status})"
        ));
    }
    let csv_path = pending_csv_path(&pending)
        .ok_or_else(|| format!("pending job '{pending_job_id}' does not contain a csv_path"))?;

    let launch_args = args
        .as_object_mut()
        .ok_or_else(|| "forge_job_run_pending arguments must be an object".to_string())?;
    launch_args.remove("job_id");
    launch_args.insert("csv_path".to_string(), json!(csv_path));
    if !launch_args.contains_key("title") {
        if let Some(title) = pending_title(&pending) {
            launch_args.insert("title".to_string(), json!(title));
        }
    }

    let strategy_args: AlphaStrategyArgs =
        serde_json::from_value(args).map_err(|e| format!("bad pending run arguments: {e}"))?;
    mark_pending_claimed(&mut pending, &pending_job_id, client, Some(&strategy_args))?;
    let pending_log_path = pending_log_path(&pending, &pending_job_id)?;
    match with_job_log_mirror(pending_log_path, || run_alpha_strategy(strategy_args, client)) {
        Ok(job) => {
            let alpha_job_value =
                serde_json::to_value(&job).map_err(|e| format!("encode alpha job: {e}"))?;
            mark_pending_finished(&pending_job_id, &mut pending, &alpha_job_value, None, client)?;
            Ok(json!({
                "pending_job_id": pending_job_id,
                "claimed": true,
                "status": "completed",
                "alpha_job_id": job.job_id,
                "strategy_hash": job.strategy_hash,
                "alpha_job": alpha_job_value,
                "safe_next_call": "read { job_id:\"...\", kind:\"artifacts\" }",
                "do_not_read_source": true,
                "token_safety": token_safety()
            }))
        }
        Err(err) => {
            mark_pending_finished(&pending_job_id, &mut pending, &Value::Null, Some(&err), client)?;
            Err(err)
        }
    }
}

fn pending_csv_path(job: &Value) -> Option<String> {
    job.get("file_path")
        .and_then(Value::as_str)
        .or_else(|| {
            job.get("mcp_hint")
                .and_then(|v| v.get("arguments"))
                .and_then(|v| v.get("csv_path"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            job.get("file_paths")
                .and_then(Value::as_array)
                .and_then(|v| v.first())
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

fn pending_title(job: &Value) -> Option<String> {
    job.get("title")
        .and_then(Value::as_str)
        .or_else(|| job.get("original_file_name").and_then(Value::as_str))
        .or_else(|| {
            job.get("original_file_names")
                .and_then(Value::as_array)
                .and_then(|v| v.first())
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn mark_pending_claimed(
    pending: &mut Value,
    pending_job_id: &str,
    client: &McpClientInfo,
    strategy_args: Option<&AlphaStrategyArgs>,
) -> Result<(), String> {
    let log_path = pending_log_path(pending, pending_job_id)?;
    if let Value::Object(obj) = pending {
        obj.insert("status".to_string(), json!("running"));
        obj.insert("claimed_ms".to_string(), json!(now_ms()));
        obj.insert("claimed_by".to_string(), json!(client));
        obj.insert("mcp_claimed".to_string(), json!(true));
    }
    persist_existing_job_value(pending_job_id, pending.clone())?;
    if let Some(args) = strategy_args {
        append_job_log_only(&log_path, &alpha_strategy_intro_line(args, client))?;
    } else {
        append_job_log_only(
            &log_path,
            &format!(
                "{} is taking over this session and preparing the calculation.",
                friendly_agent_name(client)
            ),
        )?;
    }
    append_job_log_only(&log_path, "Selecting a Forge program for this session...")?;
    append_job_log_only(
        &log_path,
        "Program selected: Market backtest and alpha signal synthesis.",
    )
}

fn mark_pending_finished(
    pending_job_id: &str,
    pending: &mut Value,
    alpha_job: &Value,
    error: Option<&str>,
    client: &McpClientInfo,
) -> Result<(), String> {
    let log_path = pending_log_path(pending, pending_job_id)?;
    if let Value::Object(obj) = pending {
        obj.insert("finished_ms".to_string(), json!(now_ms()));
        if let Some(err) = error {
            obj.insert("status".to_string(), json!("pending"));
            obj.insert("last_run_failed_ms".to_string(), json!(now_ms()));
            obj.insert("last_run_error".to_string(), json!(err));
            obj.insert("mcp_retryable".to_string(), json!(true));
        } else {
            obj.insert("status".to_string(), json!("completed"));
            obj.insert(
                "claimed_job_id".to_string(),
                alpha_job.get("job_id").cloned().unwrap_or(Value::Null),
            );
            obj.insert(
                "strategy_hash".to_string(),
                alpha_job.get("strategy_hash").cloned().unwrap_or(Value::Null),
            );
            obj.insert("result_job".to_string(), alpha_job.clone());
        }
    }
    persist_existing_job_value(pending_job_id, pending.clone())?;
    append_job_log_only(
        &log_path,
        error
            .map(|err| format!("{} could not finish the calculation: {err}", friendly_agent_name(client)))
            .unwrap_or_else(|| format!("{} finished the calculation. Results are ready in this session.", friendly_agent_name(client)))
            .as_str(),
    )
}

fn pending_log_path(job: &Value, job_id: &str) -> Result<PathBuf, String> {
    if let Some(path) = job.get("log_path").and_then(Value::as_str) {
        return Ok(PathBuf::from(path));
    }
    Ok(
        find_job_manifest_path(job_id)?
            .parent()
            .unwrap_or(&jobs_dir()?)
            .join(format!("{job_id}.log")),
    )
}

fn tail_job_log(args: &Value) -> Result<Value, String> {
    let job_id = job_id_arg(args, "forge_job_log_tail")?;
    let job = read_job_value(job_id).unwrap_or_else(|_| json!({ "job_id": job_id }));
    let log_path = pending_log_path(&job, job_id)?;
    let cursor = args.get("cursor").and_then(Value::as_u64).unwrap_or(0);
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(MCP_LOG_TAIL_DEFAULT_BYTES as u64)
        .clamp(1, MCP_LOG_TAIL_MAX_BYTES as u64) as usize;
    let mut file = match fs::File::open(&log_path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(json!({
                "job_id": job_id,
                "cursor": cursor,
                "next_cursor": cursor,
                "eof": true,
                "text": "",
                "bytes_read": 0,
                "max_bytes": max_bytes,
                "log_path": log_path.display().to_string(),
                "safe_next_call": "read { job_id:\"...\", kind:\"artifacts\" }",
                "do_not_read_source": true,
                "token_safety": token_safety()
            }));
        }
        Err(err) => return Err(format!("open log '{}': {err}", log_path.display())),
    };
    let len = file
        .metadata()
        .map_err(|e| format!("metadata log '{}': {e}", log_path.display()))?
        .len();
    let start = cursor.min(len);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| format!("seek log '{}': {e}", log_path.display()))?;
    let mut buf = vec![0u8; max_bytes.min(len.saturating_sub(start) as usize)];
    let bytes_read = file
        .read(&mut buf)
        .map_err(|e| format!("read log '{}': {e}", log_path.display()))?;
    buf.truncate(bytes_read);
    let next_cursor = start + bytes_read as u64;
    Ok(json!({
        "job_id": job_id,
        "cursor": cursor,
        "next_cursor": next_cursor,
        "eof": next_cursor >= len,
        "text": String::from_utf8_lossy(&buf),
        "bytes_read": bytes_read,
        "max_bytes": max_bytes,
        "truncated_by_limit": next_cursor < len,
        "log_bytes": len,
        "log_path": log_path.display().to_string(),
        "safe_next_call": if next_cursor >= len { "read { job_id:\"...\", kind:\"artifacts\" }" } else { "logs { job_id:\"...\", cursor:<next_cursor> }" },
        "do_not_read_source": true,
        "token_safety": token_safety()
    }))
}

fn job_artifacts(job_id: &str) -> Result<Value, String> {
    let job = read_job_value(job_id)?;
    let manifest_path = find_job_manifest_path(job_id)?;
    let log_path = pending_log_path(&job, job_id)?;
    let source_files = source_file_paths(&job)
        .into_iter()
        .map(|path| file_artifact_value("source_csv", path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut artifacts = vec![
        file_artifact_value("manifest", manifest_path)?,
        file_artifact_value("log", log_path)?,
    ];
    for key in [
        "result_path",
        "metrics_path",
        "proof_path",
        "verification_path",
        "visual_mapping_path",
    ] {
        if let Some(path) = job.get(key).and_then(Value::as_str) {
            artifacts.push(file_artifact_value(key, PathBuf::from(path))?);
        }
    }
    if let Some(path) = job.get("visualization_3d_index_path").and_then(Value::as_str) {
        artifacts.push(file_artifact_value("visualization_3d_index", PathBuf::from(path))?);
    }
    if let Some(items) = job.get("artifacts_3d").and_then(Value::as_array) {
        for item in items {
            let Some(path) = item.get("path").and_then(Value::as_str) else {
                continue;
            };
            let mut artifact = file_artifact_value("visualization_3d", PathBuf::from(path))?;
            if let (Value::Object(obj), Some(src)) = (&mut artifact, item.as_object()) {
                for key in [
                    "artifact_type",
                    "mode",
                    "format",
                    "mime",
                    "point_count",
                    "draw_mode",
                    "mcp_injectable",
                    "download",
                ] {
                    if let Some(value) = src.get(key) {
                        obj.insert(key.to_string(), value.clone());
                    }
                }
            }
            artifacts.push(artifact);
        }
    }
    Ok(json!({
        "job_id": job_id,
        "status": job.get("status").cloned().unwrap_or(Value::Null),
        "strategy_hash": job.get("strategy_hash").cloned().unwrap_or(Value::Null),
        "program_hash": job.get("program_hash").cloned().unwrap_or(Value::Null),
        "long_program_hash": job.get("long_program_hash").cloned().unwrap_or(Value::Null),
        "short_program_hash": job.get("short_program_hash").cloned().unwrap_or(Value::Null),
        "source_files": source_files,
        "artifacts": artifacts,
        "visualization_3d": job.get("visualization_3d").cloned().unwrap_or(Value::Null),
        "visual_mapping": job.get("visual_mapping").cloned().unwrap_or(Value::Null),
        "content_policy": {
            "source_content_included": false,
            "artifact_content_included": false,
            "full_log_included": false,
            "visualization_3d_content_included": false,
            "download_by_reference_only": true
        },
        "safe_next_call": "inject { job_id:\"...\" } if the agent needs to expose the compact result reference",
        "do_not_read_source": true,
        "token_safety": token_safety(),
        "verification": {
            "hash_algorithm": "forge_fnv1a64",
            "proof": [
                "source CSV hash is persisted in the manifest as file_hash",
                "strategy hash is derived from the selected strategy text",
                "manifest/log/source artifacts are hashed by forge_job_artifacts",
                "3D visualization artifacts are persisted as files and hashed by path, never inlined into LLM context",
                "visual mappings connect result metrics to display artifacts through content-addressed file references",
                "internal accounting logs are written to stderr, app logs remain client-facing compute logs"
            ]
        }
    }))
}

fn source_file_paths(job: &Value) -> Vec<PathBuf> {
    if let Some(paths) = job.get("file_paths").and_then(Value::as_array) {
        let out = paths
            .iter()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return out;
        }
    }
    job.get("file_path")
        .and_then(Value::as_str)
        .map(|path| vec![PathBuf::from(path)])
        .unwrap_or_default()
}

fn file_artifact_value(kind: &str, path: PathBuf) -> Result<Value, String> {
    let exists = path.exists();
    let bytes = if exists {
        Some(
            fs::metadata(&path)
                .map_err(|e| format!("metadata artifact '{}': {e}", path.display()))?
                .len(),
        )
    } else {
        None
    };
    let hash = if exists {
        Some(quick_file_hash_path(&path)?)
    } else {
        None
    };
    Ok(json!({
        "kind": kind,
        "path": path.display().to_string(),
        "exists": exists,
        "bytes": bytes,
        "hash_algorithm": "forge_fnv1a64",
        "hash": hash.map(|v| format!("{v:016x}"))
    }))
}

fn inject_job_result(args: &Value, client: &McpClientInfo) -> Result<Value, String> {
    let job_id = job_id_arg(args, "forge_job_inject_result")?;
    let note = args.get("note").and_then(Value::as_str).unwrap_or("");
    let mut job = read_job_value(job_id)?;
    let status = job.get("status").and_then(Value::as_str).unwrap_or("unknown");
    if !matches!(status, "completed" | "done" | "failed" | "aborted" | "cancelled" | "canceled") {
        return Err(format!("job '{job_id}' is not finished yet (status={status})"));
    }
    let visualization_3d = job.get("visualization_3d").cloned().unwrap_or(Value::Null);
    let artifacts_3d = job.get("artifacts_3d").cloned().unwrap_or_else(|| json!([]));
    let visual_mapping = job.get("visual_mapping").cloned().unwrap_or(Value::Null);
    if let Value::Object(obj) = &mut job {
        obj.insert("mcp_result_available".to_string(), json!(true));
        obj.insert(
            "mcp_result".to_string(),
            json!({
                "available": true,
                "injected_ms": now_ms(),
                "agent": client,
                "note": note,
                "delivery": "by_reference",
                "manifest": job_manifest_path(job_id)?.display().to_string()
                ,
                "visualization_3d": visualization_3d,
                "artifacts_3d": artifacts_3d,
                "visual_mapping": visual_mapping,
                "agent_delivery_guidance": [
                    "Use read { kind:'artifacts', job_id } to fetch downloadable visual mapping and 3D file references.",
                    "Do not inline .ply/.json metrics/proof artifacts into the chat context.",
                    "Attach/import the referenced files in the target agent application when supported."
                ]
            }),
        );
    }
    persist_existing_job_value(job_id, job.clone())?;
    Ok(json!({
        "job_id": job_id,
        "status": job.get("status").cloned().unwrap_or(Value::Null),
        "strategy_hash": job.get("strategy_hash").cloned().unwrap_or(Value::Null),
        "mcp_result_available": true,
        "job": sanitize_job_value(job)
    }))
}

fn update_job_title(args: &Value) -> Result<Value, String> {
    let job_id = job_id_arg(args, "forge_job_update_title")?;
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "forge_job_update_title requires a non-empty title".to_string())?;
    let mut job = read_job_value(job_id)?;
    if let Value::Object(obj) = &mut job {
        obj.insert("title".to_string(), json!(title));
        obj.insert("title_updated_ms".to_string(), json!(now_ms()));
    }
    persist_existing_job_value(job_id, job.clone())?;
    Ok(json!({ "job_id": job_id, "title": title, "job": sanitize_job_value(job) }))
}

fn request_job_cancel(args: &Value, client: &McpClientInfo) -> Result<Value, String> {
    let job_id = job_id_arg(args, "forge_job_cancel")?;
    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("cancel requested by MCP agent");
    let mut job = read_job_value(job_id)?;
    let status = job.get("status").and_then(Value::as_str).unwrap_or("unknown");
    let final_status = matches!(
        status,
        "completed" | "done" | "failed" | "aborted" | "cancelled" | "canceled"
    );
    if let Value::Object(obj) = &mut job {
        obj.insert("cancel_requested".to_string(), json!(true));
        obj.insert("cancel_requested_ms".to_string(), json!(now_ms()));
        obj.insert("cancel_requested_by".to_string(), json!(client));
        obj.insert("cancel_reason".to_string(), json!(reason));
        if !final_status {
            obj.insert("status".to_string(), json!("cancel_requested"));
        }
    }
    persist_existing_job_value(job_id, job.clone())?;
    let log_path = pending_log_path(&job, job_id)?;
    let _ = append_job_log_only(&log_path, &format!("cancel requested: {reason}"));
    Ok(json!({
        "job_id": job_id,
        "cancel_requested": true,
        "already_final": final_status,
        "status": job.get("status").cloned().unwrap_or(Value::Null),
        "note": "Current engine records cancellation in the manifest; cooperative compute interruption is the next runtime step."
    }))
}

fn persist_job_value(jobs_dir: &Path, job_id: &str, value: Value) -> Result<(), String> {
    let path = jobs_dir.join(format!("{job_id}.json"));
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&value).map_err(|e| format!("encode job manifest: {e}"))?;
    fs::write(&tmp, bytes).map_err(|e| format!("write job manifest tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("commit job manifest: {e}"))
}

fn mark_job_failed(
    jobs_dir: &Path,
    job_id: &str,
    csv_path: &Path,
    file_hash: u64,
    bars: Option<usize>,
    log_path: &Path,
    error: &str,
    client: &McpClientInfo,
) -> Result<(), String> {
    persist_job_value(
        jobs_dir,
        job_id,
        json!({
            "job_id": job_id,
            "title": resolve_job_title(None, csv_path),
            "kind": "alpha_strategy_from_csv",
            "status": "failed",
            "file_path": csv_path.display().to_string(),
            "file_hash": file_hash,
            "bars": bars,
            "strategy_hash": null,
            "error": error,
            "why_failed": error,
            "safe_next_call": "logs { job_id:\"...\", cursor:0 }",
            "suggested_retry": "Retry with run { job_id:\"...\", engine:\"threshold\" } or run { intent:\"...\", inputs:[...], plan_only:true } if the engine/params need adjustment.",
            "do_not_read_source": true,
            "log_path": log_path.display().to_string(),
            "agents": [client],
            "context_accounting": agent_context_accounting(
                client,
                fs::metadata(csv_path).map(|m| m.len() as usize).unwrap_or(0),
                fs::metadata(log_path).map(|m| m.len() as usize).unwrap_or(0)
            )
        }),
    )
}

fn push_job_log(path: &Path, log: &mut Vec<String>, line: String) -> Result<(), String> {
    log.push(line.clone());
    append_job_log_only(path, &line)?;
    let mirror_path = JOB_LOG_MIRROR.with(|slot| slot.borrow().clone());
    if let Some(mirror_path) = mirror_path {
        if mirror_path.as_path() != path {
            append_job_log_only(&mirror_path, &line)?;
        }
    }
    Ok(())
}

fn with_job_log_mirror<T>(
    mirror_path: PathBuf,
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let previous = JOB_LOG_MIRROR.with(|slot| slot.replace(Some(mirror_path)));
    let result = f();
    JOB_LOG_MIRROR.with(|slot| {
        slot.replace(previous);
    });
    result
}

fn append_job_log_only(path: &Path, line: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open job log '{}': {e}", path.display()))?;
    writeln!(file, "{line}").map_err(|e| format!("append job log '{}': {e}", path.display()))
}

fn quick_file_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn quick_file_hash_path(path: &Path) -> Result<u64, String> {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut file =
        fs::File::open(path).map_err(|e| format!("open artifact '{}': {e}", path.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut hash = FNV_OFFSET;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("hash artifact '{}': {e}", path.display()))?;
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

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn ok_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

#[derive(Debug, Clone, Copy)]
enum McpFraming {
    ContentLength,
    JsonLine,
}

struct McpMessage {
    body: Vec<u8>,
    framing: McpFraming,
}

fn read_mcp_message<R: Read>(reader: &mut R) -> Result<Option<McpMessage>, String> {
    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) if header.is_empty() => return Ok(None),
            Ok(0) => return Err("unexpected EOF while reading MCP header".to_string()),
            Ok(_) => {
                if header.is_empty() && byte[0].is_ascii_whitespace() {
                    continue;
                }
                if header.is_empty() && (byte[0] == b'{' || byte[0] == b'[') {
                    let mut body = vec![byte[0]];
                    loop {
                        match reader.read(&mut byte) {
                            Ok(0) => break,
                            Ok(_) => {
                                if byte[0] == b'\n' {
                                    break;
                                }
                                body.push(byte[0]);
                            }
                            Err(e) => return Err(format!("read MCP json line: {e}")),
                        }
                    }
                    while matches!(body.last(), Some(b'\r' | b'\n')) {
                        body.pop();
                    }
                    return Ok(Some(McpMessage {
                        body,
                        framing: McpFraming::JsonLine,
                    }));
                }
                header.push(byte[0]);
                if header.ends_with(b"\r\n\r\n") || header.ends_with(b"\n\n") {
                    break;
                }
            }
            Err(e) => return Err(format!("read MCP header: {e}")),
        }
    }
    let header_txt =
        std::str::from_utf8(&header).map_err(|e| format!("MCP header is not utf8: {e}"))?;
    let mut content_len = None;
    for line in header_txt.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_len = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|e| format!("bad Content-Length: {e}"))?,
                );
            }
        }
    }
    let len = content_len.ok_or_else(|| "missing Content-Length header".to_string())?;
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .map_err(|e| format!("read MCP body: {e}"))?;
    Ok(Some(McpMessage {
        body,
        framing: McpFraming::ContentLength,
    }))
}

fn write_mcp_message<W: Write>(
    writer: &mut W,
    value: &Value,
    framing: McpFraming,
) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|e| format!("encode MCP response: {e}"))?;
    match framing {
        McpFraming::ContentLength => {
            write!(writer, "Content-Length: {}\r\n\r\n", body.len())
                .map_err(|e| format!("write MCP header: {e}"))?;
            writer
                .write_all(&body)
                .map_err(|e| format!("write MCP body: {e}"))?;
        }
        McpFraming::JsonLine => {
            writer
                .write_all(&body)
                .map_err(|e| format!("write MCP json line body: {e}"))?;
            writer
                .write_all(b"\n")
                .map_err(|e| format!("write MCP json line newline: {e}"))?;
        }
    }
    writer.flush().map_err(|e| format!("flush MCP response: {e}"))
}

