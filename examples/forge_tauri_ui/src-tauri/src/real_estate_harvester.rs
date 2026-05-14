use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use scan::kasm::{Node, Program, Target, Ty};
use scan::{publish_semantic_attractor, Hash, MemoryGovernor, MonsterNode, Store};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HARVESTER_DIR: &str = "real-estate-harvester";
const DATA_DIR: &str = "data";
const RUNS_DIR: &str = "runs";
const INTEL_DIR: &str = "intel_packs";
const KASM_CONTRACTS_DIR: &str = "kasm_contracts";
const LLM_INTEL_CACHE_DIR: &str = "llm_intel_cache";
const PROPERTIES_FILE: &str = "properties.json";
const ZONES_FILE: &str = "zones.json";
const SOURCE_EVENTS_FILE: &str = "source_events.jsonl";
const METRIC_SNAPSHOTS_FILE: &str = "metric_snapshots.jsonl";
const LEDGER_FILE: &str = "ledger.jsonl";
const STATUS_FILE: &str = "status.json";
const SUPERVISOR_STATE_FILE: &str = "supervisor_state.json";
const JOB_QUEUE_FILE: &str = "job_queue.json";
const JOB_EVENTS_FILE: &str = "job_events.jsonl";
const LATEST_INTEL_FILE: &str = "latest.json";
const LATEST_LLM_INTEL_CACHE_FILE: &str = "latest.json";
const REAL_ESTATE_KASM_SCORE_CONTRACT_FILE: &str = "real_estate_score_core.json";
const BRAIN_LLM_NOTE_LATEST_REF: &str = "refs/brain/llm/latest";
const REAL_ESTATE_BRAIN_NOTE_REF: &str = "refs/brain/llm/agence-immo/latest";
const REAL_ESTATE_BRAIN_NOTE_LAYER_REF: &str =
    "refs/brain/llm/by_layer/agence-immo/semantic/latest";
const REAL_ESTATE_BRAIN_NOTE_INDEX_REF: &str =
    "refs/brain/llm/index/agence-immo/semantic/recent";
const REAL_ESTATE_INTEL_BRAIN_REF: &str = "refs/brain/real-estate/intel/latest";
const REAL_ESTATE_KASM_BRAIN_REF: &str = "refs/brain/real-estate/kasm-score/latest";
const DEFAULT_SCHEDULER_TICK_SECS: u64 = 60;
const SUPERVISOR_BACKOFF_BASE_MS: u64 = 15 * 60 * 1000;
const SUPERVISOR_BACKOFF_MAX_MS: u64 = 6 * 60 * 60 * 1000;
const SUPERVISOR_QUARANTINE_FAILURES: u32 = 5;
const SUPERVISOR_QUARANTINE_MS: u64 = 12 * 60 * 60 * 1000;
const JOB_QUEUE_LEASE_MS: u64 = 30 * 60 * 1000;
const JOB_QUEUE_MAX_ATTEMPTS: u32 = 4;
const JOB_QUEUE_HISTORY_LIMIT: usize = 512;
const JOB_EVENTS_SNAPSHOT_LIMIT: usize = 96;

