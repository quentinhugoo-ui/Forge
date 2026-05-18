use crate::collection_os::{plan_collection, CollectionPlanRequest};
use crate::forge_job_runtime::{append_job_ledger_event, ForgeJobCost, ForgeJobProof, ForgeJobRetry, ForgeUnifiedJob};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use scan::kasm::{Node, Program, Target, Ty};
use scan::{publish_semantic_attractor, Hash, MemoryGovernor, MonsterNode, Store};
use std::collections::{HashMap, HashSet};
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
#[allow(dead_code)]
const ONBOARDING_PROFILE_FILE: &str = "agency_onboarding_profile.json";
#[allow(dead_code)]
const ONBOARDING_EVENTS_FILE: &str = "agency_onboarding_events.jsonl";
const ONBOARDING_WEB_FINDINGS_FILE: &str = "agency_onboarding_web_findings.json";
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

#[derive(Clone, Debug, Default)]
struct CollectionArtifactSummary {
    route_count: usize,
    hypothesis_count: usize,
    entity_count: usize,
    proof_hash: String,
    artifact_path: String,
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
    pub projection_hash: String,
    pub evidence_hash: String,
    pub memory_evidence_hash: String,
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
    pub unified_jobs: Vec<ForgeUnifiedJob>,
    pub local_store: RealEstateLocalStoreSummary,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateOnboardingQuestion {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub why: String,
    pub examples: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateOnboardingProfile {
    pub schema_version: u8,
    pub status: String,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub completed_at_ms: u64,
    pub answers: HashMap<String, String>,
    pub derived_traits: HashMap<String, String>,
    pub enrichment_runs: Vec<String>,
    pub profile_hash: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateOnboardingState {
    pub required: bool,
    pub status: String,
    pub current_index: usize,
    pub total: usize,
    pub question: Option<RealEstateOnboardingQuestion>,
    pub answered: Vec<String>,
    pub derived_traits: HashMap<String, String>,
    pub profile_hash: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateOnboardingAnswerReport {
    pub status: String,
    pub answered_question_id: String,
    pub next_question: Option<RealEstateOnboardingQuestion>,
    pub state: RealEstateOnboardingState,
    pub suggested_answers: Vec<String>,
    pub triggered_collectors: Vec<HarvestRunReport>,
    pub enrichment_queries: Vec<String>,
    pub web_findings: Vec<RealEstateOnboardingWebPageFinding>,
    pub profile_hash: String,
    pub event_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealEstateOnboardingWebPageFinding {
    pub query: String,
    pub url: String,
    pub source_domain: String,
    pub title: String,
    pub snippet: String,
    pub listing_signals: Vec<String>,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub fetched_at_ms: u64,
    pub evidence_hash: String,
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

struct HttpTextFetch {
    url: String,
    status: u16,
    body: String,
    elapsed_ms: u64,
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
    let unified_jobs = job_queue
        .jobs
        .iter()
        .map(unified_job_from_real_estate)
        .collect::<Vec<_>>();
    Ok(HarvesterSnapshot {
        daemon: daemon_status(&base),
        registry: registry.clone(),
        latest_run: read_latest_run(&base).ok().flatten(),
        latest_intel_pack: read_latest_intel_pack(&base).ok().flatten(),
        latest_llm_intel_cache: read_latest_llm_intel_cache(&base).ok().flatten(),
        supervisor: load_or_init_supervisor(&base, &registry)?,
        job_queue,
        job_journal,
        unified_jobs,
        local_store: local_store_summary(&base).unwrap_or_default(),
    })
}

#[allow(dead_code)]
pub fn onboarding_state(store_path: &Path) -> Result<RealEstateOnboardingState, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let profile = read_onboarding_profile(&base)?;
    Ok(onboarding_state_from_profile(&profile))
}

#[allow(dead_code)]
pub fn record_onboarding_answer(
    store_path: &Path,
    question_id: &str,
    answer: &str,
) -> Result<RealEstateOnboardingAnswerReport, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let questions = onboarding_questions();
    let question_id = normalize_onboarding_question_id(question_id);
    if !questions.iter().any(|question| question.id == question_id) {
        return Err(format!("unknown real estate onboarding question: {question_id}"));
    }
    let answer = answer.trim();
    if answer.is_empty() {
        return Err("La réponse onboarding ne peut pas être vide.".to_string());
    }

    let mut profile = read_onboarding_profile(&base)?;
    let now = now_ms();
    if profile.started_at_ms == 0 {
        profile.started_at_ms = now;
    }
    profile.updated_at_ms = now;
    profile.answers.insert(question_id.clone(), answer.to_string());
    profile.derived_traits = derive_onboarding_traits(&profile.answers);
    let collection_plan = real_estate_onboarding_collection_plan(&question_id, &profile);
    if let Ok(plan) = &collection_plan {
        profile
            .derived_traits
            .insert("collection_os_plan_hash".to_string(), plan.proof_hash.clone());
        profile
            .derived_traits
            .insert("collection_os_sector_pack".to_string(), plan.sector_pack_id.clone());
        profile.derived_traits.insert(
            "collection_os_route".to_string(),
            plan.steps
                .iter()
                .map(|step| step.surface_id.as_str())
                .collect::<Vec<_>>()
                .join(">"),
        );
    }
    let should_refresh_google_places = should_refresh_google_places_traits(&profile.derived_traits);
    if question_id == "agency_identity" || !should_refresh_google_places {
        let profile_snapshot = profile.clone();
        enrich_onboarding_traits_with_fallback_resolution(
            &base,
            &question_id,
            &profile_snapshot,
            &mut profile.derived_traits,
        );
    }
    let web_findings: Vec<RealEstateOnboardingWebPageFinding> = Vec::new();
    let completed = questions
        .iter()
        .all(|question| profile.answers.get(&question.id).map(|value| !value.trim().is_empty()).unwrap_or(false));
    profile.status = if completed { "completed" } else { "in_progress" }.to_string();
    if completed && profile.completed_at_ms == 0 {
        profile.completed_at_ms = now;
    }

    let triggered_collectors: Vec<HarvestRunReport> = Vec::new();

    profile.profile_hash = onboarding_profile_hash(&profile);
    write_onboarding_profile(&base, &profile)?;
    let state = onboarding_state_from_profile(&profile);
    let suggested_answers = state
        .question
        .as_ref()
        .map(|question| onboarding_suggestions_for_question(question, &profile))
        .unwrap_or_default();
    let enrichment_queries = onboarding_enrichment_queries(&question_id, &profile);
    let event_hash = hash_parts(
        "real_estate_onboarding_answer:v1",
        &[
            &question_id,
            answer,
            &profile.profile_hash,
            "",
        ],
    );
    append_json_line(
        &data_path(&base, ONBOARDING_EVENTS_FILE),
        &json!({
            "type": "onboarding_answer",
            "questionId": question_id,
            "answerHash": short_hash(answer),
            "profileHash": profile.profile_hash,
            "eventHash": event_hash,
            "triggeredCollectors": triggered_collectors.iter().map(|report| report.collector_id.clone()).collect::<Vec<_>>(),
            "enrichmentQueries": enrichment_queries,
            "webFindings": web_findings.iter().map(|finding| &finding.url).collect::<Vec<_>>(),
            "collectionPlan": collection_plan.as_ref().ok().map(|plan| json!({
                "proofHash": plan.proof_hash,
                "sectorPackId": plan.sector_pack_id,
                "route": plan.steps.iter().map(|step| step.surface_id.clone()).collect::<Vec<_>>(),
                "latencyBudgetMs": plan.latency_budget_ms
            })),
            "tsMs": now
        }),
    )?;

    if should_refresh_google_places && question_id != "agency_identity" {
        spawn_onboarding_google_places_refresh(base.clone());
    }

    Ok(RealEstateOnboardingAnswerReport {
        status: profile.status.clone(),
        answered_question_id: question_id,
        next_question: state.question.clone(),
        state,
        suggested_answers,
        triggered_collectors,
        enrichment_queries,
        web_findings,
        profile_hash: profile.profile_hash,
        event_hash,
    })
}

#[allow(dead_code)]
pub fn resolve_onboarding_agency_identity(
    store_path: &Path,
    agency_name: &str,
    city: &str,
    original_user_text: &str,
) -> Result<serde_json::Value, String> {
    let agency_name = agency_name.trim();
    let city = city.trim();
    if agency_name.is_empty() {
        return Err("agency_name is required".to_string());
    }
    if city.is_empty() {
        return Err("city is required".to_string());
    }

    let base = ensure_harvester_dirs(store_path)?;
    let mut profile = read_onboarding_profile(&base)?;
    let now = now_ms();
    if profile.started_at_ms == 0 {
        profile.started_at_ms = now;
    }
    profile.updated_at_ms = now;
    let answer = format!("{agency_name} a {city}");
    profile
        .answers
        .insert("agency_identity".to_string(), answer.clone());
    profile.derived_traits = derive_onboarding_traits(&profile.answers);
    profile
        .derived_traits
        .insert("agency_display_name".to_string(), truncate_for_trait(agency_name, 180));
    profile
        .derived_traits
        .insert("agency_search_name".to_string(), truncate_for_trait(agency_name, 180));
    profile
        .derived_traits
        .insert("agency_city".to_string(), truncate_for_trait(city, 120));
    profile
        .derived_traits
        .insert("harvester_zone_seed".to_string(), truncate_for_trait(city, 120));
    profile
        .derived_traits
        .insert("priority_zones".to_string(), truncate_for_trait(city, 180));
    profile
        .derived_traits
        .insert("priority_zone_count".to_string(), "1".to_string());
    if !original_user_text.trim().is_empty() {
        profile.derived_traits.insert(
            "agency_identity_user_text".to_string(),
            truncate_for_trait(original_user_text.trim(), 320),
        );
    }

    let snapshot = profile.clone();
    enrich_onboarding_traits_with_fallback_resolution(
        &base,
        "agency_identity",
        &snapshot,
        &mut profile.derived_traits,
    );

    let questions = onboarding_questions();
    let completed = questions
        .iter()
        .all(|question| profile.answers.get(&question.id).map(|value| !value.trim().is_empty()).unwrap_or(false));
    profile.status = if completed { "completed" } else { "in_progress" }.to_string();
    if completed && profile.completed_at_ms == 0 {
        profile.completed_at_ms = now;
    }
    profile.profile_hash = onboarding_profile_hash(&profile);
    write_onboarding_profile(&base, &profile)?;
    let state = onboarding_state_from_profile(&profile);
    let traits = &profile.derived_traits;
    let contact = json!({
        "agencyName": traits.get("agency_display_name").or_else(|| traits.get("agency_search_name")).cloned().unwrap_or_else(|| agency_name.to_string()),
        "city": traits.get("agency_city").cloned().unwrap_or_else(|| city.to_string()),
        "address": traits.get("agency_address").cloned().unwrap_or_default(),
        "phone": traits.get("agency_phone").cloned().unwrap_or_default(),
        "website": traits.get("agency_website").cloned().unwrap_or_default(),
        "googleMapsUri": traits.get("agency_google_maps_uri").cloned().unwrap_or_default(),
        "source": traits.get("agency_resolution_source").or_else(|| traits.get("contact_source")).cloned().unwrap_or_default(),
        "status": traits.get("agency_resolution_status").or_else(|| traits.get("google_places_status")).cloned().unwrap_or_default()
    });
    let event_hash = hash_parts(
        "real_estate_agency_identity_resolve:v1",
        &[
            agency_name,
            city,
            contact.get("address").and_then(serde_json::Value::as_str).unwrap_or(""),
            &profile.profile_hash,
        ],
    );
    append_json_line(
        &data_path(&base, ONBOARDING_EVENTS_FILE),
        &json!({
            "type": "agency_identity_resolve",
            "agencyName": agency_name,
            "city": city,
            "profileHash": profile.profile_hash,
            "eventHash": event_hash,
            "contact": contact,
            "tsMs": now
        }),
    )?;
    Ok(json!({
        "kind": "real_estate_agency_identity_resolution",
        "status": "resolved",
        "questionId": "agency_identity",
        "eventHash": event_hash,
        "profileHash": profile.profile_hash,
        "contact": contact,
        "state": state,
        "confirmationPrompt": "Demande a l'utilisateur si ces informations sont exactes avant de continuer l'onboarding.",
        "rawDataReturned": false
    }))
}

#[allow(dead_code)]
pub fn confirm_onboarding_agency_identity(
    store_path: &Path,
    confirmed: bool,
    correction: Option<&str>,
) -> Result<serde_json::Value, String> {
    let base = ensure_harvester_dirs(store_path)?;
    let mut profile = read_onboarding_profile(&base)?;
    let now = now_ms();
    profile.updated_at_ms = now;
    if confirmed {
        profile
            .derived_traits
            .insert("agency_identity_confirmed".to_string(), "true".to_string());
        profile
            .derived_traits
            .insert("agency_identity_confirmed_at_ms".to_string(), now.to_string());
    } else {
        profile
            .derived_traits
            .insert("agency_identity_confirmed".to_string(), "false".to_string());
        if let Some(correction) = correction.map(str::trim).filter(|value| !value.is_empty()) {
            profile.derived_traits.insert(
                "agency_identity_correction".to_string(),
                truncate_for_trait(correction, 360),
            );
        }
    }
    profile.profile_hash = onboarding_profile_hash(&profile);
    write_onboarding_profile(&base, &profile)?;
    let state = onboarding_state_from_profile(&profile);
    let traits = &profile.derived_traits;
    let contact = json!({
        "agencyName": traits.get("agency_display_name").or_else(|| traits.get("agency_search_name")).cloned().unwrap_or_default(),
        "city": traits.get("agency_city").cloned().unwrap_or_default(),
        "address": traits.get("agency_address").cloned().unwrap_or_default(),
        "phone": traits.get("agency_phone").cloned().unwrap_or_default(),
        "email": traits.get("agency_email").cloned().unwrap_or_default(),
        "website": traits.get("agency_website").cloned().unwrap_or_default(),
        "googleMapsUri": traits.get("agency_google_maps_uri").cloned().unwrap_or_default(),
        "lat": traits.get("agency_lat").and_then(|value| value.parse::<f64>().ok()),
        "lng": traits.get("agency_lng").and_then(|value| value.parse::<f64>().ok()),
        "source": traits.get("agency_resolution_source").or_else(|| traits.get("contact_source")).cloned().unwrap_or_default(),
        "confirmed": confirmed
    });
    let event_hash = hash_parts(
        "real_estate_agency_identity_confirm:v1",
        &[
            contact.get("agencyName").and_then(serde_json::Value::as_str).unwrap_or(""),
            contact.get("city").and_then(serde_json::Value::as_str).unwrap_or(""),
            if confirmed { "confirmed" } else { "rejected" },
            &profile.profile_hash,
        ],
    );
    append_json_line(
        &data_path(&base, ONBOARDING_EVENTS_FILE),
        &json!({
            "type": "agency_identity_confirm",
            "confirmed": confirmed,
            "profileHash": profile.profile_hash,
            "eventHash": event_hash,
            "contact": contact.clone(),
            "tsMs": now
        }),
    )?;
    let ui_actions = if confirmed {
        json!([
            { "kind": "set_app_header", "agencyName": contact.get("agencyName").cloned().unwrap_or_default() },
            { "kind": "set_profile_agency", "contact": contact.clone() },
            { "kind": "open_google_earth", "query": contact.get("agencyName").cloned().unwrap_or_default(), "contact": contact.clone() }
        ])
    } else {
        json!([])
    };
    Ok(json!({
        "kind": "real_estate_agency_identity_confirmation",
        "status": if confirmed { "confirmed" } else { "needs_correction" },
        "questionId": "agency_identity",
        "eventHash": event_hash,
        "profileHash": profile.profile_hash,
        "contact": contact,
        "state": state,
        "uiActions": ui_actions,
        "assistantNext": if confirmed {
            "Remercie chaleureusement, prends acte que tu travailles maintenant pour cette agence, puis indique que Forge met a jour le profil et ouvre Google Earth."
        } else {
            "Demande la correction exacte a l'utilisateur avant de continuer."
        },
        "rawDataReturned": false
    }))
}

fn should_refresh_google_places_traits(traits: &HashMap<String, String>) -> bool {
    let name = traits
        .get("agency_search_name")
        .map(String::as_str)
        .or_else(|| traits.get("agency_display_name").map(String::as_str))
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        return false;
    }
    let status = traits
        .get("google_places_status")
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let has_address = traits
        .get("agency_address")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_phone = traits
        .get("agency_phone")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_website = traits
        .get("agency_website")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_coords = traits
        .get("agency_lat")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        && traits
            .get("agency_lng")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
    let has_required_contact = has_address && has_phone && has_website;
    status != "ok" || !has_required_contact || !has_coords
}

fn spawn_onboarding_google_places_refresh(base: PathBuf) {
    thread::spawn(move || {
        let _ = refresh_onboarding_google_places_traits(&base);
    });
}

fn refresh_onboarding_google_places_traits(base: &Path) -> Result<(), String> {
    let mut profile = read_onboarding_profile(base)?;
    if !should_refresh_google_places_traits(&profile.derived_traits) {
        return Ok(());
    }
    let before = profile.derived_traits.clone();
    let profile_snapshot = profile.clone();
    enrich_onboarding_traits_with_fallback_resolution(
        base,
        "agency_identity",
        &profile_snapshot,
        &mut profile.derived_traits,
    );
    if profile.derived_traits == before {
        return Ok(());
    }
    profile.updated_at_ms = now_ms();
    profile.profile_hash = onboarding_profile_hash(&profile);
    write_onboarding_profile(base, &profile)?;
    Ok(())
}

fn enrich_onboarding_traits_with_fallback_resolution(
    base: &Path,
    question_id: &str,
    profile: &RealEstateOnboardingProfile,
    traits: &mut HashMap<String, String>,
) {
    enrich_onboarding_traits_with_google_places(traits);
    let google_ok = traits
        .get("google_places_status")
        .map(|value| value.trim().eq_ignore_ascii_case("ok"))
        .unwrap_or(false);
    if google_ok {
        return;
    }
    let fallback_profile = RealEstateOnboardingProfile {
        derived_traits: traits.clone(),
        ..profile.clone()
    };
    match run_onboarding_web_research(base, question_id, &fallback_profile, traits) {
        Ok(findings) => {
            if findings.is_empty() {
                traits
                    .entry("agency_resolution_status".to_string())
                    .or_insert_with(|| "web_fallback_no_match".to_string());
                traits
                    .entry("google_places_status".to_string())
                    .or_insert_with(|| "web_fallback_no_match".to_string());
                return;
            }
            traits.insert(
                "agency_resolution_status".to_string(),
                "web_fallback_partial".to_string(),
            );
            if traits
                .get("google_places_status")
                .map(|value| value.trim().is_empty() || value.trim() == "missing_api_key")
                .unwrap_or(true)
            {
                traits.insert(
                    "google_places_status".to_string(),
                    "web_fallback_partial".to_string(),
                );
            }
        }
        Err(err) => {
            traits.insert(
                "agency_resolution_status".to_string(),
                format!("web_fallback_error:{}", truncate_for_trait(&err, 120)),
            );
            if traits
                .get("google_places_status")
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                traits.insert(
                    "google_places_status".to_string(),
                    "web_fallback_error".to_string(),
                );
            }
        }
    }
}

#[allow(dead_code)]
pub fn persist_onboarding_native_web_findings(
    store_path: &Path,
    findings: &[RealEstateOnboardingWebPageFinding],
    trait_updates: &HashMap<String, String>,
) -> Result<(), String> {
    if findings.is_empty() && trait_updates.is_empty() {
        return Ok(());
    }
    let base = ensure_harvester_dirs(store_path)?;
    let findings_path = data_path(&base, ONBOARDING_WEB_FINDINGS_FILE);
    let mut existing = if findings_path.exists() {
        let bytes = fs::read(&findings_path)
            .map_err(|e| format!("read agency onboarding web findings: {e}"))?;
        serde_json::from_slice::<Vec<RealEstateOnboardingWebPageFinding>>(&bytes)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    for finding in findings {
        if let Some(slot) = existing.iter_mut().find(|item| item.url == finding.url) {
            *slot = finding.clone();
        } else {
            existing.push(finding.clone());
        }
    }
    write_json_pretty(
        &findings_path,
        &existing,
        "agency onboarding web findings",
    )?;

    if !trait_updates.is_empty() {
        let mut profile = read_onboarding_profile(&base)?;
        for (key, value) in trait_updates {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if matches!(key.as_str(), "agency_email" | "agency_phone" | "agency_website") {
                profile
                    .derived_traits
                    .entry(key.to_string())
                    .or_insert_with(|| value.to_string());
            } else {
                profile
                    .derived_traits
                    .insert(key.to_string(), value.to_string());
            }
        }
        profile.updated_at_ms = now_ms();
        profile.profile_hash = onboarding_profile_hash(&profile);
        write_onboarding_profile(&base, &profile)?;
    }
    Ok(())
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

fn unified_job_from_real_estate(job: &RealEstateJobQueueEntry) -> ForgeUnifiedJob {
    ForgeUnifiedJob::new(
        job.job_id.clone(),
        job.job_kind.clone(),
        json!({
            "domain": "real-estate",
            "collectorId": job.collector_id.clone(),
            "toolId": job.tool_id.clone(),
            "trigger": job.trigger.clone(),
            "dependsOn": job.depends_on.clone(),
            "priority": job.priority,
            "scheduledAtMs": job.scheduled_at_ms,
            "createdAtMs": job.created_at_ms,
            "updatedAtMs": job.updated_at_ms,
            "lastError": job.last_error.clone(),
        }),
        job.status.clone(),
        ForgeJobCost {
            estimate_units: job.estimated_cost as u64,
            actual_units: 0,
            token_estimate: 0,
            budget_class: if job.estimated_cost >= 18 {
                "heavy".to_string()
            } else if job.estimated_cost >= 8 {
                "medium".to_string()
            } else {
                "light".to_string()
            },
        },
        ForgeJobRetry {
            attempt: job.attempts,
            max_attempts: job.max_attempts,
            not_before_ms: job.not_before_ms,
            leased_until_ms: job.leased_until_ms,
            next_retry_ms: job.not_before_ms,
        },
        ForgeJobProof {
            hash: job.last_proof_hash.clone(),
            artifact_path: job.artifact_path.clone(),
            source_hash: format!("{}:{}", job.collector_id, job.tool_id),
        },
    )
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
    append_json_line(&data_path(base, JOB_EVENTS_FILE), &event)?;
    if let Some(store_path) = base.parent() {
        let job = ForgeUnifiedJob::new(
            event.job_id.clone(),
            event.job_kind.clone(),
            json!({
                "domain": "real-estate",
                "collectorId": event.collector_id.clone(),
                "toolId": event.tool_id.clone(),
                "stage": event.stage.clone(),
                "message": event.message.clone(),
                "blockedBy": event.blocked_by.clone(),
            }),
            event.status.clone(),
            ForgeJobCost {
                estimate_units: event.estimated_cost as u64,
                actual_units: 0,
                token_estimate: 0,
                budget_class: if event.estimated_cost >= 18 {
                    "heavy".to_string()
                } else if event.estimated_cost >= 8 {
                    "medium".to_string()
                } else {
                    "light".to_string()
                },
            },
            ForgeJobRetry {
                attempt: event.attempt,
                max_attempts: 0,
                not_before_ms: event.next_retry_ms,
                leased_until_ms: 0,
                next_retry_ms: event.next_retry_ms,
            },
            ForgeJobProof {
                hash: event.proof_hash.clone(),
                artifact_path: event.artifact_path.clone(),
                source_hash: format!("{}:{}", event.collector_id, event.tool_id),
            },
        );
        let _ = append_job_ledger_event(store_path, &event.stage, event.at_ms, &job);
    }
    Ok(())
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
    if let Ok(collection_summary) =
        export_collection_os_artifact(&base, collector, &report.job_id, started, &store_update.data_hash)
    {
        if !collection_summary.proof_hash.is_empty() {
            normalized_outputs.extend([
                format!("collection_routes:{}", collection_summary.route_count),
                format!("collection_hypotheses:{}", collection_summary.hypothesis_count),
                format!("collection_entities:{}", collection_summary.entity_count),
                format!("collection_proof:{}", collection_summary.proof_hash),
                format!("collection_artifact:{}", collection_summary.artifact_path),
            ]);
            report
                .compliance_notes
                .push("Collection OS typed extraction artifact attached with local evidence.".to_string());
        }
    }
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

#[allow(dead_code)]
fn onboarding_questions() -> Vec<RealEstateOnboardingQuestion> {
    vec![
        onboarding_question(
            "agency_identity",
            "Identité de l'agence",
            "Comment s'appelle l'agence, dans quelle ville êtes-vous basés, et qu'est-ce qui vous distingue en quelques mots ?",
            "Forge s'en sert pour retrouver les traces publiques, la réputation, les concurrents proches et le marché local pertinent.",
            &["Agence Martin Immobilier, Lille, spécialiste maisons familiales premium."],
        ),
        onboarding_question(
            "agency_website",
            "Présence web",
            "Quel est le site internet de l'agence et les liens publics importants à surveiller ?",
            "Le site devient une source owned data pour auditer annonces, SEO, formulaires, conversion et réputation.",
            &["https://agence-exemple.fr, Google Business Profile, Instagram, LinkedIn."],
        ),
        onboarding_question(
            "agency_people",
            "Équipe",
            "Qui travaille dans l'agence, avec quels rôles et quelles zones ?",
            "Forge peut préparer la mémoire équipe, le coaching, les priorités de relance et le recrutement.",
            &["Quentin directeur, Sophie transaction Lille centre, Marc location et investisseurs."],
        ),
        onboarding_question(
            "agency_zones",
            "Zones de marché",
            "Quelles villes, quartiers, rues ou micro-zones devez-vous surveiller en priorité ?",
            "Ces zones déclenchent DVF, cadastre, DPE, risques, urbanisme, concurrence et veille locale ciblée.",
            &["Lille, Croix, Marcq-en-Baroeul, Lambersart, Vieux-Lille, Vauban."],
        ),
        onboarding_question(
            "agency_stack",
            "Données et outils",
            "Quels CRM, portails, boîtes mail, Drive, agendas, fichiers ou exports utilisez-vous déjà ?",
            "Forge sait quelles sources internes brancher ensuite, avec consentement, sans envoyer de fichiers bruts aux LLM.",
            &["Hektor, SeLoger, Bien'ici, Gmail, Drive mandats, agenda visites, exports CSV mensuels."],
        ),
        onboarding_question(
            "agency_priorities",
            "Objectifs",
            "Quels sont les objectifs prioritaires sur les 90 prochains jours ?",
            "Le moteur peut classer les scrapers, simulations KASM et alertes selon ce qui crée vraiment du chiffre.",
            &["Plus de mandats exclusifs, meilleur pricing, relance vendeurs froids, recrutement d'un négociateur."],
        ),
    ]
}

#[allow(dead_code)]
fn onboarding_question(
    id: &str,
    title: &str,
    prompt: &str,
    why: &str,
    examples: &[&str],
) -> RealEstateOnboardingQuestion {
    RealEstateOnboardingQuestion {
        id: id.to_string(),
        title: title.to_string(),
        prompt: prompt.to_string(),
        why: why.to_string(),
        examples: examples.iter().map(|example| example.to_string()).collect(),
    }
}

#[allow(dead_code)]
fn empty_onboarding_profile(now: u64) -> RealEstateOnboardingProfile {
    RealEstateOnboardingProfile {
        schema_version: 1,
        status: "not_started".to_string(),
        started_at_ms: now,
        updated_at_ms: now,
        completed_at_ms: 0,
        answers: HashMap::new(),
        derived_traits: HashMap::new(),
        enrichment_runs: Vec::new(),
        profile_hash: String::new(),
    }
}

#[allow(dead_code)]
fn read_onboarding_profile(base: &Path) -> Result<RealEstateOnboardingProfile, String> {
    let path = data_path(base, ONBOARDING_PROFILE_FILE);
    if !path.exists() {
        let mut profile = empty_onboarding_profile(now_ms());
        profile.profile_hash = onboarding_profile_hash(&profile);
        write_onboarding_profile(base, &profile)?;
        return Ok(profile);
    }
    let bytes = fs::read(&path).map_err(|e| format!("read agency onboarding profile: {e}"))?;
    let mut profile: RealEstateOnboardingProfile = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse agency onboarding profile: {e}"))?;
    if profile.schema_version == 0 {
        profile.schema_version = 1;
    }
    if profile.started_at_ms == 0 {
        profile.started_at_ms = profile.updated_at_ms.max(now_ms());
    }
    if profile.profile_hash.is_empty() {
        profile.profile_hash = onboarding_profile_hash(&profile);
    }
    Ok(profile)
}

#[allow(dead_code)]
fn write_onboarding_profile(base: &Path, profile: &RealEstateOnboardingProfile) -> Result<(), String> {
    write_json_pretty(&data_path(base, ONBOARDING_PROFILE_FILE), profile, "agency onboarding profile")
}

#[allow(dead_code)]
fn onboarding_state_from_profile(profile: &RealEstateOnboardingProfile) -> RealEstateOnboardingState {
    let questions = onboarding_questions();
    let question = questions
        .iter()
        .find(|question| {
            profile
                .answers
                .get(&question.id)
                .map(|answer| answer.trim().is_empty())
                .unwrap_or(true)
        })
        .cloned();
    let current_index = question
        .as_ref()
        .and_then(|current| questions.iter().position(|question| question.id == current.id))
        .unwrap_or(questions.len());
    let answered = questions
        .iter()
        .filter(|question| {
            profile
                .answers
                .get(&question.id)
                .map(|answer| !answer.trim().is_empty())
                .unwrap_or(false)
        })
        .map(|question| question.id.clone())
        .collect::<Vec<_>>();
    let completed = question.is_none();
    RealEstateOnboardingState {
        required: !completed,
        status: if completed { "completed" } else { profile.status.as_str() }.to_string(),
        current_index,
        total: questions.len(),
        question,
        answered,
        derived_traits: profile.derived_traits.clone(),
        profile_hash: profile.profile_hash.clone(),
    }
}

#[allow(dead_code)]
fn derive_onboarding_traits(answers: &HashMap<String, String>) -> HashMap<String, String> {
    let mut traits = HashMap::new();
    let all_answers = answers
        .values()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    if let Some(identity) = answers.get("agency_identity").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        let (agency_name, city_hint) = extract_agency_identity_parts(identity);
        traits.insert("agency_search_name".to_string(), agency_name);
        traits.insert("agency_identity_brief".to_string(), truncate_for_trait(identity, 220));
        if let Some(city) = city_hint.or_else(|| extract_city_hint(identity)) {
            let region = infer_region_from_city(&city);
            traits.insert("agency_city".to_string(), city.clone());
            traits.insert("harvester_region".to_string(), region);
            traits.insert("harvester_zone_seed".to_string(), city.clone());
            traits.insert(
                "agency_address".to_string(),
                format!("{city} (adresse precise a confirmer)"),
            );
            traits
                .entry("priority_zones".to_string())
                .or_insert(city);
            traits
                .entry("priority_zone_count".to_string())
                .or_insert("1".to_string());
        }
    }
    if let Some(website) = answers.get("agency_website").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        traits.insert("owned_web_surface".to_string(), truncate_for_trait(website, 260));
        if let Some(url) = extract_url_hint(website) {
            traits.insert("agency_website".to_string(), url);
        }
        if let Some(domain) = extract_domain_hint(website) {
            traits.insert("agency_domain".to_string(), domain.clone());
            traits
                .entry("agency_website".to_string())
                .or_insert(format!("https://{domain}"));
        }
    }
    if let Some(people) = answers.get("agency_people").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        traits.insert("team_people_hint".to_string(), truncate_for_trait(people, 260));
        traits.insert("team_size_hint".to_string(), split_hint_count(people).to_string());
    }
    if let Some(zones) = answers.get("agency_zones").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        traits.insert("priority_zones".to_string(), truncate_for_trait(zones, 320));
        traits.insert("priority_zone_count".to_string(), split_hint_count(zones).to_string());
    }
    if let Some(stack) = answers.get("agency_stack").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        traits.insert("data_stack".to_string(), truncate_for_trait(stack, 320));
    }
    if let Some(priorities) = answers.get("agency_priorities").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        traits.insert("ninety_day_priorities".to_string(), truncate_for_trait(priorities, 360));
    }
    if let Some(email) = extract_email_hint(&all_answers) {
        traits.insert("agency_email".to_string(), email);
    }
    if let Some(phone) = extract_phone_hint(&all_answers) {
        traits.insert("agency_phone".to_string(), phone);
    }
    traits.insert(
        "inventory_scope".to_string(),
        "a_vendre,a_louer,vendu,loue".to_string(),
    );
    traits.insert(
        "inventory_harvest".to_string(),
        "active_via_site_and_portals".to_string(),
    );
    traits.insert(
        "contact_source".to_string(),
        "onboarding_plus_google_enrichment_queries".to_string(),
    );
    traits
}