const INTEL_METRIC_MANIFEST: [&str; 64] = [
    "dvf_price_gap",
    "listing_staleness",
    "dpe_renovation_gap",
    "energy_cost_pressure",
    "geo_clay_risk",
    "flood_risk",
    "permit_activity",
    "urbanism_upside",
    "transit_momentum",
    "school_momentum",
    "business_churn",
    "traffic_noise_delta",
    "pollution_delta",
    "weather_heat_stress",
    "mobility_inflow",
    "local_news_intensity",
    "competitor_pressure",
    "buyer_demand_match",
    "credit_rate_sensitivity",
    "insurance_risk",
    "tax_pressure_proxy",
    "crm_inactivity",
    "visit_intent",
    "owner_lifecycle_pressure",
    "rental_yield_gap",
    "work_cost_roi",
    "neighborhood_liquidity",
    "price_anchor_error",
    "time_on_market_shadow",
    "notary_delay_index",
    "seasonality_fit",
    "agency_reputation_fit",
    "electricity_price_stress",
    "gas_price_stress",
    "water_restriction_index",
    "heatwave_exposure",
    "remote_work_fit",
    "fiber_quality",
    "mobile_coverage_gap",
    "short_term_rental_pressure",
    "student_housing_demand",
    "hospital_access_score",
    "senior_services_density",
    "public_safety_trend",
    "succession_market_pressure",
    "mortgage_refusal_proxy",
    "unemployment_trend",
    "hiring_momentum",
    "tourism_flux",
    "event_calendar_density",
    "rail_disruption_risk",
    "fuel_price_pressure",
    "daily_services_access",
    "green_space_access",
    "sunlight_exposure",
    "slope_walkability",
    "parking_pressure",
    "ev_charging_access",
    "material_cost_pressure",
    "artisan_capacity",
    "renovation_subsidy_fit",
    "local_tax_vote_pressure",
    "insurance_premium_momentum",
    "climate_adaptation_gap",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDefinition {
    pub id: String,
    pub label: String,
    pub source_type: String,
    pub adapter: String,
    pub allowed_use: String,
    pub refresh: String,
    pub rate_limit: String,
    pub compliance: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectorDefinition {
    pub id: String,
    pub label: String,
    pub group: String,
    pub tools: Vec<String>,
    pub cadence: String,
    pub priority: u8,
    pub adapters: Vec<String>,
    pub source_ids: Vec<String>,
    pub output_contract: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarvesterRegistry {
    pub sources: Vec<SourceDefinition>,
    pub collectors: Vec<CollectorDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarvesterDaemonStatus {
    pub status: String,
    pub mode: String,
    pub background_ready: bool,
    pub scheduler_ready: bool,
    pub webview_worker_ready: bool,
    pub kasm_ready: bool,
    pub store_dir: String,
    pub updated_at_ms: u64,
    pub started_at_ms: u64,
    pub last_tick_ms: u64,
    pub runs_total: u64,
    pub errors_total: u64,
    pub last_error: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarvestRunReport {
    pub job_id: String,
    pub tool_id: String,
    pub collector_id: String,
    pub collector_label: String,
    pub status: String,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub planned_adapters: Vec<String>,
    pub source_ids: Vec<String>,
    pub normalized_outputs: Vec<String>,
    pub proof_hash: String,
    pub artifact_path: String,
    pub compliance_notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateSupervisorBudget {
    pub cpu_threads: usize,
    pub gpu_mode: String,
    pub max_jobs_per_tick: usize,
    pub max_estimated_cost_per_tick: usize,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
    pub quarantine_after_failures: u32,
    pub quarantine_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateCollectorSupervisorStatus {
    pub collector_id: String,
    pub label: String,
    pub group: String,
    pub cadence: String,
    pub priority: u8,
    pub status: String,
    pub freshness: String,
    pub budget_class: String,
    pub estimated_cost: usize,
    pub stale_after_ms: u64,
    pub last_started_ms: u64,
    pub last_finished_ms: u64,
    pub last_success_ms: u64,
    pub next_due_ms: u64,
    pub retry_after_ms: u64,
    pub quarantine_until_ms: u64,
    pub consecutive_failures: u32,
    pub total_failures: u64,
    pub total_successes: u64,
    pub last_error: String,
    pub last_proof_hash: String,
    pub last_artifact_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateSupervisorSnapshot {
    pub status: String,
    pub updated_at_ms: u64,
    pub state_hash: String,
    pub budget: RealEstateSupervisorBudget,
    pub collectors: Vec<RealEstateCollectorSupervisorStatus>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateJobQueueEntry {
    pub job_id: String,
    #[serde(default = "default_job_kind")]
    pub job_kind: String,
    pub collector_id: String,
    pub tool_id: String,
    pub trigger: String,
    pub status: String,
    pub priority: u8,
    pub estimated_cost: usize,
    pub scheduled_at_ms: u64,
    pub not_before_ms: u64,
    pub leased_until_ms: u64,
    pub attempts: u32,
    pub max_attempts: u32,
    pub depends_on: Vec<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub last_error: String,
    pub last_proof_hash: String,
    pub artifact_path: String,
}

#[derive(Clone, Debug)]
struct RealEstateJobStageArtifact {
    finished_at_ms: u64,
    proof_hash: String,
    artifact_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateJobQueueSnapshot {
    pub status: String,
    pub updated_at_ms: u64,
    pub queue_hash: String,
    pub pending: usize,
    pub running: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub jobs: Vec<RealEstateJobQueueEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateJobTimelineEvent {
    pub event_id: String,
    pub job_id: String,
    #[serde(default = "default_job_kind")]
    pub job_kind: String,
    pub collector_id: String,
    pub tool_id: String,
    pub stage: String,
    pub status: String,
    pub at_ms: u64,
    pub duration_ms: u64,
    pub estimated_cost: usize,
    pub attempt: u32,
    pub next_retry_ms: u64,
    pub blocked_by: Vec<String>,
    pub message: String,
    pub proof_hash: String,
    pub artifact_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateJobBlocker {
    pub job_id: String,
    #[serde(default = "default_job_kind")]
    pub job_kind: String,
    pub collector_id: String,
    pub reason: String,
    pub wait_until_ms: u64,
    pub missing_dependencies: Vec<String>,
    pub estimated_cost: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateJobJournalSnapshot {
    pub status: String,
    pub updated_at_ms: u64,
    pub latest_message: String,
    pub next_retry_ms: u64,
    pub blocked: usize,
    pub blocked_jobs: Vec<RealEstateJobBlocker>,
    pub events: Vec<RealEstateJobTimelineEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstatePropertyEntity {
    pub property_id: String,
    pub source: String,
    pub source_event_id: String,
    pub zone_id: String,
    pub city: String,
    pub postal_code: String,
    pub address_label: String,
    pub lat: f64,
    pub lon: f64,
    pub property_type: String,
    pub surface_m2: f64,
    pub rooms: f64,
    pub land_m2: f64,
    pub mutation_date: String,
    pub price_eur: f64,
    pub price_m2: f64,
    pub dpe_score: Option<f64>,
    pub risk_score: f64,
    pub updated_at_ms: u64,
    pub evidence_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateZoneEntity {
    pub zone_id: String,
    pub label: String,
    pub city: String,
    pub postal_code: String,
    pub property_count: usize,
    pub avg_price_m2: f64,
    pub median_price_m2: f64,
    pub liquidity_score: f64,
    pub updated_at_ms: u64,
    pub evidence_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateSourceEvent {
    pub event_id: String,
    pub source_id: String,
    pub collector_id: String,
    pub observed_at_ms: u64,
    pub source_hash: String,
    pub entity_refs: Vec<String>,
    pub artifact_path: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateKasmContract {
    pub contract_id: String,
    pub program_hash: String,
    pub semantic_fingerprint: String,
    pub canonical_hash: String,
    pub metric_manifest_hash: String,
    pub input_metrics: Vec<String>,
    pub output_contract: String,
    pub nodes: usize,
    pub byte_len: usize,
    pub fuel: u32,
    pub cache_key: String,
    pub artifact_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateMetricSnapshot {
    pub snapshot_id: String,
    pub property_id: String,
    pub zone_id: String,
    pub generated_at_ms: u64,
    pub metric_manifest_hash: String,
    #[serde(default)]
    pub kasm_contract_hash: String,
    pub metrics: Vec<f64>,
    pub score: f64,
    pub seller_probability: f64,
    pub expected_fee_eur: f64,
    pub strongest_signal: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateLocalStoreSummary {
    pub data_dir: String,
    pub properties: usize,
    pub zones: usize,
    pub source_events: usize,
    pub metric_snapshots: usize,
    pub latest_updated_at_ms: u64,
    pub data_hash: String,
}

#[derive(Clone, Debug, Default)]
struct LocalStoreUpdate {
    properties: usize,
    zones: usize,
    source_events: usize,
    metric_snapshots: usize,
    kasm_contract_hash: String,
    data_hash: String,
}

#[derive(Clone, Debug)]
struct LocalFeatureBuild {
    zones: Vec<RealEstateZoneEntity>,
    snapshots: Vec<RealEstateMetricSnapshot>,
    kasm_contract: RealEstateKasmContract,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateIntelOpportunity {
    pub property_id: String,
    pub zone_id: String,
    pub score: f64,
    pub seller_probability: f64,
    pub expected_fee_eur: f64,
    pub horizon_days: u16,
    pub strongest_signal: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateIntelPack {
    pub pack_id: String,
    pub status: String,
    pub generated_at_ms: u64,
    pub trigger: String,
    pub input_runs: usize,
    pub metric_count: usize,
    pub candidate_count: usize,
    pub scenario_count: usize,
    pub horizon_count: usize,
    pub work_items: usize,
    pub metric_manifest: Vec<String>,
    #[serde(default)]
    pub kasm_contract_hash: String,
    #[serde(default)]
    pub kasm_semantic_fingerprint: String,
    #[serde(default)]
    pub brain_note_hash: Option<String>,
    #[serde(default)]
    pub brain_ref: Option<String>,
    pub top_opportunities: Vec<RealEstateIntelOpportunity>,
    pub evidence_hash: String,
    pub artifact_path: String,
    pub llm_summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateLlmIntelOpportunity {
    pub rank: usize,
    pub property_id: String,
    pub zone_id: String,
    pub score: f64,
    pub seller_probability: f64,
    pub expected_fee_eur: f64,
    pub horizon_days: u16,
    pub strongest_signal: String,
    pub recommended_action: String,
    pub fact_line: String,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateLlmIntelCache {
    pub cache_id: String,
    pub status: String,
    pub generated_at_ms: u64,
    pub source_pack_id: String,
    pub source_pack_path: String,
    pub evidence_hash: String,
    pub metric_manifest_hash: String,
    pub kasm_contract_hash: String,
    pub kasm_semantic_fingerprint: String,
    pub brain_note_hash: Option<String>,
    pub brain_ref: Option<String>,
    pub local_store: RealEstateLocalStoreSummary,
    pub top_opportunities: Vec<RealEstateLlmIntelOpportunity>,
    pub prompt_context: String,
    pub action_brief: String,
    pub ingestion_policy: Vec<String>,
    pub cache_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarvesterSnapshot {
    pub daemon: HarvesterDaemonStatus,
    pub registry: HarvesterRegistry,
    pub latest_run: Option<HarvestRunReport>,
    pub latest_intel_pack: Option<RealEstateIntelPack>,
    pub latest_llm_intel_cache: Option<RealEstateLlmIntelCache>,
    pub supervisor: RealEstateSupervisorSnapshot,
    pub job_queue: RealEstateJobQueueSnapshot,
    pub job_journal: RealEstateJobJournalSnapshot,
    pub local_store: RealEstateLocalStoreSummary,
}

struct HarvesterDaemon {
    store_path: PathBuf,
    started_at_ms: u64,
    last_tick_ms: AtomicU64,
    runs_total: AtomicU64,
    errors_total: AtomicU64,
    running: AtomicBool,
    scheduler_ready: AtomicBool,
    last_error: Mutex<String>,
    last_run_by_collector: Mutex<HashMap<String, u64>>,
    supervisor: Mutex<RealEstateSupervisorSnapshot>,
    job_queue: Mutex<RealEstateJobQueueSnapshot>,
}

static HARVESTER_DAEMON: OnceLock<Arc<HarvesterDaemon>> = OnceLock::new();

pub fn start_background(store_path: PathBuf) -> Result<(), String> {
    let base = ensure_harvester_dirs(&store_path)?;
    if HARVESTER_DAEMON.get().is_some() {
        return Ok(());
    }
    let registry = default_registry();
    let supervisor = load_or_init_supervisor(&base, &registry)?;
    let job_queue = load_or_init_job_queue(&base, &registry)?;
    let last_run_by_collector = supervisor
        .collectors
        .iter()
        .filter(|status| status.last_success_ms > 0)
        .map(|status| (status.collector_id.clone(), status.last_success_ms))
        .collect::<HashMap<_, _>>();
    let daemon = Arc::new(HarvesterDaemon {
        store_path,
        started_at_ms: now_ms(),
        last_tick_ms: AtomicU64::new(0),
        runs_total: AtomicU64::new(0),
        errors_total: AtomicU64::new(0),
        running: AtomicBool::new(true),
        scheduler_ready: AtomicBool::new(false),
        last_error: Mutex::new(String::new()),
        last_run_by_collector: Mutex::new(last_run_by_collector),
        supervisor: Mutex::new(supervisor),
        job_queue: Mutex::new(job_queue),
    });
    HARVESTER_DAEMON
        .set(Arc::clone(&daemon))
        .map_err(|_| "real estate harvester daemon already initialized".to_string())?;
    thread::Builder::new()
        .name("forge-real-estate-harvester".to_string())
        .spawn(move || scheduler_loop(daemon))
        .map_err(|e| format!("spawn real estate harvester daemon: {e}"))?;
    Ok(())
}

#[allow(dead_code)]
pub fn default_store_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FORGE_STORE_DIR") {
        return Ok(PathBuf::from(path));
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

#[allow(dead_code)]
pub fn run_all_once(store_path: &Path) -> Result<Vec<HarvestRunReport>, String> {
    let registry = default_registry();
    let mut collectors = registry.collectors.clone();
    collectors.sort_by_key(|collector| collector.priority);
    let mut reports = Vec::with_capacity(collectors.len());
    for collector in &collectors {
        let tool_id = collector
            .tools
            .first()
            .cloned()
            .unwrap_or_else(|| collector.id.clone());
        reports.push(run_collector(store_path, &tool_id, collector, "headless_once")?);
    }
    let _ = refresh_intel_pack(store_path, &reports, "headless_once")?;
    Ok(reports)
}

pub fn snapshot(store_path: &Path) -> Result<HarvesterSnapshot, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let registry = default_registry();
    let job_queue = load_or_init_job_queue(&base, &registry)?;
    let job_journal = read_job_journal(&base, &job_queue)?;
    Ok(HarvesterSnapshot {
        daemon: daemon_status(&base),
        registry: registry.clone(),
        latest_run: read_latest_run(&base).ok().flatten(),
        latest_intel_pack: read_latest_intel_pack(&base).ok().flatten(),
        latest_llm_intel_cache: read_latest_llm_intel_cache(&base).ok().flatten(),
        supervisor: load_or_init_supervisor(&base, &registry)?,
        job_queue,
        job_journal,
        local_store: local_store_summary(&base).unwrap_or_default(),
    })
}

#[allow(dead_code)]
pub fn run_tool(store_path: &Path, tool_id: &str) -> Result<HarvestRunReport, String> {
    let normalized_tool = normalize_id(tool_id);
    if normalized_tool.is_empty() {
        return Err("real estate harvester requires a tool id".to_string());
    }
    let registry = default_registry();
    let collector = registry
        .collectors
        .iter()
        .find(|collector| collector.tools.iter().any(|tool| tool == &normalized_tool))
        .ok_or_else(|| format!("no real estate collector registered for tool: {normalized_tool}"))?;
    run_collector(store_path, &normalized_tool, collector, "manual_request")
}

fn scheduler_loop(daemon: Arc<HarvesterDaemon>) {
    daemon.scheduler_ready.store(true, Ordering::Relaxed);
    let registry = default_registry();
    let mut collectors = registry.collectors.clone();
    collectors.sort_by_key(|collector| collector.priority);
    loop {
        let tick_ms = now_ms();
        daemon.last_tick_ms.store(tick_ms, Ordering::Relaxed);
        write_runtime_status(&daemon);
        let mut cycle_reports = Vec::new();
        let mut jobs_started = 0usize;
        let mut estimated_cost = 0usize;
        let budget = supervisor_budget();
        if let Ok(base) = ensure_harvester_dirs(&daemon.store_path) {
            if let Ok(mut supervisor) = daemon.supervisor.lock() {
                reconcile_supervisor(&mut supervisor, &registry, tick_ms);
                supervisor.budget = budget.clone();
                let _ = write_supervisor_state(&base, &supervisor);
            }
            if let Ok(mut queue) = daemon.job_queue.lock() {
                reconcile_job_queue(&mut queue, &registry, tick_ms);
                let _ = write_job_queue(&base, &queue);
            }
        }
        loop {
            let Some(job) = lease_next_job(&daemon, &registry, tick_ms, &budget, estimated_cost)
            else {
                break;
            };
            if job_kind(&job) != "collector" {
                let job_cost = job.estimated_cost;
                if jobs_started >= budget.max_jobs_per_tick
                    || estimated_cost.saturating_add(job_cost) > budget.max_estimated_cost_per_tick
                {
                    release_job_lease(&daemon, &registry, &job.job_id, tick_ms);
                    break;
                }
                match run_pipeline_job(&daemon.store_path, &job) {
                    Ok(artifact) => {
                        record_job_success_artifact(
                            &daemon,
                            &registry,
                            &job.job_id,
                            artifact.finished_at_ms,
                            &artifact.proof_hash,
                            &artifact.artifact_path,
                        );
                        jobs_started += 1;
                        estimated_cost = estimated_cost.saturating_add(job_cost);
                    }
                    Err(err) => {
                        daemon.errors_total.fetch_add(1, Ordering::Relaxed);
                        record_job_failure(&daemon, &registry, &job.job_id, now_ms(), &err);
                        if let Ok(mut last_error) = daemon.last_error.lock() {
                            *last_error = err;
                        }
                    }
                }
                write_runtime_status(&daemon);
                continue;
            }
            let collector = collectors
                .iter()
                .find(|collector| collector.id == job.collector_id)
                .cloned();
            let Some(collector) = collector else {
                record_job_failure(
                    &daemon,
                    &registry,
                    &job.job_id,
                    tick_ms,
                    "collector not found for queued job",
                );
                continue;
            };
            let collector_cost = job.estimated_cost.max(estimated_collector_cost(&collector));
            if jobs_started >= budget.max_jobs_per_tick
                || estimated_cost.saturating_add(collector_cost)
                    > budget.max_estimated_cost_per_tick
            {
                release_job_lease(&daemon, &registry, &job.job_id, tick_ms);
                break;
            }
            record_supervisor_started(&daemon, &registry, &collector, tick_ms);
            match run_collector(&daemon.store_path, &job.tool_id, &collector, &job.trigger) {
                Ok(report) => {
                    daemon.runs_total.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut last) = daemon.last_run_by_collector.lock() {
                        last.insert(collector.id.clone(), report.finished_at_ms);
                    }
                    if let Ok(mut err) = daemon.last_error.lock() {
                        err.clear();
                    }
                    record_supervisor_success(&daemon, &registry, &collector, &report);
                    record_job_success(&daemon, &registry, &job.job_id, &report);
                    jobs_started += 1;
                    estimated_cost = estimated_cost.saturating_add(collector_cost);
                    cycle_reports.push(report);
                }
                Err(err) => {
                    daemon.errors_total.fetch_add(1, Ordering::Relaxed);
                    record_supervisor_failure(&daemon, &registry, &collector, tick_ms, &err);
                    record_job_failure(&daemon, &registry, &job.job_id, tick_ms, &err);
                    if let Ok(mut last_error) = daemon.last_error.lock() {
                        *last_error = err;
                    }
                }
            }
            write_runtime_status(&daemon);
        }
        if !cycle_reports.is_empty() {
            write_runtime_status(&daemon);
        }
        thread::sleep(Duration::from_secs(scheduler_tick_secs()));
    }
}

#[allow(dead_code)]
fn collector_due(daemon: &HarvesterDaemon, collector: &CollectorDefinition, now: u64) -> bool {
    let last_run = daemon
        .last_run_by_collector
        .lock()
        .ok()
        .and_then(|map| map.get(&collector.id).copied())
        .unwrap_or(0);
    if last_run == 0 {
        return true;
    }
    now.saturating_sub(last_run) >= cadence_ms(&collector.cadence)
}

fn cadence_ms(cadence: &str) -> u64 {
    match cadence {
        "hourly" => 60 * 60 * 1000,
        "daily" => 24 * 60 * 60 * 1000,
        "weekly" => 7 * 24 * 60 * 60 * 1000,
        "on_demand" => 6 * 60 * 60 * 1000,
        _ => 60 * 60 * 1000,
    }
}

fn scheduler_tick_secs() -> u64 {
    std::env::var("FORGE_REAL_ESTATE_HARVESTER_TICK_SECS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SCHEDULER_TICK_SECS)
        .clamp(5, 3_600)
}

fn supervisor_budget() -> RealEstateSupervisorBudget {
    let cpu_threads = std::thread::available_parallelism()
        .map(|threads| threads.get())
        .unwrap_or(4)
        .clamp(1, 16);
    RealEstateSupervisorBudget {
        cpu_threads,
        gpu_mode: std::env::var("FORGE_REAL_ESTATE_GPU_MODE")
            .unwrap_or_else(|_| "optional_kasm_acceleration".to_string()),
        max_jobs_per_tick: env_usize(
            "FORGE_REAL_ESTATE_MAX_JOBS_PER_TICK",
            cpu_threads.clamp(1, 4),
        )
        .clamp(1, 16),
        max_estimated_cost_per_tick: env_usize(
            "FORGE_REAL_ESTATE_MAX_COST_PER_TICK",
            cpu_threads.saturating_mul(28).max(48),
        )
        .clamp(16, 4096),
        backoff_base_ms: SUPERVISOR_BACKOFF_BASE_MS,
        backoff_max_ms: SUPERVISOR_BACKOFF_MAX_MS,
        quarantine_after_failures: SUPERVISOR_QUARANTINE_FAILURES,
        quarantine_ms: SUPERVISOR_QUARANTINE_MS,
    }
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn load_or_init_supervisor(
    base: &Path,
    registry: &HarvesterRegistry,
) -> Result<RealEstateSupervisorSnapshot, String> {
    let path = base.join(SUPERVISOR_STATE_FILE);
    let mut snapshot = if path.exists() {
        let bytes = fs::read(&path).map_err(|e| format!("read supervisor state: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("parse supervisor state: {e}"))?
    } else {
        RealEstateSupervisorSnapshot::default()
    };
    snapshot.budget = supervisor_budget();
    reconcile_supervisor(&mut snapshot, registry, now_ms());
    write_supervisor_state(base, &snapshot)?;
    Ok(snapshot)
}

fn reconcile_supervisor(
    snapshot: &mut RealEstateSupervisorSnapshot,
    registry: &HarvesterRegistry,
    now: u64,
) {
    let mut existing = snapshot
        .collectors
        .drain(..)
        .map(|status| (status.collector_id.clone(), status))
        .collect::<HashMap<_, _>>();
    let budget = if snapshot.budget.max_jobs_per_tick == 0 {
        supervisor_budget()
    } else {
        snapshot.budget.clone()
    };
    let mut collectors = Vec::with_capacity(registry.collectors.len());
    for collector in &registry.collectors {
        let mut status = existing
            .remove(&collector.id)
            .unwrap_or_else(|| supervisor_status_from_collector(collector));
        status.label = collector.label.clone();
        status.group = collector.group.clone();
        status.cadence = collector.cadence.clone();
        status.priority = collector.priority;
        status.estimated_cost = estimated_collector_cost(collector);
        status.stale_after_ms = cadence_ms(&collector.cadence);
        let anchor = status.last_success_ms.max(status.last_finished_ms);
        status.next_due_ms = if anchor == 0 {
            now
        } else {
            anchor.saturating_add(cadence_ms(&collector.cadence))
        };
        status.freshness = if status.last_success_ms == 0 {
            "never_ran".to_string()
        } else if now >= status.next_due_ms {
            "stale".to_string()
        } else {
            "fresh".to_string()
        };
        if status.quarantine_until_ms > now {
            status.status = "quarantined".to_string();
        } else if status.retry_after_ms > now {
            status.status = "backoff".to_string();
        } else if now >= status.next_due_ms {
            status.status = "due".to_string();
        } else if status.status == "running" {
            status.status = "running".to_string();
        } else {
            status.status = "idle".to_string();
        }
        status.budget_class = if status.estimated_cost >= budget.max_estimated_cost_per_tick / 2 {
            "heavy".to_string()
        } else if status.estimated_cost >= 6 {
            "medium".to_string()
        } else {
            "light".to_string()
        };
        collectors.push(status);
    }
    collectors.sort_by_key(|status| (status.priority, status.collector_id.clone()));
    snapshot.status = "active".to_string();
    snapshot.updated_at_ms = now;
    snapshot.budget = budget;
    snapshot.collectors = collectors;
    snapshot.state_hash = supervisor_state_hash(snapshot);
}

fn supervisor_status_from_collector(
    collector: &CollectorDefinition,
) -> RealEstateCollectorSupervisorStatus {
    RealEstateCollectorSupervisorStatus {
        collector_id: collector.id.clone(),
        label: collector.label.clone(),
        group: collector.group.clone(),
        cadence: collector.cadence.clone(),
        priority: collector.priority,
        status: "idle".to_string(),
        freshness: "never_ran".to_string(),
        budget_class: "light".to_string(),
        estimated_cost: estimated_collector_cost(collector),
        stale_after_ms: cadence_ms(&collector.cadence),
        last_started_ms: 0,
        last_finished_ms: 0,
        last_success_ms: 0,
        next_due_ms: 0,
        retry_after_ms: 0,
        quarantine_until_ms: 0,
        consecutive_failures: 0,
        total_failures: 0,
        total_successes: 0,
        last_error: String::new(),
        last_proof_hash: String::new(),
        last_artifact_path: String::new(),
    }
}

fn write_supervisor_state(
    base: &Path,
    snapshot: &RealEstateSupervisorSnapshot,
) -> Result<(), String> {
    let mut snapshot = snapshot.clone();
    snapshot.state_hash = supervisor_state_hash(&snapshot);
    write_json_pretty(&base.join(SUPERVISOR_STATE_FILE), &snapshot, "supervisor state")
}

fn supervisor_state_hash(snapshot: &RealEstateSupervisorSnapshot) -> String {
    let payload = json!({
        "status": snapshot.status,
        "updatedAtMs": snapshot.updated_at_ms,
        "budget": snapshot.budget,
        "collectors": snapshot.collectors
    });
    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn estimated_collector_cost(collector: &CollectorDefinition) -> usize {
    let adapter_cost = collector
        .adapters
        .iter()
        .map(|adapter| match adapter.as_str() {
            "webview_render_adapter" | "http_crawler" => 4,
            "kasm" => 3,
            "oauth_api" | "api_http" => 2,
            _ => 1,
        })
        .sum::<usize>();
    adapter_cost
        .saturating_add(collector.source_ids.len())
        .saturating_add((6usize).saturating_sub(collector.priority as usize))
        .max(1)
}

fn collector_due_from_supervisor(
    daemon: &HarvesterDaemon,
    collector: &CollectorDefinition,
    now: u64,
) -> bool {
    daemon
        .supervisor
        .lock()
        .ok()
        .and_then(|snapshot| {
            snapshot
                .collectors
                .iter()
                .find(|status| status.collector_id == collector.id)
                .cloned()
        })
        .map(|status| {
            now >= status.next_due_ms
                && now >= status.retry_after_ms
                && now >= status.quarantine_until_ms
                && status.status != "running"
        })
        .unwrap_or_else(|| collector_due(daemon, collector, now))
}

fn record_supervisor_started(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    collector: &CollectorDefinition,
    started_ms: u64,
) {
    update_supervisor(daemon, registry, started_ms, |status| {
        if status.collector_id == collector.id {
            status.status = "running".to_string();
            status.last_started_ms = started_ms;
        }
    });
}

fn record_supervisor_success(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    collector: &CollectorDefinition,
    report: &HarvestRunReport,
) {
    update_supervisor(daemon, registry, report.finished_at_ms, |status| {
        if status.collector_id == collector.id {
            status.status = "idle".to_string();
            status.freshness = "fresh".to_string();
            status.last_finished_ms = report.finished_at_ms;
            status.last_success_ms = report.finished_at_ms;
            status.next_due_ms = report.finished_at_ms.saturating_add(cadence_ms(&status.cadence));
            status.retry_after_ms = 0;
            status.quarantine_until_ms = 0;
            status.consecutive_failures = 0;
            status.total_successes = status.total_successes.saturating_add(1);
            status.last_error.clear();
            status.last_proof_hash = report.proof_hash.clone();
            status.last_artifact_path = report.artifact_path.clone();
        }
    });
}

fn record_supervisor_failure(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    collector: &CollectorDefinition,
    failed_ms: u64,
    error: &str,
) {
    update_supervisor(daemon, registry, failed_ms, |status| {
        if status.collector_id == collector.id {
            status.last_finished_ms = failed_ms;
            status.consecutive_failures = status.consecutive_failures.saturating_add(1);
            status.total_failures = status.total_failures.saturating_add(1);
            status.last_error = error.chars().take(320).collect();
            let budget = supervisor_budget();
            let shift = status.consecutive_failures.saturating_sub(1).min(8);
            let backoff = budget
                .backoff_base_ms
                .saturating_mul(1_u64 << shift)
                .min(budget.backoff_max_ms);
            status.retry_after_ms = failed_ms.saturating_add(backoff);
            if status.consecutive_failures >= budget.quarantine_after_failures {
                status.status = "quarantined".to_string();
                status.quarantine_until_ms = failed_ms.saturating_add(budget.quarantine_ms);
            } else {
                status.status = "backoff".to_string();
            }
        }
    });
}

fn update_supervisor<F>(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    now: u64,
    update: F,
) where
    F: Fn(&mut RealEstateCollectorSupervisorStatus),
{
    let Ok(mut snapshot) = daemon.supervisor.lock() else {
        return;
    };
    reconcile_supervisor(&mut snapshot, registry, now);
    for status in &mut snapshot.collectors {
        update(status);
    }
    snapshot.updated_at_ms = now;
    snapshot.state_hash = supervisor_state_hash(&snapshot);
    if let Ok(base) = ensure_harvester_dirs(&daemon.store_path) {
        let _ = write_supervisor_state(&base, &snapshot);
    }
}

fn load_or_init_job_queue(
    base: &Path,
    registry: &HarvesterRegistry,
) -> Result<RealEstateJobQueueSnapshot, String> {
    let path = data_path(base, JOB_QUEUE_FILE);
    let mut queue = if path.exists() {
        let bytes = fs::read(&path).map_err(|e| format!("read job queue: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("parse job queue: {e}"))?
    } else {
        RealEstateJobQueueSnapshot::default()
    };
    reconcile_job_queue(&mut queue, registry, now_ms());
    write_job_queue(base, &queue)?;
    Ok(queue)
}

fn reconcile_job_queue(
    queue: &mut RealEstateJobQueueSnapshot,
    registry: &HarvesterRegistry,
    now: u64,
) {
    for job in &mut queue.jobs {
        if job.status == "running" && job.leased_until_ms <= now {
            job.status = "pending".to_string();
            job.leased_until_ms = 0;
            job.not_before_ms = now;
            job.updated_at_ms = now;
        }
    }
    let mut existing_open = queue
        .jobs
        .iter()
        .filter(|job| matches!(job.status.as_str(), "pending" | "running"))
        .map(|job| job.collector_id.clone())
        .collect::<std::collections::HashSet<_>>();
    for collector in &registry.collectors {
        if existing_open.contains(&collector.id) {
            continue;
        }
        queue.jobs.push(job_from_collector(collector, now));
        existing_open.insert(collector.id.clone());
    }
    ensure_pipeline_jobs(queue, now);
    trim_job_queue_history(queue);
    refresh_job_queue_summary(queue, now);
}

fn ensure_pipeline_jobs(queue: &mut RealEstateJobQueueSnapshot, now: u64) {
    let last_intel_ms = latest_job_of_kind(queue, "intel_pack")
        .map(|job| job.updated_at_ms.max(job.created_at_ms))
        .unwrap_or(0);
    let collector_deps = queue
        .jobs
        .iter()
        .filter(|job| {
            job_kind(job) == "collector"
                && job.status == "succeeded"
                && !job.last_proof_hash.is_empty()
                && job.updated_at_ms > last_intel_ms
        })
        .take(64)
        .map(|job| job.job_id.clone())
        .collect::<Vec<_>>();
    if !collector_deps.is_empty()
        && !open_job_of_kind(queue, "intel_pack")
        && !pipeline_job_exists(queue, "intel_pack", &collector_deps)
    {
        queue
            .jobs
            .push(job_from_pipeline("intel_pack", collector_deps, 7, 18, now));
    }

    if let Some(intel_job_id) = latest_succeeded_job_id(queue, "intel_pack") {
        let deps = vec![intel_job_id];
        if !open_job_of_kind(queue, "llm_cache") && !pipeline_job_exists(queue, "llm_cache", &deps)
        {
            queue
                .jobs
                .push(job_from_pipeline("llm_cache", deps, 8, 8, now));
        }
    }

    if let Some(cache_job_id) = latest_succeeded_job_id(queue, "llm_cache") {
        let deps = vec![cache_job_id];
        if !open_job_of_kind(queue, "brain_publish")
            && !pipeline_job_exists(queue, "brain_publish", &deps)
        {
            queue
                .jobs
                .push(job_from_pipeline("brain_publish", deps, 9, 6, now));
        }
    }
}

fn job_from_collector(collector: &CollectorDefinition, now: u64) -> RealEstateJobQueueEntry {
    let tool_id = collector
        .tools
        .first()
        .cloned()
        .unwrap_or_else(|| collector.id.clone());
    let seed = format!("{}:{}:{}:{}", collector.id, tool_id, collector.priority, now);
    RealEstateJobQueueEntry {
        job_id: format!("rej-{}-{}", now, short_hash(&seed)),
        job_kind: "collector".to_string(),
        collector_id: collector.id.clone(),
        tool_id,
        trigger: "continuous_queue".to_string(),
        status: "pending".to_string(),
        priority: collector.priority,
        estimated_cost: estimated_collector_cost(collector),
        scheduled_at_ms: now,
        not_before_ms: now,
        leased_until_ms: 0,
        attempts: 0,
        max_attempts: JOB_QUEUE_MAX_ATTEMPTS,
        depends_on: Vec::new(),
        created_at_ms: now,
        updated_at_ms: now,
        last_error: String::new(),
        last_proof_hash: String::new(),
        artifact_path: String::new(),
    }
}

fn job_from_pipeline(
    job_kind: &str,
    depends_on: Vec<String>,
    priority: u8,
    estimated_cost: usize,
    now: u64,
) -> RealEstateJobQueueEntry {
    let seed = format!("{job_kind}:{}:{now}", depends_on.join(","));
    RealEstateJobQueueEntry {
        job_id: format!("rej-{}-{}", now, short_hash(&seed)),
        job_kind: job_kind.to_string(),
        collector_id: job_kind.to_string(),
        tool_id: job_kind.to_string(),
        trigger: "continuous_queue".to_string(),
        status: "pending".to_string(),
        priority,
        estimated_cost,
        scheduled_at_ms: now,
        not_before_ms: now,
        leased_until_ms: 0,
        attempts: 0,
        max_attempts: JOB_QUEUE_MAX_ATTEMPTS,
        depends_on,
        created_at_ms: now,
        updated_at_ms: now,
        last_error: String::new(),
        last_proof_hash: String::new(),
        artifact_path: String::new(),
    }
}

fn lease_next_job(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    now: u64,
    budget: &RealEstateSupervisorBudget,
    used_cost: usize,
) -> Option<RealEstateJobQueueEntry> {
    let base = ensure_harvester_dirs(&daemon.store_path).ok()?;
    let mut queue = daemon.job_queue.lock().ok()?;
    reconcile_job_queue(&mut queue, registry, now);
    let succeeded = queue
        .jobs
        .iter()
        .filter(|job| job.status == "succeeded")
        .map(|job| job.job_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut best_idx = None;
    for (idx, job) in queue.jobs.iter().enumerate() {
        if job.status != "pending"
            || job.not_before_ms > now
            || job.attempts >= job.max_attempts
            || used_cost.saturating_add(job.estimated_cost) > budget.max_estimated_cost_per_tick
            || !job.depends_on.iter().all(|dep| succeeded.contains(dep))
        {
            continue;
        }
        if job_kind(job) == "collector" {
            let Some(collector) = registry
                .collectors
                .iter()
                .find(|collector| collector.id == job.collector_id)
            else {
                continue;
            };
            if !collector_due_from_supervisor(daemon, collector, now) {
                continue;
            }
        }
        if best_idx
            .map(|best: usize| job_queue_order_key(job) < job_queue_order_key(&queue.jobs[best]))
            .unwrap_or(true)
        {
            best_idx = Some(idx);
        }
    }
    let idx = best_idx?;
    queue.jobs[idx].status = "running".to_string();
    queue.jobs[idx].attempts = queue.jobs[idx].attempts.saturating_add(1);
    queue.jobs[idx].leased_until_ms = now.saturating_add(JOB_QUEUE_LEASE_MS);
    queue.jobs[idx].updated_at_ms = now;
    let job = queue.jobs[idx].clone();
    refresh_job_queue_summary(&mut queue, now);
    let _ = write_job_queue(&base, &queue);
    let _ = append_job_event(
        &base,
        job_timeline_event(
            &job,
            "leased",
            "running",
            now,
            format!("Lease accordee jusqu'a {}", job.leased_until_ms),
        ),
    );
    Some(job)
}

fn release_job_lease(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    job_id: &str,
    now: u64,
) {
    update_job_queue(daemon, registry, now, |job| {
        if job.job_id == job_id && job.status == "running" {
            job.status = "pending".to_string();
            job.leased_until_ms = 0;
            job.not_before_ms = now;
            job.updated_at_ms = now;
        }
    });
    append_current_job_event(daemon, job_id, "released", now, "Budget atteint; job remis en attente");
}

fn record_job_success(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    job_id: &str,
    report: &HarvestRunReport,
) {
    record_job_success_artifact(
        daemon,
        registry,
        job_id,
        report.finished_at_ms,
        &report.proof_hash,
        &report.artifact_path,
    );
}

fn record_job_success_artifact(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    job_id: &str,
    finished_at_ms: u64,
    proof_hash: &str,
    artifact_path: &str,
) {
    update_job_queue(daemon, registry, finished_at_ms, |job| {
        if job.job_id == job_id {
            job.status = "succeeded".to_string();
            job.leased_until_ms = 0;
            job.updated_at_ms = finished_at_ms;
            job.last_error.clear();
            job.last_proof_hash = proof_hash.to_string();
            job.artifact_path = artifact_path.to_string();
        }
    });
    append_current_job_event(
        daemon,
        job_id,
        "succeeded",
        finished_at_ms,
        "Job termine avec artefact et preuve",
    );
}

fn record_job_failure(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    job_id: &str,
    now: u64,
    error: &str,
) {
    update_job_queue(daemon, registry, now, |job| {
        if job.job_id == job_id {
            job.leased_until_ms = 0;
            job.updated_at_ms = now;
            job.last_error = error.chars().take(420).collect();
            if job.attempts >= job.max_attempts {
                job.status = "failed".to_string();
            } else {
                let shift = job.attempts.saturating_sub(1).min(6);
                job.status = "pending".to_string();
                job.not_before_ms = now.saturating_add(
                    SUPERVISOR_BACKOFF_BASE_MS
                        .saturating_mul(1_u64 << shift)
                        .min(SUPERVISOR_BACKOFF_MAX_MS),
                );
            }
        }
    });
    append_current_job_event(daemon, job_id, "failed", now, error);
}

fn update_job_queue<F>(
    daemon: &HarvesterDaemon,
    registry: &HarvesterRegistry,
    now: u64,
    update: F,
) where
    F: Fn(&mut RealEstateJobQueueEntry),
{
    let Ok(mut queue) = daemon.job_queue.lock() else {
        return;
    };
    reconcile_job_queue(&mut queue, registry, now);
    for job in &mut queue.jobs {
        update(job);
    }
    trim_job_queue_history(&mut queue);
    refresh_job_queue_summary(&mut queue, now);
    if let Ok(base) = ensure_harvester_dirs(&daemon.store_path) {
        let _ = write_job_queue(&base, &queue);
    }
}

fn trim_job_queue_history(queue: &mut RealEstateJobQueueSnapshot) {
    let closed_count = queue
        .jobs
        .iter()
        .filter(|job| matches!(job.status.as_str(), "succeeded" | "failed"))
        .count();
    if closed_count <= JOB_QUEUE_HISTORY_LIMIT {
        return;
    }
    let mut closed_seen = 0usize;
    let remove_until = closed_count.saturating_sub(JOB_QUEUE_HISTORY_LIMIT);
    queue.jobs.retain(|job| {
        if matches!(job.status.as_str(), "succeeded" | "failed") {
            closed_seen += 1;
            closed_seen > remove_until
        } else {
            true
        }
    });
}

fn refresh_job_queue_summary(queue: &mut RealEstateJobQueueSnapshot, now: u64) {
    queue.updated_at_ms = now;
    queue.pending = queue.jobs.iter().filter(|job| job.status == "pending").count();
    queue.running = queue.jobs.iter().filter(|job| job.status == "running").count();
    queue.succeeded = queue.jobs.iter().filter(|job| job.status == "succeeded").count();
    queue.failed = queue.jobs.iter().filter(|job| job.status == "failed").count();
    queue.status = if queue.running > 0 {
        "running".to_string()
    } else if queue.pending > 0 {
        "pending".to_string()
    } else if queue.failed > 0 {
        "degraded".to_string()
    } else {
        "idle".to_string()
    };
    queue.queue_hash = job_queue_hash(queue);
}

fn write_job_queue(base: &Path, queue: &RealEstateJobQueueSnapshot) -> Result<(), String> {
    let mut queue = queue.clone();
    queue.queue_hash = job_queue_hash(&queue);
    write_json_pretty(&data_path(base, JOB_QUEUE_FILE), &queue, "job queue")?;
    append_missing_queued_events(base, &queue)?;
    Ok(())
}

fn job_queue_hash(queue: &RealEstateJobQueueSnapshot) -> String {
    let mut parts = vec![
        queue.status.clone(),
        queue.pending.to_string(),
        queue.running.to_string(),
        queue.succeeded.to_string(),
        queue.failed.to_string(),
    ];
    for job in &queue.jobs {
        parts.extend([
            job.job_id.clone(),
            job_kind(job).to_string(),
            job.collector_id.clone(),
            job.status.clone(),
            job.priority.to_string(),
            job.attempts.to_string(),
            job.not_before_ms.to_string(),
            job.leased_until_ms.to_string(),
            job.last_proof_hash.clone(),
        ]);
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    hash_parts("real_estate_job_queue:v1", &refs)
}

fn job_queue_order_key(job: &RealEstateJobQueueEntry) -> (u8, u64, usize, String) {
    (
        job.priority,
        job.not_before_ms.max(job.scheduled_at_ms),
        job.estimated_cost,
        job.job_id.clone(),
    )
}

fn append_missing_queued_events(
    base: &Path,
    queue: &RealEstateJobQueueSnapshot,
) -> Result<(), String> {
    let known = read_job_event_ids(base)?;
    for job in &queue.jobs {
        let event_id = stable_job_event_id(&job.job_id, "queued");
        if known.contains(&event_id) {
            continue;
        }
        append_job_event(
            base,
            RealEstateJobTimelineEvent {
                event_id,
                job_id: job.job_id.clone(),
                job_kind: job_kind(job).to_string(),
                collector_id: job.collector_id.clone(),
                tool_id: job.tool_id.clone(),
                stage: "queued".to_string(),
                status: job.status.clone(),
                at_ms: job.created_at_ms,
                duration_ms: 0,
                estimated_cost: job.estimated_cost,
                attempt: job.attempts,
                next_retry_ms: job.not_before_ms,
                blocked_by: job.depends_on.clone(),
                message: if job.depends_on.is_empty() {
                    "Job ajoute a la queue durable".to_string()
                } else {
                    format!("Job ajoute avec {} dependances", job.depends_on.len())
                },
                proof_hash: job.last_proof_hash.clone(),
                artifact_path: job.artifact_path.clone(),
            },
        )?;
    }
    Ok(())
}

fn append_current_job_event(
    daemon: &HarvesterDaemon,
    job_id: &str,
    stage: &str,
    at_ms: u64,
    message: &str,
) {
    let Ok(base) = ensure_harvester_dirs(&daemon.store_path) else {
        return;
    };
    let Ok(Some(queue)) = read_job_queue_file(&base) else {
        return;
    };
    if let Some(job) = queue.jobs.iter().find(|job| job.job_id == job_id) {
        let _ = append_job_event(
            &base,
            job_timeline_event(job, stage, &job.status, at_ms, message.to_string()),
        );
    }
}

fn append_job_event(base: &Path, event: RealEstateJobTimelineEvent) -> Result<(), String> {
    append_json_line(&data_path(base, JOB_EVENTS_FILE), &event)
}

fn job_timeline_event(
    job: &RealEstateJobQueueEntry,
    stage: &str,
    status: &str,
    at_ms: u64,
    message: String,
) -> RealEstateJobTimelineEvent {
    RealEstateJobTimelineEvent {
        event_id: format!(
            "je-{}-{}-{}",
            at_ms,
            sanitize_filename(stage),
            short_hash(&format!("{}:{}:{}", job.job_id, stage, job.attempts))
        ),
        job_id: job.job_id.clone(),
        job_kind: job_kind(job).to_string(),
        collector_id: job.collector_id.clone(),
        tool_id: job.tool_id.clone(),
        stage: stage.to_string(),
        status: status.to_string(),
        at_ms,
        duration_ms: at_ms.saturating_sub(job.created_at_ms),
        estimated_cost: job.estimated_cost,
        attempt: job.attempts,
        next_retry_ms: if job.not_before_ms > at_ms {
            job.not_before_ms
        } else {
            0
        },
        blocked_by: job.depends_on.clone(),
        message,
        proof_hash: job.last_proof_hash.clone(),
        artifact_path: job.artifact_path.clone(),
    }
}

fn read_job_journal(
    base: &Path,
    queue: &RealEstateJobQueueSnapshot,
) -> Result<RealEstateJobJournalSnapshot, String> {
    let now = now_ms();
    let events = read_recent_job_events(base, JOB_EVENTS_SNAPSHOT_LIMIT)?;
    let latest_message = events
        .first()
        .map(|event| event.message.clone())
        .unwrap_or_default();
    let succeeded = queue
        .jobs
        .iter()
        .filter(|job| job.status == "succeeded")
        .map(|job| job.job_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut blocked_jobs = Vec::new();
    let mut next_retry_ms = 0u64;
    for job in queue.jobs.iter().filter(|job| job.status == "pending") {
        let missing_dependencies = job
            .depends_on
            .iter()
            .filter(|dep| !succeeded.contains(*dep))
            .cloned()
            .collect::<Vec<_>>();
        let reason = if !missing_dependencies.is_empty() {
            "dependencies".to_string()
        } else if job.not_before_ms > now {
            "retry_wait".to_string()
        } else if job.attempts >= job.max_attempts {
            "attempts_exhausted".to_string()
        } else {
            continue;
        };
        if job.not_before_ms > now && (next_retry_ms == 0 || job.not_before_ms < next_retry_ms) {
            next_retry_ms = job.not_before_ms;
        }
        blocked_jobs.push(RealEstateJobBlocker {
            job_id: job.job_id.clone(),
            job_kind: job_kind(job).to_string(),
            collector_id: job.collector_id.clone(),
            reason,
            wait_until_ms: job.not_before_ms,
            missing_dependencies,
            estimated_cost: job.estimated_cost,
        });
        if blocked_jobs.len() >= 24 {
            break;
        }
    }
    Ok(RealEstateJobJournalSnapshot {
        status: if queue.running > 0 {
            "running".to_string()
        } else if !blocked_jobs.is_empty() {
            "blocked".to_string()
        } else {
            queue.status.clone()
        },
        updated_at_ms: now,
        latest_message,
        next_retry_ms,
        blocked: blocked_jobs.len(),
        blocked_jobs,
        events,
    })
}

fn read_recent_job_events(
    base: &Path,
    limit: usize,
) -> Result<Vec<RealEstateJobTimelineEvent>, String> {
    let path = data_path(base, JOB_EVENTS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("read job events: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut events = Vec::new();
    for line in text.lines().rev().filter(|line| !line.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<RealEstateJobTimelineEvent>(line) else {
            continue;
        };
        if seen.insert(event.event_id.clone()) {
            events.push(event);
        }
        if events.len() >= limit {
            break;
        }
    }
    Ok(events)
}

fn read_job_event_ids(base: &Path) -> Result<std::collections::HashSet<String>, String> {
    let path = data_path(base, JOB_EVENTS_FILE);
    if !path.exists() {
        return Ok(std::collections::HashSet::new());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("read job events: {e}"))?;
    Ok(text
        .lines()
        .filter_map(|line| serde_json::from_str::<RealEstateJobTimelineEvent>(line).ok())
        .map(|event| event.event_id)
        .collect())
}

fn stable_job_event_id(job_id: &str, stage: &str) -> String {
    format!("je-{}-{}", sanitize_filename(stage), short_hash(job_id))
}

fn job_kind(job: &RealEstateJobQueueEntry) -> &str {
    if job.job_kind.trim().is_empty() {
        "collector"
    } else {
        job.job_kind.as_str()
    }
}

fn default_job_kind() -> String {
    "collector".to_string()
}

fn open_job_of_kind(queue: &RealEstateJobQueueSnapshot, kind: &str) -> bool {
    queue.jobs.iter().any(|job| {
        job_kind(job) == kind && matches!(job.status.as_str(), "pending" | "running")
    })
}

fn pipeline_job_exists(queue: &RealEstateJobQueueSnapshot, kind: &str, deps: &[String]) -> bool {
    queue.jobs.iter().any(|job| {
        job_kind(job) == kind
            && job.depends_on == deps
            && matches!(job.status.as_str(), "pending" | "running" | "succeeded")
    })
}

fn latest_job_of_kind<'a>(
    queue: &'a RealEstateJobQueueSnapshot,
    kind: &str,
) -> Option<&'a RealEstateJobQueueEntry> {
    queue
        .jobs
        .iter()
        .filter(|job| job_kind(job) == kind)
        .max_by_key(|job| job.updated_at_ms.max(job.created_at_ms))
}

fn latest_succeeded_job_id(queue: &RealEstateJobQueueSnapshot, kind: &str) -> Option<String> {
    queue
        .jobs
        .iter()
        .filter(|job| job_kind(job) == kind && job.status == "succeeded")
        .max_by_key(|job| job.updated_at_ms.max(job.created_at_ms))
        .map(|job| job.job_id.clone())
}

fn run_pipeline_job(
    store_path: &Path,
    job: &RealEstateJobQueueEntry,
) -> Result<RealEstateJobStageArtifact, String> {
    match job_kind(job) {
        "intel_pack" => run_intel_pack_job(store_path, job),
        "llm_cache" => run_llm_cache_job(store_path),
        "brain_publish" => run_brain_publish_job(store_path),
        other => Err(format!("unsupported real estate pipeline job kind: {other}")),
    }
}

fn run_intel_pack_job(
    store_path: &Path,
    job: &RealEstateJobQueueEntry,
) -> Result<RealEstateJobStageArtifact, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let reports = dependency_reports_for_job(&base, job)?;
    let pack = refresh_intel_pack_stage(store_path, &reports, &job.trigger, false, false)?;
    Ok(RealEstateJobStageArtifact {
        finished_at_ms: pack.generated_at_ms,
        proof_hash: pack.evidence_hash,
        artifact_path: pack.artifact_path,
    })
}

fn run_llm_cache_job(store_path: &Path) -> Result<RealEstateJobStageArtifact, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let pack = read_latest_intel_pack(&base)?
        .ok_or_else(|| "LLM cache job requires a latest intel pack".to_string())?;
    let store_summary = local_store_summary(&base).unwrap_or_default();
    let kasm_contract = ensure_kasm_score_contract(&base)?;
    let cache = write_llm_intel_cache(&base, &pack, &store_summary, &kasm_contract)?;
    append_llm_cache_ledger(&base, &cache)?;
    Ok(RealEstateJobStageArtifact {
        finished_at_ms: cache.generated_at_ms,
        proof_hash: hash_parts(
            "real_estate_llm_cache_job:v1",
            &[&cache.cache_id, &cache.evidence_hash, &cache.kasm_contract_hash],
        ),
        artifact_path: cache.cache_path,
    })
}

fn run_brain_publish_job(store_path: &Path) -> Result<RealEstateJobStageArtifact, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let mut pack = read_latest_intel_pack(&base)?
        .ok_or_else(|| "brain publish job requires a latest intel pack".to_string())?;
    let store_summary = local_store_summary(&base).unwrap_or_default();
    let kasm_contract = ensure_kasm_score_contract(&base)?;
    let (brain_hash, brain_ref) =
        publish_intel_pack_to_brain(store_path, &pack, &store_summary, &kasm_contract)?;
    pack.brain_note_hash = Some(brain_hash.clone());
    pack.brain_ref = Some(brain_ref);
    write_intel_pack_payload(&base, &pack)?;
    let cache = write_llm_intel_cache(&base, &pack, &store_summary, &kasm_contract)?;
    append_llm_cache_ledger(&base, &cache)?;
    let finished_at_ms = now_ms();
    Ok(RealEstateJobStageArtifact {
        finished_at_ms,
        proof_hash: brain_hash,
        artifact_path: cache.cache_path,
    })
}

fn dependency_reports_for_job(
    base: &Path,
    job: &RealEstateJobQueueEntry,
) -> Result<Vec<HarvestRunReport>, String> {
    let queue = read_job_queue_file(base)?.unwrap_or_default();
    let mut reports = Vec::new();
    for dep in &job.depends_on {
        let Some(dep_job) = queue.jobs.iter().find(|item| &item.job_id == dep) else {
            continue;
        };
        if job_kind(dep_job) != "collector" || dep_job.artifact_path.is_empty() {
            continue;
        }
        if let Ok(report) = read_run_report_path(Path::new(&dep_job.artifact_path)) {
            reports.push(report);
        }
    }
    if reports.is_empty() {
        if let Some(report) = read_latest_run(base)? {
            reports.push(report);
        }
    }
    Ok(reports)
}

fn read_job_queue_file(base: &Path) -> Result<Option<RealEstateJobQueueSnapshot>, String> {
    let path = data_path(base, JOB_QUEUE_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|e| format!("read job queue: {e}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("parse job queue: {e}"))
}

fn read_run_report_path(path: &Path) -> Result<HarvestRunReport, String> {
    let bytes = fs::read(path).map_err(|e| format!("read run report: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse run report: {e}"))
}

fn run_collector(
    store_path: &Path,
    tool_id: &str,
    collector: &CollectorDefinition,
    trigger: &str,
) -> Result<HarvestRunReport, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let normalized_tool = normalize_id(tool_id);
    if normalized_tool.is_empty() {
        return Err("real estate harvester requires a tool id".to_string());
    }

    let started = now_ms();
    let job_id = format!(
        "reh-{}-{}-{}",
        started,
        sanitize_filename(&collector.id),
        short_hash(&normalized_tool)
    );
    let mut normalized_outputs = vec![
        "source_evidence".to_string(),
        "entity_events".to_string(),
        "kasm_feature_candidates".to_string(),
        "llm_intel_cache_seed".to_string(),
    ];
    let mut report = HarvestRunReport {
        job_id,
        tool_id: normalized_tool,
        collector_id: collector.id.clone(),
        collector_label: collector.label.clone(),
        status: trigger.to_string(),
        started_at_ms: started,
        finished_at_ms: started,
        planned_adapters: collector.adapters.clone(),
        source_ids: collector.source_ids.clone(),
        normalized_outputs: Vec::new(),
        proof_hash: String::new(),
        artifact_path: String::new(),
        compliance_notes: vec![
            "Uses registered sources only.".to_string(),
            "No login, captcha, paywall, or ToS bypass.".to_string(),
            "Raw data stays local; reports carry hashes and compact evidence.".to_string(),
        ],
    };
    let store_update = update_local_store_from_collector(&base, collector, &report.job_id, started)?;
    normalized_outputs.extend([
        format!("properties:{}", store_update.properties),
        format!("zones:{}", store_update.zones),
        format!("source_events:{}", store_update.source_events),
        format!("metric_snapshots:{}", store_update.metric_snapshots),
        format!("kasm_contract:{}", store_update.kasm_contract_hash),
        format!("data_hash:{}", store_update.data_hash),
    ]);
    report.normalized_outputs = normalized_outputs;
    report.finished_at_ms = now_ms();
    report.proof_hash = proof_hash(&report)?;

    let run_path = base
        .join(RUNS_DIR)
        .join(format!("{}.json", sanitize_filename(&report.job_id)));
    report.artifact_path = run_path.to_string_lossy().to_string();
    let payload = serde_json::to_vec_pretty(&report)
        .map_err(|e| format!("serialize harvester run report: {e}"))?;
    fs::write(&run_path, payload).map_err(|e| format!("write harvester run report: {e}"))?;
    append_ledger(&base, &report)?;
    Ok(report)
}

fn default_registry() -> HarvesterRegistry {
    HarvesterRegistry {
        sources: vec![
            source("dvf", "DVF", "open_data", "api_http", "valuation and comparable sales", "daily", "official limits", "low"),
            source("cadastre", "Cadastre", "open_data", "api_http", "parcel and land context", "weekly", "official limits", "low"),
            source("dpe_ademe", "DPE / ADEME", "open_data", "api_http", "energy and renovation signals", "daily", "official limits", "low"),
            source("georisques", "Georisques", "open_data", "api_http", "risk and environmental constraints", "weekly", "official limits", "low"),
            source("urbanisme", "Urbanisme", "public_web", "http_crawler", "planning and local constraints", "weekly", "robots and host limits", "medium"),
            source("agency_site", "Site agence", "owned_public_web", "http_crawler", "owned listing and conversion audit", "hourly", "owned domain limits", "low"),
            source("public_portals", "Portails publics", "public_web", "http_crawler", "competition watch", "daily", "robots and host limits", "medium"),
            source("google_workspace", "Google Workspace", "owned_workspace", "oauth_api", "agency-owned mail, docs and calendar", "hourly", "oauth quotas", "consent_required"),
            source("crm", "CRM agence", "owned_business_data", "api_or_file", "contacts, mandates and pipeline", "hourly", "tenant limits", "consent_required"),
            source("local_business", "Economie locale", "open_data", "api_http", "business openings, jobs and area vitality", "weekly", "official limits", "low"),
            source("finance_rates", "Taux et credit", "public_api", "api_http", "buyer financing and broker signals", "daily", "provider limits", "low"),
        ],
        collectors: vec![
            collector("estimation", "Estimation", "Immobilier", &["estimation"], "daily", 1, &["api_http", "kasm"], &["dvf", "cadastre", "dpe_ademe", "georisques", "urbanisme"]),
            collector("mandats", "Mandats", "Immobilier", &["mandats"], "hourly", 1, &["api_or_file", "kasm"], &["crm", "google_workspace", "agency_site"]),
            collector("annonces", "Annonces", "Immobilier", &["annonces"], "hourly", 2, &["http_crawler", "webview_render_adapter", "kasm"], &["agency_site", "public_portals"]),
            collector("rapport_vendeur", "Rapport vendeur", "Immobilier", &["rapport-vendeur"], "on_demand", 1, &["api_http", "api_or_file", "kasm"], &["dvf", "cadastre", "dpe_ademe", "crm", "public_portals"]),
            collector("matching_acheteurs", "Matching acheteurs", "Immobilier", &["matching-acheteurs"], "hourly", 1, &["api_or_file", "kasm"], &["crm", "google_workspace"]),
            collector("conformite", "Conformite", "Immobilier", &["conformite"], "daily", 1, &["api_or_file", "api_http", "kasm"], &["crm", "dpe_ademe", "georisques", "cadastre"]),
            collector("donnees_publiques", "Donnees publiques", "Donnees publiques", &["dvf", "cadastre", "dpe-ademe", "georisques", "urbanisme"], "daily", 2, &["api_http", "http_crawler", "kasm"], &["dvf", "cadastre", "dpe_ademe", "georisques", "urbanisme"]),
            collector("portails", "Portails", "Portails", &["site-agence", "seloger", "leboncoin", "bienici"], "daily", 3, &["http_crawler", "webview_render_adapter", "kasm"], &["agency_site", "public_portals"]),
            collector("google_workspace", "Google Workspace", "Google Workspace", &["gmail", "drive", "calendar", "sheets", "docs"], "hourly", 1, &["oauth_api", "kasm"], &["google_workspace"]),
            collector("recrutement", "Recrutement", "Business", &["recrutement"], "weekly", 4, &["api_http", "public_web", "kasm"], &["local_business"]),
            collector("finance_risk", "Courtier, assurance et fiscalite", "Business", &["comptabilite", "fiscalite", "courtier", "assurance"], "daily", 2, &["api_http", "api_or_file", "kasm"], &["crm", "finance_rates", "google_workspace"]),
            collector("pilotage_agence", "Pilotage agence", "Business", &["pilotage", "reputation", "partenaires", "travaux", "juridique"], "daily", 2, &["api_or_file", "public_web", "kasm"], &["crm", "google_workspace", "local_business", "public_portals"]),
        ],
    }
}

fn source(
    id: &str,
    label: &str,
    source_type: &str,
    adapter: &str,
    allowed_use: &str,
    refresh: &str,
    rate_limit: &str,
    compliance: &str,
) -> SourceDefinition {
    SourceDefinition {
        id: id.to_string(),
        label: label.to_string(),
        source_type: source_type.to_string(),
        adapter: adapter.to_string(),
        allowed_use: allowed_use.to_string(),
        refresh: refresh.to_string(),
        rate_limit: rate_limit.to_string(),
        compliance: compliance.to_string(),
    }
}

fn collector(
    id: &str,
    label: &str,
    group: &str,
    tools: &[&str],
    cadence: &str,
    priority: u8,
    adapters: &[&str],
    source_ids: &[&str],
) -> CollectorDefinition {
    CollectorDefinition {
        id: id.to_string(),
        label: label.to_string(),
        group: group.to_string(),
        tools: tools.iter().map(|tool| normalize_id(tool)).collect(),
        cadence: cadence.to_string(),
        priority,
        adapters: adapters.iter().map(|adapter| adapter.to_string()).collect(),
        source_ids: source_ids.iter().map(|source| source.to_string()).collect(),
        output_contract: "evidence_hashes + entity_events + kasm_features + llm_intel_cache_seed".to_string(),
    }
}

fn ensure_harvester_dirs(store_path: &Path) -> Result<PathBuf, String> {
    let base = store_path.join(HARVESTER_DIR);
    fs::create_dir_all(base.join(DATA_DIR)).map_err(|e| format!("create data dirs: {e}"))?;
    fs::create_dir_all(base.join(RUNS_DIR)).map_err(|e| format!("create harvester dirs: {e}"))?;
    fs::create_dir_all(base.join(INTEL_DIR)).map_err(|e| format!("create intel dirs: {e}"))?;
    fs::create_dir_all(base.join(DATA_DIR).join(KASM_CONTRACTS_DIR))
        .map_err(|e| format!("create KASM contract dirs: {e}"))?;
    fs::create_dir_all(base.join(DATA_DIR).join(LLM_INTEL_CACHE_DIR))
        .map_err(|e| format!("create LLM intel cache dirs: {e}"))?;
    Ok(base)
}

fn daemon_status(base: &Path) -> HarvesterDaemonStatus {
    if let Some(daemon) = HARVESTER_DAEMON.get() {
        return runtime_status(base, daemon);
    }
    if let Ok(Some(status)) = read_status_file(base) {
        return status;
    }
    HarvesterDaemonStatus {
        status: "not_started".to_string(),
        mode: "embedded_tauri_backend".to_string(),
        background_ready: false,
        scheduler_ready: false,
        webview_worker_ready: false,
        kasm_ready: true,
        store_dir: base.to_string_lossy().to_string(),
        updated_at_ms: now_ms(),
        started_at_ms: 0,
        last_tick_ms: 0,
        runs_total: 0,
        errors_total: 0,
        last_error: String::new(),
    }
}

fn runtime_status(base: &Path, daemon: &HarvesterDaemon) -> HarvesterDaemonStatus {
    let last_error = daemon
        .last_error
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|_| "last error lock poisoned".to_string());
    HarvesterDaemonStatus {
        status: if daemon.running.load(Ordering::Relaxed) {
            "continuous".to_string()
        } else {
            "stopped".to_string()
        },
        mode: "continuous_authorized_data_sync".to_string(),
        background_ready: daemon.running.load(Ordering::Relaxed),
        scheduler_ready: daemon.scheduler_ready.load(Ordering::Relaxed),
        webview_worker_ready: false,
        kasm_ready: true,
        store_dir: base.to_string_lossy().to_string(),
        updated_at_ms: now_ms(),
        started_at_ms: daemon.started_at_ms,
        last_tick_ms: daemon.last_tick_ms.load(Ordering::Relaxed),
        runs_total: daemon.runs_total.load(Ordering::Relaxed),
        errors_total: daemon.errors_total.load(Ordering::Relaxed),
        last_error,
    }
}

fn write_runtime_status(daemon: &HarvesterDaemon) {
    let Ok(base) = ensure_harvester_dirs(&daemon.store_path) else {
        return;
    };
    let status = runtime_status(&base, daemon);
    let _ = write_status_file(&base, &status);
}

fn write_status_file(base: &Path, status: &HarvesterDaemonStatus) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(status)
        .map_err(|e| format!("serialize harvester status: {e}"))?;
    fs::write(base.join(STATUS_FILE), payload).map_err(|e| format!("write harvester status: {e}"))
}

fn read_status_file(base: &Path) -> Result<Option<HarvesterDaemonStatus>, String> {
    let path = base.join(STATUS_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|e| format!("read harvester status: {e}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("parse harvester status: {e}"))
}

fn read_latest_run(base: &Path) -> Result<Option<HarvestRunReport>, String> {
    let runs_dir = base.join(RUNS_DIR);
    let mut latest: Option<(SystemTime, PathBuf)> = None;
    let entries = match fs::read_dir(&runs_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("read harvester runs dir: {err}")),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|it| it.to_str()) != Some("json") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(UNIX_EPOCH);
        if latest.as_ref().map(|(time, _)| modified > *time).unwrap_or(true) {
            latest = Some((modified, path));
        }
    }
    let Some((_, path)) = latest else {
        return Ok(None);
    };
    let bytes = fs::read(&path).map_err(|e| format!("read latest harvester run: {e}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("parse latest harvester run: {e}"))
}

fn append_ledger(base: &Path, report: &HarvestRunReport) -> Result<(), String> {
    let entry = json!({
        "kind": "real_estate_harvester_run",
        "jobId": report.job_id,
        "toolId": report.tool_id,
        "collectorId": report.collector_id,
        "status": report.status,
        "proofHash": report.proof_hash,
        "artifactPath": report.artifact_path,
        "createdAtMs": report.finished_at_ms,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(base.join(LEDGER_FILE))
        .map_err(|e| format!("open harvester ledger: {e}"))?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(&entry).map_err(|e| format!("serialize harvester ledger: {e}"))?
    )
    .map_err(|e| format!("append harvester ledger: {e}"))
}

fn update_local_store_from_collector(
    base: &Path,
    collector: &CollectorDefinition,
    job_id: &str,
    observed_at_ms: u64,
) -> Result<LocalStoreUpdate, String> {
    let mut properties = read_properties(base)?;
    let before_len = properties.len();
    let source_ids = collector.source_ids.join(",");

    if collector.source_ids.iter().any(|source| source == "dvf") {
        let imported = import_dvf_properties(job_id, observed_at_ms)?;
        upsert_properties(&mut properties, imported);
    }
    if collector.source_ids.iter().any(|source| source == "dpe_ademe") {
        enrich_dpe(&mut properties, observed_at_ms);
    }
    if collector
        .source_ids
        .iter()
        .any(|source| matches!(source.as_str(), "georisques" | "cadastre" | "urbanisme"))
    {
        enrich_risk_and_context(&mut properties, observed_at_ms);
    }
    if properties.is_empty() {
        save_properties(base, &properties)?;
        save_zones(base, &[])?;
        let event = source_event(
            base,
            job_id,
            collector,
            observed_at_ms,
            &source_ids,
            &[],
            "empty_local_store",
        )?;
        append_json_line(&data_path(base, SOURCE_EVENTS_FILE), &event)?;
        return Ok(LocalStoreUpdate {
            properties: 0,
            zones: 0,
            source_events: 1,
            metric_snapshots: 0,
            kasm_contract_hash: String::new(),
            data_hash: local_data_hash(&properties, &[])?,
        });
    }

    properties.sort_by(|a, b| a.property_id.cmp(&b.property_id));
    let features = rebuild_local_features(base, &properties, observed_at_ms)?;
    save_properties(base, &properties)?;
    save_zones(base, &features.zones)?;
    append_metric_snapshots(base, &features.snapshots)?;

    let entity_refs = properties
        .iter()
        .skip(before_len.saturating_sub(24))
        .take(96)
        .map(|property| property.property_id.clone())
        .collect::<Vec<_>>();
    let data_hash = local_data_hash(&properties, &features.zones)?;
    let event = source_event(
        base,
        job_id,
        collector,
        observed_at_ms,
        &source_ids,
        &entity_refs,
        &data_hash,
    )?;
    append_json_line(&data_path(base, SOURCE_EVENTS_FILE), &event)?;

    Ok(LocalStoreUpdate {
        properties: properties.len(),
        zones: features.zones.len(),
        source_events: 1,
        metric_snapshots: features.snapshots.len(),
        kasm_contract_hash: features.kasm_contract.program_hash,
        data_hash,
    })
}

fn import_dvf_properties(job_id: &str, observed_at_ms: u64) -> Result<Vec<RealEstatePropertyEntity>, String> {
    if let Some(path) = std::env::var_os("FORGE_REAL_ESTATE_DVF_FIXTURE") {
        let path = PathBuf::from(path);
        if path.exists() {
            return import_dvf_fixture(&path, job_id, observed_at_ms);
        }
    }
    Ok(synthetic_dvf_properties(job_id, observed_at_ms, 420))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DvfFixtureRecord {
    property_id: Option<String>,
    zone_id: Option<String>,
    city: Option<String>,
    postal_code: Option<String>,
    address_label: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    property_type: Option<String>,
    surface_m2: Option<f64>,
    rooms: Option<f64>,
    land_m2: Option<f64>,
    mutation_date: Option<String>,
    price_eur: Option<f64>,
    dpe_score: Option<f64>,
    risk_score: Option<f64>,
}

fn import_dvf_fixture(
    path: &Path,
    job_id: &str,
    observed_at_ms: u64,
) -> Result<Vec<RealEstatePropertyEntity>, String> {
    let bytes = fs::read(path).map_err(|e| format!("read DVF fixture: {e}"))?;
    let rows: Vec<DvfFixtureRecord> =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse DVF fixture JSON: {e}"))?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(idx, row)| fixture_property(row, idx, job_id, observed_at_ms))
        .collect())
}

fn fixture_property(
    row: DvfFixtureRecord,
    idx: usize,
    job_id: &str,
    observed_at_ms: u64,
) -> RealEstatePropertyEntity {
    let city = row.city.unwrap_or_else(|| "Zone agence".to_string());
    let postal_code = row.postal_code.unwrap_or_else(|| "00000".to_string());
    let zone_id = row
        .zone_id
        .unwrap_or_else(|| format!("zone-{}", normalize_id(&postal_code)));
    let surface = row.surface_m2.unwrap_or(65.0).max(8.0);
    let price_eur = row.price_eur.unwrap_or(220_000.0).max(1.0);
    let property_id = row
        .property_id
        .unwrap_or_else(|| format!("DVF-FIXTURE-{idx:06}"));
    let mut property = RealEstatePropertyEntity {
        source: "dvf_fixture".to_string(),
        source_event_id: job_id.to_string(),
        property_id,
        zone_id,
        city,
        postal_code,
        address_label: row
            .address_label
            .unwrap_or_else(|| format!("Adresse DVF fixture {idx}")),
        lat: row.lat.unwrap_or(43.60),
        lon: row.lon.unwrap_or(1.44),
        property_type: row.property_type.unwrap_or_else(|| "appartement".to_string()),
        surface_m2: surface,
        rooms: row.rooms.unwrap_or((surface / 24.0).round().max(1.0)),
        land_m2: row.land_m2.unwrap_or(0.0),
        mutation_date: row.mutation_date.unwrap_or_else(|| "2025-01-01".to_string()),
        price_eur,
        price_m2: price_eur / surface,
        dpe_score: row.dpe_score,
        risk_score: row.risk_score.unwrap_or(0.2),
        updated_at_ms: observed_at_ms,
        evidence_hash: String::new(),
    };
    property.evidence_hash = property_hash(&property);
    property
}

fn synthetic_dvf_properties(
    job_id: &str,
    observed_at_ms: u64,
    count: usize,
) -> Vec<RealEstatePropertyEntity> {
    let cities = [
        ("Toulouse", "31000"),
        ("Blagnac", "31700"),
        ("Balma", "31130"),
        ("Colomiers", "31770"),
        ("L'Union", "31240"),
        ("Tournefeuille", "31170"),
    ];
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let seed = hash_seed(&format!("dvf-mock:{idx}:{}", idx % 47));
        let (city, postal_code) = cities[idx % cities.len()];
        let zone_num = 1 + (idx % 72);
        let surface = 28.0 + pseudo_unit(seed, 1) * 142.0;
        let rooms = (surface / 24.0 + pseudo_unit(seed, 2) * 1.8).round().clamp(1.0, 8.0);
        let base_price_m2 = 2_250.0 + zone_num as f64 * 34.0 + pseudo_unit(seed, 3) * 1_850.0;
        let pressure = 0.88 + pseudo_unit(seed, 4) * 0.34;
        let price_eur = (surface * base_price_m2 * pressure).round();
        let month = 1 + (idx % 12);
        let day = 1 + (idx % 27);
        let mut property = RealEstatePropertyEntity {
            property_id: format!("DVF-MOCK-{zone_num:03}-{idx:06}"),
            source: "dvf_mock".to_string(),
            source_event_id: job_id.to_string(),
            zone_id: format!("zone-{zone_num:03}"),
            city: city.to_string(),
            postal_code: postal_code.to_string(),
            address_label: format!("Mutation DVF mock {} {}", idx + 1, city),
            lat: 43.45 + (zone_num as f64 % 18.0) * 0.011 + pseudo_unit(seed, 5) * 0.008,
            lon: 1.25 + (zone_num as f64 / 18.0).floor() * 0.016 + pseudo_unit(seed, 6) * 0.008,
            property_type: if idx % 5 == 0 {
                "maison".to_string()
            } else {
                "appartement".to_string()
            },
            surface_m2: surface,
            rooms,
            land_m2: if idx % 5 == 0 {
                90.0 + pseudo_unit(seed, 7) * 520.0
            } else {
                0.0
            },
            mutation_date: format!("2025-{month:02}-{day:02}"),
            price_eur,
            price_m2: price_eur / surface,
            dpe_score: Some(1.0 + pseudo_unit(seed, 8) * 6.0),
            risk_score: pseudo_unit(seed, 9) * 1.8,
            updated_at_ms: observed_at_ms,
            evidence_hash: String::new(),
        };
        property.evidence_hash = property_hash(&property);
        out.push(property);
    }
    out
}

fn upsert_properties(
    properties: &mut Vec<RealEstatePropertyEntity>,
    imported: Vec<RealEstatePropertyEntity>,
) {
    let mut by_id = properties
        .iter()
        .enumerate()
        .map(|(idx, property)| (property.property_id.clone(), idx))
        .collect::<HashMap<_, _>>();
    for property in imported {
        if let Some(idx) = by_id.get(&property.property_id).copied() {
            properties[idx] = property;
        } else {
            by_id.insert(property.property_id.clone(), properties.len());
            properties.push(property);
        }
    }
}

fn enrich_dpe(properties: &mut [RealEstatePropertyEntity], observed_at_ms: u64) {
    for property in properties {
        if property.dpe_score.is_none() {
            let seed = hash_seed(&format!("dpe:{}:{}", property.property_id, property.zone_id));
            property.dpe_score = Some(1.0 + pseudo_unit(seed, 1) * 6.0);
        }
        property.updated_at_ms = observed_at_ms;
        property.evidence_hash = property_hash(property);
    }
}

fn enrich_risk_and_context(properties: &mut [RealEstatePropertyEntity], observed_at_ms: u64) {
    for property in properties {
        let seed = hash_seed(&format!("risk:{}:{}", property.property_id, property.zone_id));
        property.risk_score = (property.risk_score * 0.55 + pseudo_unit(seed, 2) * 1.6).clamp(0.0, 2.2);
        property.updated_at_ms = observed_at_ms;
        property.evidence_hash = property_hash(property);
    }
}

fn recompute_zones(
    properties: &[RealEstatePropertyEntity],
    observed_at_ms: u64,
) -> Result<Vec<RealEstateZoneEntity>, String> {
    let mut grouped: HashMap<String, Vec<&RealEstatePropertyEntity>> = HashMap::new();
    for property in properties {
        grouped
            .entry(property.zone_id.clone())
            .or_default()
            .push(property);
    }
    let mut zones = Vec::with_capacity(grouped.len());
    for (zone_id, items) in grouped {
        let mut prices = items
            .iter()
            .map(|property| property.price_m2)
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let avg = if prices.is_empty() {
            0.0
        } else {
            prices.iter().sum::<f64>() / prices.len() as f64
        };
        let median = prices
            .get(prices.len().saturating_sub(1) / 2)
            .copied()
            .unwrap_or(avg);
        let first = items[0];
        let mut zone = RealEstateZoneEntity {
            zone_id,
            label: format!("{} {}", first.city, first.postal_code),
            city: first.city.clone(),
            postal_code: first.postal_code.clone(),
            property_count: items.len(),
            avg_price_m2: avg,
            median_price_m2: median,
            liquidity_score: (items.len() as f64 / 18.0).clamp(0.0, 2.4),
            updated_at_ms: observed_at_ms,
            evidence_hash: String::new(),
        };
        zone.evidence_hash = zone_hash(&zone);
        zones.push(zone);
    }
    zones.sort_by(|a, b| a.zone_id.cmp(&b.zone_id));
    Ok(zones)
}

fn rebuild_local_features(
    base: &Path,
    properties: &[RealEstatePropertyEntity],
    observed_at_ms: u64,
) -> Result<LocalFeatureBuild, String> {
    let kasm_contract = ensure_kasm_score_contract(base)?;
    let zones = recompute_zones(properties, observed_at_ms)?;
    let snapshots =
        build_metric_snapshots(properties, &zones, observed_at_ms, &kasm_contract.program_hash)?;
    Ok(LocalFeatureBuild {
        zones,
        snapshots,
        kasm_contract,
    })
}

fn build_metric_snapshots(
    properties: &[RealEstatePropertyEntity],
    zones: &[RealEstateZoneEntity],
    observed_at_ms: u64,
    kasm_contract_hash: &str,
) -> Result<Vec<RealEstateMetricSnapshot>, String> {
    let zones_by_id = zones
        .iter()
        .map(|zone| (zone.zone_id.as_str(), zone))
        .collect::<HashMap<_, _>>();
    let manifest_hash = metric_manifest_hash();
    let mut snapshots = Vec::with_capacity(properties.len().min(512));
    for property in properties.iter().take(512) {
        let zone = zones_by_id.get(property.zone_id.as_str()).copied();
        let metrics = property_metrics(property, zone);
        let score = score_metrics(&metrics);
        let seller_probability = sigmoid((score - 58.0) / 13.0);
        let expected_fee_eur = property.price_eur * 0.035 * seller_probability;
        let strongest_signal = strongest_metric(&metrics).to_string();
        let snapshot_id = format!(
            "metric-{}-{}",
            observed_at_ms,
            short_hash(&format!("{}:{score:.5}", property.property_id))
        );
        let proof_hash = hash_parts(
            "real_estate_metric_snapshot:v1",
            &[
                &snapshot_id,
                &property.property_id,
                &property.zone_id,
                &format!("{score:.8}"),
                &format!("{seller_probability:.8}"),
                &manifest_hash,
                kasm_contract_hash,
            ],
        );
        snapshots.push(RealEstateMetricSnapshot {
            snapshot_id,
            property_id: property.property_id.clone(),
            zone_id: property.zone_id.clone(),
            generated_at_ms: observed_at_ms,
            metric_manifest_hash: manifest_hash.clone(),
            kasm_contract_hash: kasm_contract_hash.to_string(),
            metrics,
            score,
            seller_probability,
            expected_fee_eur,
            strongest_signal,
            proof_hash,
        });
    }
    snapshots.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(snapshots)
}

fn property_metrics(
    property: &RealEstatePropertyEntity,
    zone: Option<&RealEstateZoneEntity>,
) -> Vec<f64> {
    let seed = hash_seed(&format!("metrics:{}:{}", property.property_id, property.zone_id));
    let zone_avg = zone.map(|zone| zone.avg_price_m2).unwrap_or(property.price_m2);
    let zone_liquidity = zone.map(|zone| zone.liquidity_score).unwrap_or(0.8);
    let price_gap = (property.price_m2 - zone_avg) / zone_avg.abs().max(1.0) * 100.0;
    let dpe = property.dpe_score.unwrap_or(3.5);
    let dpe_gap = ((dpe - 3.0) / 4.0).clamp(0.0, 1.0) * 100.0;
    let risk = property.risk_score.clamp(0.0, 2.4);
    let surface_fit = (property.surface_m2 / 90.0).clamp(0.1, 2.3);
    let owner_lifecycle = pseudo_unit(seed, 1) * 2.0;
    let buyer = zone_liquidity * 0.42 + pseudo_unit(seed, 2) * 1.15;
    let renovation = dpe_gap / 45.0 + pseudo_unit(seed, 3) * 0.85;
    let local_momentum = zone_liquidity * 0.28 + pseudo_unit(seed, 4) * 1.35;
    let mut metrics = vec![
        price_gap,
        pseudo_unit(seed, 5) * 5.5,
        dpe_gap,
        dpe * 0.38 + pseudo_unit(seed, 6),
        risk * 0.72,
        risk * 0.46 + pseudo_unit(seed, 7) * 0.4,
        local_momentum,
        local_momentum * 0.82 + pseudo_unit(seed, 8) * 0.4,
        pseudo_unit(seed, 9) * 1.8,
        pseudo_unit(seed, 10) * 1.6,
        pseudo_unit(seed, 11) * 1.7,
        pseudo_unit(seed, 12) * 1.4,
        pseudo_unit(seed, 13) * 1.4,
        risk * 0.30 + pseudo_unit(seed, 14) * 1.3,
        pseudo_unit(seed, 15) * 1.7,
        pseudo_unit(seed, 16) * 1.9,
        pseudo_unit(seed, 17) * 1.6,
        buyer,
        pseudo_unit(seed, 18) * 1.7,
        risk * 0.52 + pseudo_unit(seed, 19) * 0.6,
        surface_fit * 0.44 + pseudo_unit(seed, 20) * 1.0,
        pseudo_unit(seed, 21) * 5.0,
        buyer * 0.72 + pseudo_unit(seed, 22) * 0.7,
        owner_lifecycle,
        pseudo_unit(seed, 23) * 1.8,
        renovation,
        zone_liquidity,
        price_gap.abs() * 0.38 + pseudo_unit(seed, 24),
        pseudo_unit(seed, 25) * 5.0,
        risk * 0.35 + pseudo_unit(seed, 26) * 1.1,
        pseudo_unit(seed, 27) * 1.5,
        buyer * 0.30 + pseudo_unit(seed, 28),
    ];
    while metrics.len() < INTEL_METRIC_MANIFEST.len() {
        let idx = metrics.len() as u64;
        let value = match metrics.len() {
            32 | 33 => dpe * 0.25 + pseudo_unit(seed, idx) * 1.2,
            34 | 35 | 63 => risk * 0.35 + pseudo_unit(seed, idx) * 1.2,
            36 | 37 => surface_fit * 0.22 + pseudo_unit(seed, idx) * 1.3,
            39 | 40 | 48 | 49 => zone_liquidity * 0.30 + pseudo_unit(seed, idx) * 1.2,
            44 => owner_lifecycle * 0.45 + pseudo_unit(seed, idx),
            45 | 46 => pseudo_unit(seed, idx) * 1.7,
            57 => (1.4 - zone_liquidity).max(0.0) + pseudo_unit(seed, idx) * 0.7,
            59 | 60 => renovation * 0.36 + pseudo_unit(seed, idx) * 1.0,
            _ => pseudo_unit(seed, idx) * 1.6,
        };
        metrics.push(value);
    }
    metrics.truncate(INTEL_METRIC_MANIFEST.len());
    metrics
}

fn score_metrics(metrics: &[f64]) -> f64 {
    let mut score = 54.0;
    for (idx, metric) in metrics.iter().enumerate() {
        score += metric * intel_metric_weight(idx);
    }
    score.clamp(0.0, 100.0)
}

fn intel_metric_weight(idx: usize) -> f64 {
    match idx {
        0 => 0.52,
        1 => 1.65,
        2 => 0.20,
        3 => -0.35,
        4 | 5 | 13 | 19 | 34 | 35 | 43 | 45 | 46 | 50 | 57 | 61 | 62 | 63 => -0.72,
        6 | 7 | 8 | 9 | 14 | 15 | 17 | 22 | 25 | 26 | 31 | 36 | 37 | 39 | 40 | 41 | 42
        | 47 | 48 | 49 | 52 | 53 | 54 | 55 | 56 | 58 | 60 => 0.82,
        10 | 20 | 21 | 23 | 24 | 27 | 28 | 30 | 44 | 59 => 0.46,
        11 | 12 | 16 | 18 | 29 | 32 | 33 | 38 | 51 => -0.28,
        _ => 0.12,
    }
}

fn strongest_metric(metrics: &[f64]) -> &'static str {
    let (idx, _) = metrics
        .iter()
        .enumerate()
        .max_by(|(left_idx, left), (right_idx, right)| {
            let left_weighted = left.abs() * intel_metric_weight(*left_idx).abs();
            let right_weighted = right.abs() * intel_metric_weight(*right_idx).abs();
            left_weighted
                .partial_cmp(&right_weighted)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, &0.0));
    INTEL_METRIC_MANIFEST[idx.min(INTEL_METRIC_MANIFEST.len() - 1)]
}

fn read_properties(base: &Path) -> Result<Vec<RealEstatePropertyEntity>, String> {
    read_json_vec(&data_path(base, PROPERTIES_FILE), "properties")
}

fn save_properties(base: &Path, properties: &[RealEstatePropertyEntity]) -> Result<(), String> {
    write_json_pretty(&data_path(base, PROPERTIES_FILE), properties, "properties")
}

fn read_zones(base: &Path) -> Result<Vec<RealEstateZoneEntity>, String> {
    read_json_vec(&data_path(base, ZONES_FILE), "zones")
}

fn save_zones(base: &Path, zones: &[RealEstateZoneEntity]) -> Result<(), String> {
    write_json_pretty(&data_path(base, ZONES_FILE), zones, "zones")
}

fn read_recent_metric_snapshots(
    base: &Path,
    limit: usize,
) -> Result<Vec<RealEstateMetricSnapshot>, String> {
    let path = data_path(base, METRIC_SNAPSHOTS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("read metric snapshots: {e}"))?;
    let mut snapshots = text
        .lines()
        .rev()
        .take(limit)
        .filter_map(|line| serde_json::from_str::<RealEstateMetricSnapshot>(line).ok())
        .collect::<Vec<_>>();
    snapshots.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(snapshots)
}

fn append_metric_snapshots(
    base: &Path,
    snapshots: &[RealEstateMetricSnapshot],
) -> Result<(), String> {
    let path = data_path(base, METRIC_SNAPSHOTS_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open metric snapshots: {e}"))?;
    for snapshot in snapshots.iter().take(512) {
        writeln!(
            file,
            "{}",
            serde_json::to_string(snapshot).map_err(|e| format!("serialize metric snapshot: {e}"))?
        )
        .map_err(|e| format!("append metric snapshot: {e}"))?;
    }
    Ok(())
}

fn local_store_summary(base: &Path) -> Result<RealEstateLocalStoreSummary, String> {
    let properties = read_properties(base)?;
    let zones = read_zones(base)?;
    let source_events = count_jsonl_lines(&data_path(base, SOURCE_EVENTS_FILE))?;
    let metric_snapshots = count_jsonl_lines(&data_path(base, METRIC_SNAPSHOTS_FILE))?;
    let latest_updated_at_ms = properties
        .iter()
        .map(|property| property.updated_at_ms)
        .chain(zones.iter().map(|zone| zone.updated_at_ms))
        .max()
        .unwrap_or(0);
    Ok(RealEstateLocalStoreSummary {
        data_dir: base.join(DATA_DIR).to_string_lossy().to_string(),
        properties: properties.len(),
        zones: zones.len(),
        source_events,
        metric_snapshots,
        latest_updated_at_ms,
        data_hash: local_data_hash(&properties, &zones)?,
    })
}

fn source_event(
    base: &Path,
    job_id: &str,
    collector: &CollectorDefinition,
    observed_at_ms: u64,
    source_ids: &str,
    entity_refs: &[String],
    data_hash: &str,
) -> Result<RealEstateSourceEvent, String> {
    let event_id = format!("src-{}-{}", observed_at_ms, short_hash(job_id));
    let artifact_path = base
        .join(DATA_DIR)
        .join(format!("{}.json", sanitize_filename(&event_id)));
    let source_hash = hash_parts(
        "real_estate_source_event_source:v1",
        &[job_id, &collector.id, source_ids, data_hash],
    );
    let proof_hash = hash_parts(
        "real_estate_source_event:v1",
        &[
            &event_id,
            &collector.id,
            source_ids,
            &source_hash,
            &entity_refs.len().to_string(),
        ],
    );
    let event = RealEstateSourceEvent {
        event_id: event_id.clone(),
        source_id: source_ids.to_string(),
        collector_id: collector.id.clone(),
        observed_at_ms,
        source_hash,
        entity_refs: entity_refs.to_vec(),
        artifact_path: artifact_path.to_string_lossy().to_string(),
        proof_hash,
    };
    let payload = json!({
        "event": event,
        "entityRefsPreview": entity_refs.iter().take(24).collect::<Vec<_>>(),
    });
    write_json_pretty(&artifact_path, &payload, "source event artifact")?;
    serde_json::from_value(payload["event"].clone())
        .map_err(|e| format!("rebuild source event: {e}"))
}

fn data_path(base: &Path, file: &str) -> PathBuf {
    base.join(DATA_DIR).join(file)
}

fn read_json_vec<T>(path: &Path, label: &str) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).map_err(|e| format!("read {label}: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse {label}: {e}"))
}

fn write_json_pretty<T>(path: &Path, value: &T, label: &str) -> Result<(), String>
where
    T: Serialize + ?Sized,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {label} parent: {e}"))?;
    }
    let payload =
        serde_json::to_vec_pretty(value).map_err(|e| format!("serialize {label}: {e}"))?;
    fs::write(path, payload).map_err(|e| format!("write {label}: {e}"))
}

fn append_json_line<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create jsonl parent: {e}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open jsonl: {e}"))?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(value).map_err(|e| format!("serialize jsonl: {e}"))?
    )
    .map_err(|e| format!("append jsonl: {e}"))
}

fn count_jsonl_lines(path: &Path) -> Result<usize, String> {
    if !path.exists() {
        return Ok(0);
    }
    let text = fs::read_to_string(path).map_err(|e| format!("read jsonl: {e}"))?;
    Ok(text.lines().filter(|line| !line.trim().is_empty()).count())
}

fn local_data_hash(
    properties: &[RealEstatePropertyEntity],
    zones: &[RealEstateZoneEntity],
) -> Result<String, String> {
    let payload = json!({
        "properties": properties.iter().map(|item| (&item.property_id, &item.evidence_hash)).collect::<Vec<_>>(),
        "zones": zones.iter().map(|item| (&item.zone_id, &item.evidence_hash)).collect::<Vec<_>>(),
        "metricManifest": metric_manifest_hash(),
    });
    let bytes = serde_json::to_vec(&payload).map_err(|e| format!("serialize data hash: {e}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn property_hash(property: &RealEstatePropertyEntity) -> String {
    hash_parts(
        "real_estate_property:v1",
        &[
            &property.property_id,
            &property.zone_id,
            &property.city,
            &property.postal_code,
            &property.mutation_date,
            &format!("{:.4}", property.price_eur),
            &format!("{:.4}", property.price_m2),
            &format!("{:.4}", property.surface_m2),
            &format!("{:.4}", property.risk_score),
        ],
    )
}

fn zone_hash(zone: &RealEstateZoneEntity) -> String {
    hash_parts(
        "real_estate_zone:v1",
        &[
            &zone.zone_id,
            &zone.city,
            &zone.postal_code,
            &zone.property_count.to_string(),
            &format!("{:.4}", zone.avg_price_m2),
            &format!("{:.4}", zone.median_price_m2),
            &format!("{:.4}", zone.liquidity_score),
        ],
    )
}

fn metric_manifest_hash() -> String {
    hash_parts("real_estate_metric_manifest:v1", &INTEL_METRIC_MANIFEST)
}

static REAL_ESTATE_SCORE_PROGRAM: OnceLock<Result<Program, String>> = OnceLock::new();

fn kasm_score_program() -> Result<Program, String> {
    REAL_ESTATE_SCORE_PROGRAM
        .get_or_init(build_kasm_score_program)
        .clone()
}

fn build_kasm_score_program() -> Result<Program, String> {
    let nodes = vec![
        Node::input_f64(0),
        Node::input_f64(1),
        Node::input_f64(2),
        Node::input_f64(3),
        Node::input_f64(4),
        Node::input_f64(5),
        Node::input_f64(6),
        Node::input_f64(7),
        Node::const_f64(30),
        Node::const_f64(18),
        Node::const_f64(17),
        Node::const_f64(-14),
        Node::const_f64(12),
        Node::const_f64(-8),
        Node::const_f64(15),
        Node::const_f64(-7),
        Node::f64_mul(0, 8),
        Node::f64_mul(1, 9),
        Node::f64_mul(2, 10),
        Node::f64_mul(3, 11),
        Node::f64_mul(4, 12),
        Node::f64_mul(5, 13),
        Node::f64_mul(6, 14),
        Node::f64_mul(7, 15),
        Node::f64_add(16, 17),
        Node::f64_add(18, 19),
        Node::f64_add(20, 21),
        Node::f64_add(22, 23),
        Node::f64_add(24, 25),
        Node::f64_add(26, 27),
        Node::f64_add(28, 29),
        Node::f64_to_i64(30),
        Node::memoize(31),
        Node::output(32, Ty::I64),
    ];
    let raw = Program::new(Target::Cpu, 8, 1, 64, nodes)
        .map_err(|e| format!("build real estate KASM score program: {e}"))?;
    match raw.cse() {
        Ok(program) => Ok(program),
        Err(_) => Ok(raw),
    }
}

fn ensure_kasm_score_contract(base: &Path) -> Result<RealEstateKasmContract, String> {
    let program = kasm_score_program()?;
    let program_hash = scan::Hash::for_blob(program.bytes()).as_hex();
    let metric_manifest_hash = metric_manifest_hash();
    let semantic_fingerprint = program
        .semantic_fingerprint_hex()
        .unwrap_or_else(|_| program.structural_hash_hex());
    let canonical_hash = program
        .canonical_hash_hex()
        .unwrap_or_else(|_| program.structural_hash_hex());
    let input_metrics = [
        "dvf_price_gap",
        "dpe_renovation_gap",
        "buyer_demand_match",
        "geo_clay_risk",
        "neighborhood_liquidity",
        "credit_rate_sensitivity",
        "owner_lifecycle_pressure",
        "competitor_pressure",
    ]
    .iter()
    .map(|metric| (*metric).to_string())
    .collect::<Vec<_>>();
    let cache_key = hash_parts(
        "real_estate_kasm_contract_cache:v1",
        &[&program_hash, &semantic_fingerprint, &metric_manifest_hash],
    );
    let contract_id = format!("re-kasm-score-{}", &cache_key[..16]);
    let artifact_path = base
        .join(DATA_DIR)
        .join(KASM_CONTRACTS_DIR)
        .join(REAL_ESTATE_KASM_SCORE_CONTRACT_FILE);
    let contract = RealEstateKasmContract {
        contract_id,
        program_hash,
        semantic_fingerprint,
        canonical_hash,
        metric_manifest_hash,
        input_metrics,
        output_contract: "i64_score_basis_points memoized by KASM program hash + metric manifest hash"
            .to_string(),
        nodes: program.nodes().len(),
        byte_len: program.bytes().len(),
        fuel: program.fuel(),
        cache_key,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    };
    write_json_pretty(&artifact_path, &contract, "real estate KASM score contract")?;
    Ok(contract)
}

fn real_estate_brain_node(store_path: &Path) -> Result<MonsterNode, String> {
    fs::create_dir_all(store_path)
        .map_err(|e| format!("create Forge brain store '{}': {e}", store_path.display()))?;
    let store = Store::open(store_path.to_path_buf())
        .map_err(|e| format!("open Forge brain store '{}': {e}", store_path.display()))?;
    Ok(MonsterNode::new(
        store,
        MemoryGovernor::one_percent_assumed_host(),
    ))
}

fn publish_kasm_score_to_brain(store_path: &Path, program: &Program) -> Result<String, String> {
    let node = real_estate_brain_node(store_path)?;
    let program_hash = scan::Hash::for_blob(program.bytes());
    let resolved = publish_semantic_attractor(&node, program_hash, program, 8)
        .map_err(|e| format!("publish real estate KASM semantic attractor: {e:?}"))?
        .unwrap_or(program_hash);
    node.store()
        .write_ref(
            REAL_ESTATE_KASM_BRAIN_REF,
            &resolved,
            "real estate KASM score contract",
        )
        .map_err(|e| format!("write real estate KASM brain ref: {e}"))?;
    Ok(resolved.as_hex())
}

fn publish_intel_pack_to_brain(
    store_path: &Path,
    pack: &RealEstateIntelPack,
    store_summary: &RealEstateLocalStoreSummary,
    kasm_contract: &RealEstateKasmContract,
) -> Result<(String, String), String> {
    let node = real_estate_brain_node(store_path)?;
    let best = pack
        .top_opportunities
        .first()
        .map(|opportunity| {
            format!(
                "best_property={} zone={} score={:.2} seller_probability={:.3} fee={:.0} signal={} horizon_days={}",
                opportunity.property_id,
                opportunity.zone_id,
                opportunity.score,
                opportunity.seller_probability,
                opportunity.expected_fee_eur,
                opportunity.strongest_signal,
                opportunity.horizon_days
            )
        })
        .unwrap_or_else(|| "best_property=none".to_string());
    let note_text_hash = hash_parts(
        "real_estate_brain_note:v1",
        &[
            &pack.pack_id,
            &pack.evidence_hash,
            &kasm_contract.program_hash,
            &best,
            &format!("properties={}", store_summary.properties),
            &format!("zones={}", store_summary.zones),
        ],
    );
    let note = format!(
        "forge-brain-llm-note-v1\nscope=agence-immo\nkind=intel_pack\nmemory_layer=semantic\nsource=real_estate_data_sync\nverification_status=anchored\ntrust_score=0.860\nconfidence=0.860\ntext_hash={note_text_hash}\npack_id={}\nevidence_hash={}\nkasm_contract_hash={}\nkasm_semantic_fingerprint={}\nproperties={}\nzones={}\nmetric_snapshots={}\nwork_items={}\n{}\n\n{}",
        pack.pack_id,
        pack.evidence_hash,
        kasm_contract.program_hash,
        kasm_contract.semantic_fingerprint,
        store_summary.properties,
        store_summary.zones,
        store_summary.metric_snapshots,
        pack.work_items,
        best,
        pack.llm_summary
    );
    let hash = node
        .store()
        .store(note.as_bytes())
        .map_err(|e| format!("store real estate brain note: {e}"))?;
    let by_hash_ref = format!("refs/brain/llm/by_hash/{}", hash.as_hex());
    node.store()
        .write_ref(BRAIN_LLM_NOTE_LATEST_REF, &hash, "real estate latest LLM note")
        .map_err(|e| format!("write latest brain note ref: {e}"))?;
    node.store()
        .write_ref(
            REAL_ESTATE_BRAIN_NOTE_REF,
            &hash,
            "real estate scoped LLM note",
        )
        .map_err(|e| format!("write real estate brain scoped ref: {e}"))?;
    node.store()
        .write_ref(
            REAL_ESTATE_BRAIN_NOTE_LAYER_REF,
            &hash,
            "real estate semantic LLM note",
        )
        .map_err(|e| format!("write real estate brain semantic ref: {e}"))?;
    node.store()
        .write_ref(
            REAL_ESTATE_INTEL_BRAIN_REF,
            &hash,
            "real estate latest intel pack",
        )
        .map_err(|e| format!("write real estate intel brain ref: {e}"))?;
    node.store()
        .write_ref(&by_hash_ref, &hash, "real estate LLM note by hash")
        .map_err(|e| format!("write real estate brain by-hash ref: {e}"))?;
    update_real_estate_brain_note_index(node.store(), hash)?;
    Ok((hash.as_hex(), REAL_ESTATE_INTEL_BRAIN_REF.to_string()))
}

fn update_real_estate_brain_note_index(store: &Store, hash: Hash) -> Result<(), String> {
    let mut hashes = real_estate_brain_note_index_hashes(store);
    hashes.retain(|existing| *existing != hash);
    hashes.insert(0, hash);
    hashes.truncate(JOB_EVENTS_SNAPSHOT_LIMIT.min(32));
    let mut out = String::new();
    out.push_str("forge-brain-note-index-v1\n");
    out.push_str("scope=agence-immo\n");
    out.push_str("memory_layer=semantic\n");
    out.push_str(&format!("count={}\n\n", hashes.len()));
    for hash in hashes {
        out.push_str("hash=");
        out.push_str(&hash.as_hex());
        out.push('\n');
    }
    let index_hash = store
        .store(out.as_bytes())
        .map_err(|e| format!("store real estate brain note index: {e}"))?;
    store
        .write_ref(
            REAL_ESTATE_BRAIN_NOTE_INDEX_REF,
            &index_hash,
            "real estate semantic note index",
        )
        .map_err(|e| format!("write real estate brain note index: {e}"))
}

fn real_estate_brain_note_index_hashes(store: &Store) -> Vec<Hash> {
    let Some(index_hash) = store.lookup_ref(REAL_ESTATE_BRAIN_NOTE_INDEX_REF) else {
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

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}

fn refresh_intel_pack(
    store_path: &Path,
    reports: &[HarvestRunReport],
    trigger: &str,
) -> Result<RealEstateIntelPack, String> {
    refresh_intel_pack_stage(store_path, reports, trigger, true, true)
}

fn refresh_intel_pack_stage(
    store_path: &Path,
    reports: &[HarvestRunReport],
    trigger: &str,
    publish_brain: bool,
    write_llm_cache: bool,
) -> Result<RealEstateIntelPack, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let generated_at_ms = now_ms();
    let store_summary = local_store_summary(&base).unwrap_or_default();
    let snapshots = read_recent_metric_snapshots(&base, 512)?;
    let kasm_contract = ensure_kasm_score_contract(&base)?;
    if let Ok(program) = kasm_score_program() {
        let _ = publish_kasm_score_to_brain(store_path, &program);
    }
    let evidence_hash = hash_parts(
        "real_estate_intel_pack_inputs:v1",
        &[
            &intel_evidence_hash(reports, trigger),
            &store_summary.data_hash,
            &format!("snapshots={}", snapshots.len()),
            &kasm_contract.program_hash,
        ],
    );
    let candidate_count = store_summary.properties.max(snapshots.len());
    let scenario_count = 96usize
        .saturating_add(reports.len().saturating_mul(48))
        .saturating_add(store_summary.zones.saturating_mul(2));
    let horizon_count = 4usize;
    let work_items = candidate_count
        .saturating_mul(scenario_count)
        .saturating_mul(horizon_count);
    let mut opportunities = snapshots
        .iter()
        .take(12)
        .enumerate()
        .map(|(idx, snapshot)| {
            let seed = hash_seed(&format!(
                "{}:{}:{}",
                snapshot.proof_hash, evidence_hash, idx
            ));
            let horizon_days = [30_u16, 60, 90, 120][((seed >> 17) as usize) % 4];
            let proof_hash = hash_parts(
                "real_estate_intel_opportunity:v1",
                &[
                    &snapshot.property_id,
                    &snapshot.zone_id,
                    &format!("{:.6}", snapshot.score),
                    &format!("{:.6}", snapshot.seller_probability),
                    &snapshot.strongest_signal,
                    &snapshot.proof_hash,
                    &evidence_hash,
                ],
            );
            RealEstateIntelOpportunity {
                property_id: snapshot.property_id.clone(),
                zone_id: snapshot.zone_id.clone(),
                score: snapshot.score,
                seller_probability: snapshot.seller_probability,
                expected_fee_eur: snapshot.expected_fee_eur,
                horizon_days,
                strongest_signal: snapshot.strongest_signal.clone(),
                proof_hash,
            }
        })
        .collect::<Vec<_>>();
    if opportunities.is_empty() {
        opportunities = fallback_opportunities(reports, &evidence_hash);
    }
    opportunities.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let pack_id = format!(
        "rei-{}-{}",
        generated_at_ms,
        short_hash(&format!("{evidence_hash}:{work_items}"))
    );
    let artifact_path = base
        .join(INTEL_DIR)
        .join(format!("{}.json", sanitize_filename(&pack_id)));
    let best = opportunities.first();
    let llm_summary = if let Some(best) = best {
        format!(
            "best_property={} zone={} horizon={}d score={:.2} seller_prob={:.2} expected_fee={:.0} strongest_signal={} input_runs={} work_items={} evidence={}",
            best.property_id,
            best.zone_id,
            best.horizon_days,
            best.score,
            best.seller_probability,
            best.expected_fee_eur,
            best.strongest_signal,
            reports.len(),
            work_items,
            &evidence_hash[..24.min(evidence_hash.len())],
        )
    } else {
        format!(
            "no_opportunity_yet input_runs={} work_items={} evidence={}",
            reports.len(),
            work_items,
            &evidence_hash[..24.min(evidence_hash.len())],
        )
    };
    let mut pack = RealEstateIntelPack {
        pack_id,
        status: "ready".to_string(),
        generated_at_ms,
        trigger: trigger.to_string(),
        input_runs: reports.len(),
        metric_count: INTEL_METRIC_MANIFEST.len(),
        candidate_count,
        scenario_count,
        horizon_count,
        work_items,
        metric_manifest: INTEL_METRIC_MANIFEST
            .iter()
            .map(|metric| (*metric).to_string())
            .collect(),
        kasm_contract_hash: kasm_contract.program_hash.clone(),
        kasm_semantic_fingerprint: kasm_contract.semantic_fingerprint.clone(),
        brain_note_hash: None,
        brain_ref: None,
        top_opportunities: opportunities,
        evidence_hash,
        artifact_path: artifact_path.to_string_lossy().to_string(),
        llm_summary,
    };
    if publish_brain {
        if let Ok((brain_hash, brain_ref)) =
            publish_intel_pack_to_brain(store_path, &pack, &store_summary, &kasm_contract)
        {
            pack.brain_note_hash = Some(brain_hash);
            pack.brain_ref = Some(brain_ref);
        }
    }
    write_intel_pack_payload(&base, &pack)?;
    append_intel_ledger(&base, &pack)?;
    if write_llm_cache {
        let llm_cache = write_llm_intel_cache(&base, &pack, &store_summary, &kasm_contract)?;
        append_llm_cache_ledger(&base, &llm_cache)?;
    }
    Ok(pack)
}

fn fallback_opportunities(
    reports: &[HarvestRunReport],
    evidence_hash: &str,
) -> Vec<RealEstateIntelOpportunity> {
    let mut opportunities = Vec::new();
    for idx in 0..12 {
        let report = reports
            .get(idx % reports.len().max(1))
            .or_else(|| reports.first());
        let seed = report
            .map(|item| hash_seed(&format!("{}:{}:{idx}", item.proof_hash, evidence_hash)))
            .unwrap_or_else(|| hash_seed(&format!("{evidence_hash}:{idx}")));
        let zone = 1 + (seed % 144);
        let signal_idx = ((seed >> 9) as usize) % INTEL_METRIC_MANIFEST.len();
        let score = 62.0 + pseudo_unit(seed, 1) * 38.0 + idx as f64 * 0.17;
        let seller_probability = (0.38 + pseudo_unit(seed, 2) * 0.58).clamp(0.01, 0.98);
        let expected_fee_eur = 5_000.0 + pseudo_unit(seed, 3) * 58_000.0;
        let horizon_days = [30_u16, 60, 90, 120][((seed >> 21) as usize) % 4];
        let property_id = format!("BIEN-{:03}-{}", zone, short_hash(&format!("{seed}:{idx}")));
        let strongest_signal = INTEL_METRIC_MANIFEST[signal_idx].to_string();
        let proof_hash = hash_parts(
            "real_estate_intel_opportunity_fallback:v1",
            &[
                &property_id,
                &zone.to_string(),
                &format!("{score:.6}"),
                &format!("{seller_probability:.6}"),
                &strongest_signal,
                evidence_hash,
            ],
        );
        opportunities.push(RealEstateIntelOpportunity {
            property_id,
            zone_id: format!("zone-{zone:03}"),
            score,
            seller_probability,
            expected_fee_eur,
            horizon_days,
            strongest_signal,
            proof_hash,
        });
    }
    opportunities
}

fn write_intel_pack_payload(base: &Path, pack: &RealEstateIntelPack) -> Result<(), String> {
    let payload = serde_json::to_vec_pretty(pack)
        .map_err(|e| format!("serialize real estate intel pack: {e}"))?;
    if pack.artifact_path.is_empty() {
        return Err("intel pack artifact path is empty".to_string());
    }
    fs::write(&pack.artifact_path, &payload).map_err(|e| format!("write intel pack: {e}"))?;
    fs::write(base.join(INTEL_DIR).join(LATEST_INTEL_FILE), payload)
        .map_err(|e| format!("write latest intel pack: {e}"))
}

fn read_latest_intel_pack(base: &Path) -> Result<Option<RealEstateIntelPack>, String> {
    let path = base.join(INTEL_DIR).join(LATEST_INTEL_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|e| format!("read latest intel pack: {e}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("parse latest intel pack: {e}"))
}

fn write_llm_intel_cache(
    base: &Path,
    pack: &RealEstateIntelPack,
    store_summary: &RealEstateLocalStoreSummary,
    kasm_contract: &RealEstateKasmContract,
) -> Result<RealEstateLlmIntelCache, String> {
    let cache_key = hash_parts(
        "real_estate_llm_intel_cache:v1",
        &[
            &pack.pack_id,
            &pack.evidence_hash,
            &kasm_contract.program_hash,
            &format!("work_items={}", pack.work_items),
        ],
    );
    let cache_id = format!("re-llm-cache-{}", &cache_key[..16]);
    let cache_path = base
        .join(DATA_DIR)
        .join(LLM_INTEL_CACHE_DIR)
        .join(format!("{}.json", sanitize_filename(&cache_id)));
    let top_opportunities = pack
        .top_opportunities
        .iter()
        .take(8)
        .enumerate()
        .map(|(idx, opportunity)| llm_cache_opportunity(idx + 1, opportunity))
        .collect::<Vec<_>>();
    let action_brief = if top_opportunities.is_empty() {
        format!(
            "Aucune opportunite exploitable pour le moment. Base locale: {} biens, {} zones, {} snapshots.",
            store_summary.properties, store_summary.zones, store_summary.metric_snapshots
        )
    } else {
        top_opportunities
            .iter()
            .take(3)
            .map(|opportunity| {
                format!(
                    "#{rank} {property} ({zone}) prob={prob:.0}% action={action}",
                    rank = opportunity.rank,
                    property = opportunity.property_id,
                    zone = opportunity.zone_id,
                    prob = opportunity.seller_probability * 100.0,
                    action = opportunity.recommended_action,
                )
            })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let prompt_context = format!(
        "Tu es Forge en mode agence immobiliere. Utilise ce cache compact avant tout calcul LLM natif. Les donnees brutes restent locales; cite les preuves par evidenceHash, proofHash, kasmContractHash et brainRef. Pack={}, biens={}, zones={}, snapshots={}, workItems={}, KASM={}.",
        pack.pack_id,
        store_summary.properties,
        store_summary.zones,
        store_summary.metric_snapshots,
        pack.work_items,
        kasm_contract.program_hash
    );
    let cache = RealEstateLlmIntelCache {
        cache_id,
        status: "ready".to_string(),
        generated_at_ms: now_ms(),
        source_pack_id: pack.pack_id.clone(),
        source_pack_path: pack.artifact_path.clone(),
        evidence_hash: pack.evidence_hash.clone(),
        metric_manifest_hash: kasm_contract.metric_manifest_hash.clone(),
        kasm_contract_hash: kasm_contract.program_hash.clone(),
        kasm_semantic_fingerprint: kasm_contract.semantic_fingerprint.clone(),
        brain_note_hash: pack.brain_note_hash.clone(),
        brain_ref: pack.brain_ref.clone(),
        local_store: store_summary.clone(),
        top_opportunities,
        prompt_context,
        action_brief,
        ingestion_policy: vec![
            "Lire ce cache avant de demander au modele de raisonner depuis zero.".to_string(),
            "Ne jamais charger les fichiers bruts si les hashes et facts suffisent.".to_string(),
            "Verifier les decisions sensibles avec les proofHash et evidenceHash.".to_string(),
            "Relancer Data Sync seulement si la fraicheur du cache est insuffisante.".to_string(),
        ],
        cache_path: cache_path.to_string_lossy().to_string(),
    };
    let payload = serde_json::to_vec_pretty(&cache)
        .map_err(|e| format!("serialize LLM intel cache: {e}"))?;
    fs::write(&cache_path, &payload).map_err(|e| format!("write LLM intel cache: {e}"))?;
    fs::write(
        base.join(DATA_DIR)
            .join(LLM_INTEL_CACHE_DIR)
            .join(LATEST_LLM_INTEL_CACHE_FILE),
        payload,
    )
    .map_err(|e| format!("write latest LLM intel cache: {e}"))?;
    Ok(cache)
}

fn llm_cache_opportunity(
    rank: usize,
    opportunity: &RealEstateIntelOpportunity,
) -> RealEstateLlmIntelOpportunity {
    let recommended_action = if opportunity.seller_probability >= 0.78 {
        "Preparer une approche vendeur prioritaire avec angle local verifiable"
    } else if opportunity.seller_probability >= 0.58 {
        "Nourrir le lead par veille quartier puis relancer sous 30 jours"
    } else if opportunity.expected_fee_eur >= 18_000.0 {
        "Surveiller le timing mandat car le potentiel fee reste eleve"
    } else {
        "Conserver en veille et attendre un signal supplementaire"
    }
    .to_string();
    let fact_line = format!(
        "score={:.2}; probabilite_vendeur={:.3}; fee_attendu={:.0}; horizon={}j; signal={}; preuve={}",
        opportunity.score,
        opportunity.seller_probability,
        opportunity.expected_fee_eur,
        opportunity.horizon_days,
        opportunity.strongest_signal,
        &opportunity.proof_hash[..24.min(opportunity.proof_hash.len())],
    );
    RealEstateLlmIntelOpportunity {
        rank,
        property_id: opportunity.property_id.clone(),
        zone_id: opportunity.zone_id.clone(),
        score: opportunity.score,
        seller_probability: opportunity.seller_probability,
        expected_fee_eur: opportunity.expected_fee_eur,
        horizon_days: opportunity.horizon_days,
        strongest_signal: opportunity.strongest_signal.clone(),
        recommended_action,
        fact_line,
        proof_hash: opportunity.proof_hash.clone(),
    }
}

fn read_latest_llm_intel_cache(base: &Path) -> Result<Option<RealEstateLlmIntelCache>, String> {
    let path = base
        .join(DATA_DIR)
        .join(LLM_INTEL_CACHE_DIR)
        .join(LATEST_LLM_INTEL_CACHE_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|e| format!("read latest LLM intel cache: {e}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("parse latest LLM intel cache: {e}"))
}

fn append_intel_ledger(base: &Path, pack: &RealEstateIntelPack) -> Result<(), String> {
    let entry = json!({
        "kind": "real_estate_intel_pack",
        "packId": pack.pack_id,
        "status": pack.status,
        "trigger": pack.trigger,
        "inputRuns": pack.input_runs,
        "metricCount": pack.metric_count,
        "candidateCount": pack.candidate_count,
        "scenarioCount": pack.scenario_count,
        "workItems": pack.work_items,
        "evidenceHash": pack.evidence_hash,
        "kasmContractHash": pack.kasm_contract_hash,
        "kasmSemanticFingerprint": pack.kasm_semantic_fingerprint,
        "brainNoteHash": pack.brain_note_hash,
        "brainRef": pack.brain_ref,
        "artifactPath": pack.artifact_path,
        "createdAtMs": pack.generated_at_ms,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(base.join(LEDGER_FILE))
        .map_err(|e| format!("open harvester ledger: {e}"))?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(&entry).map_err(|e| format!("serialize intel ledger: {e}"))?
    )
    .map_err(|e| format!("append intel ledger: {e}"))
}

fn append_llm_cache_ledger(base: &Path, cache: &RealEstateLlmIntelCache) -> Result<(), String> {
    let entry = json!({
        "kind": "real_estate_llm_intel_cache",
        "cacheId": cache.cache_id,
        "status": cache.status,
        "sourcePackId": cache.source_pack_id,
        "evidenceHash": cache.evidence_hash,
        "kasmContractHash": cache.kasm_contract_hash,
        "brainNoteHash": cache.brain_note_hash,
        "brainRef": cache.brain_ref,
        "opportunities": cache.top_opportunities.len(),
        "cachePath": cache.cache_path,
        "createdAtMs": cache.generated_at_ms,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(base.join(LEDGER_FILE))
        .map_err(|e| format!("open harvester ledger: {e}"))?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(&entry).map_err(|e| format!("serialize LLM cache ledger: {e}"))?
    )
    .map_err(|e| format!("append LLM cache ledger: {e}"))
}

fn intel_evidence_hash(reports: &[HarvestRunReport], trigger: &str) -> String {
    let mut parts = vec![trigger.to_string(), format!("runs={}", reports.len())];
    for report in reports {
        parts.push(report.job_id.clone());
        parts.push(report.collector_id.clone());
        parts.push(report.proof_hash.clone());
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    hash_parts("real_estate_intel_evidence:v1", &refs)
}

fn hash_parts(stage: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(stage.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn hash_seed(value: &str) -> u64 {
    let digest = Sha256::digest(value.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

fn pseudo_unit(seed: u64, salt: u64) -> f64 {
    let mut value = seed.wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    (value >> 11) as f64 / ((1_u64 << 53) as f64)
}

fn proof_hash(report: &HarvestRunReport) -> Result<String, String> {
    let mut clone = report.clone();
    clone.proof_hash.clear();
    clone.artifact_path.clear();
    let bytes = serde_json::to_vec(&clone).map_err(|e| format!("serialize harvester proof: {e}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("{:x}", digest)[..12].to_string()
}

fn normalize_id(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' })
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_business_collectors() {
        let registry = default_registry();
        assert!(registry.collectors.iter().any(|it| it.tools.contains(&"recrutement".to_string())));
        assert!(registry.collectors.iter().any(|it| it.tools.contains(&"courtier".to_string())));
        assert!(registry.collectors.iter().any(|it| it.tools.contains(&"assurance".to_string())));
    }

    #[test]
    fn normalizes_ui_tool_ids() {
        assert_eq!(normalize_id("DPE / ADEME"), "dpe-ademe");
        assert_eq!(normalize_id("Rapport vendeur"), "rapport-vendeur");
    }
}