#[allow(dead_code)]
fn onboarding_trigger_tools(question_id: &str, profile: &RealEstateOnboardingProfile) -> Vec<String> {
    let raw = match question_id {
        "agency_identity" => vec!["reputation", "dvf", "site-agence", "seloger", "leboncoin", "bienici"],
        "agency_website" => vec!["site-agence", "seloger", "leboncoin", "bienici", "annonces"],
        "agency_people" => vec!["recrutement", "matching-acheteurs", "mandats"],
        "agency_zones" => vec!["dvf", "cadastre", "dpe-ademe", "georisques", "urbanisme", "seloger"],
        "agency_stack" => vec!["mandats", "matching-acheteurs", "conformite", "drive", "gmail"],
        "agency_priorities" => vec!["estimation", "rapport-vendeur", "pilotage", "annonces", "mandats"],
        _ => Vec::new(),
    };
    let known_tools = default_registry()
        .collectors
        .iter()
        .flat_map(|collector| collector.tools.iter().cloned())
        .collect::<HashSet<_>>();
    let mut ids = Vec::new();
    for item in raw {
        let normalized = normalize_id(item);
        if known_tools.contains(&normalized) && !ids.iter().any(|id| id == &normalized) {
            ids.push(normalized);
        }
    }
    if profile.derived_traits.contains_key("agency_domain") && known_tools.contains("site-agence") && !ids.iter().any(|id| id == "site-agence") {
        ids.insert(0, "site-agence".to_string());
    }
    ids.truncate(6);
    ids
}

#[allow(dead_code)]
fn onboarding_suggestions_for_question(
    question: &RealEstateOnboardingQuestion,
    profile: &RealEstateOnboardingProfile,
) -> Vec<String> {
    let traits = &profile.derived_traits;
    match question.id.as_str() {
        "agency_website" => {
            let mut suggestions = traits
                .get("agency_search_name")
                .map(|name| vec![format!("Site officiel et Google Business Profile de {name}.")])
                .unwrap_or_else(|| question.examples.clone());
            if let Some(scope) = harvester_scope_hint(traits) {
                suggestions.push(scope);
            }
            if let Some(summary) = agency_profile_hint(traits) {
                suggestions.push(summary);
            }
            suggestions
        }
        "agency_people" => traits
            .get("agency_search_name")
            .map(|name| vec![format!("Équipe publique connue de {name}, à compléter avec les rôles internes.")])
            .unwrap_or_else(|| question.examples.clone()),
        "agency_zones" => traits
            .get("agency_identity_brief")
            .map(|brief| vec![format!("Zones citées ou déduites depuis: {brief}")])
            .unwrap_or_else(|| question.examples.clone()),
        "agency_stack" => vec![
            "CRM, portails, Google Workspace, exports CSV, téléphonie, chatbot, dossiers mandats.".to_string(),
        ],
        "agency_priorities" => vec![
            "Mandats exclusifs, estimation plus juste, relance vendeurs, matching acquéreurs, veille concurrence, recrutement.".to_string(),
        ],
        _ => question.examples.clone(),
    }
}

#[allow(dead_code)]
fn onboarding_enrichment_queries(
    question_id: &str,
    profile: &RealEstateOnboardingProfile,
) -> Vec<String> {
    let name = profile
        .derived_traits
        .get("agency_search_name")
        .cloned()
        .unwrap_or_else(|| "agence immobilière".to_string());
    let zones = profile
        .derived_traits
        .get("priority_zones")
        .cloned()
        .unwrap_or_else(|| "zone agence".to_string());
    let website = profile
        .derived_traits
        .get("agency_website")
        .cloned()
        .or_else(|| profile.derived_traits.get("agency_domain").map(|domain| format!("https://{domain}")))
        .unwrap_or_default();
    let website_host = profile
        .derived_traits
        .get("agency_domain")
        .cloned()
        .unwrap_or_else(|| website.clone());
    match question_id {
        "agency_identity" => vec![
            format!("{name} avis Google immobilier"),
            format!("{name} annonces immobilières"),
            format!("{name} recrutement immobilier"),
            format!("{name} {zones} immobilier a vendre"),
            format!("{name} {zones} immobilier a louer"),
            format!("{name} {zones} vendu loue"),
        ],
        "agency_website" => vec![
            format!("{name} site officiel"),
            format!("{name} Google Business Profile"),
            format!("site:{name} immobilier annonces"),
            format!("site:seloger.com \"{name}\""),
            format!("site:leboncoin.fr \"{name}\" immobilier"),
            format!("site:bienici.com \"{name}\""),
            if website.is_empty() {
                format!("{name} annonces agence immobiliere")
            } else {
                format!("site:{website_host} biens a vendre")
            },
        ],
        "agency_people" => vec![
            format!("{name} équipe immobilier"),
            format!("{name} LinkedIn négociateur immobilier"),
        ],
        "agency_zones" => vec![
            format!("DVF immobilier {zones}"),
            format!("urbanisme permis construire {zones}"),
            format!("actualité locale immobilier {zones}"),
        ],
        "agency_stack" => vec![
            format!("{name} portails SeLoger Bien'ici Leboncoin"),
            format!("{name} formulaires estimation"),
        ],
        "agency_priorities" => vec![
            format!("concurrents immobilier {zones}"),
            format!("marché immobilier {zones} 90 jours"),
        ],
        _ => Vec::new(),
    }
}

fn real_estate_onboarding_collection_plan(
    question_id: &str,
    profile: &RealEstateOnboardingProfile,
) -> Result<crate::collection_os::CollectionPlan, String> {
    let name = profile
        .derived_traits
        .get("agency_search_name")
        .or_else(|| profile.derived_traits.get("agency_display_name"))
        .cloned()
        .unwrap_or_default();
    let city = profile
        .derived_traits
        .get("agency_city")
        .or_else(|| profile.derived_traits.get("harvester_zone_seed"))
        .cloned()
        .unwrap_or_default();
    let objective = match question_id {
        "agency_identity" => format!("collect real estate agency profile for {name} {city}"),
        "agency_website" => format!("collect real estate agency website and public listings for {name}"),
        "agency_zones" => format!("collect real estate local market sources for {city}"),
        _ => format!("collect real estate onboarding evidence for {name} {city}"),
    };
    plan_collection(CollectionPlanRequest {
        objective,
        sector_hint: Some("real_estate".to_string()),
        max_latency_ms: Some(8_000),
        allow_paid_provider: Some(false),
    })
}

#[allow(dead_code)]
fn onboarding_profile_hash(profile: &RealEstateOnboardingProfile) -> String {
    let mut refs = vec![
        profile.schema_version.to_string(),
        profile.status.clone(),
        profile.started_at_ms.to_string(),
        profile.updated_at_ms.to_string(),
        profile.completed_at_ms.to_string(),
    ];
    let mut answers = profile.answers.iter().collect::<Vec<_>>();
    answers.sort_by(|a, b| a.0.cmp(b.0));
    refs.extend(answers.into_iter().map(|(key, value)| format!("{key}={value}")));
    let mut traits = profile.derived_traits.iter().collect::<Vec<_>>();
    traits.sort_by(|a, b| a.0.cmp(b.0));
    refs.extend(traits.into_iter().map(|(key, value)| format!("{key}={value}")));
    let ref_slices = refs.iter().map(|value| value.as_str()).collect::<Vec<_>>();
    hash_parts("real_estate_agency_onboarding_profile:v1", &ref_slices)
}

fn run_onboarding_web_research(
    base: &Path,
    question_id: &str,
    profile: &RealEstateOnboardingProfile,
    traits: &mut HashMap<String, String>,
) -> Result<Vec<RealEstateOnboardingWebPageFinding>, String> {
    if !onboarding_web_scan_enabled() || !onboarding_web_scan_question(question_id) {
        return Ok(Vec::new());
    }
    let search_name = traits
        .get("agency_search_name")
        .cloned()
        .unwrap_or_default();
    if search_name.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut queries = onboarding_enrichment_queries(question_id, profile);
    let city = traits
        .get("agency_city")
        .cloned()
        .or_else(|| traits.get("harvester_zone_seed").cloned())
        .unwrap_or_default();
    if !city.trim().is_empty() {
        queries.push(format!("{search_name} {city} immobilier"));
        queries.push(format!("{search_name} {city} avis"));
    }
    queries.push(format!("site:seloger.com \"{search_name}\""));
    queries.push(format!("site:bienici.com \"{search_name}\""));
    queries.push(format!("site:leboncoin.fr \"{search_name}\" immobilier"));
    let mut dedup_queries = Vec::new();
    for query in queries {
        let normalized = query.trim().to_string();
        if normalized.is_empty() || dedup_queries.iter().any(|entry: &String| entry == &normalized) {
            continue;
        }
        dedup_queries.push(normalized);
        if dedup_queries.len() >= 5 {
            break;
        }
    }

    let mut query_links: Vec<(String, String)> = Vec::new();
    for query in &dedup_queries {
        match google_search_links(query) {
            Ok(urls) => {
                for url in urls.into_iter().take(8) {
                    query_links.push((query.clone(), url));
                }
            }
            Err(err) => {
                record_onboarding_block_or_error(base, question_id, Some(query), None, &err);
                let _ = append_json_line(
                    &data_path(base, ONBOARDING_EVENTS_FILE),
                    &json!({
                        "type": "onboarding_web_search_error",
                        "questionId": question_id,
                        "query": query,
                        "error": truncate_for_trait(&err, 160),
                        "tsMs": now_ms()
                    }),
                );
            }
        }
    }

    let mut seen_urls = HashSet::new();
    let mut findings = Vec::new();
    for (query, url) in query_links {
        if !seen_urls.insert(url.clone()) {
            continue;
        }
        if findings.len() >= 10 {
            break;
        }
        match fetch_web_page_finding(&query, &url) {
            Ok(finding) => findings.push(finding),
            Err(err) => record_onboarding_block_or_error(base, question_id, Some(&query), Some(&url), &err),
        }
    }
    if findings.is_empty() {
        return Ok(Vec::new());
    }

    let mut domains = Vec::new();
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut listing_signal_hits = 0usize;
    for finding in &findings {
        if !domains.iter().any(|domain| domain == &finding.source_domain) {
            domains.push(finding.source_domain.clone());
        }
        for email in &finding.emails {
            if !emails.iter().any(|item| item == email) {
                emails.push(email.clone());
            }
        }
        for phone in &finding.phones {
            if !phones.iter().any(|item| item == phone) {
                phones.push(phone.clone());
            }
        }
        listing_signal_hits = listing_signal_hits.saturating_add(finding.listing_signals.len());
    }
    if !domains.is_empty() {
        traits.insert("agency_web_domains".to_string(), domains.join(","));
    }
    traits.insert(
        "agency_web_pages_scanned".to_string(),
        findings.len().to_string(),
    );
    traits.insert(
        "agency_listing_signal_hits".to_string(),
        listing_signal_hits.to_string(),
    );
    if let Some(email) = emails.first() {
        traits
            .entry("agency_email".to_string())
            .or_insert_with(|| email.clone());
    }
    if let Some(phone) = phones.first() {
        traits
            .entry("agency_phone".to_string())
            .or_insert_with(|| phone.clone());
    }
    if traits.get("agency_website").map(|it| it.trim().is_empty()).unwrap_or(true) {
        if let Some(site) = findings
            .iter()
            .find(|finding| !finding.url.contains("seloger") && !finding.url.contains("leboncoin") && !finding.url.contains("bienici"))
            .map(|finding| finding.url.clone())
        {
            traits.insert("agency_website".to_string(), site.clone());
            if let Some(domain) = extract_domain_hint(&site) {
                traits.insert("agency_domain".to_string(), domain);
            }
        }
    }
    write_json_pretty(
        &data_path(base, ONBOARDING_WEB_FINDINGS_FILE),
        &findings,
        "agency onboarding web findings",
    )?;
    Ok(findings)
}

fn onboarding_web_scan_enabled() -> bool {
    if cfg!(test) {
        return false;
    }
    std::env::var("FORGE_ONBOARDING_WEB_SCAN")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| !matches!(value.as_str(), "0" | "false" | "off"))
        .unwrap_or(true)
}

fn onboarding_web_scan_question(question_id: &str) -> bool {
    matches!(
        question_id,
        "agency_identity" | "agency_website" | "agency_zones" | "agency_priorities"
    )
}

fn google_search_links(query: &str) -> Result<Vec<String>, String> {
    let encoded = percent_encode_query(query);
    let url = format!("https://www.google.com/search?hl=fr&num=10&q={encoded}");
    let fetch = http_get_text(&url, 280_000)?;
    ensure_fetch_not_blocked(&fetch)?;
    Ok(extract_google_search_links(&fetch.body))
}

fn fetch_web_page_finding(
    query: &str,
    url: &str,
) -> Result<RealEstateOnboardingWebPageFinding, String> {
    let fetch = http_get_text(url, 420_000)?;
    ensure_fetch_not_blocked(&fetch)?;
    let html = fetch.body;
    let title = extract_html_title(&html);
    let snippet = extract_meta_description(&html)
        .or_else(|| extract_html_text_excerpt(&html))
        .unwrap_or_default();
    let lower = html.to_ascii_lowercase();
    let mut listing_signals = Vec::new();
    for signal in ["a vendre", "a louer", "vendu", "loué", "loue", "mandat", "estimation"] {
        if lower.contains(signal) && !listing_signals.iter().any(|entry: &String| entry == signal) {
            listing_signals.push(signal.to_string());
        }
    }
    let emails = extract_emails(&html).into_iter().take(6).collect::<Vec<_>>();
    let phones = extract_phone_numbers(&html)
        .into_iter()
        .take(6)
        .collect::<Vec<_>>();
    let source_domain = extract_host(url).unwrap_or_else(|| "unknown".to_string());
    let fetched_at_ms = now_ms();
    let evidence_hash = hash_parts(
        "real_estate_onboarding_web_page:v1",
        &[
            query,
            url,
            &title,
            &snippet,
            &listing_signals.join(","),
            &emails.join(","),
            &phones.join(","),
            &fetched_at_ms.to_string(),
        ],
    );
    Ok(RealEstateOnboardingWebPageFinding {
        query: query.to_string(),
        url: url.to_string(),
        source_domain,
        title: truncate_for_trait(&title, 180),
        snippet: truncate_for_trait(&snippet, 260),
        listing_signals,
        emails,
        phones,
        fetched_at_ms,
        evidence_hash,
    })
}

fn http_get_text(url: &str, max_bytes: usize) -> Result<HttpTextFetch, String> {
    tauri::async_runtime::block_on(async move {
        let started = std::time::Instant::now();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ForgeHarvester/1.0")
            .build()
            .map_err(|err| format!("build web client: {err}"))?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|err| format!("web request failed: {err}"))?;
        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| format!("read web body: {err}"))?;
        let limited = if bytes.len() > max_bytes {
            &bytes[..max_bytes]
        } else {
            bytes.as_ref()
        };
        Ok(HttpTextFetch {
            url: url.to_string(),
            status,
            body: String::from_utf8_lossy(limited).to_string(),
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    })
}

fn ensure_fetch_not_blocked(fetch: &HttpTextFetch) -> Result<(), String> {
    let proof = crate::collection_os::classify_block_signal(crate::collection_os::CollectionBlockSignalInput {
        source_url: fetch.url.clone(),
        http_status: Some(fetch.status),
        title: extract_html_title(&fetch.body).into(),
        body_preview: Some(truncate_for_trait(&clean_html_text(&fetch.body), 600)),
        node_count: None,
        elapsed_ms: Some(fetch.elapsed_ms),
    });
    if proof.status != "clear" {
        return Err(format!(
            "collection_block:{}:{}:{}",
            proof.status, proof.severity, proof.proof_hash
        ));
    }
    if !(200..=299).contains(&fetch.status) {
        return Err(format!("web request status {}", fetch.status));
    }
    Ok(())
}

fn record_onboarding_block_or_error(
    base: &Path,
    question_id: &str,
    query: Option<&str>,
    url: Option<&str>,
    err: &str,
) {
    if let Some((status, severity, proof_hash)) = parse_collection_block_error(err) {
        let _ = append_json_line(
            &data_path(base, ONBOARDING_EVENTS_FILE),
            &json!({
                "type": "onboarding_web_blocked",
                "questionId": question_id,
                "query": query,
                "url": url,
                "status": status,
                "severity": severity,
                "proofHash": proof_hash,
                "tsMs": now_ms()
            }),
        );
    }
}

fn parse_collection_block_error(err: &str) -> Option<(&str, &str, &str)> {
    let rest = err.strip_prefix("collection_block:")?;
    let mut parts = rest.splitn(3, ':');
    let status = parts.next()?;
    let severity = parts.next()?;
    let proof_hash = parts.next()?;
    Some((status, severity, proof_hash))
}

fn extract_google_search_links(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = html[index..].find("href=\"/url?q=") {
        let start = index + offset + "href=\"/url?q=".len();
        let rest = &html[start..];
        let Some(end) = rest.find('"') else {
            break;
        };
        let raw = &rest[..end];
        let candidate = raw.split('&').next().unwrap_or_default();
        let decoded = percent_decode(candidate).replace("&amp;", "&");
        if is_usable_web_url(&decoded) && !urls.iter().any(|url| url == &decoded) {
            urls.push(decoded);
        }
        index = start + end;
        if urls.len() >= 24 {
            break;
        }
    }
    if urls.is_empty() {
        let mut cursor = 0usize;
        while let Some(offset) = html[cursor..].find("href=\"https://") {
            let start = cursor + offset + "href=\"".len();
            let rest = &html[start..];
            let Some(end) = rest.find('"') else {
                break;
            };
            let candidate = &rest[..end];
            if is_usable_web_url(candidate) && !urls.iter().any(|url| url == candidate) {
                urls.push(candidate.to_string());
            }
            cursor = start + end;
            if urls.len() >= 24 {
                break;
            }
        }
    }
    urls
}

fn is_usable_web_url(url: &str) -> bool {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return false;
    }
    let Some(host) = extract_host(url) else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    if host.contains("google.") || host.contains("gstatic.com") || host.contains("googleusercontent.com") {
        return false;
    }
    true
}

fn extract_host(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = without_scheme.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn percent_encode_query(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*byte as char),
            b' ' => out.push('+'),
            other => {
                out.push('%');
                out.push_str(&format!("{other:02X}"));
            }
        }
    }
    out
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hi = (bytes[index + 1] as char).to_digit(16);
                let lo = (bytes[index + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push(((hi << 4) as u8) | lo as u8);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn extract_html_title(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(title_start) = lower.find("<title") else {
        return String::new();
    };
    let Some(gt_offset) = lower[title_start..].find('>') else {
        return String::new();
    };
    let content_start = title_start + gt_offset + 1;
    let Some(end_offset) = lower[content_start..].find("</title>") else {
        return String::new();
    };
    clean_html_text(&html[content_start..content_start + end_offset])
}

fn extract_meta_description(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut index = 0usize;
    while let Some(offset) = lower[index..].find("<meta") {
        let start = index + offset;
        let Some(end_offset) = lower[start..].find('>') else {
            break;
        };
        let tag = &html[start..start + end_offset + 1];
        let tag_lower = tag.to_ascii_lowercase();
        if tag_lower.contains("name=\"description\"") || tag_lower.contains("property=\"og:description\"") {
            if let Some(content) = extract_meta_content_attr(tag) {
                return Some(content);
            }
        }
        index = start + end_offset + 1;
    }
    None
}

fn extract_meta_content_attr(tag: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = "content=\"";
    let start = lower.find(needle)? + needle.len();
    let tail = &tag[start..];
    let end = tail.find('"')?;
    let raw = &tail[..end];
    let cleaned = clean_html_text(raw);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn extract_html_text_excerpt(html: &str) -> Option<String> {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(ch),
            _ => {}
        }
        if text.len() >= 2400 {
            break;
        }
    }
    let cleaned = clean_html_text(&text);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn clean_html_text(value: &str) -> String {
    let normalized = value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn extract_emails(text: &str) -> Vec<String> {
    let mut emails = Vec::new();
    for token in text.split_whitespace() {
        let candidate = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '@' && ch != '.' && ch != '-' && ch != '_');
        if candidate.len() < 6 || !candidate.contains('@') {
            continue;
        }
        let mut parts = candidate.split('@');
        let Some(local) = parts.next() else { continue };
        let Some(domain) = parts.next() else { continue };
        if local.is_empty() || domain.is_empty() || !domain.contains('.') {
            continue;
        }
        let lowered = candidate.to_ascii_lowercase();
        if !emails.iter().any(|item| item == &lowered) {
            emails.push(lowered);
        }
    }
    emails
}

fn extract_phone_numbers(text: &str) -> Vec<String> {
    let mut phones = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if ch == '+' && current.is_empty() {
            current.push(ch);
            continue;
        }
        if matches!(ch, ' ' | '.' | '-' | '(' | ')' | '\u{a0}') && !current.is_empty() {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Some(normalized) = normalize_phone_candidate(&current) {
                if !phones.iter().any(|item| item == &normalized) {
                    phones.push(normalized);
                }
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Some(normalized) = normalize_phone_candidate(&current) {
            if !phones.iter().any(|item| item == &normalized) {
                phones.push(normalized);
            }
        }
    }
    phones
}

fn normalize_phone_candidate(candidate: &str) -> Option<String> {
    let digits = candidate
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.len() < 10 || digits.len() > 15 {
        return None;
    }
    Some(digits)
}

#[allow(dead_code)]
fn first_clause(value: &str) -> String {
    value
        .split(|ch| matches!(ch, ',' | ';' | '\n' | '\r'))
        .next()
        .unwrap_or(value)
        .trim()
        .chars()
        .take(96)
        .collect()
}

#[allow(dead_code)]
fn truncate_for_trait(value: &str, max_chars: usize) -> String {
    let clean = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_chars {
        return clean;
    }
    let mut out = clean.chars().take(max_chars.saturating_sub(3)).collect::<String>();
    out.push_str("...");
    out
}

#[allow(dead_code)]
fn split_hint_count(value: &str) -> usize {
    value
        .split(|ch| matches!(ch, ',' | ';' | '\n' | '\r' | '/'))
        .filter(|part| !part.trim().is_empty())
        .count()
        .max(1)
}

#[allow(dead_code)]
fn extract_domain_hint(value: &str) -> Option<String> {
    let token = value
        .split_whitespace()
        .find(|part| part.contains('.') || part.starts_with("http://") || part.starts_with("https://"))?
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '(' | '"' | '\''));
    let without_scheme = token
        .strip_prefix("https://")
        .or_else(|| token.strip_prefix("http://"))
        .unwrap_or(token);
    let domain = without_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches("www.");
    if domain.contains('.') {
        Some(domain.to_ascii_lowercase())
    } else {
        None
    }
}

fn extract_city_hint(value: &str) -> Option<String> {
    let parts = value
        .split(|ch| matches!(ch, ',' | ';' | '\n' | '\r'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    Some(
        parts[1]
            .chars()
            .take(80)
            .collect::<String>()
            .trim()
            .to_string(),
    )
}

fn extract_agency_identity_parts(value: &str) -> (String, Option<String>) {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let lowered = compact.to_lowercase();
    for separator in [" a ", " based in ", " based at ", " located in ", " situe a ", " base a "] {
        if let Some(byte_index) = lowered.find(separator) {
            let agency = compact[..byte_index].trim();
            let city = compact[byte_index + separator.len()..].trim();
            let agency_name = agency.trim().trim_matches(|ch: char| matches!(ch, ',' | ';' | '.' | ':')).to_string();
            let city_name = city.trim().trim_matches(|ch: char| matches!(ch, ',' | ';' | '.' | ':')).to_string();
            if !agency_name.is_empty() && !city_name.is_empty() {
                return (agency_name, Some(city_name));
            }
        }
    }
    (first_clause(&compact), None)
}

fn extract_city_from_formatted_address(value: &str) -> Option<String> {
    let city_segment = value
        .split(',')
        .map(str::trim)
        .find(|part| {
            part.split_whitespace().any(|token| token.len() == 5 && token.chars().all(|ch| ch.is_ascii_digit()))
                && part.chars().any(|ch| ch.is_alphabetic())
        })?;
    let clean = city_segment
        .split_whitespace()
        .skip_while(|part| part.chars().all(|ch| ch.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn extract_postal_code_from_formatted_address(value: &str) -> Option<String> {
    value
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';'))
        .find(|part| part.len() == 5 && part.chars().all(|ch| ch.is_ascii_digit()))
        .map(|part| part.to_string())
}

fn infer_region_from_city(city: &str) -> String {
    match normalize_city_key(city).as_str() {
        "paris" | "boulogne billancourt" | "saint denis" | "versailles" => "Ile-de-France".to_string(),
        "lyon" | "villeurbanne" | "grenoble" | "annecy" | "saint etienne" => "Auvergne-Rhone-Alpes".to_string(),
        "lille" | "roubaix" | "tourcoing" | "dunkerque" | "amiens" | "marcq en baroeul" => "Hauts-de-France".to_string(),
        "marseille" | "nice" | "toulon" | "avignon" | "aix en provence" => "Provence-Alpes-Cote d'Azur".to_string(),
        "toulouse" | "montpellier" | "nimes" | "perpignan" => "Occitanie".to_string(),
        "bordeaux" | "limoges" | "poitiers" | "pau" | "la rochelle" => "Nouvelle-Aquitaine".to_string(),
        "nantes" | "angers" | "le mans" | "saint nazaire" => "Pays de la Loire".to_string(),
        "strasbourg" | "reims" | "metz" | "nancy" | "mulhouse" => "Grand Est".to_string(),
        "rennes" | "brest" | "quimper" | "lorient" => "Bretagne".to_string(),
        "caen" | "rouen" | "le havre" | "cherbourg en cotentin" => "Normandie".to_string(),
        _ => "region_non_precisee".to_string(),
    }
}

fn harvester_scope_hint(traits: &HashMap<String, String>) -> Option<String> {
    let city = traits.get("agency_city")?;
    let region = traits
        .get("harvester_region")
        .map(String::as_str)
        .unwrap_or("region_non_precisee");
    Some(format!(
        "Scope harvester provisoire: region {region}, zone prioritaire {city}."
    ))
}

fn agency_profile_hint(traits: &HashMap<String, String>) -> Option<String> {
    let name = traits.get("agency_search_name").cloned().unwrap_or_default();
    let city = traits.get("agency_city").cloned().unwrap_or_default();
    let website = traits
        .get("agency_website")
        .cloned()
        .or_else(|| traits.get("agency_domain").map(|domain| format!("https://{domain}")))
        .unwrap_or_default();
    let phone = traits.get("agency_phone").cloned().unwrap_or_else(|| "a confirmer".to_string());
    let email = traits.get("agency_email").cloned().unwrap_or_else(|| "a confirmer".to_string());
    if name.is_empty() {
        return None;
    }
    let mut parts = vec![format!("Profil agence: {name}")];
    if !city.is_empty() {
        parts.push(format!("ville {city}"));
    }
    if !website.is_empty() {
        parts.push(format!("site {website}"));
    }
    parts.push(format!("tel {phone}"));
    parts.push(format!("email {email}"));
    Some(parts.join(" | "))
}

fn enrich_onboarding_traits_with_google_places(traits: &mut HashMap<String, String>) {
    let name = traits
        .get("agency_search_name")
        .cloned()
        .or_else(|| traits.get("agency_display_name").cloned())
        .unwrap_or_default();
    if name.trim().is_empty() {
        traits
            .entry("google_places_status".to_string())
            .or_insert("skipped_missing_agency_name".to_string());
        return;
    }
    let city = traits
        .get("agency_city")
        .cloned()
        .or_else(|| traits.get("harvester_zone_seed").cloned())
        .unwrap_or_default();
    let query = if city.trim().is_empty() {
        format!("{name} agence immobiliere")
    } else {
        format!("{name} {city} agence immobiliere")
    };
    if let Some(endpoint) = remote_agency_resolver_url() {
        match remote_agency_resolve_contact_with_retry(&endpoint, remote_agency_resolver_token(), &name, &city, &query) {
            Ok(Some(contact)) => {
                apply_resolved_agency_contact_traits(
                    traits,
                    contact,
                    "forge_backend_resolver",
                    "ok",
                );
                return;
            }
            Ok(None) => {
                traits.insert(
                    "agency_resolution_status".to_string(),
                    "remote_no_match".to_string(),
                );
            }
            Err(err) => {
                traits.insert(
                    "agency_resolution_status".to_string(),
                    format!("remote_error:{}", truncate_for_trait(&err, 120)),
                );
            }
        }
    }

    let Some(api_key) = google_places_api_key() else {
        traits.insert(
            "google_places_status".to_string(),
            traits
                .get("agency_resolution_status")
                .cloned()
                .unwrap_or_else(|| "missing_api_key".to_string()),
        );
        return;
    };
    match google_places_text_search_contact(&api_key, &query) {
        Ok(Some(contact)) => {
            apply_resolved_agency_contact_traits(
                traits,
                contact,
                "google_places_text_search",
                "ok",
            );
        }
        Ok(None) => {
            traits.insert("google_places_status".to_string(), "no_match".to_string());
            traits.insert("agency_resolution_status".to_string(), "no_match".to_string());
        }
        Err(err) => {
            traits.insert(
                "google_places_status".to_string(),
                format!("error:{}", truncate_for_trait(&err, 120)),
            );
            traits.insert(
                "agency_resolution_status".to_string(),
                format!("error:{}", truncate_for_trait(&err, 120)),
            );
        }
    }
}

fn remote_agency_resolve_contact_with_retry(
    endpoint: &str,
    token: Option<String>,
    agency_name: &str,
    city: &str,
    query: &str,
) -> Result<Option<GooglePlaceContact>, String> {
    match remote_agency_resolve_contact(endpoint, token.clone(), agency_name, city, query) {
        Ok(contact) => Ok(contact),
        Err(first_err) => {
            let should_retry = first_err.contains("timed out")
                || first_err.contains("deadline")
                || first_err.contains("operation timed out")
                || first_err.contains("ConnectError")
                || first_err.contains("connection")
                || first_err.contains("502")
                || first_err.contains("503")
                || first_err.contains("504");
            if !should_retry {
                return Err(first_err);
            }
            let _ = warm_remote_agency_resolver();
            thread::sleep(Duration::from_millis(1400));
            remote_agency_resolve_contact(endpoint, token, agency_name, city, query).map_err(|second_err| {
                format!("{first_err}; retry={second_err}")
            })
        }
    }
}

fn apply_resolved_agency_contact_traits(
    traits: &mut HashMap<String, String>,
    contact: GooglePlaceContact,
    source: &str,
    _status: &str,
) {
    let contact_complete = contact.formatted_address.as_ref().map(|it| !it.trim().is_empty()).unwrap_or(false)
        && contact.national_phone.as_ref().map(|it| !it.trim().is_empty()).unwrap_or(false)
        && contact.website_uri.as_ref().map(|it| !it.trim().is_empty()).unwrap_or(false);
    if let Some(address) = contact.formatted_address.filter(|it| !it.trim().is_empty()) {
        let city_from_address = extract_city_from_formatted_address(&address);
        let postal_code = extract_postal_code_from_formatted_address(&address);
        traits.insert("agency_address".to_string(), address);
        if let Some(city) = city_from_address {
            let region = infer_region_from_city(&city);
            traits.insert("agency_city".to_string(), city.clone());
            traits.insert("harvester_region".to_string(), region);
            traits.insert("harvester_zone_seed".to_string(), city.clone());
            traits.insert("priority_zones".to_string(), city);
            traits.insert("priority_zone_count".to_string(), "1".to_string());
        }
        if let Some(postal_code) = postal_code {
            traits.insert("agency_postal_code".to_string(), postal_code);
        }
    }
    if let Some(phone) = contact.national_phone.filter(|it| !it.trim().is_empty()) {
        traits.insert("agency_phone".to_string(), phone);
    }
    if let Some(website) = contact.website_uri.filter(|it| !it.trim().is_empty()) {
        traits.insert("agency_website".to_string(), website.clone());
        if let Some(domain) = extract_domain_hint(&website) {
            traits
                .entry("agency_domain".to_string())
                .or_insert(domain);
        }
    }
    if let Some(label) = contact.display_name.filter(|it| !it.trim().is_empty()) {
        traits
            .entry("agency_display_name".to_string())
            .or_insert(label);
    }
    if let Some(maps_uri) = contact.google_maps_uri.filter(|it| !it.trim().is_empty()) {
        traits.insert("agency_google_maps_uri".to_string(), maps_uri);
    }
    if let Some(lat) = contact.lat {
        traits.insert("agency_lat".to_string(), lat.to_string());
    }
    if let Some(lng) = contact.lng {
        traits.insert("agency_lng".to_string(), lng.to_string());
    }
    traits.insert("contact_source".to_string(), source.to_string());
    traits.insert("agency_resolution_source".to_string(), source.to_string());
    let status = if contact_complete { "ok" } else { "partial_missing_contact" };
    traits.insert("agency_resolution_status".to_string(), status.to_string());
    traits.insert("google_places_status".to_string(), status.to_string());
}

fn remote_agency_resolver_url() -> Option<String> {
    std::env::var("FORGE_REAL_ESTATE_RESOLVER_URL")
        .ok()
        .or_else(|| {
            std::env::var("FORGE_REAL_ESTATE_BACKEND_URL")
                .ok()
                .map(|base| {
                    format!(
                        "{}/api/agency/resolve",
                        base.trim().trim_end_matches('/')
                    )
                })
        })
        .or_else(|| {
            let path = real_estate_backend_env_path()?;
            read_dotenv_value(&path, "FORGE_REAL_ESTATE_RESOLVER_URL").or_else(|| {
                read_dotenv_value(&path, "FORGE_REAL_ESTATE_BACKEND_URL").map(|base| {
                    format!(
                        "{}/api/agency/resolve",
                        base.trim().trim_end_matches('/')
                    )
                })
            })
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn remote_agency_resolver_health_url() -> Option<String> {
    std::env::var("FORGE_REAL_ESTATE_BACKEND_URL")
        .ok()
        .or_else(|| {
            let path = real_estate_backend_env_path()?;
            read_dotenv_value(&path, "FORGE_REAL_ESTATE_BACKEND_URL")
        })
        .map(|base| format!("{}/health", base.trim().trim_end_matches('/')))
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let resolver = remote_agency_resolver_url()?;
            if let Some((base, _)) = resolver.split_once("/api/agency/resolve") {
                let health = format!("{}/health", base.trim().trim_end_matches('/'));
                if !health.trim().is_empty() {
                    return Some(health);
                }
            }
            None
        })
}

fn remote_agency_resolver_token() -> Option<String> {
    ["FORGE_REAL_ESTATE_RESOLVER_TOKEN", "FORGE_REAL_ESTATE_BACKEND_TOKEN"]
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            let path = real_estate_backend_env_path()?;
            read_dotenv_value(&path, "FORGE_REAL_ESTATE_RESOLVER_TOKEN")
                .or_else(|| read_dotenv_value(&path, "FORGE_REAL_ESTATE_BACKEND_TOKEN"))
        })
}

#[allow(dead_code)]
pub fn warm_remote_agency_resolver() -> Result<bool, String> {
    let Some(health_url) = remote_agency_resolver_health_url() else {
        return Ok(false);
    };
    tauri::async_runtime::block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .build()
            .map_err(|err| format!("remote agency resolver warm client: {err}"))?;
        let response = client
            .get(&health_url)
            .header("X-Forge-Client", "forge-ui/real-estate-warmup")
            .send()
            .await
            .map_err(|err| format!("remote agency resolver warm request: {err}"))?;
        Ok(response.status().is_success())
    })
}

fn real_estate_backend_env_path() -> Option<PathBuf> {
    preferred_user_env_path(&[".forge", "real-estate.env"])
}

fn remote_agency_resolve_contact(
    endpoint: &str,
    token: Option<String>,
    agency_name: &str,
    city: &str,
    query: &str,
) -> Result<Option<GooglePlaceContact>, String> {
    if endpoint.trim().is_empty() || agency_name.trim().is_empty() {
        return Ok(None);
    }
    tauri::async_runtime::block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|err| format!("remote agency resolver client: {err}"))?;
        let mut request = client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .header("X-Forge-Client", "forge-ui/real-estate-onboarding")
            .json(&json!({
                "agencyName": agency_name,
                "city": city,
                "query": query,
                "countryCode": "FR",
                "surface": "forge-ui",
                "scope": "real-estate-onboarding"
            }));
        if let Some(token) = token.as_deref().filter(|it| !it.trim().is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|err| format!("remote agency resolver request: {err}"))?;
        if !response.status().is_success() {
            let code = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "remote agency resolver status {code}: {}",
                truncate_for_trait(&body, 160)
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|err| format!("remote agency resolver parse: {err}"))?;
        let candidate = payload
            .get("agency")
            .cloned()
            .unwrap_or_else(|| payload.clone());
        parse_remote_agency_contact(&candidate)
    })
}

fn parse_remote_agency_contact(payload: &serde_json::Value) -> Result<Option<GooglePlaceContact>, String> {
    if payload.is_null() {
        return Ok(None);
    }
    let display_name = payload
        .get("displayName")
        .or_else(|| payload.get("display_name"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let formatted_address = payload
        .get("formattedAddress")
        .or_else(|| payload.get("formatted_address"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let national_phone = payload
        .get("nationalPhoneNumber")
        .or_else(|| payload.get("national_phone"))
        .or_else(|| payload.get("phone"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let website_uri = payload
        .get("websiteUri")
        .or_else(|| payload.get("website_uri"))
        .or_else(|| payload.get("website"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let google_maps_uri = payload
        .get("googleMapsUri")
        .or_else(|| payload.get("google_maps_uri"))
        .or_else(|| payload.get("mapsUri"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let lat = payload
        .get("lat")
        .or_else(|| payload.get("latitude"))
        .and_then(|item| item.as_f64())
        .or_else(|| {
            payload
                .get("location")
                .and_then(|item| item.get("lat").or_else(|| item.get("latitude")))
                .and_then(|item| item.as_f64())
        });
    let lng = payload
        .get("lng")
        .or_else(|| payload.get("longitude"))
        .and_then(|item| item.as_f64())
        .or_else(|| {
            payload
                .get("location")
                .and_then(|item| item.get("lng").or_else(|| item.get("longitude")))
                .and_then(|item| item.as_f64())
        });
    let contact = GooglePlaceContact {
        display_name,
        formatted_address,
        national_phone,
        website_uri,
        google_maps_uri,
        lat,
        lng,
    };
    let has_signal = contact.display_name.is_some()
        || contact.formatted_address.is_some()
        || contact.website_uri.is_some()
        || contact.google_maps_uri.is_some()
        || contact.lat.is_some()
        || contact.lng.is_some();
    if !has_signal {
        return Ok(None);
    }
    Ok(Some(contact))
}

fn google_places_api_key() -> Option<String> {
    [
        "FORGE_GOOGLE_PLACES_API_KEY",
        "GOOGLE_PLACES_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
    ]
    .iter()
    .find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
    .or_else(google_places_saved_api_key)
}

fn google_places_saved_api_key() -> Option<String> {
    let path = google_gemini_env_path()?;
    read_dotenv_value(&path, "GEMINI_API_KEY")
        .or_else(|| read_dotenv_value(&path, "GOOGLE_API_KEY"))
}

fn google_gemini_env_path() -> Option<PathBuf> {
    preferred_user_env_path(&[".gemini", ".env"])
}

fn preferred_user_env_path(parts: &[&str]) -> Option<PathBuf> {
    let candidates = preferred_user_home_candidates(
        std::env::var_os("USERPROFILE"),
        std::env::var_os("HOMEDRIVE"),
        std::env::var_os("HOMEPATH"),
        std::env::var_os("HOME"),
    );
    let mut fallback = None;
    for home in candidates {
        let mut candidate = home;
        for part in parts {
            candidate = candidate.join(part);
        }
        if fallback.is_none() {
            fallback = Some(candidate.clone());
        }
        if candidate.exists() {
            return Some(candidate);
        }
    }
    fallback
}

fn preferred_user_home_candidates(
    userprofile: Option<std::ffi::OsString>,
    homedrive: Option<std::ffi::OsString>,
    homepath: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut push_unique = |path: PathBuf| {
        if !candidates.iter().any(|existing| existing == &path) {
            candidates.push(path);
        }
    };
    if cfg!(windows) {
        if let Some(path) = userprofile.map(PathBuf::from) {
            push_unique(path);
        }
        if let (Some(drive), Some(path)) = (homedrive, homepath) {
            let mut combined = PathBuf::from(drive);
            combined.push(path);
            push_unique(combined);
        }
        if let Some(path) = home.map(PathBuf::from) {
            push_unique(path);
        }
    } else {
        if let Some(path) = home.map(PathBuf::from) {
            push_unique(path);
        }
        if let Some(path) = userprofile.map(PathBuf::from) {
            push_unique(path);
        }
    }
    candidates
}

fn read_dotenv_value(path: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let (name, value) = line.split_once('=')?;
            if name.trim() != key {
                return None;
            }
            let value = value.trim().trim_matches('"').trim_matches('\'').trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
}

#[derive(Default, Clone)]
struct GooglePlaceContact {
    display_name: Option<String>,
    formatted_address: Option<String>,
    national_phone: Option<String>,
    website_uri: Option<String>,
    google_maps_uri: Option<String>,
    lat: Option<f64>,
    lng: Option<f64>,
}

fn parse_google_place_contact_value(place: &serde_json::Value) -> GooglePlaceContact {
    let display_name = place
        .get("displayName")
        .and_then(|item| item.get("text"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let formatted_address = place
        .get("formattedAddress")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let national_phone = place
        .get("nationalPhoneNumber")
        .or_else(|| place.get("internationalPhoneNumber"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let website_uri = place
        .get("websiteUri")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let google_maps_uri = place
        .get("googleMapsUri")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let lat = place
        .get("location")
        .and_then(|item| item.get("latitude"))
        .and_then(|item| item.as_f64());
    let lng = place
        .get("location")
        .and_then(|item| item.get("longitude"))
        .and_then(|item| item.as_f64());
    GooglePlaceContact {
        display_name,
        formatted_address,
        national_phone,
        website_uri,
        google_maps_uri,
        lat,
        lng,
    }
}

fn google_place_contact_completeness(contact: &GooglePlaceContact) -> usize {
    let mut score = 0usize;
    if contact
        .formatted_address
        .as_ref()
        .map(|it| !it.trim().is_empty())
        .unwrap_or(false)
    {
        score += 1;
    }
    if contact
        .national_phone
        .as_ref()
        .map(|it| !it.trim().is_empty())
        .unwrap_or(false)
    {
        score += 1;
    }
    if contact
        .website_uri
        .as_ref()
        .map(|it| !it.trim().is_empty())
        .unwrap_or(false)
    {
        score += 1;
    }
    if contact.lat.is_some() && contact.lng.is_some() {
        score += 1;
    }
    score
}

fn merge_google_place_contacts(base: GooglePlaceContact, details: GooglePlaceContact) -> GooglePlaceContact {
    GooglePlaceContact {
        display_name: details.display_name.or(base.display_name),
        formatted_address: details.formatted_address.or(base.formatted_address),
        national_phone: details.national_phone.or(base.national_phone),
        website_uri: details.website_uri.or(base.website_uri),
        google_maps_uri: details.google_maps_uri.or(base.google_maps_uri),
        lat: details.lat.or(base.lat),
        lng: details.lng.or(base.lng),
    }
}

fn google_places_text_search_contact(
    api_key: &str,
    text_query: &str,
) -> Result<Option<GooglePlaceContact>, String> {
    if api_key.trim().is_empty() || text_query.trim().is_empty() {
        return Ok(None);
    }
    tauri::async_runtime::block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|err| format!("google places client: {err}"))?;
        let response = client
            .post("https://places.googleapis.com/v1/places:searchText")
            .header("Content-Type", "application/json")
            .header("X-Goog-Api-Key", api_key)
            .header(
                "X-Goog-FieldMask",
                "places.name,places.id,places.displayName,places.formattedAddress,places.nationalPhoneNumber,places.internationalPhoneNumber,places.websiteUri,places.googleMapsUri,places.location",
            )
            .json(&json!({
                "textQuery": text_query,
                "languageCode": "fr",
                "pageSize": 5
            }))
            .send()
            .await
            .map_err(|err| format!("google places request: {err}"))?;
        if !response.status().is_success() {
            let code = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "google places status {code}: {}",
                truncate_for_trait(&body, 160)
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|err| format!("google places parse: {err}"))?;
        let Some(places) = payload
            .get("places")
            .and_then(|places| places.as_array())
        else {
            return Ok(None);
        };
        if places.is_empty() {
            return Ok(None);
        }

        let details_mask = "displayName,formattedAddress,nationalPhoneNumber,internationalPhoneNumber,websiteUri,googleMapsUri,location";
        let mut best_contact: Option<GooglePlaceContact> = None;
        let mut best_score = 0usize;

        for place in places.iter().take(5) {
            let mut contact = parse_google_place_contact_value(place);
            if google_place_contact_completeness(&contact) < 3 {
                if let Some(resource_name) = place.get("name").and_then(|item| item.as_str()) {
                    let details_url = format!("https://places.googleapis.com/v1/{resource_name}");
                    if let Ok(details_resp) = client
                        .get(&details_url)
                        .header("X-Goog-Api-Key", api_key)
                        .header("X-Goog-FieldMask", details_mask)
                        .send()
                        .await
                    {
                        if details_resp.status().is_success() {
                            if let Ok(details_payload) = details_resp.json::<serde_json::Value>().await {
                                let details_contact = parse_google_place_contact_value(&details_payload);
                                contact = merge_google_place_contacts(contact, details_contact);
                            }
                        }
                    }
                }
            }

            let score = google_place_contact_completeness(&contact);
            if best_contact.is_none() || score > best_score {
                best_score = score;
                best_contact = Some(contact);
            }
        }

        Ok(best_contact)
    })
}

fn normalize_city_key(city: &str) -> String {
    fold_ascii(city)
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', '\''], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn fold_ascii(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
            'ç' | 'Ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
            'ñ' | 'Ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
            'ý' | 'ÿ' | 'Ý' => 'y',
            'œ' | 'Œ' => 'o',
            'æ' | 'Æ' => 'a',
            _ => ch,
        })
        .collect()
}

fn extract_url_hint(value: &str) -> Option<String> {
    let token = value
        .split_whitespace()
        .find(|part| part.contains('.') || part.starts_with("http://") || part.starts_with("https://"))?
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '(' | '"' | '\''));
    if token.starts_with("http://") || token.starts_with("https://") {
        return Some(token.to_string());
    }
    Some(format!("https://{token}"))
}

fn extract_email_hint(value: &str) -> Option<String> {
    for token in value.split_whitespace() {
        let clean = token.trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '(' | '"' | '\''));
        if clean.contains('@') && clean.contains('.') && clean.len() >= 6 {
            return Some(clean.to_ascii_lowercase());
        }
    }
    None
}

fn extract_phone_hint(value: &str) -> Option<String> {
    let mut digits = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() || (ch == '+' && digits.is_empty()) {
            digits.push(ch);
            continue;
        }
        if digits.len() >= 10 {
            break;
        }
        if !digits.is_empty() && !ch.is_ascii_whitespace() && ch != '.' && ch != '-' {
            digits.clear();
        }
    }
    if digits.len() >= 10 {
        Some(digits)
    } else {
        None
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

fn export_collection_os_artifact(
    base: &Path,
    collector: &CollectorDefinition,
    job_id: &str,
    observed_at_ms: u64,
    data_hash: &str,
) -> Result<CollectionArtifactSummary, String> {
    let properties = read_properties(base)?;
    let zones = read_zones(base)?;
    if properties.is_empty() && zones.is_empty() {
        return Ok(CollectionArtifactSummary::default());
    }
    let observe = build_collection_observe_from_real_estate_store(
        collector,
        &properties,
        &zones,
        observed_at_ms,
        data_hash,
    );
    let expert_routes = crate::collection_os::derive_expert_routes_v2(&observe);
    let hypotheses = crate::collection_os::derive_hypothesis_sets_v2(&observe);
    let contract_ids = ["offer_listing", "document_record", "company_profile", "image_asset"];
    let extractions = contract_ids
        .iter()
        .filter_map(|contract_id| {
            crate::collection_os::extract_typed_entities_v2(&observe, contract_id)
                .ok()
                .filter(|extraction| !extraction.entities.is_empty())
        })
        .collect::<Vec<_>>();
    let entity_count = extractions
        .iter()
        .map(|extraction| extraction.entities.len())
        .sum::<usize>();
    let proof_hash = hash_parts(
        "real_estate_collection_os_artifact:v1",
        &[
            job_id,
            &collector.id,
            &observe.proof_hash,
            data_hash,
            &entity_count.to_string(),
        ],
    );
    let artifact_dir = base.join(DATA_DIR).join("collection_os");
    fs::create_dir_all(&artifact_dir)
        .map_err(|e| format!("create collection os artifact dir: {e}"))?;
    let artifact_path = artifact_dir.join(format!("{}.json", sanitize_filename(job_id)));
    let payload = json!({
        "kind": "real_estate_collection_os_artifact",
        "jobId": job_id,
        "collectorId": collector.id,
        "collectorLabel": collector.label,
        "observedAtMs": observed_at_ms,
        "dataHash": data_hash,
        "observeV2": {
            "sourceUrl": observe.source_url,
            "title": observe.title,
            "treeHash": observe.tree_hash,
            "proofHash": observe.proof_hash,
            "nodeCount": observe.nodes.len(),
            "sceneBlocks": observe.scene_blocks.iter().map(|block| json!({
                "id": block.id,
                "blockType": block.block_type,
                "summary": block.summary,
                "nodeIds": block.node_ids,
                "evidenceHash": block.evidence_hash,
            })).collect::<Vec<_>>(),
        },
        "expertRoutes": expert_routes,
        "hypotheses": hypotheses,
        "typedExtractions": extractions,
        "proofHash": proof_hash,
    });
    write_json_pretty(&artifact_path, &payload, "real estate collection os artifact")?;
    Ok(CollectionArtifactSummary {
        route_count: payload
            .get("expertRoutes")
            .and_then(|value| value.as_array())
            .map(|items| items.len())
            .unwrap_or(0),
        hypothesis_count: payload
            .get("hypotheses")
            .and_then(|value| value.as_array())
            .map(|items| items.len())
            .unwrap_or(0),
        entity_count,
        proof_hash,
        artifact_path: artifact_path.to_string_lossy().to_string(),
    })
}

fn build_collection_observe_from_real_estate_store(
    collector: &CollectorDefinition,
    properties: &[RealEstatePropertyEntity],
    zones: &[RealEstateZoneEntity],
    observed_at_ms: u64,
    data_hash: &str,
) -> crate::collection_os::CollectionObserveInputV2 {
    let mut nodes = Vec::new();
    let mut root_children = Vec::new();
    let source_url = format!("https://forge.local/real-estate/{}/{}", collector.id, short_hash(data_hash));
    let root_id = format!("reh-root-{}", sanitize_filename(&collector.id));
    for (index, property) in properties.iter().take(48).enumerate() {
        let card_id = format!("reh-property-{}", sanitize_filename(&property.property_id));
        let text = format!(
            "{} {} {} {:.0} m2 {:.0} EUR {:.0} rooms",
            property.property_type,
            property.city,
            property.address_label,
            property.surface_m2,
            property.price_eur,
            property.rooms
        );
        nodes.push(crate::collection_os::CollectionObservedNodeV2 {
            id: card_id.clone(),
            parent_id: Some(root_id.clone()),
            role: "group".to_string(),
            tag_name: "article".to_string(),
            selector_hint: format!(".listing-card[data-property-id='{}']", property.property_id),
            label: property.address_label.clone(),
            href: format!("https://forge.local/property/{}", property.property_id),
            visible: true,
            enabled: true,
            bounds: Some(crate::collection_os::CollectionBounds {
                x: 40.0,
                y: 120.0 + (index as f64 * 126.0),
                width: 920.0,
                height: 110.0,
            }),
            child_count: 0,
            source: property.source.clone(),
            frame_path: vec![source_url.clone()],
            shadow_path: Vec::new(),
            text: text.clone(),
            name: property.address_label.clone(),
            class_name: "listing-card property-card".to_string(),
            aria_role: "article".to_string(),
            aria_name: property.address_label.clone(),
            evidence: vec![crate::collection_os::CollectionObservedEvidence {
                source_kind: "real_estate_property".to_string(),
                extractor: "build_collection_observe_from_real_estate_store".to_string(),
                snippet: text,
                evidence_hash: property.evidence_hash.clone(),
            }],
            ..crate::collection_os::CollectionObservedNodeV2::default()
        });
        root_children.push(card_id);
    }
    for (index, zone) in zones.iter().take(24).enumerate() {
        let zone_id = format!("reh-zone-{}", sanitize_filename(&zone.zone_id));
        let text = format!(
            "{} {} {:.0} EUR/m2 {} biens liquidite {:.2}",
            zone.label, zone.city, zone.avg_price_m2, zone.property_count, zone.liquidity_score
        );
        nodes.push(crate::collection_os::CollectionObservedNodeV2 {
            id: zone_id.clone(),
            parent_id: Some(root_id.clone()),
            role: "article".to_string(),
            tag_name: "article".to_string(),
            selector_hint: format!(".zone-summary[data-zone-id='{}']", zone.zone_id),
            label: zone.label.clone(),
            visible: true,
            enabled: true,
            bounds: Some(crate::collection_os::CollectionBounds {
                x: 1010.0,
                y: 120.0 + (index as f64 * 84.0),
                width: 360.0,
                height: 72.0,
            }),
            child_count: 0,
            source: "zone_summary".to_string(),
            frame_path: vec![source_url.clone()],
            shadow_path: Vec::new(),
            text: text.clone(),
            name: zone.label.clone(),
            class_name: "zone-article market-summary".to_string(),
            aria_role: "article".to_string(),
            aria_name: zone.label.clone(),
            evidence: vec![crate::collection_os::CollectionObservedEvidence {
                source_kind: "real_estate_zone".to_string(),
                extractor: "build_collection_observe_from_real_estate_store".to_string(),
                snippet: text,
                evidence_hash: zone.evidence_hash.clone(),
            }],
            ..crate::collection_os::CollectionObservedNodeV2::default()
        });
        root_children.push(zone_id);
    }
    nodes.insert(
        0,
        crate::collection_os::CollectionObservedNodeV2 {
            id: root_id.clone(),
            role: "document".to_string(),
            tag_name: "section".to_string(),
            selector_hint: format!("#{}", root_id),
            label: collector.label.clone(),
            href: source_url.clone(),
            visible: true,
            enabled: true,
            bounds: Some(crate::collection_os::CollectionBounds {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 2200.0,
            }),
            child_count: root_children.len(),
            source: "real_estate_harvester".to_string(),
            frame_path: vec![source_url.clone()],
            shadow_path: Vec::new(),
            text: format!(
                "{} immobilier local properties={} zones={} data_hash={}",
                collector.label,
                properties.len(),
                zones.len(),
                short_hash(data_hash)
            ),
            name: collector.label.clone(),
            class_name: "real-estate-harvester-root".to_string(),
            aria_role: "document".to_string(),
            aria_name: collector.label.clone(),
            evidence: vec![crate::collection_os::CollectionObservedEvidence {
                source_kind: "real_estate_harvester".to_string(),
                extractor: "build_collection_observe_from_real_estate_store".to_string(),
                snippet: collector.label.clone(),
                evidence_hash: hash_parts("real_estate_collection_root:v1", &[&collector.id, data_hash]),
            }],
            ..crate::collection_os::CollectionObservedNodeV2::default()
        },
    );
    crate::collection_os::finalize_collection_observe_v2(crate::collection_os::CollectionObserveInputV2 {
        source_url,
        title: format!("{} Collection OS", collector.label),
        tree_hash: hash_parts(
            "real_estate_collection_tree:v1",
            &[&collector.id, data_hash, &properties.len().to_string(), &zones.len().to_string()],
        ),
        captured_at_ms: observed_at_ms,
        viewport: Some(crate::collection_os::CollectionObserveViewport {
            width: 1440.0,
            height: 2200.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }),
        nodes,
        scene_blocks: Vec::new(),
        proof_hash: String::new(),
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
    let projection_hash =
        real_estate_llm_cache_projection_hash(pack, kasm_contract, store_summary, &top_opportunities);
    let memory_evidence_hash =
        real_estate_llm_cache_memory_evidence_hash(pack, &top_opportunities);
    let cache = RealEstateLlmIntelCache {
        cache_id,
        status: "ready".to_string(),
        generated_at_ms: now_ms(),
        source_pack_id: pack.pack_id.clone(),
        source_pack_path: pack.artifact_path.clone(),
        projection_hash,
        evidence_hash: pack.evidence_hash.clone(),
        memory_evidence_hash,
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

fn real_estate_llm_cache_projection_hash(
    pack: &RealEstateIntelPack,
    kasm_contract: &RealEstateKasmContract,
    store_summary: &RealEstateLocalStoreSummary,
    top_opportunities: &[RealEstateLlmIntelOpportunity],
) -> String {
    let opportunity_refs = top_opportunities
        .iter()
        .map(|item| format!("{}:{}:{}:{}", item.rank, item.property_id, item.zone_id, item.proof_hash))
        .collect::<Vec<_>>();
    hash_parts(
        "real_estate_llm_cache_projection:v1",
        &[
            &pack.pack_id,
            &pack.evidence_hash,
            &kasm_contract.program_hash,
            &kasm_contract.semantic_fingerprint,
            &store_summary.properties.to_string(),
            &store_summary.zones.to_string(),
            &store_summary.metric_snapshots.to_string(),
            &pack.work_items.to_string(),
            &opportunity_refs.join("|"),
        ],
    )
}

fn real_estate_llm_cache_memory_evidence_hash(
    pack: &RealEstateIntelPack,
    top_opportunities: &[RealEstateLlmIntelOpportunity],
) -> String {
    let opportunity_refs = top_opportunities
        .iter()
        .map(|item| item.proof_hash.as_str())
        .collect::<Vec<_>>();
    let mut parts = vec![
        pack.evidence_hash.as_str(),
        pack.brain_note_hash.as_deref().unwrap_or(""),
        pack.brain_ref.as_deref().unwrap_or(""),
    ];
    parts.extend(opportunity_refs);
    hash_parts("real_estate_llm_cache_memory_evidence:v1", &parts)
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
        "projectionHash": cache.projection_hash,
        "evidenceHash": cache.evidence_hash,
        "memoryEvidenceHash": cache.memory_evidence_hash,
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

fn normalize_onboarding_question_id(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch
            } else if ch == '_' || ch == '-' {
                '_'
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .replace('-', "_")
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
    use std::fs;

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

    #[test]
    fn onboarding_starts_with_identity_question() {
        let store = test_store_dir("onboarding-initial");
        let state = onboarding_state(&store).expect("onboarding state should initialize");
        assert!(state.required);
        assert_eq!(state.current_index, 0);
        let question = state.question.expect("first question should exist");
        assert_eq!(question.id, "agency_identity");
        assert!(question.prompt.contains("ville"));
        let _ = cleanup_test_store(&store);
    }

    #[test]
    fn onboarding_identity_answer_triggers_collectors_and_next_step() {
        let store = test_store_dir("onboarding-identity");
        let report = record_onboarding_answer(
            &store,
            "agency_identity",
            "Agence Horizon Immobilier, Lyon, positionnement premium familial.",
        )
        .expect("identity answer should be recorded");
        assert_eq!(report.answered_question_id, "agency_identity");
        assert!(!report.triggered_collectors.is_empty());
        assert!(
            report
                .enrichment_queries
                .iter()
                .any(|query| query.to_ascii_lowercase().contains("google"))
        );
        let next = report.next_question.expect("next question should exist");
        assert_eq!(next.id, "agency_website");
        assert!(
            report
                .state
                .derived_traits
                .contains_key("agency_search_name")
        );
        assert_eq!(
            report.state.derived_traits.get("agency_city").map(String::as_str),
            Some("Lyon")
        );
        assert_eq!(
            report
                .state
                .derived_traits
                .get("harvester_region")
                .map(String::as_str),
            Some("Auvergne-Rhone-Alpes")
        );
        assert_eq!(
            report
                .state
                .derived_traits
                .get("collection_os_sector_pack")
                .map(String::as_str),
            Some("real_estate")
        );
        assert!(
            report
                .state
                .derived_traits
                .get("collection_os_plan_hash")
                .map(|hash| hash.starts_with("kasm://sha256/"))
                .unwrap_or(false)
        );
        assert!(
            report
                .state
                .derived_traits
                .get("collection_os_route")
                .map(|route| route.contains("official_api") && route.contains("extractor_program"))
                .unwrap_or(false)
        );
        let _ = cleanup_test_store(&store);
    }

    #[test]
    fn onboarding_identity_marcq_en_baroeul_sets_region_and_portal_harvest() {
        let store = test_store_dir("onboarding-marcq");
        let report = record_onboarding_answer(
            &store,
            "agency_identity",
            "Agence Valerie Duparque, Marcq en Baroeul",
        )
        .expect("identity answer should be recorded");
        assert_eq!(
            report
                .state
                .derived_traits
                .get("harvester_region")
                .map(String::as_str),
            Some("Hauts-de-France")
        );
        assert!(
            report
                .triggered_collectors
                .iter()
                .any(|collector| collector.collector_id == "portails")
        );
        let _ = cleanup_test_store(&store);
    }

    #[test]
    fn parses_remote_agency_contact_payload() {
        let payload = json!({
            "agency": {
                "displayName": "Agence Horizon",
                "formattedAddress": "12 rue des Fleurs, 69000 Lyon, France",
                "websiteUri": "https://horizon.example",
                "googleMapsUri": "https://maps.google.com/?q=45.764,4.8357",
                "location": {
                    "lat": 45.764,
                    "lng": 4.8357
                }
            }
        });
        let contact = parse_remote_agency_contact(payload.get("agency").unwrap())
            .expect("payload should parse")
            .expect("contact should exist");
        assert_eq!(contact.display_name.as_deref(), Some("Agence Horizon"));
        assert_eq!(contact.formatted_address.as_deref(), Some("12 rue des Fleurs, 69000 Lyon, France"));
        assert_eq!(contact.website_uri.as_deref(), Some("https://horizon.example"));
        assert_eq!(contact.google_maps_uri.as_deref(), Some("https://maps.google.com/?q=45.764,4.8357"));
        assert_eq!(contact.lat, Some(45.764));
        assert_eq!(contact.lng, Some(4.8357));
    }

    #[test]
    fn resolved_google_address_updates_harvester_scope_traits() {
        let mut traits = HashMap::new();
        apply_resolved_agency_contact_traits(
            &mut traits,
            GooglePlaceContact {
                display_name: Some("Valerie Duparque Immobilier".to_string()),
                formatted_address: Some("20 Rue Albert Bailly, 59700 Marcq-en-Baroeul, France".to_string()),
                national_phone: Some("03 20 89 39 00".to_string()),
                website_uri: Some("http://www.valerie-duparque-immobilier.com/".to_string()),
                google_maps_uri: Some("https://maps.google.com/?cid=1703054339125386130".to_string()),
                lat: Some(50.6774748),
                lng: Some(3.0922027),
            },
            "google_places",
            "ok",
        );
        assert_eq!(
            traits.get("agency_city").map(String::as_str),
            Some("Marcq-en-Baroeul")
        );
        assert_eq!(
            traits.get("harvester_region").map(String::as_str),
            Some("Hauts-de-France")
        );
        assert_eq!(
            traits.get("harvester_zone_seed").map(String::as_str),
            Some("Marcq-en-Baroeul")
        );
        assert_eq!(
            traits.get("agency_postal_code").map(String::as_str),
            Some("59700")
        );
    }

    #[test]
    fn windows_home_candidates_prefer_userprofile_before_home() {
        let candidates = preferred_user_home_candidates(
            Some("C:\\Users\\quent".into()),
            Some("C:".into()),
            Some("\\Users\\quent".into()),
            Some("C:\\Users\\CodexSandboxOffline".into()),
        );
        assert_eq!(
            candidates.first().map(|path| path.to_string_lossy().to_string()),
            Some("C:\\Users\\quent".to_string())
        );
        assert!(
            candidates
                .iter()
                .any(|path| path.to_string_lossy() == "C:\\Users\\CodexSandboxOffline")
        );
    }

    #[test]
    fn onboarding_fetch_classifies_rate_limit_as_block() {
        let fetch = HttpTextFetch {
            url: "https://example.test/search".to_string(),
            status: 429,
            body: "Too Many Requests".to_string(),
            elapsed_ms: 50,
        };
        let err = ensure_fetch_not_blocked(&fetch).expect_err("429 should block");
        assert!(err.starts_with("collection_block:backoff:soft_block:"));
        assert!(parse_collection_block_error(&err).is_some());
    }

    #[test]
    fn onboarding_fetch_classifies_captcha_as_hard_block() {
        let fetch = HttpTextFetch {
            url: "https://example.test/protected".to_string(),
            status: 403,
            body: "<html><title>Captcha</title><body>reCAPTCHA required</body></html>".to_string(),
            elapsed_ms: 120,
        };
        let err = ensure_fetch_not_blocked(&fetch).expect_err("captcha should block");
        assert!(err.starts_with("collection_block:blocked:hard_block:"));
    }

    #[test]
    fn estimation_run_persists_collection_os_artifact_outputs() {
        let store = test_store_dir("collection-os-artifact");
        let report = run_tool(&store, "estimation").expect("estimation run should succeed");
        let collection_artifact = report
            .normalized_outputs
            .iter()
            .find(|entry| entry.starts_with("collection_artifact:"))
            .cloned()
            .expect("collection artifact output should exist");
        let artifact_path = collection_artifact
            .split_once(':')
            .map(|(_, value)| value.to_string())
            .expect("artifact path should be present");
        assert!(Path::new(&artifact_path).exists());
        let artifact = fs::read_to_string(&artifact_path).expect("artifact should be readable");
        let payload: serde_json::Value = serde_json::from_str(&artifact).expect("artifact should parse");
        assert_eq!(
            payload.get("kind").and_then(|value| value.as_str()),
            Some("real_estate_collection_os_artifact")
        );
        assert!(
            payload
                .get("expertRoutes")
                .and_then(|value| value.as_array())
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        );
        assert!(
            payload
                .get("typedExtractions")
                .and_then(|value| value.as_array())
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        );
        let _ = cleanup_test_store(&store);
    }

    #[test]
    fn llm_cache_ledger_carries_projection_and_memory_evidence_hashes() {
        let store = test_store_dir("llm-cache-ledger");
        let cache_dir = store.join(DATA_DIR).join(LLM_INTEL_CACHE_DIR);
        fs::create_dir_all(&cache_dir).expect("llm cache dir should exist");
        let pack = RealEstateIntelPack {
            pack_id: "pack-123".to_string(),
            status: "ready".to_string(),
            generated_at_ms: 1,
            trigger: "unit_test".to_string(),
            input_runs: 2,
            metric_count: 4,
            candidate_count: 2,
            scenario_count: 1,
            horizon_count: 1,
            work_items: 3,
            metric_manifest: vec!["dvf_price_gap".to_string(), "listing_staleness".to_string()],
            kasm_contract_hash: "kasm-contract-123".to_string(),
            kasm_semantic_fingerprint: "semantic-123".to_string(),
            brain_note_hash: Some("note-123".to_string()),
            brain_ref: Some("refs/brain/real-estate/intel/latest".to_string()),
            top_opportunities: vec![
                RealEstateIntelOpportunity {
                    property_id: "property-1".to_string(),
                    zone_id: "zone-a".to_string(),
                    score: 0.91,
                    seller_probability: 0.82,
                    expected_fee_eur: 12_500.0,
                    horizon_days: 30,
                    strongest_signal: "price_gap".to_string(),
                    proof_hash: "proof-alpha".to_string(),
                },
                RealEstateIntelOpportunity {
                    property_id: "property-2".to_string(),
                    zone_id: "zone-b".to_string(),
                    score: 0.73,
                    seller_probability: 0.64,
                    expected_fee_eur: 9_800.0,
                    horizon_days: 45,
                    strongest_signal: "staleness".to_string(),
                    proof_hash: "proof-beta".to_string(),
                },
            ],
            evidence_hash: "evidence-123".to_string(),
            artifact_path: store.join("pack.json").to_string_lossy().to_string(),
            llm_summary: "synthetic intel pack".to_string(),
        };
        let store_summary = RealEstateLocalStoreSummary {
            data_dir: store.join(DATA_DIR).to_string_lossy().to_string(),
            properties: 8,
            zones: 3,
            source_events: 6,
            metric_snapshots: 10,
            latest_updated_at_ms: 42,
            data_hash: "data-hash-123".to_string(),
        };
        let kasm_contract = RealEstateKasmContract {
            contract_id: "re-score".to_string(),
            program_hash: "program-hash-123".to_string(),
            semantic_fingerprint: "semantic-fingerprint-123".to_string(),
            canonical_hash: "canonical-hash-123".to_string(),
            metric_manifest_hash: "metric-manifest-123".to_string(),
            input_metrics: vec!["dvf_price_gap".to_string(), "listing_staleness".to_string()],
            output_contract: "score + probability + fee + signal".to_string(),
            nodes: 4,
            byte_len: 128,
            fuel: 64,
            cache_key: "cache-key-123".to_string(),
            artifact_path: store.join("contract.json").to_string_lossy().to_string(),
        };
        let cache =
            write_llm_intel_cache(&store, &pack, &store_summary, &kasm_contract).expect("cache write should succeed");
        assert!(!cache.projection_hash.is_empty());
        assert!(!cache.memory_evidence_hash.is_empty());
        append_llm_cache_ledger(&store, &cache).expect("ledger append should succeed");
        let ledger_path = store.join(LEDGER_FILE);
        let ledger = fs::read_to_string(&ledger_path).expect("ledger should be readable");
        let entry = ledger
            .lines()
            .last()
            .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .expect("ledger entry should parse");
        assert_eq!(
            entry.get("projectionHash").and_then(|value| value.as_str()),
            Some(cache.projection_hash.as_str())
        );
        assert_eq!(
            entry.get("memoryEvidenceHash").and_then(|value| value.as_str()),
            Some(cache.memory_evidence_hash.as_str())
        );
        let _ = cleanup_test_store(&store);
    }

    fn test_store_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("forge-real-estate-harvester-tests-{label}-{}", now_ms()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup_test_store(path: &Path) -> Result<(), String> {
        if path.exists() {
            fs::remove_dir_all(path).map_err(|err| format!("cleanup test store: {err}"))?;
        }
        Ok(())
    }
}
